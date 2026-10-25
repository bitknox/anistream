// The Mac player's pointer: hidden while the controls are, over the video, and never left hidden
// once the player is gone.
//
// It is hidden with `NSCursor.setHiddenUntilMouseMoves`, which keeps no count: the next move of
// the mouse shows it whatever the app does, and showing it again is one call that is safe to
// repeat. `NSCursor.hide()` and SwiftUI's `pointerVisibility(.hidden)` keep a count of their own,
// which a player that goes away while its pointer is hidden (Back or Escape during the idle
// hide, full screen ending, the next episode, Picture in Picture) can leave unbalanced, the
// pointer gone everywhere in the app until it quits.

#if os(macOS)
import AppKit
import SwiftUI

/// What hides and shows the pointer: AppKit's, or a test's.
@MainActor
protocol PointerCursor: AnyObject {
    /// Hide the pointer until the mouse next moves.
    func hideUntilMoved()
    /// Show it now, hidden or not.
    func show()
}

/// AppKit's pointer.
@MainActor
final class SystemPointerCursor: PointerCursor {
    func hideUntilMoved() { NSCursor.setHiddenUntilMouseMoves(true) }
    func show() { NSCursor.setHiddenUntilMouseMoves(false) }
}

/// Whether the player's pointer is hidden, from what the player says of itself: the controls
/// gone, the pointer over the video, the window in front. Anything else shows it.
@MainActor
final class PlayerPointer {
    static let shared = PlayerPointer(cursor: SystemPointerCursor(), safetyNet: true)

    private let cursor: PointerCursor
    private var observers: [NSObjectProtocol] = []

    /// With `safetyNet`, the app becoming active and any window closing show the pointer too,
    /// whether or not a player is there to say so.
    init(cursor: PointerCursor, safetyNet: Bool = false) {
        self.cursor = cursor
        guard safetyNet else { return }
        let centre = NotificationCenter.default
        for name in [NSApplication.didBecomeActiveNotification, NSWindow.willCloseNotification] {
            observers.append(centre.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.release() }
            })
        }
    }

    /// Whether the pointer may hide: the controls have gone with nobody using them, and nothing
    /// on screen asks for a click. Never while the episode is loading, nor over the failure's
    /// Choose Source and Close, nor over Are You Still Watching.
    static func idle(controlsShown: Bool, loading: Bool, failed: Bool, askingStillWatching: Bool) -> Bool {
        !controlsShown && !loading && !failed && !askingStillWatching
    }

    /// The controls have gone (`idle`) or come back, with the pointer over the video of the key
    /// window of the active app (`over`) or not.
    func update(idle: Bool, over: Bool) {
        if idle, over { cursor.hideUntilMoved() } else { cursor.show() }
    }

    /// The player is going, or its window is no longer in front: the pointer shows.
    func release() {
        cursor.show()
    }
}

/// The player's pointer hidden while `idle`, and shown when the pointer leaves the video, the
/// window stops being key, full screen ends, the app stops being active, or the view goes.
/// `PlayerPointer.shared` is the one owner of the hidden pointer.
struct IdlePointer: ViewModifier {
    let idle: Bool
    let window: NSWindow?
    var pointer: PlayerPointer = .shared
    @State private var inside = false

    func body(content: Content) -> some View {
        content
            .onContinuousHover { phase in
                switch phase {
                case .active: if !inside { inside = true }
                case .ended: inside = false
                }
            }
            .onChange(of: idle) { update() }
            .onChange(of: inside) { update() }
            .onReceive(NotificationCenter.default.publisher(for: NSWindow.didResignKeyNotification)) { note in
                guard (note.object as? NSWindow) === window else { return }
                pointer.release()
            }
            .onReceive(NotificationCenter.default.publisher(for: NSApplication.didResignActiveNotification)) { _ in
                pointer.release()
            }
            .onReceive(NotificationCenter.default.publisher(for: NSWindow.willExitFullScreenNotification)) { note in
                guard (note.object as? NSWindow) === window else { return }
                pointer.release()
            }
            .onDisappear { pointer.release() }
    }

    private func update() {
        let front = window?.isKeyWindow == true && NSApp.isActive
        pointer.update(idle: idle, over: inside && front)
    }
}
#endif
