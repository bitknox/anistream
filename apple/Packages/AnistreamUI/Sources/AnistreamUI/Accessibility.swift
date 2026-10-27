// How the screens read with VoiceOver and Voice Control (ux_part3, Accessibility): a cover or row
// is one element named for its title, valued by its progress in words, with its context menu's
// items as actions; notices are announced.

import Accessibility
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

extension Entry {
    /// The title as VoiceOver reads it: the show title, then the other form where it differs.
    /// The romaji, whichever it is, is read in Japanese.
    var spokenTitle: Text {
        var spoken = romajiRead(showTitle)
        if let other = alternateTitle {
            spoken.append(AttributedString(", "))
            spoken.append(romajiRead(other))
        }
        return Text(spoken)
    }

    private func romajiRead(_ form: String) -> AttributedString {
        var words = AttributedString(form)
        if form == titleRomaji { words.languageIdentifier = "ja" }
        return words
    }
}

enum Spoken {
    /// Screen shorthand in words a voice reads well: "Ep 7 of 24" reads "Episode 7 of 24", "out
    /// 2h ago" reads "out 2 hours ago", "14 min left" reads "14 minutes left".
    static func words(_ text: String) -> String {
        expand(text.replacingOccurrences(of: "Ep ", with: "Episode "))
    }

    /// "2h 5m" to "2 hours 5 minutes", "1d" to "1 day", "14 min" to "14 minutes".
    private static func expand(_ text: String) -> String {
        let pattern = /(\d+)\s?(min|d|h|m|s)\b/
        return text.replacing(pattern) { match in
            let count = Int(match.1) ?? 0
            let word = switch match.2 {
            case "d": "day"
            case "h": "hour"
            case "s": "second"
            default: "minute"
            }
            return "\(count) \(word)\(count == 1 ? "" : "s")"
        }
    }
}

extension ContinueItem {
    /// Where it stands, in words: "Episode 7, 14 minutes left", "Episode 9, out 2 hours ago", and
    /// for a dub, as the row marks it, "Episode 7, Dub, 14 minutes left".
    var spokenProgress: String {
        if !playable {
            return "Caught up, episode \(episode) " + (airing.airs().map { "airs \($0)" } ?? "is not out yet")
        }
        var parts = ["Episode \(episode)"] + (mark.map { [$0] } ?? [])
        if let minutes = minutesLeft {
            parts.append(Wording.plural(minutes, "minute") + " left")
        } else if let resume {
            parts.append("stopped at \(resume.clock)")
        } else if let airedAgo {
            parts.append("out " + Spoken.words(Wording.ago(airedAgo)))
        }
        return parts.joined(separator: ", ")
    }
}

extension Broadcast {
    /// The broadcast in words: "Episode 8, out 1 day 2 hours ago", "Episode 9 at 21:30, in 1
    /// hour 30 minutes", "Episode 9, Dub at about 21:30, in 2 days", "Episode 7, watched".
    func spoken(now: Date) -> String {
        let episode = "Episode \(episode)" + (mark.map { ", \($0)" } ?? "")
        if watched { return "\(episode), watched" }
        let seconds = Int64(airs.timeIntervalSince(now))
        if seconds <= 0 { return "\(episode), out " + Spoken.words(Wording.ago(-seconds)) }
        let delay = delayedFrom.map { ", delayed from \(CalendarWords.time($0))" } ?? ""
        let at = entry.airingApproximate ? "at about" : "at"
        return "\(episode) \(at) \(CalendarWords.time(airs))\(delay), in " + Spoken.words(Wording.countdown(seconds))
    }
}

extension EpisodeRow {
    /// The episode's name as VoiceOver reads it: "Episode 7, The Lantern Road".
    var spokenName: String {
        title.map { "Episode \(number), \($0)" } ?? "Episode \(number)"
    }

    /// Where the episode stands, in words: "watched", "14 minutes left", "airs Sunday 21:30",
    /// "24 minutes", with "filler" after.
    func spokenValue(_ entry: Entry, current: Bool, upcoming: Bool) -> [String] {
        var parts: [String] = []
        if completed { parts.append("watched") }
        if current, let resume = entry.resume, let fraction = resume.fraction, fraction > 0 {
            let left = max(1, Int(((resume.position / fraction - resume.position) / 60).rounded()))
            parts.append(Wording.plural(left, "minute") + " left")
        } else if upcoming {
            parts.append(nextAiring(entry).map { "airs \($0)" } ?? "not out yet")
        } else if let seconds = durationSecs {
            parts.append(Wording.plural(max(1, Int((Double(seconds) / 60).rounded())), "minute"))
        }
        if kind == .filler { parts.append("filler") }
        if unsourced { parts.append(L.episodeUnsourced) }
        return parts
    }
}

extension View {
    /// One element for a title: its name, its progress in words, and its context menu's items as
    /// its custom actions. The default action is the control's own: it opens the title. Voice
    /// Control takes either title.
    func titleElement(_ entry: Entry, value: String?, @ViewBuilder actions: () -> some View) -> some View {
        accessibilityElement(children: .combine)
            .accessibilityLabel(entry.spokenTitle)
            .accessibilityValue(Text(value.map(Spoken.words) ?? ""))
            .accessibilityActions(actions)
            .accessibilityInputLabels(Array(Set([entry.showTitle, entry.title])).sorted().map { Text($0) })
    }

