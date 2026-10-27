// How a title's fields read on screen.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

extension Entry {
    /// The title as the screens head it: the engine's, already in the form Settings › Title
    /// language chose (romaji, English, or as the AniList account reads).
    public var showTitle: String {
        title
    }

    /// The other form, beneath the first: romaji under English, English under romaji. `nil` when
    /// it would only repeat the title.
    public var alternateTitle: String? {
        secondary.flatMap { $0.isEmpty || $0 == title ? nil : $0 }
    }

    /// The metadata line under the title: "TV · 24 episodes · Summer 2026 ·
    /// Atelier Hotaru", with "18+" first for a title AniList marks adult.
    public var metadataLine: String {
        var parts: [String] = adult ? ["18+"] : []
        if let format { parts.append(TitleWords.format(format)) }
        if let episodes, episodes > 0, format != "MOVIE" {
            parts.append(L.episodesCount(count: Int(episodes)))
        }
        if let seasonWords { parts.append(seasonWords) }
        if let studio { parts.append(studio) }
        return parts.joined(separator: " · ")
    }

    /// The cover's own colour, for its plate, when AniList gives one.
    var plateColor: Color? {
        coverColor.flatMap(Color.init(hex:))
    }

    /// "Also on Lantern Stream", when the title's page names where else it streams.
    public var alsoOn: String? {
        availableOn.isEmpty ? nil : L.titleAlsoOn(places: ListFormatter.localizedString(byJoining: availableOn))
    }
}

extension Int {
    /// An episode's number, which the engine keeps as text, as a count for the words: a half
    /// episode counts as the whole.
    init(episode number: String) {
        self = Int(number) ?? Int(Double(number) ?? 0)
    }
}

extension ContinueItem {
    /// "Episode 7 · The Lantern Road", with the episode's name when its table has one, and a dub
    /// marked after the episode: "Episode 7 · Dubbed · The Lantern Road".
    @MainActor
    func episodeLine(_ titles: TitleCache) -> String {
        let number = String(episode)
        let name = titles.episodes[id]?.first { $0.number == number }?.title
        let n = Int(episode)
        if let name {
            return dubbed ? L.episodeWithTitleDubbed(number: n, title: name) : L.episodeWithTitle(number: n, title: name)
        }
        return dubbed ? L.titleEpisodeLongDubbed(episode: n) : L.episodeNumber(number: n)
    }

    /// "E7 · 14 min left", or "E7 Dubbed · 14 min left" for a dub, when the episode was left
    /// part-way and its runtime is known.
    var leftLine: String? {
        timeLeft.map { dubbed ? L.continueRowLeftDubbed(episode: Int(episode), left: $0) : L.continueRowLeft(episode: Int(episode), left: $0) }
    }

    /// "14 min left", when the episode was left part-way and its runtime is known.
    var timeLeft: String? {
        minutesLeft.map { L.timeMinutesLeft(minutes: $0) }
    }
}
