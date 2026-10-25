// Closing the Mac window while in a party: the red button and ⌘W ask first, as quitting does, and
// outside a party the window closes as it always has. The window here is never shown.

#if os(macOS)
@testable import AnistreamUI
import AppKit
import Testing

@MainActor
struct PartyCloseTests {
    /// The delegate SwiftUI would have given the window.
    final class Own: NSObject, NSWindowDelegate {
        var asked = 0
        var resized = 0

        func windowShouldClose(_ sender: NSWindow) -> Bool {
            asked += 1
            return true
        }

        func windowDidResize(_ notification: Notification) {
            resized += 1
        }
    }

    func window() -> NSWindow {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 200, height: 100),
            styleMask: [.titled, .closable], backing: .buffered, defer: true)
        window.isReleasedWhenClosed = false
        return window
    }

    @Test func closingAsksOnlyWhileInAParty() {
        var inParty = false
        let own = Own()
        let window = window()
        window.delegate = own
        let guarded = PartyCloseGuard.Holder(asks: { inParty }, found: { _ in })
        window.contentView = guarded
        // In front of the window's own delegate, which still hears the rest.
        #expect(window.delegate === guarded.asker)
        #expect(guarded.asker.original === own)

        #expect(guarded.asker.windowShouldClose(window))
        #expect(own.asked == 1)

        inParty = true
        #expect(!guarded.asker.windowShouldClose(window))
        // The question stops it before the window's own delegate is asked.
        #expect(own.asked == 1)

        window.setContentSize(NSSize(width: 240, height: 120))
        #expect(own.resized >= 1)
    }

    @Test func theGuardGoesInOnce() {
        let own = Own()
        let window = window()
        window.delegate = own
        let guarded = PartyCloseGuard.Holder(asks: { false }, found: { _ in })
        window.contentView = guarded
        guarded.install()
        guarded.install()
        #expect(guarded.asker.original === own)
    }
}
#endif
