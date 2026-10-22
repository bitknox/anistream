import QuartzCore

#if os(macOS)
import AppKit
#endif

/// The layer mpv renders into: its pointer is mpv's `wid`, and MoltenVK makes a Vulkan
/// surface of it (`gpu-context=moltenvk`).
///
/// Two workarounds, both from MPVKit's Metal demo:
///
/// - MoltenVK sometimes sets `drawableSize` to 1×1 to force a presentation to complete
///   (mpv-player/mpv#13651). That flickers and can leave the layer at 1×1, so sizes of one
///   pixel or less are ignored.
/// - MoltenVK switches `wantsExtendedDynamicRangeContent` on from mpv's render thread when
///   libplacebo picks an HDR swapchain colour space, and the display only enters EDR when it
///   is set on the main thread. So the setter hops there.
public final class MPVMetalLayer: CAMetalLayer, @unchecked Sendable {
    public override init() {
        super.init()
        framebufferOnly = true
        backgroundColor = CGColor(gray: 0, alpha: 1)
    }

    public override init(layer: Any) { super.init(layer: layer) }

    public required init?(coder: NSCoder) { super.init(coder: coder) }

    public override var drawableSize: CGSize {
        get { super.drawableSize }
        set {
            if Int(newValue.width) > 1 && Int(newValue.height) > 1 {
                super.drawableSize = newValue
            }
        }
    }

    public override var wantsExtendedDynamicRangeContent: Bool {
        get { super.wantsExtendedDynamicRangeContent }
        set {
            if Thread.isMainThread {
                super.wantsExtendedDynamicRangeContent = newValue
            } else {
                nonisolated(unsafe) let layer = self  // the compiler flags it as unneeded, yet errors without it
                DispatchQueue.main.sync { layer.setEDR(newValue) }
            }
        }
    }

    private func setEDR(_ on: Bool) {
        super.wantsExtendedDynamicRangeContent = on
    }

    /// mpv's `wid`: the layer's own address.
    public var wid: Int64 {
        Int64(Int(bitPattern: Unmanaged.passUnretained(self).toOpaque()))
    }

    /// What the layer is set to now, for the report.
    public var state: [String: Any] {
        [
            "drawableSize": [Int(drawableSize.width), Int(drawableSize.height)],
            "bounds": [Double(bounds.width), Double(bounds.height)],
            "contentsScale": Double(contentsScale),
            "pixelFormat": Self.name(of: pixelFormat),
            "colorspace": (colorspace?.name as String?) ?? "none",
            "wantsExtendedDynamicRangeContent": wantsExtendedDynamicRangeContent,
        ]
    }

    static func name(of format: MTLPixelFormat) -> String {
        switch format {
        case .bgra8Unorm: "bgra8Unorm"
        case .bgra8Unorm_srgb: "bgra8Unorm_srgb"
        case .rgba16Float: "rgba16Float"
        case .rgb10a2Unorm: "rgb10a2Unorm"
        case .bgr10a2Unorm: "bgr10a2Unorm"
        case .bgr10_xr: "bgr10_xr"
        case .bgr10_xr_srgb: "bgr10_xr_srgb"
        default: "raw \(format.rawValue)"
        }
    }
}
