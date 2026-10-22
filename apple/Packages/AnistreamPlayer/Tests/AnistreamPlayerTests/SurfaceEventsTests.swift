// The picture takes no clicks from the SwiftUI around it, so the player's own gestures get them,
// and it reports the pointer moving over it, so the controls can come up.

#if os(macOS)
import AnistreamPlayer
import AppKit
import SwiftUI
import Testing

extension PlayerTests {
    @MainActor @Suite struct SurfaceEventsTests {
        final class Pointer {
            var moves = 0
        }

        /// A window, never shown, holding the picture under a SwiftUI gesture.
        func window(pointer: Pointer) -> NSWindow {
            let view = ZStack {
                Color.black
                PlayerSurface(layer: MpvLayer(), resized: {}, display: { _ in }, pointer: { pointer.moves += 1 })
            }
            .contentShape(.rect)
            .onTapGesture {}
            let window = NSWindow(
                contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled],
                backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.contentView = NSHostingView(rootView: view.frame(width: 400, height: 300))
            window.layoutIfNeeded()
            return window
        }

        @Test func clicksOnThePictureGoToSwiftUI() throws {
            let window = window(pointer: Pointer())
            defer { window.close() }
            // The picture covers the window; AppKit hands a click to the deepest view that takes
            // it, which must be SwiftUI's, where the gestures are, not the picture's.
            let hit = try #require(window.contentView?.hitTest(NSPoint(x: 200, y: 150)))
            #expect(!(hit is PlayerHostView), "\(type(of: hit))")
        }

        @Test func thePointerMovingOverThePictureIsReported() throws {
            let pointer = Pointer()
            let window = window(pointer: pointer)
            defer { window.close() }
            let host = try #require(Self.host(in: window.contentView))
            // AppKit asks for these as the view comes on screen, which this one never does.
            host.updateTrackingAreas()
            #expect(host.trackingAreas.contains { $0.options.contains(.mouseMoved) && $0.options.contains(.activeAlways) })
            let move = try #require(NSEvent.mouseEvent(
                with: .mouseMoved, location: NSPoint(x: 200, y: 150), modifierFlags: [],
                timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 0,
                pressure: 0))
            host.mouseMoved(with: move)
            #expect(pointer.moves == 1)
        }

        static func host(in view: NSView?) -> PlayerHostView? {
            guard let view else { return nil }
            if let host = view as? PlayerHostView { return host }
            return view.subviews.lazy.compactMap { host(in: $0) }.first
        }
    }
}
#endif
