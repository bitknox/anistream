// TVs with Chromecast built in on the network, for the engine's sender: Network's browse for
// `_googlecast._tcp` beside Play on TV's `_anistream._tcp`, each device resolved to its address
// through DNS-SD, with its TXT record as it came. The engine reads the record: the name, the
// model, and whether it has a screen.
//
// It asks for local network access as Play on TV's browse does, under the same Info.plist key,
// with `_googlecast._tcp` in NSBonjourServices.

import AnistreamKit
import Foundation
import Network
import Synchronization

/// The platform's Cast browse, as the engine's `FfiCastBrowser`.
public final class BonjourCastBrowser: FfiCastBrowser {
    public static let serviceType = "_googlecast._tcp"

    private let queue = DispatchQueue(label: "tv.anistream.cast-browser") // i18n-skip
    private struct Browse {
        var browser: NWBrowser
        var sink: FfiCastSink
        var resolving: [String: Resolve] = [:]
    }
    private let browse = Mutex<Browse?>(nil)
    /// Told whenever the browse learns whether the system lets it look on the local network.
    private let access: @Sendable (Bool) -> Void

    init(access: @escaping @Sendable (Bool) -> Void) {
        self.access = access
    }

    public func start(sink: FfiCastSink) {
        stop()
        let browser = NWBrowser(for: .bonjourWithTXTRecord(type: Self.serviceType, domain: nil), using: .tcp)
        browser.browseResultsChangedHandler = { [weak self] _, changes in
            self?.changed(changes)
        }
        browser.stateUpdateHandler = { [access] state in
            if case .failed(let error) = state {
                AppLog.error("cast", "browse failed: \(error)")
            }
            if let denied = LocalNetworkAccess.denied(state) { access(denied) }
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
                let sink = browse.withLock { browse -> FfiCastSink? in
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
            let addresses = BonjourTVBrowser.addresses(of: host)
            let sink = self.browse.withLock { $0?.sink }
            guard !addresses.isEmpty else {
                AppLog.info("cast", "found \(name), but not its address")
                return
            }
            sink?.found(instance: name, addresses: addresses, port: port, txt: txt)
        }
        browse.withLock { browse in
            browse?.resolving.updateValue(resolve, forKey: name)?.cancel()
        }
        resolve.start()
    }
}
