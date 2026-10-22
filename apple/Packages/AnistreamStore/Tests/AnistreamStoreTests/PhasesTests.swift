// Where the app is and the network the device uses, as the engine is told them.

import AnistreamKit
@testable import AnistreamStore
import Testing

/// What a test's phases sent, and whether its player plays.
@MainActor
private final class Told {
    var sent: [Command] = []
    var playing = false
    var downloading = false

    func phases(backgrounds: Bool) -> SystemPhases {
        SystemPhases(
            send: { [unowned self] in sent.append($0) },
            playing: { [unowned self] in playing },
            downloading: { [unowned self] in downloading },
            backgrounds: backgrounds)
    }
}

@MainActor
struct PhasesTests {
    @Test func aPhoneSaysTheBackgroundAndWhetherItPlaysThereEachTimeItChanges() {
        let told = Told()
        let phases = told.phases(backgrounds: true)
        phases.scene(.active)
        // The app switcher's moment over the app: nothing.
        phases.scene(.inactive)
        told.playing = true
        phases.scene(.background)
        phases.playingChanged()
        // The episode ends, or is paused, with the phone locked: everything waits now.
        told.playing = false
        phases.playingChanged()
        phases.scene(.active)
        // In the foreground the player is no business of the phases.
        told.playing = true
        phases.playingChanged()
        #expect(told.sent == [
            .appPhase(phase: .foreground),
            .appPhase(phase: .background(playing: true)),
            .appPhase(phase: .background(playing: false)),
            .appPhase(phase: .foreground),
        ])
    }

    @Test func aDownloadsWindowInTheBackgroundIsSaidThereAndItsEndToo() {
        let told = Told()
        let phases = told.phases(backgrounds: true)
        // A window already open as the app leaves: the downloads go on.
        told.downloading = true
        phases.scene(.background)
        // The player is no business of the phases while the window is open.
        told.playing = true
        phases.playingChanged()
        // The window ends, its downloads done or its time up: what plays goes on, the rest waits.
        told.downloading = false
        phases.downloadsWindowChanged()
        // One opened later, away.
        told.downloading = true
        phases.downloadsWindowChanged()
        phases.downloadsWindowChanged()
        // Back in front, a window is nothing to the engine.
        phases.scene(.active)
        phases.downloadsWindowChanged()
        #expect(told.sent == [
            .appPhase(phase: .backgroundDownloads),
            .appPhase(phase: .background(playing: true)),
            .appPhase(phase: .backgroundDownloads),
            .appPhase(phase: .foreground),
        ])
    }

    @Test func aMacIsNeverInTheBackground() {
        // Behind other windows, minimised or hidden, its downloads and seeding run on.
        let told = Told()
        let phases = told.phases(backgrounds: false)
        phases.scene(.active)
        phases.scene(.background)
        phases.playingChanged()
        phases.scene(.active)
        #expect(told.sent == [.appPhase(phase: .foreground)])
    }

    @Test func theNetworkIsSaidAsCellularAndLowDataModeEachTimeItChanges() {
        let told = Told()
        let phases = told.phases(backgrounds: true)
        phases.network(expensive: false, constrained: false)
        phases.network(expensive: false, constrained: false)
        phases.network(expensive: true, constrained: false)
        phases.network(expensive: true, constrained: true)
        phases.network(expensive: false, constrained: false)
        #expect(told.sent == [
            .networkPath(cellular: false, constrained: false),
            .networkPath(cellular: true, constrained: false),
            .networkPath(cellular: true, constrained: true),
            .networkPath(cellular: false, constrained: false),
        ])
    }
}
