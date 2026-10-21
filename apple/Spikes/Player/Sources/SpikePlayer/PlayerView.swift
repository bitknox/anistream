import SwiftUI

#if os(macOS)
import AppKit

/// An NSView whose layer is the `MPVMetalLayer` mpv renders into. It keeps the layer's
/// drawable at the view's size in pixels, and says when that size, the screen or the EDR
/// headroom changes.
public final class PlayerNSView: NSView {
    public let metalLayer = MPVMetalLayer()
    /// The drawable's new size in pixels, after a resize or a move to another screen.
    public var onDrawableSize: ((CGSize) -> Void)?
    public var onScreenChange: ((NSScreen?) -> Void)?
    private var observers: [NSObjectProtocol] = []

    public init() {
        super.init(frame: NSRect(x: 0, y: 0, width: 640, height: 360))
        // Layer-hosting: the layer is set before wantsLayer, so AppKit leaves its contents
        // alone and mpv owns them.
        layer = metalLayer
        wantsLayer = true
        layerContentsRedrawPolicy = .never
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    public override var isOpaque: Bool { true }

    public override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        updateDrawable()
    }

    public override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        updateDrawable()
    }

    public override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        observers.forEach(NotificationCenter.default.removeObserver)
        observers = []
        guard let window else { return }
        updateDrawable()
        let center = NotificationCenter.default
        observers.append(
            center.addObserver(forName: NSWindow.didChangeScreenNotification, object: window, queue: .main) {
                [weak self] _ in
                MainActor.assumeIsolated {
                    self?.updateDrawable()
                    self?.onScreenChange?(self?.window?.screen)
                }
            })
        observers.append(
            center.addObserver(
                forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated { self?.onScreenChange?(self?.window?.screen) }
            })
    }

    private func updateDrawable() {
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        metalLayer.contentsScale = scale
        metalLayer.frame = bounds
        let size = CGSize(width: (bounds.width * scale).rounded(), height: (bounds.height * scale).rounded())
        metalLayer.drawableSize = size
        CATransaction.commit()
        onDrawableSize?(size)
    }
}

/// The player surface for SwiftUI on the Mac.
public struct PlayerSurface: NSViewRepresentable {
    let player: Player

    public init(player: Player) { self.player = player }

    public func makeNSView(context: Context) -> PlayerNSView {
        let view = PlayerNSView()
        view.onDrawableSize = { [player] size in player.drawableSizeChanged(size) }
        view.onScreenChange = { [player] screen in player.screenChanged(screen) }
        player.attach(view.metalLayer)
        return view
    }

    public func updateNSView(_ view: PlayerNSView, context: Context) {}
}

#elseif os(iOS)
import UIKit

/// A UIView whose backing layer is the `MPVMetalLayer`, sized in pixels on every layout:
/// rotation, split view and Stage Manager all arrive through `layoutSubviews`.
public final class PlayerUIView: UIView {
    public override class var layerClass: AnyClass { MPVMetalLayer.self }
    public var metalLayer: MPVMetalLayer { layer as! MPVMetalLayer }
    public var onDrawableSize: ((CGSize) -> Void)?
    public var onScreen: ((UIScreen?) -> Void)?

    public override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .black
        isOpaque = true
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    public override func didMoveToWindow() {
        super.didMoveToWindow()
        onScreen?(window?.windowScene?.screen)
    }

    public override func layoutSubviews() {
        super.layoutSubviews()
        let scale = window?.windowScene?.screen.nativeScale ?? traitCollection.displayScale
        metalLayer.contentsScale = scale
        let size = CGSize(width: (bounds.width * scale).rounded(), height: (bounds.height * scale).rounded())
        metalLayer.drawableSize = size
        onDrawableSize?(size)
    }
}

/// The player surface for SwiftUI on iPhone and iPad.
public struct PlayerSurface: UIViewRepresentable {
    let player: Player

    public init(player: Player) { self.player = player }

    public func makeUIView(context: Context) -> PlayerUIView {
        let view = PlayerUIView(frame: .zero)
        view.onDrawableSize = { [player] size in player.drawableSizeChanged(size) }
        view.onScreen = { [player] screen in player.screenChanged(screen) }
        player.attach(view.metalLayer)
        return view
    }

    public func updateUIView(_ view: PlayerUIView, context: Context) {}
}
#endif
