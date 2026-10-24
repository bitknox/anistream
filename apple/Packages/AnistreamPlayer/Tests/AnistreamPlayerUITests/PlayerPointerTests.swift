// The Mac player's pointer: hidden only while the controls are gone over the video, and shown
// again on every way out of the player, so it can never be left hidden.

#if os(macOS)
import AppKit
import Foundation
import Testing

@testable import AnistreamPlayerUI

/// A pointer that keeps a count, as `NSCursor.hide()` does: it shows only at zero. The player's
/// pointer must never take it above one, and must leave it at zero.
@MainActor
final class CountedCursor: PointerCursor {
    private(set) var hides = 0
    private(set) var most = 0

    var visible: Bool { hides == 0 }

    func hideUntilMoved() {
        // Hidden until the mouse moves is a state, not a count: hiding twice is hiding once.
        hides = 1
        most = max(most, hides)
    }

    func show() {
        hides = 0
    }

    /// The mouse moved: AppKit shows a pointer hidden until it does.
    func moved() {
        hides = 0
    }
}

@MainActor
struct PlayerPointerTests {
    @Test func theControlsGoingOverTheVideoHideItAndTheirComingBackShowsIt() {
        let cursor = CountedCursor()
        let pointer = PlayerPointer(cursor: cursor)
        pointer.update(idle: true, over: true)
        #expect(!cursor.visible)
        pointer.update(idle: false, over: true)
        #expect(cursor.visible)
    }

    @Test func itIsNeverHiddenOffTheVideoNorBehindAnotherWindow() {
        let cursor = CountedCursor()
        let pointer = PlayerPointer(cursor: cursor)
        pointer.update(idle: true, over: false)
        #expect(cursor.visible)
        pointer.update(idle: true, over: true)
        pointer.update(idle: true, over: false)
        #expect(cursor.visible, "the pointer left the video, or the window stopped being key")
    }

    /// A way out of the player while the pointer is hidden: Back or Escape during the idle hide,
    /// the close button, full screen ending, Picture in Picture. Each ends in `release` (the
    /// view going, the window resigning key, the app resigning active), and the pointer shows.
    @Test func aWayOutWhileItIsHiddenShowsIt() {
        let cursor = CountedCursor()
        let pointer = PlayerPointer(cursor: cursor)
        // Hidden several times over, as the idle hide comes and goes: one hide, however many.
        for _ in 0..<5 {
            pointer.update(idle: true, over: true)
            cursor.moved()
            pointer.update(idle: false, over: true)
            pointer.update(idle: true, over: true)
        }
        #expect(!cursor.visible)
        pointer.release()
        #expect(cursor.visible)
        #expect(cursor.most == 1, "never more than one hide to undo")
    }

    /// Johan's way in: the player opens, the episode loads, no source has it, and the failure's
    /// Choose Source and Close come up. The pointer never hides through any of it, and Close
    /// leaves it showing.
    @Test func loadingThenFailingNeverHidesItAndClosingLeavesItShowing() {
        let cursor = CountedCursor()
        let pointer = PlayerPointer(cursor: cursor)
        // Loading, with the controls long gone and the pointer still over the video.
        let loading = PlayerPointer.idle(controlsShown: false, loading: true, failed: false, askingStillWatching: false)
        #expect(!loading)
        pointer.update(idle: loading, over: true)
        #expect(cursor.visible)
        // No source had it: the failure, over the curtain.
        let failed = PlayerPointer.idle(controlsShown: false, loading: true, failed: true, askingStillWatching: false)
        #expect(!failed)
        pointer.update(idle: failed, over: true)
        #expect(cursor.visible, "the failure's buttons need the pointer")
        pointer.release()
        #expect(cursor.visible)
        #expect(cursor.most == 0, "nothing was ever hidden")
    }

    @Test func onlyAnIdlePlayingVideoHidesIt() {
        #expect(PlayerPointer.idle(controlsShown: false, loading: false, failed: false, askingStillWatching: false))
        #expect(!PlayerPointer.idle(controlsShown: true, loading: false, failed: false, askingStillWatching: false))
        #expect(!PlayerPointer.idle(controlsShown: false, loading: false, failed: false, askingStillWatching: true))
        #expect(!PlayerPointer.idle(controlsShown: false, loading: false, failed: true, askingStillWatching: false))
    }

    @Test func theAppComingBackOrAWindowClosingShowsItWithNoPlayerThere() {
        let cursor = CountedCursor()
        let pointer = PlayerPointer(cursor: cursor, safetyNet: true)
        pointer.update(idle: true, over: true)
        #expect(!cursor.visible)
        NotificationCenter.default.post(name: NSApplication.didBecomeActiveNotification, object: nil)
        #expect(cursor.visible)
        pointer.update(idle: true, over: true)
        NotificationCenter.default.post(name: NSWindow.willCloseNotification, object: nil)
        #expect(cursor.visible)
    }

    @Test func releasingAPointerThatShowsAlreadyIsHarmless() {
        let cursor = CountedCursor()
        let pointer = PlayerPointer(cursor: cursor)
        pointer.release()
        pointer.release()
        #expect(cursor.visible)
        pointer.update(idle: true, over: true)
        #expect(!cursor.visible, "and the next idle hide still hides")
    }

    /// Nothing in the apps hides the pointer with a count of its own: `NSCursor.hide()` or
    /// SwiftUI's `pointerVisibility(.hidden)`, which a view going away can leave unbalanced.
    @Test func nothingElseHidesThePointer() throws {
        let packages = URL(filePath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent()
        let banned = ["NSCursor.hide()", ".pointerVisibility(", "CGDisplayHideCursor"]
        var found: [String] = []
        let files = FileManager.default.enumerator(at: packages, includingPropertiesForKeys: nil)
        while let url = files?.nextObject() as? URL {
            let path = url.path
            guard url.pathExtension == "swift", path.contains("/Sources/"), !path.contains("/.build/")
            else { continue }
            let text = try String(contentsOf: url, encoding: .utf8)
            for word in banned where text.contains(word) {
                // Its own note, which names them.
                guard url.lastPathComponent != "PlayerPointer.swift" else { continue }
                found.append("\(url.lastPathComponent): \(word)")
            }
        }
        #expect(found.isEmpty, "\(found)")
    }
}
#endif
