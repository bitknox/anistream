// Picture in Picture on the Mac, as states and the steps between them. Pure, so the tests can
// walk it without the system's window; `PictureInPicture` carries out the effects.

import Foundation

/// Where the picture is, as Picture in Picture goes.
public enum PictureInPicturePhase: Equatable, Sendable {
    /// In the main window's player: no Picture in Picture.
    case inline
    /// Asked for: the frames are being copied and the system's window is opening.
    case starting
    /// In the system's window. The main window browses.
    case active
    /// The system's window is closing; `restore` says whether the picture goes back into the main
    /// window's player or the playback stops.
    case stopping(restore: Bool)
}

/// What happens to Picture in Picture: the viewer's asks, and the system's answers.
public enum PictureInPictureEvent: Equatable, Sendable {
    /// The button, ⌥⌘P, or the window being minimised or the app hidden while it plays.
    case toggle
    case started
    case failedToStart
    /// The window's button that returns the picture to the app.
    case restore
    /// The system's window closed; after `restore`, or by its close button.
    case stopped
    /// The playback ended on its own (the engine closed the player, the viewer stopped it).
    case playbackClosed
}

/// What `PictureInPicture` does for a step.
public enum PictureInPictureEffect: Equatable, Sendable {
    /// Begin copying mpv's frames, and ask the system for its window.
    case start
    /// Ask the system to close its window, returning the picture to the app.
    case stop
    /// Close the system's window with nothing to return to.
    case dismiss
    /// The picture leaves the main window, which goes back to browsing until the picture
    /// returns or the player closes.
    case leaveWindow
    /// The picture comes back into the main window's player, which comes to the front.
    case returnToWindow
    /// Stop copying frames.
    case endFeed
    /// The viewer closed the window: stop the playback, as the player's own close does.
    case closePlayback
}

public struct PictureInPictureMachine: Equatable, Sendable {
    public private(set) var phase: PictureInPicturePhase = .inline

    public init() {}

    /// The step for `event` from where the machine is, and what to do for it. An event that
    /// means nothing where the machine is changes nothing.
    public mutating func handle(_ event: PictureInPictureEvent) -> [PictureInPictureEffect] {
        switch (phase, event) {
        case (.inline, .toggle):
            phase = .starting
            return [.start]
        case (.starting, .started):
            phase = .active
            return [.leaveWindow]
        case (.starting, .failedToStart):
            phase = .inline
            return [.endFeed]
        case (.starting, .toggle):
            // Asked again before the window opened: close it as soon as it does.
            phase = .stopping(restore: true)
            return [.stop]
        case (.active, .toggle):
            // The app's own button or ⌥⌘P: the system then asks to restore, and the picture
            // comes back.
            phase = .stopping(restore: true)
            return [.stop]
        case (.active, .restore), (.stopping(restore: true), .restore):
            phase = .stopping(restore: true)
            return [.returnToWindow]
        case (.stopping(restore: false), .restore):
            // Nothing to return to: the playback has gone.
            return []
        case (.active, .stopped):
            // Closed with the window's own close button: the viewer is done. The main window
            // stays on the browsing, with no player flashing up in it first.
            phase = .inline
            return [.endFeed, .closePlayback]
        case (.stopping, .stopped):
            phase = .inline
            return [.endFeed]
        case (.stopping, .failedToStart):
            // Closed before the system's window opened, or it never could: the system was never
            // asked, or said no, so no `stopped` follows.
            phase = .inline
            return [.endFeed]
        case (.starting, .playbackClosed), (.active, .playbackClosed), (.stopping(restore: true), .playbackClosed):
            phase = .stopping(restore: false)
            return [.dismiss]
        case (.starting, .stopped):
            phase = .inline
            return [.endFeed]
        default:
            return []
        }
    }

}
