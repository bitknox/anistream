// The engine's hosted player protocol (E5) as this package speaks it: what the engine asks of
// the player, and what the player reports back, in mpv's own vocabulary so the terminal's session
// logic reads both the same way.

import CoreGraphics
import Foundation

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// One of the engine's `PlayerOps`, one case each; `MpvClient.apply(_:)` carries it out.
/// The engine's `Close` is `MpvClient.shutdown()`, since it ends the client.
public enum PlayerOp: Sendable, Equatable {
    /// Replace what is playing, with mpv options for this file only (`start`, `http-header-fields`).
    case load(url: String, options: [String: String])
    /// An external subtitle file, selected or only listed.
    case addSubtitle(url: String, title: String?, language: String?, select: Bool)
    /// Any property, as mpv names it.
    case set(property: String, value: MpvValue)
    case seek(seconds: Double, mode: SeekMode)
    /// The shader chain, as absolute paths in order; empty for none.
    case shaders([String])
    /// An `af` graph, or nil for none (the night mode's compressor and the like).
    case audioFilter(String?)
    case stop
}

public enum SeekMode: String, Sendable, Equatable {
    /// To the second, decoding from the keyframe before it.
    case absolute = "absolute+exact"
    case relative = "relative+exact"
    /// To the nearest keyframe: quick, for scrubbing.
    case keyframe = "absolute+keyframes"
    /// `seconds` is a percentage of the file.
    case percent = "absolute-percent+exact"
}

/// What the player tells the engine, mirroring the engine's `PlaybackEvent`: a property as mpv
/// names it with its value as JSON, which the engine's existing parsers read; one of mpv's
/// events by its name; the end of a file; the display.
public enum PlayerReport: Sendable, Equatable {
    case property(name: String, json: String)
    case event(String)
    case ended(MpvEnd)
    case display(DisplayInfo)
}

/// The screen the player is on, which mpv cannot find out for itself on Apple's platforms: the
/// moltenvk context answers none of mpv's display queries.
public struct DisplayInfo: Sendable, Equatable, Codable {
    /// The whole screen, in pixels, as the engine's upscaler chooser wants it.
    public var width: Int
    public var height: Int
    /// Points to pixels.
    public var scale: Double
    /// The most frames a second the screen shows (120 on ProMotion).
    public var refreshRate: Double
    /// How far above SDR white the screen can go now, and at most (both 1 without EDR).
    public var edrHeadroom: Double
    public var edrPotential: Double
    public var name: String?

    public init(
        width: Int, height: Int, scale: Double, refreshRate: Double, edrHeadroom: Double = 1,
        edrPotential: Double = 1, name: String? = nil
    ) {
        self.width = width
        self.height = height
        self.scale = scale
        self.refreshRate = refreshRate
        self.edrHeadroom = edrHeadroom
        self.edrPotential = edrPotential
        self.name = name
    }

    #if os(macOS)
    @MainActor
    public init(screen: NSScreen) {
        let scale = Double(screen.backingScaleFactor)
        self.init(
            width: Int(screen.frame.width * scale), height: Int(screen.frame.height * scale), scale: scale,
            refreshRate: Double(screen.maximumFramesPerSecond),
            edrHeadroom: Double(screen.maximumExtendedDynamicRangeColorComponentValue),
            edrPotential: Double(screen.maximumPotentialExtendedDynamicRangeColorComponentValue),
            name: screen.localizedName)
    }
    #elseif os(iOS)
    @MainActor
    public init(screen: UIScreen) {
        let size = screen.nativeBounds.size
        self.init(
            width: Int(max(size.width, size.height)), height: Int(min(size.width, size.height)),
            scale: Double(screen.nativeScale), refreshRate: Double(screen.maximumFramesPerSecond),
            edrHeadroom: Double(screen.currentEDRHeadroom), edrPotential: Double(screen.potentialEDRHeadroom),
            name: nil)
    }
    #endif
}
