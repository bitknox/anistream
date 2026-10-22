// Where the app is and the network the device uses, as the system says, told to the engine: in the
// background its torrents wait, but for the episode playing, and on cellular or in Low Data Mode
// only what the settings let run there does.

import AnistreamKit
import Foundation
import Network

/// Tells the engine where the app is (`Command::AppPhase`) and the network the device uses
/// (`Command::NetworkPath`), each when it changes.
///
/// - iPhone and iPad: in the background when the scene is, playing or not as the player is, or
///   with its downloads going on while the system gives them a window (`ContinuedDownloads`),
///   and said again when either changes there. Back in the foreground when the scene is active.
///   It alone tells the engine where the app is, so the two never say different things.
/// - The Mac: always in the foreground. A Mac app keeps running behind other windows, minimised
///   or hidden, and its downloads and seeding with it; nothing takes its loopback servers away.
/// - Both: the network from `NWPathMonitor`, an expensive path as cellular (a phone's own, or
///   the Mac on a phone's hotspot) and a constrained one as Low Data Mode.
@MainActor
public final class SystemPhases {
    /// What the system says of the app's scene.
    public enum Scene: Sendable {
        case active, inactive, background
    }

    private let send: @MainActor (Command) -> Void
    private let playing: @MainActor () -> Bool
    private let downloading: @MainActor () -> Bool
    private let backgrounds: Bool
    private var away = false
    private var said: AppState?
    private var path: (cellular: Bool, constrained: Bool)?
    private var monitor: NWPathMonitor?

    /// Whether this device has a background as the engine means one: a phone's or an iPad's.
    public static var hasBackground: Bool {
        #if os(iOS)
        true
        #else
        false
        #endif
    }

    /// `send` hands the engine a command; `playing` says whether the player plays now, its audio
    /// going on behind the lock screen; `downloading`, whether the system has a window open for
    /// the downloads to go on in.
    public init(
        send: @escaping @MainActor (Command) -> Void,
        playing: @escaping @MainActor () -> Bool,
        downloading: @escaping @MainActor () -> Bool = { false },
        backgrounds: Bool = SystemPhases.hasBackground
    ) {
        self.send = send
        self.playing = playing
        self.downloading = downloading
        self.backgrounds = backgrounds
    }

    /// The scene moved to `scene`. Inactive is the moment the app switcher or a call's banner
    /// covers it, and changes nothing.
    public func scene(_ scene: Scene) {
        switch scene {
        case .active: away = false
        case .background: away = backgrounds
        case .inactive: return
        }
        tell()
    }

    /// The player started or stopped playing: in the background, what waits follows it.
    public func playingChanged() {
        guard away else { return }
        tell()
    }

    /// The system opened or closed the downloads' window: in the background, what waits
    /// follows it.
    public func downloadsWindowChanged() {
        guard away else { return }
        tell()
    }

    private func tell() {
        let phase: AppState =
            !away ? .foreground : downloading() ? .backgroundDownloads : .background(playing: playing())
        guard phase != said else { return }
        said = phase
        let how = phase == .backgroundDownloads ? ", downloading" : away && playing() ? ", playing" : ""
        AppLog.info("phases", "app \(away ? "in the background" : "in the foreground")\(how)")
        send(.appPhase(phase: phase))
    }

    /// The device uses a network that is `expensive`, cellular or a hotspot, or `constrained`,
    /// in Low Data Mode, or neither.
    public func network(expensive: Bool, constrained: Bool) {
        guard path.map({ $0 != (expensive, constrained) }) ?? true else { return }
        path = (expensive, constrained)
        AppLog.info("phases", "network: \(expensive ? "cellular" : "not cellular")\(constrained ? ", low data mode" : "")")
        send(.networkPath(cellular: expensive, constrained: constrained))
    }

    /// Follow the system's network path from now on, said once as it first stands.
    public func watchNetwork() {
        guard monitor == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            let (expensive, constrained) = (path.isExpensive, path.isConstrained)
            Task { @MainActor in self?.network(expensive: expensive, constrained: constrained) }
        }
        monitor.start(queue: DispatchQueue(label: "tv.anistream.network-path"))
        self.monitor = monitor
    }
}
