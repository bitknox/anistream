// The Fixtures app's TVs: two made-up TVs on the loopback interface, each a real remote server
// with a scripted player behind it (`startFakeTv`), found by a browse that lists them as DNS-SD
// would. They pair, take commands and send their state as a TV does; what they play is made up.

#if FIXTURES
import AnistreamKit
import Foundation

/// A browse that finds the fake TVs.
final class FixtureTVBrowser: FfiTvBrowser {
    let tvs: [FfiFakeTv]
    /// Finds nothing: the network with no TV on it, for the empty scene.
    let silent: Bool

    init(tvs: [FfiFakeTv], silent: Bool = false) {
        self.tvs = tvs
        self.silent = silent
    }

    func start(sink: FfiDiscoverySink) {
        guard !silent else { return }
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
///     lost     the remote scene, then Living room gone from the network mid-episode
///     empty    the picker on a network where no TV answers, past its 10 s wait
public enum TVScene: String, Sendable {
    case paired, picker, code, remote, lost, empty
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
            ], silent: TVScene.named() == .empty)
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
            case .lost: card?.phase == .retrying && card?.now != nil
            case .empty: sheet == .picker && rows.isEmpty
            case .bar: sheet == nil && card?.now.map { !$0.loading } ?? false
            }
            if there {
                // The sheet's own animation in, and for the empty picker its wait.
                try? await Task.sleep(for: scene == .empty ? .seconds(11) : .milliseconds(700))
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
        if scene == .empty {
            // Nothing paired and nothing found.
            openPicker()
            return
        }
        do {
            let pairing = try await sender.pair(target: bedroom.pairLink(code: bedroom.openCode()))
            _ = try await pairing.pair(code: nil)
            refresh()
            switch scene {
            case .paired, .empty:
                break
            case .picker:
                openPicker()
            case .code:
                openPicker()
                begin(rows.first { $0.name == "Living room" }?.key ?? living.pairLink(code: nil))
            case .remote, .bar, .lost:
                let pairing = try await sender.pair(target: living.pairLink(code: living.openCode()))
                let tv = try await pairing.pair(code: nil)
                refresh()
                offer(play)
                use(tv.fingerprint)
                if scene == .bar || scene == .lost {
                    while card?.now.map({ !$0.loading }) != true { try await Task.sleep(for: .milliseconds(50)) }
                }
                if scene == .bar { sheetClosed() }
                if scene == .lost { living.stop() }
            }
        } catch {
            AppLog.error("tv", "the TV scene did not set up: \(error)")
        }
    }
}
#endif
