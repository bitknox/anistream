import Foundation

/// The engine's upscaling chains, rebuilt from `crates/anistream-engine/src/playback/shaders.rs`
/// (`choose()` and `Mode::chain`) so the spike sets the same files in the same order. The
/// shaders themselves are read from the repo's `assets/shaders`, where the engine vendors them.
public enum Look: String, CaseIterable, Sendable, Identifiable {
    case off, natural, sharp
    public var id: String { rawValue }
    public var label: String {
        switch self {
        case .off: "Off"
        case .natural: "Natural"
        case .sharp: "Sharp"
        }
    }
}

public enum HDRKind: String, Sendable { case hdr10, hdr10Plus, hlg, dolbyVision }

public struct ShaderSource: Sendable, Equatable {
    public var width: Int
    public var height: Int
    public var hdr: HDRKind?
    public init(width: Int, height: Int, hdr: HDRKind?) {
        self.width = width
        self.height = height
        self.hdr = hdr
    }

    /// `picture::classify`, from mpv's `video-params` and the video track.
    public init(videoParams p: MPVValue, track: MPVValue = .none) {
        width = Int(p["dw"].int ?? p["w"].int ?? 0)
        height = Int(p["dh"].int ?? p["h"].int ?? 0)
        if p["colormatrix"].string == "dolbyvision" || track["dolby-vision-profile"].int == 5 {
            hdr = .dolbyVision
        } else {
            switch p["gamma"].string {
            case "hlg": hdr = .hlg
            case "pq":
                let dynamic = ["scene-max-r", "scene-max-g", "scene-max-b", "scene-avg"]
                    .contains { (p[$0].double ?? 0) > 0 }
                hdr = dynamic ? .hdr10Plus : .hdr10
            default: hdr = nil
            }
        }
    }
}

public struct ShaderDisplay: Sendable, Equatable {
    public var width: Int
    public var height: Int
    public init(width: Int, height: Int) {
        self.width = width
        self.height = height
    }
}

public struct ShaderBudget: Sendable, Equatable {
    public var onBattery = false
    public var eased = 0
    public init(onBattery: Bool = false, eased: Int = 0) {
        self.onBattery = onBattery
        self.eased = eased
    }
}

public struct ShaderChain: Sendable, Equatable {
    public var files: [String]
    public var label: String
    public var upscaler: String?
    public static let off = ShaderChain(files: [], label: "Off", upscaler: nil)
}

public enum Shaders {
    /// Every vendored shader, as `SHADERS` lists them.
    public static let all = [
        "Anime4K_Clamp_Highlights.glsl",
        "Anime4K_Restore_CNN_M.glsl",
        "Anime4K_Restore_CNN_VL.glsl",
        "Anime4K_Restore_CNN_Soft_M.glsl",
        "Anime4K_Restore_CNN_Soft_VL.glsl",
        "Anime4K_Upscale_CNN_x2_S.glsl",
        "Anime4K_Upscale_CNN_x2_M.glsl",
        "Anime4K_Upscale_CNN_x2_VL.glsl",
        "Anime4K_Upscale_Denoise_CNN_x2_M.glsl",
        "Anime4K_Upscale_Denoise_CNN_x2_VL.glsl",
        "Anime4K_AutoDownscalePre_x2.glsl",
        "Anime4K_AutoDownscalePre_x4.glsl",
        "ArtCNN_C4F16.glsl",
        "AniSD_ArtCNN_C4F32_i4.glsl",
        "KrigBilateral.glsl",
        "SSimDownscaler.glsl",
    ]

    static let anisd = "AniSD_ArtCNN_C4F32_i4.glsl"
    static let artcnnLight = "ArtCNN_C4F16.glsl"
    static let krig = "KrigBilateral.glsl"
    static let ssim = "SSimDownscaler.glsl"
    static let fallbackDisplay = ShaderDisplay(width: 1920, height: 1080)
    static let cnnMinScale = 1.3
    static let levels = 2

    /// Where the shaders are: `ANISTREAM_SHADERS`, a bundle's `shaders` folder, or the repo's
    /// `assets/shaders` found from this file.
    public static var directory: URL {
        if let env = ProcessInfo.processInfo.environment["ANISTREAM_SHADERS"] {
            return URL(fileURLWithPath: env)
        }
        if let bundled = Bundle.main.url(forResource: "shaders", withExtension: nil) {
            return bundled
        }
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<6 { url.deleteLastPathComponent() }
        return url.appending(path: "assets/shaders")
    }

    public static func paths(_ chain: ShaderChain) -> [String] {
        chain.files.map { directory.appending(path: $0).path }
    }

    private enum Tier { case sd, hd, fhd }

    private static func tier(_ s: ShaderSource) -> Tier {
        let lines = max(s.height, s.width * 9 / 16)
        return lines < 600 ? .sd : lines < 900 ? .hd : .fhd
    }

