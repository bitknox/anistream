// The layer mpv renders into, and the SwiftUI views that host it on the Mac, iPhone and iPad.

import QuartzCore
import SwiftUI

#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

/// The `CAMetalLayer` mpv draws into: its address is mpv's `wid`, and MoltenVK makes a Vulkan
/// surface of it. Made before the view that shows it, so the client can start on it, and usable
/// with no view at all (the tests render with no window).
///
/// - MoltenVK sizes its swapchain from the layer's bounds times `contentsScale`, not from
///   `drawableSize`, so `resize(to:scale:)` sets all three together.
/// - MoltenVK sometimes sets `drawableSize` to 1×1 to force a presentation to finish
///   (mpv-player/mpv#13651), which can leave the layer at 1×1; such sizes are ignored.
/// - MoltenVK switches `wantsExtendedDynamicRangeContent` on from mpv's render thread when
///   libplacebo picks an HDR colour space, and the display enters EDR only when that is set on
///   the main thread, so the setter goes there.
public final class MpvLayer: CAMetalLayer, @unchecked Sendable {
    public override init() {
        super.init()
        framebufferOnly = true
        isOpaque = true
        backgroundColor = CGColor(gray: 0, alpha: 1)
    }

    public override init(layer: Any) { super.init(layer: layer) }

    public required init?(coder: NSCoder) { super.init(coder: coder) }

    /// mpv's `wid`: the layer's own address.
    public var wid: Int64 { Int64(Int(bitPattern: Unmanaged.passUnretained(self).toOpaque())) }

    /// Sizes the layer to `size` points at `scale` pixels a point, with no animation.
    public func resize(to size: CGSize, scale: CGFloat) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        contentsScale = scale
        frame = CGRect(origin: .zero, size: size)
        drawableSize = CGSize(width: (size.width * scale).rounded(), height: (size.height * scale).rounded())
        CATransaction.commit()
    }

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
                // The layer is only touched on the main thread, while this waits.
                let layer = MainThreadOnly(layer: self)
                DispatchQueue.main.sync { layer.layer.setExtendedDynamicRange(newValue) }
            }
        }
    }

    private func setExtendedDynamicRange(_ on: Bool) {
        super.wantsExtendedDynamicRangeContent = on
    }

    private let tapLock = NSLock()
    nonisolated(unsafe) private var tap: (@Sendable (CAMetalDrawable) -> Void)?

    /// Called with each drawable mpv's renderer takes from the layer, on mpv's render thread,
    /// while set: how Picture in Picture sees the frames mpv draws. Nil, the default, costs one
    /// lock per frame.
    public func tapDrawables(_ tap: (@Sendable (CAMetalDrawable) -> Void)?) {
        tapLock.withLock { self.tap = tap }
    }

    /// MoltenVK asks the layer for each frame's drawable here.
    public override func nextDrawable() -> CAMetalDrawable? {
        let drawable = super.nextDrawable()
        if let drawable, let tap = tapLock.withLock({ tap }) { tap(drawable) }
        return drawable
    }
}

private struct MainThreadOnly: @unchecked Sendable {
    let layer: MpvLayer
}

/// The player's picture in SwiftUI: hosts the client's layer, keeps it the view's size, wakes
/// mpv after a resize, and tells it about the screen. The client must have started on `layer`.
public struct PlayerSurface {
    let layer: MpvLayer
    private let onResize: @MainActor () -> Void
    private let onDisplay: @MainActor (DisplayInfo) -> Void
    private let onPointer: @MainActor () -> Void

    public init(layer: MpvLayer, client: MpvClient) {
        self.init(
            layer: layer,
            resized: { Task { await client.surfaceResized() } },
            display: { display in Task { try? await client.updateDisplay(display) } })
    }

    /// The layer, with what to do after it changed size and when it is on a screen, for a host
    /// that holds the client itself: `MpvClient.surfaceResized()` and `updateDisplay(_:)`. On
    /// the Mac, `pointer` is told each time the pointer moves over the picture, whichever window
    /// is key: what brings a player's controls up.
    public init(
        layer: MpvLayer, resized: @escaping @MainActor () -> Void,
        display: @escaping @MainActor (DisplayInfo) -> Void,
        pointer: @escaping @MainActor () -> Void = {}
    ) {
        self.layer = layer
        self.onResize = resized
        self.onDisplay = display
        self.onPointer = pointer
    }

    @MainActor
    fileprivate func pointed() { onPointer() }

    @MainActor
    fileprivate func resized() { onResize() }

