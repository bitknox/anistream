// The Fixtures app's TVs with Chromecast built in: a made-up device list handed to the engine's
// Cast sink, as the network's browse would find it, and a scripted cast behind the card. Nothing
// reaches a real device: the list's addresses are loopback ones nothing listens on, and a pick
// opens the script, not the engine's cast.

#if FIXTURES
import AnistreamKit
import AnistreamStrings
import Foundation
import Synchronization

/// What the casting shots open on: `-AnistreamCast <scene>`.
///
///     found      the picker with Bedroom (anistream) and two TVs with Chromecast built in
///     searching  the picker before anything answers, past its quiet 600 ms
///     none       the picker on a network where nothing answers, past its 10 s wait
///     gone       the found scene, then Kitchen display gone from the network
///     blocked    the picker while the system won't let the app look on the local network
///     starting   the card just after a pick, starting on Living room
///     counting   the same card six seconds on, counting
///     playing    Living room playing episode 3, its volume set on the TV
///     muted      the playing scene, muted
///     level      Kitchen display playing, with a volume level and the skip offer
///     refused    the card after Living room refused every stream
///     never      the card after Living room never read the stream
///     network    the card when this device isn't on the TV's network
///     codec      the card when the TV can't play the video
///     deeplink   the card when the episode opens in another app
///     lost       Living room dropped off mid-episode: Casting stopped
///     takenover  someone else took Living room: the card gone, the notice offering Play here
///     bar        the playing scene with its card closed: the toolbar's button, or the bar
public enum CastScene: String, Sendable, CaseIterable {
    case found, searching, none, gone, blocked
    case starting, counting, playing, muted, level
    case refused, never, network, codec, deeplink
    case lost, takenover, bar

    public static let argument = "-AnistreamCast"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> CastScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1)
        else { return nil }
        return CastScene(rawValue: arguments[index + 1])
    }

    /// The picker's scenes, where the browse finds nothing.
    var silent: Bool { self == .searching || self == .none || self == .blocked }

    var picker: Bool { [.found, .searching, .none, .gone, .blocked].contains(self) }
}

/// The made-up device list, found as the network's browse finds TVs.
final class FixtureCastBrowser: FfiCastBrowser, @unchecked Sendable {
    struct Device {
        let instance: String
        let id: String
        let name: String
        let address: String
    }

    static let devices = [
        Device(instance: "Fixture-living", id: "living", name: "Living room", address: "127.0.0.2"), // i18n-skip
        Device(instance: "Fixture-kitchen", id: "kitchen", name: "Kitchen display", address: "127.0.0.3"), // i18n-skip
    ]

    let silent: Bool
    private let sink = Mutex<FfiCastSink?>(nil)

    init(silent: Bool) {
        self.silent = silent
    }

    func start(sink: FfiCastSink) {
        self.sink.withLock { $0 = sink }
        guard !silent else { return }
        for device in Self.devices {
            let txt = [
                FfiTxtPair(key: "id", value: device.id), // i18n-skip
                FfiTxtPair(key: "fn", value: device.name), // i18n-skip
                FfiTxtPair(key: "md", value: "Fixture screen"), // i18n-skip
                FfiTxtPair(key: "ca", value: "465413"), // i18n-skip
            ]
            sink.found(instance: device.instance, addresses: [device.address], port: 8009, txt: txt)
        }
    }

    func stop() {
        sink.withLock { $0 = nil }
    }

    /// A device leaves the network.
    func lose(_ instance: String) {
        sink.withLock { $0 }?.lost(instance: instance)
    }
}

