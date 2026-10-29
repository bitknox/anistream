// The menu bar's Playback menu and Window › Picture in Picture, as MacMenuBar draws them, carried
// out on the player the app hosts. The app's shell puts them in its menus.

import AnistreamKit
import AnistreamStrings
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
        Button(model.paused ? L.actionPlay : L.actionPause) { model.togglePause() }
            .keyboardShortcut(.space, modifiers: [])
            .disabled(!keys)
        Divider()
        Button(L.menuSkipForward) { model.seek(by: 10) }
            .keyboardShortcut(.rightArrow, modifiers: [])
            .disabled(!keys)
        Button(L.menuSkipBack) { model.seek(by: -10) }
            .keyboardShortcut(.leftArrow, modifiers: [])
            .disabled(!keys)
        Divider()
        Button(L.menuNextEpisode) { model.step(1) }
            .keyboardShortcut(.rightArrow, modifiers: .command)
            .disabled(!playing)
        Button(L.menuPreviousEpisode) { model.step(-1) }
            .keyboardShortcut(.leftArrow, modifiers: .command)
            .disabled(!playing)
        Button(model.skip?.kind == .ending ? L.menuSkipEnding : L.menuSkipOpening) { model.takeSkip() }
            .keyboardShortcut("s", modifiers: [])
            .disabled(!keys || model.skip == nil)
        Divider()
        Menu(L.playerSpeed) {
            Button(L.playerSlower) { model.setSpeed(Self.step(model.speed, by: -1)) }
                .keyboardShortcut("[", modifiers: [])
                .disabled(!keys || model.speed <= Self.speeds.first!)
            Button(L.playerFaster) { model.setSpeed(Self.step(model.speed, by: 1)) }
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
        Menu(L.menuSubtitlesAndAudio) {
            Section(L.playerAudio) {
                ForEach(model.audioTracks) { track in
                    Toggle(track.name, isOn: Binding(
                        get: { track.id == model.audioTrack },
                        set: { _ in model.selectAudio(track.id) }
                    ))
                }
            }
            Section(L.playerSubtitles) {
                Toggle(L.valueOff, isOn: Binding(
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
            #if os(macOS)
            // Last, apart from the picks the engine remembers: a file is this episode's alone.
            Divider()
            Button(TrackFiles.title(.sub)) { TrackFiles.load(.sub, into: model) }
            Button(TrackFiles.title(.audio)) { TrackFiles.load(.audio, into: model) }
            #endif
        }
        .disabled(!playing)
        Divider()
        Button(L.menuVolumeUp) { model.stepVolume(by: 5) }
            .keyboardShortcut(.upArrow, modifiers: [])
            .disabled(!keys)
        Button(L.menuVolumeDown) { model.stepVolume(by: -5) }
            .keyboardShortcut(.downArrow, modifiers: [])
            .disabled(!keys)
        Button(model.muted ? L.menuUnmute : L.menuMute) { model.toggleMute() }
            .keyboardShortcut("m", modifiers: [])
            .disabled(!keys)
        Divider()
        Button(model.fullScreen ? L.menuExitFullScreen : L.menuEnterFullScreen) { model.fullScreen.toggle() }
            .keyboardShortcut("f", modifiers: [.command, .control])
            .disabled(!playing)
        Toggle(L.menuStatsForNerds, isOn: Binding(get: { model.statsShown }, set: { model.showStats($0) }))
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
        speed == 1 ? L.menuSpeedNormal : speedLabel(speed, whole: false)
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
L.menuPictureInPicture,
            isOn: Binding(
                get: { model.pictureInPicturePhase != .inline },
                set: { _ in model.togglePictureInPicture() })
        )
        .keyboardShortcut("p", modifiers: [.command, .option])
        .disabled(!model.pictureInPictureAvailable && model.pictureInPicturePhase == .inline)
    }
}
