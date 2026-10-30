// The match sheet's states for screenshots, on the Unsure scene: `-AnistreamSheet match-<state>`
// over 900001's page. Only the Fixtures app stages them; elsewhere nothing happens.

import AnistreamKit
import AnistreamStrings
import Foundation

/// A match sheet state staged for a screenshot.
public struct MatchShot: Equatable, Sendable {
    /// Whether the sheet stands as if opened from Fix this match.
    public let fromMenu: Bool
    /// What its field holds.
    public let query: String
}

extension AppStore {
    /// The Unsure scene's first source, as the scene names it.
    static let shotSource = (id: "hotaru-open-archive", name: "Hotaru Open Archive")

    /// Whether the launch sheet `name` is one of the match sheet's, and so opens it: every
    /// `match-` state but `match-picked` and `match-cleared`, which are the page's.
    public static func matchSheetOpens(_ name: String?) -> MatchPickerShape? {
        guard let name, name == "match" || name.hasPrefix("match-") else { return nil }
        switch name {
        case "match-picked", "match-cleared": return nil
        case "match", "match-picking": return .asked
        default: return .fromMenu
        }
    }

    /// Bring the match for `id` to the state the launch sheet `name` names, for a screenshot,
    /// once the page's load has said what the title is matched to. What the sheet's field then
    /// holds, or `nil` when there is no such state.
    public func stageMatchShot(_ id: AnilistId, name: String?) async -> MatchShot? {
        #if FIXTURES
        guard let name, name == "match" || name.hasPrefix("match-") else { return nil }
        let deadline = ContinuousClock.now + .seconds(20)
        while !match.known.contains(id), ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(50))
        }
        let source = Self.shotSource
        let title = titles[id]?.title ?? ""
        let chosen = { (byYou: Bool) in
            self.match.matched(id: id, to: CurrentMatch(
                providerId: source.id, source: source.name, title: "Hoshi Tabi Diaries", chosenByYou: byYou
            ))
        }
        defer { lastApplied = .now }
        switch name {
        case "match-menu":
            chosen(false)
            // Searching, with no answer to come.
            match.searching(id, query: title)
            return MatchShot(fromMenu: true, query: title)
        case "match-results":
            chosen(true)
            searchMatch(id, query: "Stardust Wayfarer", providerId: source.id)
            return MatchShot(fromMenu: true, query: "Stardust Wayfarer")
        case "match-empty":
            searchMatch(id, query: "hoshikuzu tabibto", providerId: source.id)
            return MatchShot(fromMenu: true, query: "hoshikuzu tabibto")
        case "match-failed":
            match.searching(id, query: "Stardust Wayfarer")
            match.searchFailed(id: id, query: "Stardust Wayfarer", reason: L.matchSearchTimedOut)
            return MatchShot(fromMenu: true, query: "Stardust Wayfarer")
        case "match-picking":
            // The best one picked, with no answer to come.
            if let best = match.asked[id]?.candidates.first {
                match.picking(id, key: best.key, title: best.title, source: source.name)
            }
            return MatchShot(fromMenu: false, query: "")
        case "match-picked", "match-cleared":
            let show = MatchCandidate(
                title: "Stardust Wayfarer", key: "\(source.id):stardust-wayfarer", similarity: 1,
                rejected: nil, year: 2024, episodes: 12, format: .tv
            )
            pickMatch(id, providerId: source.id, source: source.name, candidate: show)
            if name == "match-cleared" {
                while match.current[id]?.chosenByYou != true, ContinuousClock.now < deadline {
                    try? await Task.sleep(for: .milliseconds(50))
                }
                matchAutomatically(id)
            }
            return MatchShot(fromMenu: false, query: "")
        default:
            return MatchShot(fromMenu: false, query: "")
        }
        #else
        return nil
        #endif
    }
}

/// How a launch sheet opens the match sheet.
public enum MatchPickerShape: Sendable {
    case asked, fromMenu
}