/// A cast that plays by script: its card, moved on as the scene says and as the card's controls
/// ask, the way a TV's reports would.
final class FixtureCastSession: FfiCastSessionProtocol, @unchecked Sendable {
    struct State {
        var name: String
        var phase: FfiLinkPhase = .connecting
        var phaseLine: String?
        var loading = true
        var position: Double = 0
        var duration: Double?
        var paused = false
        var volume: Double?
        var volumeControl: FfiVolumeControl = .muteOnly
        var muted = false
        var audio: [FfiTvTrack] = []
        var subtitles: [FfiTvTrack] = []
        var subtitlesOff = false
        var skip: FfiSkipCard?
        var notice: Msg?
        var over = false
        var rev: UInt64 = 0
        var stopped = false
    }

    private let state: Mutex<State>
    private let request: TVPlayRequest
    private let episodeLine: String

    init(name: String, request: TVPlayRequest, episodeLine: String) {
        state = Mutex(State(name: name, position: request.from ?? 0))
        self.request = request
        self.episodeLine = episodeLine
    }

    /// Change the card, as a report from the TV would.
    func change(_ edit: (inout State) -> Void) {
        state.withLock { state in
            edit(&state)
            state.rev += 1
        }
    }

    func card() -> FfiCastCard {
        let state = state.withLock { $0 }
        let now = FfiNowCard(
            id: request.id, title: request.title, episode: request.episode, episodeLine: episodeLine,
            loading: state.loading, position: state.position, duration: state.loading ? nil : state.duration,
            paused: state.paused, speed: 1, volume: state.volume, audio: state.audio, subtitles: state.subtitles,
            subtitlesOff: state.subtitlesOff, skip: state.skip)
        let card = FfiRemoteCard(
            tvName: state.name, phase: state.phase, phaseLine: state.phaseLine, screen: .player, now: now,
            rev: state.rev, over: state.over, reached: true)
        return FfiCastCard(card: card, volumeControl: state.volumeControl, muted: state.muted, notice: state.notice)
    }

    func nextCard() async -> FfiCastCard? {
        let seen = state.withLock { $0.rev }
        while !Task.isCancelled {
            let (rev, stopped) = state.withLock { ($0.rev, $0.stopped) }
            if stopped { return nil }
            if rev != seen { return card() }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return nil
    }

    func command(command: FfiTvCommand) {
        change { state in
            switch command {
            case .togglePause: state.paused.toggle()
            case .seek(let by): state.position = max(0, state.position + by)
            case .seekTo(let position): state.position = position
            case .mute(let muted): state.muted = muted
            case .volume(let step): state.volume = state.volume.map { min(100, max(0, $0 + step)) }
            case .track(let kind, let id):
                if kind == .subtitle {
                    state.subtitlesOff = id == nil
                    state.subtitles = state.subtitles.map { FfiTvTrack(id: $0.id, language: $0.language, title: $0.title, selected: $0.id == id) }
                } else {
                    state.audio = state.audio.map { FfiTvTrack(id: $0.id, language: $0.language, title: $0.title, selected: $0.id == id) }
                }
            case .skip:
                if let skip = state.skip { state.position = skip.to }
                state.skip = nil
            default: break
            }
        }
    }

    func deviceName() -> String { state.withLock { $0.name } }

    func playHere() -> Double? {
        let position = state.withLock { $0.position }
        stop()
        return position
    }

    func stop() {
        change { $0.stopped = true }
    }
}

extension TVRemoteModel {
    /// The scene's device list, for the engine's sender to browse.
    static func fixtureCasts() -> FixtureCastBrowser {
        FixtureCastBrowser(silent: CastScene.named()?.silent ?? false)
    }

