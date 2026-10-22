// How a title's fields read on screen.

import AnistreamKit
@testable import AnistreamUI
import Testing

func entry(_ change: (inout Entry) -> Void) -> Entry {
    var entry = Entry(
        id: 900_001, title: "Stardust Wayfarer", secondary: "Hoshikuzu no Tabibito", format: "TV",
        episodes: 12, year: 2024, score: 82, studio: "Kujaku Animation Works", synopsis: "",
        coverUrl: nil, coverColor: nil, bannerUrl: nil, genres: [], availableOn: [], progress: nil, airingIn: nil,
        airingEpisode: nil, nextEpisode: nil, nextAiringIn: nil, lastAired: nil, resume: nil,
        related: [], recommended: [], trailerUrl: nil, listStatus: nil,
        titleRomaji: nil, titleEnglish: nil, titleNative: nil, season: nil, startDate: nil, endDate: nil,
        source: nil, adult: false, detail: nil, status: nil, synopsisSpans: []
    )
    change(&entry)
    return entry
}

struct PresentationTests {
    @Test func theRomajiHeadsATitleWithTheEnglishBeneath() {
        let both = entry { _ in }
        #expect(both.showTitle == "Hoshikuzu no Tabibito")
        #expect(both.alternateTitle == "Stardust Wayfarer")
        let one = entry { $0.title = "Natsukaze Relay"; $0.secondary = nil }
        #expect(one.showTitle == "Natsukaze Relay")
        #expect(one.alternateTitle == nil)
    }

    @Test func theMetadataLineSaysWhatIsKnown() {
        #expect(entry { _ in }.metadataLine == "TV · 12 episodes · 2024 · Kujaku Animation Works")
        #expect(entry { $0.episodes = 1; $0.studio = nil }.metadataLine == "TV · 1 episode · 2024")
        #expect(entry { $0.format = "MOVIE"; $0.episodes = 1 }.metadataLine == "Movie · 2024 · Kujaku Animation Works")
        #expect(entry { $0.adult = true; $0.season = .spring }.metadataLine.hasPrefix("18+ · TV · 12 episodes · Spring 2024"))
    }

    @Test func whereElseItStreamsIsSaidOnce() {
        #expect(entry { _ in }.alsoOn == nil)
        #expect(entry { $0.availableOn = ["Lantern Stream"] }.alsoOn == "Also on Lantern Stream")
    }
}
