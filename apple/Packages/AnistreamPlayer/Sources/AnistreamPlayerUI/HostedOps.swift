// The engine's hosted player protocol (E5) in mpv's terms: each `PlayerOp` as the one call on
// mpv it is, and mpv's events as the `PlayerReport`s the engine reads.

import AnistreamKit
import AnistreamPlayer
import Foundation

/// One call on mpv, for one of the engine's ops.
public enum MpvCall: Sendable, Equatable {
    /// `loadfile <url> replace` with per-file options.
    case load(url: String, options: [String: String])
    /// `sub-add <url> auto`: listed, not selected.
    case addSubtitle(url: String, title: String?, language: String?)
    case set(property: String, value: AnistreamPlayer.MpvValue)
    /// An input command as mpv spells it: `cycle pause`, `add volume 5`, `af add …`, `stop`.
    case command([String])
    case seek(seconds: Double, mode: SeekMode)
    case shaders([String])
    /// Read a property and report it as it stands: `chapter-list` for `RestateChapters`.
    case restate(property: String)
    /// Read each property and report the ones mpv has, for stats for nerds.
    case sample(properties: [String])
}

/// What the host does for one of the engine's ops.
public enum HostedStep: Sendable, Equatable {
    /// Show the player and start mpv in it, at the viewer's speed and volume.
    case open(fullscreen: Bool, speed: Double?, volume: Double?)
    case call(MpvCall)
    /// Full screen on, off, or (`nil`) the other way: the window's, since an mpv drawing into
    /// the app's view has no window of its own to fill the screen with.
    case fullScreen(Bool?)
    /// The viewer stopped: end the file, report that it ended by quitting and that the player
    /// closed, and close it.
    case quit
    /// The playback is over: close the player, and say nothing more.
    case close

    public init(_ op: PlayerOp) {
        switch op {
        case .open(let fullscreen, let speed, let volume):
            self = .open(fullscreen: fullscreen, speed: speed, volume: volume)
        case .load(let url, let options):
            // A later option of the same name wins, as it would on mpv's command line.
            let named = Dictionary(options.map { ($0.name, $0.value) }, uniquingKeysWith: { $1 })
            self = .call(.load(url: url, options: named))
        case .addSubtitle(let url, let title, let language):
            self = .call(.addSubtitle(url: url, title: title, language: language))
        case .set("fullscreen", .flag(let on)):
            self = .fullScreen(on)
        case .set(let property, let value):
            self = .call(.set(property: property, value: AnistreamPlayer.MpvValue(engine: value)))
        case .cycle("fullscreen"), .toggleFullscreen:
            self = .fullScreen(nil)
        case .cycle(let property):
            self = .call(.command(["cycle", property]))
        case .add(let property, let delta):
            self = .call(.command(["add", property, String(delta)]))
        case .seek(let seconds, let absolute):
            self = .call(.seek(seconds: seconds, mode: absolute ? .absolute : .relative))
        case .shaders(let paths):
            self = .call(.shaders(paths))
        case .audioFilter(let add, let filter):
            self = .call(.command(["af", add ? "add" : "remove", filter]))
        case .restateChapters:
            self = .call(.restate(property: "chapter-list"))
        case .sample(let properties):
            self = .call(.sample(properties: properties))
        case .stop:
            self = .call(.command(["stop"]))
        case .quit:
            self = .quit
        case .close:
            self = .close
        }
    }
}

extension AnistreamPlayer.MpvValue {
    /// The engine's value as mpv takes it: a node arrives as JSON text.
    public init(engine value: AnistreamKit.MpvValue) {
        switch value {
        case .flag(let flag): self = .flag(flag)
        case .int(let int): self = .int(int)
        case .double(let double): self = .double(double)
        case .string(let string): self = .string(string)
        case .node(let json): self = (try? AnistreamPlayer.MpvValue(json: json)) ?? .none
        }
    }
}

/// The properties the engine reads, which the host observes from the start.
public let reportedProperties = [
    "time-pos", "duration", "pause", "speed", "volume", "chapter-list", "video-params",
    "current-tracks/video", "track-list", "aid", "sid", "sub-delay", "paused-for-cache",
    "frame-drop-count", "seeking", "demuxer-cache-state", "hwdec-current",
]

extension PlayerReport {
    /// The report for one of mpv's events, when the engine reads it: a property it observes,
    /// a file loaded, a file ended.
    init?(_ event: MpvEvent) {
        switch event {
        case .property(let name, let value):
            guard reportedProperties.contains(name) else { return nil }
            self = .property(name: name, json: value.json)
        case .fileLoaded:
            self = .fileLoaded
        case .endFile(let end):
            self = .ended(reason: PlayerEnd(mpv: end.reason))
        default:
            return nil
        }
    }

    /// The display as the engine reads it. libmpv renders through libplacebo on Metal (by way
    /// of MoltenVK's Vulkan), which runs compute shaders, so ArtCNN can run.
    init(_ display: DisplayInfo) {
        self = .display(
            width: UInt32(clamping: display.width), height: UInt32(clamping: display.height),
            scale: display.scale, refreshHz: display.refreshRate > 0 ? display.refreshRate : nil,
            edrHeadroom: display.edrPotential > 1 ? display.edrHeadroom : nil, compute: true)
    }
}

extension PlayerEnd {
    /// mpv's `end-file` reason by its name.
    init(mpv reason: String) {
        switch reason {
        case "eof": self = .eof
        case "quit": self = .quit
        case "stop": self = .stop
        case "redirect": self = .redirect
        case "error": self = .error
        default: self = .unknown
        }
    }
}
