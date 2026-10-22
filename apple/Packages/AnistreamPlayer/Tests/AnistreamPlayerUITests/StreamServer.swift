// A web stream as a provider's CDN serves one, on loopback: files from a directory, refused
// with a 403 unless the request carries the referer the stream is locked to.

import Foundation
import Network
import Synchronization

final class StreamServer: Sendable {
    let directory: URL
    let referer: String
    private let listener: NWListener
    private let queue = DispatchQueue(label: "tv.anistream.test.stream-server")
    /// The paths asked for, with whether each was let through.
    private let served = Mutex<[(path: String, allowed: Bool)]>([])

    init(directory: URL, referer: String) throws {
        self.directory = directory
        self.referer = referer
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = NWEndpoint.hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: parameters)
    }

    /// Start listening, and say on which port.
    func start() async throws -> UInt16 {
        let (ready, continuation) = AsyncStream.makeStream(of: UInt16?.self)
        listener.stateUpdateHandler = { [listener] state in
            switch state {
            case .ready: continuation.yield(listener.port?.rawValue)
            case .failed, .cancelled: continuation.yield(nil)
            default: break
            }
        }
        listener.newConnectionHandler = { [weak self] connection in self?.serve(connection) }
        listener.start(queue: queue)
        for await port in ready {
            guard let port else { break }
            return port
        }
        throw URLError(.cannotConnectToHost)
    }

    func stop() { listener.cancel() }

    var requests: [(path: String, allowed: Bool)] { served.withLock { $0 } }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        receive(on: connection, buffer: Data())
    }

    private func receive(on connection: NWConnection, buffer: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, done, error in
            guard let self else { return }
            var buffer = buffer
            if let data { buffer.append(data) }
            guard let end = buffer.range(of: Data("\r\n\r\n".utf8)) else {
                if done || error != nil { connection.cancel() } else { self.receive(on: connection, buffer: buffer) }
                return
            }
            let head = String(decoding: buffer[..<end.lowerBound], as: UTF8.self)
            self.respond(to: head, on: connection)
        }
    }

    private func respond(to head: String, on connection: NWConnection) {
        let lines = head.components(separatedBy: "\r\n")
        let path = lines.first?.split(separator: " ").dropFirst().first.map(String.init) ?? "/"
        let headers = lines.dropFirst().compactMap { line -> (String, String)? in
            guard let colon = line.firstIndex(of: ":") else { return nil }
            return (line[..<colon].lowercased(), line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces))
        }
        let allowed = headers.contains { $0.0 == "referer" && $0.1 == referer }
        served.withLock { $0.append((path, allowed)) }
        let name = path.split(separator: "?").first.map { String($0.dropFirst()) } ?? ""
        let file = directory.appending(path: name)
        let status: String
        var body = Data()
        var type = "text/plain"
        if !allowed {
            status = "403 Forbidden"
        } else if !name.contains(".."), let data = try? Data(contentsOf: file) {
            status = "200 OK"
            body = data
            type = name.hasSuffix(".m3u8") ? "application/vnd.apple.mpegurl" : "video/mp2t"
        } else {
            status = "404 Not Found"
        }
        var response = Data(
            "HTTP/1.1 \(status)\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nConnection: close\r\n\r\n".utf8)
        response.append(body)
        connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
    }
}
