// The store hands the playback's events to the player the app hosts.

import AnistreamKit
import Testing

@testable import AnistreamStore

@MainActor
struct PlaybackHookTests {
    @Test func thePlaybacksEventsGoToThePlayer() {
        let store = AppStore()
        var seen: [Event] = []
        store.onPlayback = { seen.append($0) }
        store.apply([
            .playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1),
            .playerOps(playback: 1, ops: [.open(fullscreen: false, speed: nil, volume: nil)]),
            .status(.verbatim(text: "Finding a source")),
            .playerUi(playback: 1, ui: .skipCleared),
            .playbackKept,
            .playbackEnded(watched: true, ending: .complete(next: nil), rollingOn: false),
        ])
        #expect(seen == [
            .playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1),
            .playerOps(playback: 1, ops: [.open(fullscreen: false, speed: nil, volume: nil)]),
            .playerUi(playback: 1, ui: .skipCleared),
            .playbackKept,
            .playbackEnded(watched: true, ending: .complete(next: nil), rollingOn: false),
        ])
    }
}
