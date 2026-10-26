// What the Mac's Now Playing says of a TV it drives.

#if os(macOS)
import AnistreamKit
import MediaPlayer
import Testing

@testable import AnistreamPlayerUI

@MainActor
struct TVNowPlayingTests {
    func card(paused: Bool = false, loading: Bool = false) -> FfiNowCard {
        FfiNowCard(
            id: 900_001, title: "Stardust Wayfarer", episode: "2", episodeLine: "Episode 2 · The Night Platform",
            loading: loading, position: 30, duration: 1440, paused: paused, speed: 1.25, volume: 40,
            audio: [], subtitles: [], subtitlesOff: false, skip: nil)
    }

    @Test func theTVsEpisodeIsShownAtItsPlayhead() {
        let info = TVNowPlaying.info(card(), tvName: "Living room", playhead: 42, live: true)
        #expect(info[MPMediaItemPropertyTitle] as? String == "Episode 2 · The Night Platform")
        #expect(info[MPMediaItemPropertyArtist] as? String == "Stardust Wayfarer")
        #expect(info[MPMediaItemPropertyAlbumTitle] as? String == "Playing on Living room")
        #expect(info[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double == 42)
        #expect(info[MPMediaItemPropertyPlaybackDuration] as? Double == 1440)
        #expect(info[MPNowPlayingInfoPropertyPlaybackRate] as? Double == 1.25)
    }

    @Test func theClockStopsWhilePausedStartingOrLost() {
        for (now, live) in [(card(paused: true), true), (card(loading: true), true), (card(), false)] {
            #expect(!TVNowPlaying.playing(now, live: live))
            let info = TVNowPlaying.info(now, tvName: "Living room", playhead: 42, live: live)
            #expect(info[MPNowPlayingInfoPropertyPlaybackRate] as? Double == 0)
        }
        #expect(TVNowPlaying.playing(card(), live: true))
    }
}
#endif
