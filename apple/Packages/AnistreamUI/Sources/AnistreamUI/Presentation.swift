// How a title's fields read on screen.

import AnistreamKit
import AnistreamStore
import Foundation

extension Entry {
    /// The title as the screens head it: the romaji when AniList has an English title beside it,
    /// as the boards set it ("Hoshikuzu no Tabibito"), and the one title there is otherwise.
    public var showTitle: String {
        secondary ?? title
    }

    /// The other title, beneath the first: the English one ("Stardust Wayfarer").
    public var alternateTitle: String? {
        secondary == nil ? nil : title
    }

    /// The metadata line under the title, set in spaced capitals: "TV · 12 episodes · 2024 ·
    /// Kujaku Animation Works".
    public var metadataLine: String {
        var parts: [String] = []
        if let format { parts.append(format) }
        if let episodes, episodes > 0, format != "MOVIE" {
            parts.append(Wording.plural(Int(episodes), "episode"))
        }
        if let year { parts.append(String(year)) }
        if let studio { parts.append(studio) }
        return parts.joined(separator: " · ")
    }

    /// "Also on Lantern Stream", when the title's page names where else it streams.
    public var alsoOn: String? {
        availableOn.isEmpty ? nil : "Also on " + ListFormatter.localizedString(byJoining: availableOn)
    }
}

extension ContinueItem {
    /// "Episode 7 · The Lantern Road", with the episode's name when its table has one.
    @MainActor
    func episodeLine(_ titles: TitleCache) -> String {
        let number = String(episode)
        let name = titles.episodes[id]?.first { $0.number == number }?.title
        return name.map { "Episode \(number) · \($0)" } ?? "Episode \(number)"
    }

    /// "14 min left", when the episode was left part-way and its runtime is known.
    var timeLeft: String? {
        minutesLeft.map { "\($0) min left" }
    }
}