    /// `choose()`, rule for rule; see the Rust for the reasons.
    public static func choose(
        _ look: Look, source: ShaderSource, display: ShaderDisplay?, budget: ShaderBudget = .init()
    ) -> ShaderChain {
        if look == .off { return .off }
        switch source.hdr {
        case .dolbyVision: return ShaderChain(files: [], label: "Dolby Vision: none", upscaler: nil)
        case .some: return ShaderChain(files: [krig], label: "HDR: chroma only", upscaler: nil)
        case .none: break
        }
        let d = display.flatMap { $0.width > 0 && $0.height > 0 ? $0 : nil } ?? fallbackDisplay
        let scale =
            source.width == 0 || source.height == 0
            ? 1.0
            : min(Double(d.width) / Double(source.width), Double(d.height) / Double(source.height))
        let level = min(budget.eased + (budget.onBattery ? 1 : 0), levels)
        let tier = tier(source)
        let chroma = {
            scale < 1.0
                ? ShaderChain(files: [krig, ssim], label: "Chroma only", upscaler: nil)
                : ShaderChain(files: [krig], label: "Chroma only", upscaler: nil)
        }

        if scale <= cnnMinScale {
            if look == .sharp, scale >= 1.0, let native = sharpNative(tier, level) { return native }
            return chroma()
        }
        switch look {
        case .off: return .off
        case .natural:
            let rungs: [(String, String)] =
                tier == .sd ? [(anisd, "ArtCNN AniSD"), (artcnnLight, "ArtCNN C4F16")] : [(artcnnLight, "ArtCNN C4F16")]
            guard level < rungs.count else { return chroma() }
            var files = [rungs[level].0, krig]
            if scale < 2.0 { files.append(ssim) }
            return ShaderChain(files: files, label: rungs[level].1, upscaler: "ArtCNN")
        case .sharp:
            let (mode, rungs): (Mode, [Bool]) =
                switch tier {
                case .sd: (.c, [true, false])
                case .hd: (.b, [true, false])
                case .fhd: (.a, [false])
                }
            guard level < rungs.count else { return chroma() }
            let hq = rungs[level]
            return ShaderChain(files: mode.chain(hq: hq), label: mode.label(hq: hq), upscaler: "Anime4K")
        }
    }

    /// Anime4K's documented presets, for measuring rungs `choose` would not pick on this Mac.
    public static func anime4K(_ mode: String, hq: Bool) -> ShaderChain? {
        let m: Mode? =
            switch mode.lowercased() {
            case "a": .a
            case "b": .b
            case "c": .c
            default: nil
            }
        return m.map { ShaderChain(files: $0.chain(hq: hq), label: $0.label(hq: hq), upscaler: "Anime4K") }
    }

    private enum Mode {
        case a, b, c

        func chain(hq: Bool) -> [String] {
            let first: [String] =
                switch (self, hq) {
                case (.a, true): ["Anime4K_Restore_CNN_VL.glsl", "Anime4K_Upscale_CNN_x2_VL.glsl"]
                case (.a, false): ["Anime4K_Restore_CNN_M.glsl", "Anime4K_Upscale_CNN_x2_M.glsl"]
                case (.b, true): ["Anime4K_Restore_CNN_Soft_VL.glsl", "Anime4K_Upscale_CNN_x2_VL.glsl"]
                case (.b, false): ["Anime4K_Restore_CNN_Soft_M.glsl", "Anime4K_Upscale_CNN_x2_M.glsl"]
                case (.c, true): ["Anime4K_Upscale_Denoise_CNN_x2_VL.glsl"]
                case (.c, false): ["Anime4K_Upscale_Denoise_CNN_x2_M.glsl"]
                }
            let last = hq ? "Anime4K_Upscale_CNN_x2_M.glsl" : "Anime4K_Upscale_CNN_x2_S.glsl"
            return ["Anime4K_Clamp_Highlights.glsl"] + first
                + ["Anime4K_AutoDownscalePre_x2.glsl", "Anime4K_AutoDownscalePre_x4.glsl", last]
        }

        func label(hq: Bool) -> String {
            let name =
                switch self {
                case .a: "A"
                case .b: "B"
                case .c: "C"
                }
            return "Anime4K \(name) (\(hq ? "HQ" : "Fast"))"
        }
    }

    private static func sharpNative(_ tier: Tier, _ level: Int) -> ShaderChain? {
        let restore: (String, String)
        switch (tier, level) {
        case (.fhd, 0): restore = ("Anime4K_Restore_CNN_VL.glsl", "Anime4K A restore (HQ)")
        case (.fhd, 1): restore = ("Anime4K_Restore_CNN_M.glsl", "Anime4K A restore (Fast)")
        case (.hd, 0): restore = ("Anime4K_Restore_CNN_Soft_VL.glsl", "Anime4K B restore (HQ)")
        case (.hd, 1): restore = ("Anime4K_Restore_CNN_Soft_M.glsl", "Anime4K B restore (Fast)")
        default: return nil
        }
        return ShaderChain(
            files: ["Anime4K_Clamp_Highlights.glsl", restore.0], label: restore.1, upscaler: "Anime4K")
    }
}
