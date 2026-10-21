// What the store's tests build their events from.

import AnistreamKit
import AnistreamStore
import Foundation

/// An entry with its id and title and whatever `change` sets, as `Entry::new` makes one.
func entry(_ id: AnilistId, _ title: String, _ change: (inout Entry) -> Void = { _ in }) -> Entry {
    var entry = Entry(
        id: id, title: title, secondary: nil, format: nil, episodes: nil, year: nil, score: nil,
        studio: nil, synopsis: "", coverUrl: nil, coverColor: nil, bannerUrl: nil, genres: [], availableOn: [],
        progress: nil, airingIn: nil, airingEpisode: nil, nextEpisode: nil, nextAiringIn: nil,
        lastAired: nil, resume: nil, related: [], recommended: [], trailerUrl: nil, listStatus: nil,
        titleRomaji: nil, titleEnglish: nil, titleNative: nil, season: nil, startDate: nil, endDate: nil,
        source: nil, adult: false, detail: nil, status: nil, synopsisSpans: []
    )
    change(&entry)
    return entry
}

/// A directory of the test's own, which it removes.
func temporaryRoot() -> URL {
    FileManager.default.temporaryDirectory
        .appending(path: "anistream-store-\(UUID().uuidString)", directoryHint: .isDirectory)
}

/// A host that opens nothing and keeps secrets in memory.
func quietHost() -> AppHost {
    AppHost(secrets: MemorySecrets())
}