    /// Set up the casting scene the arguments name, for a shot: `play` is what the card casts.
    func stage(_ scene: CastScene, casts: FixtureCastBrowser, play: TVPlayRequest) async {
        if scene == .blocked { cast.localNetwork(denied: true) }
        if scene.picker {
            offer(play)
            guard scene == .gone else { return }
            while cast.rows.count < 2 { try? await Task.sleep(for: .milliseconds(50)) }
            try? await Task.sleep(for: .milliseconds(900))
            casts.lose(FixtureCastBrowser.devices[1].instance)
            return
        }
        let kitchen = scene == .level
        let device = FixtureCastBrowser.devices[kitchen ? 1 : 0]
        let session = FixtureCastSession(
            name: device.name, request: play,
            episodeLine: L.episodeNumberString(number: play.episode))
        cast.opener = { _, _ in session }
        offer(play)
        let row = FfiCastRow(
            key: "cast:\(device.id)", name: device.name, model: "", address: device.address, // i18n-skip
            routeHint: .throughThisDevice)
        choose(cast: row)
        guard scene != .starting, scene != .counting else { return }
        try? await Task.sleep(for: .milliseconds(400))
        switch scene {
        case .refused, .never, .network, .codec, .deeplink:
            let notice: Msg = switch scene {
            case .refused: .noticeCastRefused
            case .never: .noticeCastNeverFetches
            case .network: .noticeCastNotSameNetwork
            case .codec: .noticeCastVideoCodec(codec: "AV1") // i18n-skip
            default: .noticeCastDeepLink
            }
            session.change {
                $0.notice = notice
                $0.over = true
            }
            return
        default:
            break
        }
        session.change { state in
            state.phase = .connected
            state.loading = false
            state.duration = 1420
            state.position = kitchen ? 58 : 492
            state.subtitles = [
                FfiTvTrack(id: 1, language: "en", title: nil, selected: !kitchen), // i18n-skip
                FfiTvTrack(id: 2, language: "en", title: "Signs only", selected: false), // i18n-skip
            ]
            state.subtitlesOff = kitchen
            state.audio = [
                FfiTvTrack(id: 1, language: "ja", title: nil, selected: true), // i18n-skip
                FfiTvTrack(id: 2, language: "en", title: nil, selected: false), // i18n-skip
            ]
            if kitchen {
                state.volumeControl = .level
                state.volume = 45
                state.skip = FfiSkipCard(to: 88, label: L.playerSkipOpening)
            }
            state.muted = scene == .muted
        }
        switch scene {
        case .lost:
            try? await Task.sleep(for: .milliseconds(600))
            session.change {
                $0.phase = .retrying
                $0.notice = .noticeCastLost
                $0.over = true
            }
        case .takenover:
            try? await Task.sleep(for: .milliseconds(600))
            sheetClosed()
            session.change {
                $0.notice = .noticeCastTakenOver
                $0.over = true
            }
        case .bar:
            try? await Task.sleep(for: .milliseconds(300))
            sheetClosed()
        default:
            break
        }
    }
}

extension CastModel {
    /// Wait, up to fifteen seconds, for `scene` to be on screen: true once it is.
    public func reached(_ scene: CastScene, tv: TVRemoteModel) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(15)
        while ContinuousClock.now < deadline {
            let there = switch scene {
            case .found: rows.count == 2 && !tv.rows.isEmpty
            case .searching: tv.sheet == .picker && rows.isEmpty
            case .none: tv.sheet == .picker && rows.isEmpty
            case .gone: rows.contains(where: \.gone)
            case .blocked: tv.sheet == .picker && localNetworkDenied
            case .starting, .counting: stage == .starting && tv.sheet == .remote
            case .playing, .muted, .level: stage == .playing && (scene != .muted || muted)
            case .refused, .never, .network, .codec, .deeplink: stage == .failed
            case .lost: stage == .stopped
            case .takenover: !active && takenOver != nil
            case .bar: stage == .playing && tv.sheet == nil
            }
            if there {
                // The sheet's own animation in; for the searching picker its quiet start, for
                // the empty one its wait, and for the counting card its seconds.
                let settle: Duration = switch scene {
                case .searching: .seconds(1.5)
                case .none: .seconds(11)
                case .counting: .seconds(6.5)
                default: .milliseconds(800)
                }
                try? await Task.sleep(for: settle)
                return true
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return false
    }
}
#endif
