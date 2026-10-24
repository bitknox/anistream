// The Fixtures app's TVs: two made-up TVs on the loopback interface, each a real remote server
// with a scripted player behind it (`startFakeTv`), found by a browse that lists them as DNS-SD
// would. They pair, take commands and send their state as a TV does; what they play is made up.

#if FIXTURES
import AnistreamKit
import Foundation

/// A browse that finds the fake TVs.
final class FixtureTVBrowser: FfiTvBrowser {
    let tvs: [FfiFakeTv]

    init(tvs: [FfiFakeTv]) {
        self.tvs = tvs
    }

    func start(sink: FfiDiscoverySink) {
        for tv in tvs {
            let advert = tv.advert()
            sink.found(instance: advert.instance, addresses: [tv.host()], port: tv.port(), txt: advert.txt)
        }
    }

    func stop() {}
}

/// What the TV shots open on: `-AnistreamTV picker`, `code`, `remote` or `paired`.
///
///     paired   one TV paired already (Bedroom) and one not (Living room), nothing open
///     picker   the same, with the picker open
///     code     the picker pairing with Living room, its code to type
///     remote   Living room paired and playing an episode, its card open
public enum TVScene: String, Sendable {
    case paired, picker, code, remote
    /// The remote scene with its card closed: the now-playing bar over the tab bar.
    case bar

    public static let argument = "-AnistreamTV"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> TVScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1)
        else { return nil }
        return TVScene(rawValue: arguments[index + 1])
    }
}

extension TVRemoteModel {
    /// The fake TVs, started on `engine`, and the browse that finds them.
    static func fixtures(_ engine: FfiEngine) -> FixtureTVBrowser? {
        do {
            return FixtureTVBrowser(tvs: [
                try engine.startFakeTv(name: "Living room"),
                try engine.startFakeTv(name: "Bedroom"),
            ])
        } catch {
            AppLog.error("tv", "fake TVs did not start: \(error)")
            return nil
        }
    }

    /// Wait, up to ten seconds, for `scene` to be on screen: true once it is.
    public func reached(_ scene: TVScene) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(10)
        while ContinuousClock.now < deadline {
            let there = switch scene {
            case .paired: !paired.isEmpty
            case .picker: sheet == .picker && rows.count == 2
            case .code: pairing?.needsCode == true && pairing?.working == false && !(pairing?.tvName.isEmpty ?? true)
            case .remote: card?.now.map { !$0.loading && !$0.subtitles.isEmpty } ?? false
            case .bar: sheet == nil && card?.now.map { !$0.loading } ?? false
            }
            if there {
                // The sheet's own animation in.
                try? await Task.sleep(for: .milliseconds(700))
                return true
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return false
    }

    /// Set up the scene the arguments name, for a shot. `play` is what the remote scene plays.
    func stage(_ scene: TVScene, fakes: FixtureTVBrowser, play: TVPlayRequest) async {
        guard let sender, fakes.tvs.count == 2 else { return }
        let living = fakes.tvs[0]
        let bedroom = fakes.tvs[1]
        do {
            let pairing = try await sender.pair(target: bedroom.pairLink(code: bedroom.openCode()))
            _ = try await pairing.pair(code: nil)
            refresh()
            switch scene {
            case .paired:
                break
            case .picker:
                openPicker()
            case .code:
                openPicker()
                begin(rows.first { $0.name == "Living room" }?.key ?? living.pairLink(code: nil))
            case .remote, .bar:
                let pairing = try await sender.pair(target: living.pairLink(code: living.openCode()))
                let tv = try await pairing.pair(code: nil)
                refresh()
                offer(play)
                use(tv.fingerprint)
                if scene == .bar {
                    while card?.now.map({ !$0.loading }) != true { try await Task.sleep(for: .milliseconds(50)) }
                    sheetClosed()
                }
            }
        } catch {
            AppLog.error("tv", "the TV scene did not set up: \(error)")
        }
    }
}
#endif
