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
    /// A span of time in full words a voice reads well, in the person's language: "2 hours 5
    /// minutes", "1 day". The units follow `Wording.countdown`, the largest two that still read
    /// at a glance.
    static func span(_ seconds: Int64) -> String {
        let minutes = max(1, seconds / 60)
        let hours = minutes / 60
        let days = hours / 24
        let allowed: Set<Duration.UnitsFormatStyle.Unit> =
            days >= 7 ? [.days] : days >= 1 ? [.days, .hours] : hours >= 1 ? [.hours, .minutes] : [.minutes]
        return Duration.seconds(minutes * 60)
            .formatted(.units(allowed: allowed, width: .wide, maximumUnitCount: 2).locale(AirTime.locale))
    }

    /// How long ago, said in full: "out 2 hours ago", or "just now" under a minute.
    static func out(_ seconds: Int64) -> String {
        seconds < 60 ? L.timeJustNow : L.accessOutAgo(span: span(seconds))
    }

    /// Descriptions put one after the other, as a voice reads them.
    static func join(_ parts: [String]) -> String {
        parts.dropFirst().reduce(parts.first ?? "") { L.accessThen(first: $0, next: $1) }
    }

    /// An episode as the descriptions name it: "Episode 7", with "Dubbed" after a dub.
    static func episode(_ number: Int, dubbed: Bool) -> String {
        let name = L.episodeNumber(number: number)
        return dubbed ? L.accessThen(first: name, next: L.audioDubbed) : name
    }
}

extension ContinueItem {
    /// Where it stands, in words: "Episode 7, 14 minutes left", "Episode 9, out 2 hours ago", and
    /// for a dub, "Episode 7, Dubbed, 14 minutes left".
    var spokenProgress: String {
        if !playable {
            return airing.airs().map { L.accessCaughtUpAirs(episode: Int(episode), when: $0) }
                ?? L.accessCaughtUpUnaired(episode: Int(episode))
        }
        var parts = [Spoken.episode(Int(episode), dubbed: dubbed)]
        if let minutes = minutesLeft {
            parts.append(L.accessMinutesLeft(minutes: minutes))
        } else if let resume {
            parts.append(L.accessStoppedAt(time: resume.clock))
        } else if let airedAgo {
            parts.append(Spoken.out(airedAgo))
        }
        return Spoken.join(parts)
    }
}

extension Broadcast {
    /// The broadcast in words: "Episode 8, out 1 day 2 hours ago", "Episode 9 at 21:30, in 1
    /// hour 30 minutes", "Episode 9, Dub at about 21:30, in 2 days", "Episode 7, watched".
    func spoken(now: Date) -> String {
        let name = Spoken.episode(Int(episode), dubbed: dubbed)
        if watched { return Spoken.join([name, L.episodeWatched.lowercased(with: AirTime.locale)]) }
        let seconds = Int64(airs.timeIntervalSince(now))
        if seconds <= 0 { return Spoken.join([name, Spoken.out(-seconds)]) }
        let time = CalendarWords.time(airs)
        var parts = [entry.airingApproximate ? L.accessAtAbout(episode: name, time: time) : L.accessAt(episode: name, time: time)]
        if let delayedFrom { parts.append(L.accessDelayedFrom(time: CalendarWords.time(delayedFrom))) }
        parts.append(L.accessInSpan(span: Spoken.span(seconds)))
        return Spoken.join(parts)
    }
}

extension EpisodeRow {
    /// The episode's name as VoiceOver reads it: "Episode 7, The Lantern Road".
    var spokenName: String {
        Spoken.join([L.episodeNumber(number: Int(episode: number))] + (title.map { [$0] } ?? []))
    }

    /// Where the episode stands, in words: "watched", "14 minutes left", "airs Sunday 21:30",
    /// "24 minutes", with "filler" after.
    func spokenValue(_ entry: Entry, current: Bool, upcoming: Bool) -> [String] {
        var parts: [String] = []
        if completed { parts.append(L.episodeWatched.lowercased(with: AirTime.locale)) }
        if current, let resume = entry.resume, let fraction = resume.fraction, fraction > 0 {
            let left = max(1, Int(((resume.position / fraction - resume.position) / 60).rounded()))
            parts.append(L.accessMinutesLeft(minutes: left))
        } else if upcoming {
            parts.append(nextAiring(entry).map { L.accessAirs(when: $0) } ?? L.accessNotOut)
        } else if let seconds = durationSecs {
            parts.append(L.accessMinutes(minutes: max(1, Int((Double(seconds) / 60).rounded()))))
        }
        if kind == .filler { parts.append(L.episodeFiller.lowercased(with: AirTime.locale)) }
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
            .accessibilityValue(Text(value ?? ""))
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
            .accessibilityInputLabels([Text(L.episodeNumber(number: Int(episode: row.number)))])
    }

    /// The Unwatched episodes rotor over a title's episode rows, each marked as an entry of
    /// `space` by its number.
    func unwatchedRotor(_ rows: [EpisodeRow], in space: Namespace.ID) -> some View {
        let unwatched = rows.filter { !$0.completed }
        return accessibilityRotor(Text(L.accessRotorUnwatched)) {
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
            let airing = count == 0 ? L.accessNothingAiring : L.episodesCount(count: count)
            return RotorItem(id: day, label: L.accessDayAiring(day: CalendarWords.weekday(day), airing: airing))
        }
        return accessibilityRotor(Text(L.accessRotorDays)) {
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
                let standing = row.state == .queued ? L.accessQueued : row.fraction.formatted(.percent.precision(.fractionLength(0)).locale(AirTime.locale))
                entries.append(RotorItem(id: row.id, label: L.accessDownload(title: title, episode: L.episodeNumber(number: Int(episode: row.episode)), standing: standing)))
            }
        }
        return accessibilityRotor(Text(L.accessRotorInProgress)) {
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
        let action = TitleAction(entry, episodes: store.titles.episodes[entry.id])
        if action.playable, store.titles.unlisted(entry.id) {
            Button(AddSourceWords.menu) { navigator.openProviders() }
        } else if action.playable {
            Button(action.resume.map { L.titleButtonResume(episode: Int(action.episode), time: $0.clock) } ?? L.titleButtonPlay(episode: Int(action.episode))) {
                store.send(.play(id: entry.id, episode: String(action.episode)))
            }
        }
        Button(L.titleOpenAnilist) { store.openOnAniList(entry.id) }
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
            var words = AttributedString(item.notice.words.firstCapitalized)
            words.accessibilitySpeechAnnouncementPriority = item.isAlert ? .high : .default
            AccessibilityNotification.Announcement(words).post()
        }
    }
}
