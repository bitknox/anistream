// How a title's fields read on screen.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
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

    /// The metadata line under the title, set in spaced capitals: "TV · 24 episodes · Summer 2026 ·
    /// Atelier Hotaru", with "18+" first for a title AniList marks adult.
    public var metadataLine: String {
        var parts: [String] = adult ? ["18+"] : []
        if let format { parts.append(TitleWords.format(format)) }
        if let episodes, episodes > 0, format != "MOVIE" {
            parts.append(Wording.plural(Int(episodes), "episode"))
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
        availableOn.isEmpty ? nil : "Also on " + ListFormatter.localizedString(byJoining: availableOn)
    }
}

extension ContinueItem {
    /// "Episode 7 · The Lantern Road", with the episode's name when its table has one, and a dub
    /// marked after the episode: "Episode 7 · Dub · The Lantern Road".
    @MainActor
    func episodeLine(_ titles: TitleCache) -> String {
        let number = String(episode)
        let name = titles.episodes[id]?.first { $0.number == number }?.title
        let episode = marked("Episode \(number)")
        return name.map { "\(episode) · \($0)" } ?? episode
    }

    /// "14 min left", when the episode was left part-way and its runtime is known.
    var timeLeft: String? {
        minutesLeft.map { "\($0) min left" }
    }
}
