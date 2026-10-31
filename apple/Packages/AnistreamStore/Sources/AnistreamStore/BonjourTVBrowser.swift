// The TVs on the network, for the engine's sender: Network's browse for `_anistream._tcp`, each
// service found resolved to its addresses through DNS-SD, since the engine connects to an
// address and a port.
//
// The first browse is what asks for local network access, on iPhone, iPad and the Mac alike:
// the first time Play on TV is used, with the Info.plist's NSLocalNetworkUsageDescription.

import AnistreamKit
import Foundation
import Network
import Synchronization
import dnssd

/// The platform's browse, as the engine's `FfiTvBrowser`.
public final class BonjourTVBrowser: FfiTvBrowser {
    public static let serviceType = "_anistream._tcp"

    private let queue = DispatchQueue(label: "tv.anistream.tv-browser") // i18n-skip
    private struct Browse {
        var browser: NWBrowser
        var sink: FfiDiscoverySink
        var resolving: [String: Resolve] = [:]
    }
    private let browse = Mutex<Browse?>(nil)
    /// Told whenever the browse learns whether the system lets it look on the local network.
    let access: (@Sendable (Bool) -> Void)?

    public init() {
        access = nil
    }

    init(access: @escaping @Sendable (Bool) -> Void) {
        self.access = access
    }

    public func start(sink: FfiDiscoverySink) {
        stop()
        let browser = NWBrowser(for: .bonjourWithTXTRecord(type: Self.serviceType, domain: nil), using: .tcp)
        browser.browseResultsChangedHandler = { [weak self] _, changes in
            self?.changed(changes)
        }
        browser.stateUpdateHandler = { [access] state in
            if case .failed(let error) = state {
                AppLog.error("tv", "browse failed: \(error)")
            }
            if let denied = LocalNetworkAccess.denied(state) { access?(denied) }
        }
        browse.withLock { $0 = Browse(browser: browser, sink: sink) }
        browser.start(queue: queue)
    }

    public func stop() {
        let stopped = browse.withLock { browse -> Browse? in
            defer { browse = nil }
            return browse
        }
        stopped?.browser.cancel()
        stopped?.resolving.values.forEach { $0.cancel() }
    }

    private func changed(_ changes: Set<NWBrowser.Result.Change>) {
        for change in changes {
            switch change {
            case .added(let result):
                found(result)
            case .changed(_, let new, _):
                found(new)
            case .removed(let result):
                guard case .service(let name, _, _, _) = result.endpoint else { continue }
                let sink = browse.withLock { browse -> FfiDiscoverySink? in
                    browse?.resolving.removeValue(forKey: name)?.cancel()
                    return browse?.sink
                }
                sink?.lost(instance: name)
            default:
                break
            }
        }
    }

    private func found(_ result: NWBrowser.Result) {
        guard case .service(let name, let type, let domain, let interface) = result.endpoint else { return }
        let txt: [FfiTxtPair] = if case .bonjour(let record) = result.metadata {
            record.dictionary.sorted { $0.key < $1.key }.map { FfiTxtPair(key: $0.key, value: $0.value) }
        } else {
            []
        }
        let resolve = Resolve(name: name, type: type, domain: domain, interface: UInt32(interface?.index ?? 0), queue: queue) {
            [weak self] host, port in
            guard let self else { return }
            let addresses = Self.addresses(of: host)
            let sink = self.browse.withLock { $0?.sink }
            guard !addresses.isEmpty else {
                AppLog.info("tv", "found \(name), but not its address")
                return
            }
            sink?.found(instance: name, addresses: addresses, port: port, txt: txt)
        }
        browse.withLock { browse in
            browse?.resolving.updateValue(resolve, forKey: name)?.cancel()
        }
        resolve.start()
    }

    /// The numeric addresses `host` resolves to, its `.local` name through mDNS.
    static func addresses(of host: String) -> [String] {
        var hints = addrinfo()
        hints.ai_family = AF_UNSPEC
        hints.ai_socktype = SOCK_STREAM
        var list: UnsafeMutablePointer<addrinfo>?
        guard getaddrinfo(host, nil, &hints, &list) == 0, let first = list else { return [] }
        defer { freeaddrinfo(list) }
        var addresses: [String] = []
        var next: UnsafeMutablePointer<addrinfo>? = first
        while let info = next {
            var name = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            if getnameinfo(info.pointee.ai_addr, info.pointee.ai_addrlen, &name, socklen_t(name.count), nil, 0, NI_NUMERICHOST) == 0 {
                let address = String(decoding: name.prefix { $0 != 0 }.map { UInt8(bitPattern: $0) }, as: UTF8.self)
                if !addresses.contains(address) { addresses.append(address) }
            }
            next = info.pointee.ai_next
        }
        return addresses
    }
}

/// One DNS-SD resolve: the service's host name and port, handed to `done` once, off the queue
/// the address lookup may block.
final class Resolve: @unchecked Sendable {
    private let name: String
    private let type: String
    private let domain: String
    private let interface: UInt32
    private let queue: DispatchQueue
    private let done: @Sendable (String, UInt16) -> Void
    private var ref: DNSServiceRef?

    init(
        name: String, type: String, domain: String, interface: UInt32, queue: DispatchQueue,
        done: @escaping @Sendable (String, UInt16) -> Void
    ) {
        self.name = name
        self.type = type
        self.domain = domain
        self.interface = interface
        self.queue = queue
        self.done = done
    }

    func start() {
        queue.async { [self] in
            let context = Unmanaged.passUnretained(self).toOpaque()
            let status = DNSServiceResolve(&ref, 0, interface, name, type, domain, { _, _, _, error, _, host, port, _, _, context in
                guard let context else { return }
                let resolve = Unmanaged<Resolve>.fromOpaque(context).takeUnretainedValue()
                guard error == kDNSServiceErr_NoError, let host else { return }
                resolve.resolved(String(cString: host), port: UInt16(bigEndian: port))
            }, context)
            guard status == kDNSServiceErr_NoError, let ref else {
                AppLog.error("tv", "could not resolve \(name): \(status)")
                self.ref = nil
                return
            }
            DNSServiceSetDispatchQueue(ref, queue)
        }
    }

    private func resolved(_ host: String, port: UInt16) {
        cancelOnQueue()
        let done = done
        DispatchQueue.global(qos: .userInitiated).async { done(host, port) }
    }

    func cancel() {
        queue.async { [self] in cancelOnQueue() }
    }

    private func cancelOnQueue() {
        if let ref { DNSServiceRefDeallocate(ref) }
        ref = nil
    }
}