    /// One element for an episode: "Episode 7, The Lantern Road", valued "watched", "14 minutes
    /// left" or "not out yet", with its menu's items as actions. The default action plays it.
    func episodeElement(_ row: EpisodeRow, value: [String], @ViewBuilder actions: () -> some View) -> some View {
        accessibilityElement(children: .combine)
            .accessibilityLabel(row.spokenName)
            .accessibilityValue(value.joined(separator: ", "))
            .accessibilityActions(actions)
            .accessibilityInputLabels([Text("Episode \(row.number)")])
    }

    /// The Unwatched episodes rotor over a title's episode rows, each marked as an entry of
    /// `space` by its number.
    func unwatchedRotor(_ rows: [EpisodeRow], in space: Namespace.ID) -> some View {
        let unwatched = rows.filter { !$0.completed }
        return accessibilityRotor("Unwatched episodes") {
            ForEach(unwatched, id: \.number) { row in
                AccessibilityRotorEntry(Text(row.spokenName), id: row.number, in: space)
            }
        }
    }

    /// The Days rotor over a calendar's day headings, each with how many episodes air on it,
    /// marked as entries of `space` by their date.
    func daysRotor(_ days: [Date], broadcasts: [Broadcast], in space: Namespace.ID) -> some View {
        let entries = days.map { day in
            let count = broadcasts.filter { Calendar.current.isDate($0.airs, inSameDayAs: day) }.count
            let airing = count == 0 ? "nothing airing" : Wording.plural(count, "episode")
            return RotorItem(id: day, label: "\(CalendarWords.weekday(day)), \(airing)")
        }
        return accessibilityRotor("Days") {
            ForEach(entries) { item in
                AccessibilityRotorEntry(Text(item.label), id: item.id, in: space)
            }
        }
    }

    /// The In progress rotor over the downloads still under way, running, paused or waiting,
    /// each read as "Hoshikuzu no Tabibito, Episode 8, 45 percent", marked as entries of `space`
    /// by their id.
    func inProgressRotor(_ groups: [DownloadGroup], titles: TitleCache, in space: Namespace.ID) -> some View {
        var entries: [RotorItem<Int64>] = []
        for group in groups {
            let title = titles[group.id]?.showTitle ?? group.title
            for row in group.rows where [.active, .paused, .queued].contains(row.state) {
                let standing = row.state == .queued ? "queued" : "\(Int((row.fraction * 100).rounded())) percent"
                entries.append(RotorItem(id: row.id, label: "\(title), Episode \(row.episode), \(standing)"))
            }
        }
        return accessibilityRotor("In progress") {
            ForEach(entries) { item in
                AccessibilityRotorEntry(Text(item.label), id: item.id, in: space)
            }
        }
    }
}

extension View {
    /// For a list that moves its own selection with the arrow keys: no focus ring, since the
    /// selection says where the keys are, except with Full Keyboard Access on, where the system's
    /// ring, in the accent colour, shows which part of the window has them (ux_part3: never
    /// hidden).
    func listFocusEffect() -> some View {
        #if os(macOS)
        focusEffectDisabled(!NSApplication.shared.isFullKeyboardAccessEnabled)
        #else
        focusEffectDisabled()
        #endif
    }
}

extension View {
    /// Esc closes a sheet whose one button is Done, as Cancel closes the others (ux_part3: every
    /// sheet has a default button on Return and a cancel on Esc).
    func closesOnEscape(_ close: @escaping () -> Void) -> some View {
        #if os(macOS)
        onExitCommand(perform: close)
        #else
        self
        #endif
    }
}

/// One entry of a custom rotor: what it reads, and the view it moves to.
struct RotorItem<ID: Hashable>: Identifiable {
    let id: ID
    let label: String
}

/// What a cover offers beside opening it, as VoiceOver actions: what plays next, and the
/// title's AniList page. The list a title is on stays in the title page's own menu.
struct TitleActions: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    var body: some View {
        let action = TitleAction(entry)
        if action.playable, store.titles.unlisted(entry.id) {
            Button(AddSourceWords.menu) { navigator.openProviders() }
        } else if action.playable {
            Button(action.resume == nil
                   ? cased(mac: "Start Episode \(action.episode)", touch: "Start episode \(action.episode)")
                   : cased(mac: "Resume Episode \(action.episode)", touch: "Resume episode \(action.episode)")) {
                store.send(.play(id: entry.id, episode: String(action.episode)))
            }
        }
        Button("Open on AniList") { store.openOnAniList(entry.id) }
    }
}

/// Every notice said aloud as it is posted, an alert at high priority.
struct NoticeAnnouncer: ViewModifier {
    @Environment(AppStore.self) private var store
    /// Off where the notice is shown elsewhere, so it is read once.
    var enabled = true

    func body(content: Content) -> some View {
        content.onChange(of: store.notices.current?.id) { _, id in
            guard enabled, id != nil, let item = store.notices.current else { return }
            var words = AttributedString(item.notice.text.firstCapitalized)
            words.accessibilitySpeechAnnouncementPriority = item.isAlert ? .high : .default
            AccessibilityNotification.Announcement(words).post()
        }
    }
}
