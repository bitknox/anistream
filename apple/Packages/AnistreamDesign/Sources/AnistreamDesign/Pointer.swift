// The pointer over the app's own buttons on the Mac, and the controls laid over a window's title
// bar strip.

import SwiftUI
#if os(macOS)
import AppKit
#endif

extension View {
    /// The pointing hand over a button the app draws itself, as the boards have every button:
    /// nothing else says it can be clicked. The system's own controls show their own. Nothing
    /// on iPhone and iPad, where the pointer's hover effects do it.
    public func pointingHand(_ on: Bool = true) -> some View {
        #if os(macOS)
        pointerStyle(on ? .link : nil)
        #else
        self
        #endif
    }

    #if os(macOS)
    /// A control laid over the window's title bar strip, as the player's top bar is over the
    /// video. AppKit takes a mouse-down in that strip for a drag of the window whenever the view
    /// under it lets the window move, as SwiftUI's own hosting view does, so the control never
    /// sees the click; and the strip sets the arrow over whatever is in it. So while `active`,
    /// outside full screen (where there is no strip), the clicks on the control are caught as
    /// the app receives them and `action` runs on the release, as the button's would; the
    /// pointer over it is the pointing hand, and `pointed` is told as it comes and goes, for the
    /// hover the control's own cannot see there. The rest of the strip still drags the window,
    /// and VoiceOver and the keys still press the control itself.
    public func titleBarControl(
        active: Bool = true, pointed: @escaping (Bool) -> Void = { _ in }, action: @escaping () -> Void
    ) -> some View {
        background(TitleBarClicks(active: active, pointed: pointed, action: action))
    }
    #endif
}

#if os(macOS)
/// A view at the control's frame that catches its clicks for it.
struct TitleBarClicks: NSViewRepresentable {
    let active: Bool
    let pointed: (Bool) -> Void
    let action: () -> Void

    func makeNSView(context: Context) -> TitleBarClicksView {
        let view = TitleBarClicksView()
        view.pointed = pointed
        view.action = action
        view.active = active
        return view
    }

    func updateNSView(_ view: TitleBarClicksView, context: Context) {
        view.pointed = pointed
        view.action = action
        view.active = active
    }
}

final class TitleBarClicksView: NSView {
    var active = true {
        didSet { if !active { point(false) } }
    }
    var pointed: (Bool) -> Void = { _ in }
    var action: () -> Void = {}
    private var monitor: Any?
    /// Pressed on the control, and not yet released.
    private var pressed = false
    /// The pointer over the control, as last told.
    private var over = false

    /// Never in the way of SwiftUI's own hit testing, nor of accessibility: the control is.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func isAccessibilityElement() -> Bool { false }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        pressed = false
        guard window != nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(
            matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp, .mouseMoved]
        ) { [weak self] event in
            guard let self, event.window === self.window else { return event }
            let taken = MainActor.assumeIsolated { self.take(event) }
            return taken ? nil : event
        }
    }

    /// Whether the event is the control's, and taken from the window.
    private func take(_ event: NSEvent) -> Bool {
        switch event.type {
        case .leftMouseDown:
            guard catching, inside(event) else { return false }
            pressed = true
            return true
        case .leftMouseDragged:
            return pressed
        case .leftMouseUp:
            guard pressed else { return false }
            pressed = false
            if active, inside(event) { action() }
            return true
        case .mouseMoved:
            let now = catching && inside(event)
            point(now)
            // After the strip has set its arrow for this move.
            if now { DispatchQueue.main.async { NSCursor.pointingHand.set() } }
            return false
        default:
            return false
        }
    }

    /// The window has its title bar strip, and the control is up.
    private var catching: Bool {
        guard active, let window else { return false }
        return !window.styleMask.contains(.fullScreen)
    }

    private func inside(_ event: NSEvent) -> Bool {
        bounds.contains(convert(event.locationInWindow, from: nil))
    }

    private func point(_ now: Bool) {
        guard now != over else { return }
        over = now
        pointed(now)
    }

    isolated deinit {
        if let monitor { NSEvent.removeMonitor(monitor) }
    }
}
#endif
