// The menu bar's Playback menu and Window › Picture in Picture, as MacMenuBar draws them, carried
// out on the player the app hosts. The app's shell puts them in its menus.

import AnistreamKit
import SwiftUI

extension FocusedValues {
    /// Whether the player's window is the one in front, set by the app on the player's scene.
    /// The Playback menu's keys without a modifier (Space, the arrows, S, M, [ and ]) only act
    /// then, so typing in the main window never pauses the player.
    @Entry public var playerInFront: Bool?
}

/// The items of the Playback menu. Dimmed in place when nothing plays, never hidden.
public struct PlaybackMenu: View {
    let model: PlayerModel
    @FocusedValue(\.playerInFront) private var inFront

    public init(model: PlayerModel) {
        self.model = model
    }

    /// The speeds the Speed menu offers, slowest first.
    static let speeds: [Double] = [0.5, 0.75, 1, 1.25, 1.5, 2]

    public var body: some View {
        let playing = model.isPresented
        // Keys with no modifier only while the player is in front.
        let keys = playing && inFront == true
        Button(model.paused ? "Play" : "Pause") { model.togglePause() }
            .keyboardShortcut(.space, modifiers: [])
            .disabled(!keys)
        Divider()
        Button("Skip Forward 10 Seconds") { model.seek(by: 10) }
            .keyboardShortcut(.rightArrow, modifiers: [])
            .disabled(!keys)
        Button("Skip Back 10 Seconds") { model.seek(by: -10) }
            .keyboardShortcut(.leftArrow, modifiers: [])
            .disabled(!keys)
        Divider()
        Button("Next Episode") { model.step(1) }
            .keyboardShortcut(.rightArrow, modifiers: .command)
            .disabled(!playing)
        Button("Previous Episode") { model.step(-1) }
            .keyboardShortcut(.leftArrow, modifiers: .command)
            .disabled(!playing)
        Button(model.skip?.kind == .ending ? "Skip Ending" : "Skip Opening") { model.takeSkip() }
            .keyboardShortcut("s", modifiers: [])
            .disabled(!keys || model.skip == nil)
        Divider()
        Menu("Speed") {
            Button("Slower") { model.setSpeed(Self.step(model.speed, by: -1)) }
                .keyboardShortcut("[", modifiers: [])
                .disabled(!keys || model.speed <= Self.speeds.first!)
            Button("Faster") { model.setSpeed(Self.step(model.speed, by: 1)) }
                .keyboardShortcut("]", modifiers: [])
                .disabled(!keys || model.speed >= Self.speeds.last!)
            Divider()
            ForEach(Self.speeds, id: \.self) { speed in
                Toggle(Self.name(speed), isOn: Binding(
                    get: { abs(model.speed - speed) < 0.01 },
                    set: { _ in model.setSpeed(speed) }
                ))
                .keyboardShortcut(speed == 1 ? KeyboardShortcut(.delete, modifiers: []) : nil)
            }
        }
        .disabled(!playing)
        Menu("Subtitles and Audio") {
            Section("Audio") {
                ForEach(model.audioTracks) { track in
                    Toggle(track.name, isOn: Binding(
                        get: { track.id == model.audioTrack },
                        set: { _ in model.selectAudio(track.id) }
                    ))
                }
            }
            Section("Subtitles") {
                Toggle("Off", isOn: Binding(
                    get: { model.subtitleTrack == nil },
                    set: { _ in model.selectSubtitle(nil) }
                ))
                ForEach(model.subtitleTracks) { track in
                    Toggle(track.name, isOn: Binding(
                        get: { track.id == model.subtitleTrack },
                        set: { _ in model.selectSubtitle(track.id) }
                    ))
                }
            }
        }
        .disabled(!playing)
        Divider()
        Button("Volume Up") { model.stepVolume(by: 5) }
            .keyboardShortcut(.upArrow, modifiers: [])
            .disabled(!keys)
        Button("Volume Down") { model.stepVolume(by: -5) }
            .keyboardShortcut(.downArrow, modifiers: [])
            .disabled(!keys)
        Button(model.muted ? "Unmute" : "Mute") { model.toggleMute() }
            .keyboardShortcut("m", modifiers: [])
            .disabled(!keys)
        Divider()
        Button(model.fullScreen ? "Exit Full Screen" : "Enter Full Screen") { model.fullScreen.toggle() }
            .keyboardShortcut("f", modifiers: [.command, .control])
            .disabled(!playing)
        Toggle("Stats for Nerds", isOn: Binding(get: { model.statsShown }, set: { model.showStats($0) }))
            .keyboardShortcut("i", modifiers: [])
            .disabled(!keys)
        // Picture in Picture is the Window menu's, PictureInPictureMenuItem below.
    }

    /// The next speed on the ladder toward `by`'s sign, from wherever `speed` is.
    static func step(_ speed: Double, by: Int) -> Double {
        by < 0
            ? speeds.last { $0 < speed - 0.01 } ?? speeds.first!
            : speeds.first { $0 > speed + 0.01 } ?? speeds.last!
    }

    /// "Normal", "1.25×".
    static func name(_ speed: Double) -> String {
        speed == 1 ? "Normal" : String(format: "%g×", speed)
    }
}

/// Window › Picture in Picture: ⌥⌘P.
public struct PictureInPictureMenuItem: View {
    let model: PlayerModel

    public init(model: PlayerModel) {
        self.model = model
    }

    public var body: some View {
        Toggle(
            "Picture in Picture",
            isOn: Binding(
                get: { model.pictureInPicturePhase != .inline },
                set: { _ in model.togglePictureInPicture() })
        )
        .keyboardShortcut("p", modifiers: [.command, .option])
        .disabled(!model.pictureInPictureAvailable && model.pictureInPicturePhase == .inline)
    }
}
