import Foundation
import Observation
import QuartzCore

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

public struct Track: Sendable, Identifiable, Equatable {
    public let id: Int64
    public let type: String
    public let title: String?
    public let lang: String?
    public let codec: String?
    public let selected: Bool
    public var name: String {
        [title, lang, codec].compactMap { $0 }.joined(separator: " · ")
    }
}

/// One player: the mpv instance, the state SwiftUI draws, and the chain of shaders for the
/// look. mpv starts when its surface is made, because `wid` must be set before
/// `mpv_initialize`.
@MainActor @Observable
public final class Player {
    public struct Options: Sendable {
        public var hwdec = "videotoolbox"
        public var hdrHint = true
        public var logLevel = "info"
        public var cacheDir: String?
        public var extra: [(String, String)] = []
        public init() {}
    }

    public nonisolated let mpv: MPVClient
    public let options: Options
    public private(set) var attached = false
    public private(set) var timePos: Double = 0
    public private(set) var duration: Double = 0
    public private(set) var paused = false
    public private(set) var tracks: [Track] = []
    public private(set) var hwdecCurrent = ""
    public private(set) var videoParams: MPVValue = .none
    public private(set) var look: Look = .off
    public private(set) var chain: ShaderChain = .off
    /// Set to pin a chain (a measurement of a rung `choose` would not pick here).
    public var pinnedChain: ShaderChain?
    public private(set) var drawableSize: CGSize = .zero
    /// The screen in physical pixels, as the engine's chooser wants it.
    public private(set) var display: ShaderDisplay?
    public private(set) var edrHeadroom: Double = 1
    public private(set) var edrPotential: Double = 1
    public private(set) var layer: MPVMetalLayer?
    /// Runs on every drawable size change; the spike uses it to try resize workarounds.
    public var onDrawableSize: ((CGSize) -> Void)?

    public init(options: Options = .init()) throws {
        mpv = try MPVClient()
        self.options = options
    }

    func attach(_ layer: MPVMetalLayer) {
        guard !attached else { return }
        self.layer = layer
        do {
            try mpv.setOption("wid", int64: layer.wid)
            try mpv.setOption("vo", "gpu-next")
            try mpv.setOption("gpu-api", "vulkan")
            try mpv.setOption("gpu-context", "moltenvk")
            try mpv.setOption("hwdec", options.hwdec)
            // HDR: let libplacebo pick the swapchain's colour space from the source, so
            // MoltenVK can hand the layer a PQ or extended-linear space and switch EDR on.
            // MPVKit notes it cannot be changed after init without stalls.
            try mpv.setOption("target-colorspace-hint", options.hdrHint ? "yes" : "no")
            try mpv.setOption("keep-open", "yes")
            try mpv.setOption("idle", "yes")
            try mpv.setOption("input-default-bindings", "no")
            try mpv.setOption("ytdl", "no")
            try mpv.setOption("embeddedfonts", "yes")
            if let dir = options.cacheDir {
                try mpv.setOption("gpu-shader-cache-dir", dir)
                try mpv.setOption("icc-cache-dir", dir)
            }
            for (name, value) in options.extra { try mpv.setOption(name, value) }
            mpv.requestLog(level: options.logLevel)
            mpv.addHandler { [weak self] event in
                guard case .property(let name, let value) = event else { return }
                Task { @MainActor in self?.apply(name, value) }
            }
            try mpv.initialize()
            for name in ["time-pos", "duration", "pause", "track-list", "hwdec-current", "video-params"] {
                mpv.observe(name)
            }
            attached = true
        } catch {
            print("mpv setup failed: \(error)")
        }
        #if os(macOS)
        screenChanged(NSScreen.main)
        #endif
    }

    private func apply(_ name: String, _ value: MPVValue) {
        switch name {
        case "time-pos": timePos = value.double ?? 0
        case "duration": duration = value.double ?? 0
        case "pause": paused = value.bool ?? false
        case "hwdec-current": hwdecCurrent = value.string ?? ""
        case "track-list":
            tracks = value.array.map { t in
                Track(
                    id: t["id"].int ?? 0, type: t["type"].string ?? "", title: t["title"].string,
                    lang: t["lang"].string, codec: t["codec"].string, selected: t["selected"].bool ?? false)
            }
        case "video-params":
            let changed = ShaderSource(videoParams: value) != ShaderSource(videoParams: videoParams)
            videoParams = value
            if changed { applyChain() }
        default: break
        }
    }

    // MARK: Controls

    public func load(_ url: String) {
        mpv.commandAsync(["loadfile", url, "replace"])
    }

    public func togglePause() {
        mpv.commandAsync(["cycle", "pause"])
    }

    public func seek(to seconds: Double) {
        mpv.commandAsync(["seek", String(seconds), "absolute+exact"])
    }

    public func selectSubtitle(_ id: Int64?) {
        mpv.setAsync("sid", id.map(String.init) ?? "no")
    }

    public func setLook(_ look: Look) {
        self.look = look
        applyChain()
    }

    /// The chain for the look, the source and the screen, set as one native array.
    public func applyChain() {
        let source = ShaderSource(videoParams: videoParams)
        let chain = pinnedChain ?? Shaders.choose(look, source: source, display: display)
        guard chain != self.chain || pinnedChain != nil else { return }
        self.chain = chain
        let paths = Shaders.paths(chain)
        let mpv = self.mpv
        Task.detached { mpv.set("glsl-shaders", strings: paths) }
    }

    // MARK: Surface

    func drawableSizeChanged(_ size: CGSize) {
        drawableSize = size
        onDrawableSize?(size)
    }

    #if os(macOS)
    func screenChanged(_ screen: NSScreen?) {
        guard let screen else { return }
        let scale = screen.backingScaleFactor
        let newDisplay = ShaderDisplay(
            width: Int(screen.frame.width * scale), height: Int(screen.frame.height * scale))
        edrHeadroom = Double(screen.maximumExtendedDynamicRangeColorComponentValue)
        edrPotential = Double(screen.maximumPotentialExtendedDynamicRangeColorComponentValue)
        if newDisplay != display {
            display = newDisplay
            applyChain()
        }
    }
    #endif

    #if os(iOS)
    func screenChanged(_ screen: UIScreen?) {
        guard let screen else { return }
        let size = screen.nativeBounds.size
        let newDisplay = ShaderDisplay(width: Int(max(size.width, size.height)), height: Int(min(size.width, size.height)))
        edrHeadroom = Double(screen.currentEDRHeadroom)
        edrPotential = Double(screen.potentialEDRHeadroom)
        if newDisplay != display {
            display = newDisplay
            applyChain()
        }
    }
    #endif
}