    @MainActor
    fileprivate func moved(to display: DisplayInfo) { onDisplay(display) }
}

#if os(macOS)
/// An NSView with the layer as a sublayer, sized in `layout()`.
public final class PlayerHostView: NSView {
    let mpvLayer: MpvLayer
    var onResize: (() -> Void)?
    var onScreen: ((NSScreen) -> Void)?
    var onPointer: (() -> Void)?
    private var observers: [NSObjectProtocol] = []

    init(layer mpvLayer: MpvLayer) {
        self.mpvLayer = mpvLayer
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = CGColor(gray: 0, alpha: 1)
        layer?.addSublayer(mpvLayer)
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    public override var isOpaque: Bool { true }

    /// Clicks pass through the picture to the SwiftUI around it, where the player's gestures
    /// are: as the deepest view under the pointer it would otherwise take them, and AppKit would
    /// hand SwiftUI none.
    public override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// The pointer over the picture, as a tracking area of the view's own reports it: SwiftUI's
    /// hover does not see through to the picture beneath it.
    public override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas { removeTrackingArea(area) }
        addTrackingArea(NSTrackingArea(
            rect: .zero, options: [.mouseMoved, .mouseEnteredAndExited, .activeAlways, .inVisibleRect],
            owner: self, userInfo: nil))
    }

    public override func mouseMoved(with event: NSEvent) { onPointer?() }
    public override func mouseEntered(with event: NSEvent) { onPointer?() }

    public override func layout() {
        super.layout()
        resizeLayer()
    }

    public override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        resizeLayer()
    }

    private func resizeLayer() {
        // The layer moves between hosts (the main window's player and the mini player's panel);
        // only the host that has it now sizes it.
        guard mpvLayer.superlayer === layer else { return }
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        let before = mpvLayer.drawableSize
        mpvLayer.resize(to: bounds.size, scale: scale)
        if mpvLayer.drawableSize != before { onResize?() }
    }

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        guard let window else { return }
        resizeLayer()
        if let screen = window.screen { onScreen?(screen) }
        let center = NotificationCenter.default
        // Another screen, or the same screen's EDR headroom or refresh rate changing.
        for name in [
            NSWindow.didChangeScreenNotification, NSApplication.didChangeScreenParametersNotification,
        ] {
            observers.append(
                center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                    MainActor.assumeIsolated {
                        guard let self, let screen = self.window?.screen else { return }
                        self.resizeLayer()
                        self.onScreen?(screen)
                    }
                })
        }
    }
}

extension PlayerSurface: NSViewRepresentable {
    public func makeNSView(context: Context) -> PlayerHostView {
        let view = PlayerHostView(layer: layer)
        view.onResize = { resized() }
        view.onScreen = { screen in moved(to: DisplayInfo(screen: screen)) }
        view.onPointer = { pointed() }
        return view
    }

    public func updateNSView(_ view: PlayerHostView, context: Context) {}
}

#elseif os(iOS)
/// A UIView with the layer as a sublayer, sized in `layoutSubviews`: rotation, split view and
/// Stage Manager all arrive there.
public final class PlayerHostView: UIView {
    let mpvLayer: MpvLayer
    var onResize: (() -> Void)?
    var onScreen: ((UIScreen) -> Void)?

    init(layer mpvLayer: MpvLayer) {
        self.mpvLayer = mpvLayer
        super.init(frame: .zero)
        backgroundColor = .black
        isOpaque = true
        // Touches belong to the player's gestures around the picture.
        isUserInteractionEnabled = false
        layer.addSublayer(mpvLayer)
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    public override func layoutSubviews() {
        super.layoutSubviews()
        guard mpvLayer.superlayer === layer else { return }
        let scale = window?.windowScene?.screen.nativeScale ?? traitCollection.displayScale
        let before = mpvLayer.drawableSize
        mpvLayer.resize(to: bounds.size, scale: scale)
        if mpvLayer.drawableSize != before { onResize?() }
    }

    public override func didMoveToWindow() {
        super.didMoveToWindow()
        if let screen = window?.windowScene?.screen { onScreen?(screen) }
    }
}

extension PlayerSurface: UIViewRepresentable {
    public func makeUIView(context: Context) -> PlayerHostView {
        let view = PlayerHostView(layer: layer)
        view.onResize = { resized() }
        view.onScreen = { screen in moved(to: DisplayInfo(screen: screen)) }
        return view
    }

    public func updateUIView(_ view: PlayerHostView, context: Context) {}
}
#endif
