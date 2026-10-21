// The rules the screens are drawn by: broadcasts, weeks, a title's action, its seasons, and what
// the palette lists.

import AnistreamKit
import AnistreamStore
@testable import AnistreamUI
import Foundation
import Testing

struct ScreenRulesTests {
    @Test func aBroadcastSaysHowLongSinceOrUntil() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        var e = entry { $0.progress = TitleProgress(done: 7, next: 8) }
        let aired = Broadcast(entry: e, episode: 8, airs: now.addingTimeInterval(-19 * 3600))
        #expect(aired.line(now: now) == "Ep 8 out 19h ago")
        #expect(aired.isNew(now: now))
        #expect(!aired.watched)
        let coming = Broadcast(entry: e, episode: 9, airs: now.addingTimeInterval(90 * 60))
        #expect(coming.line(now: now) == "Ep 9 in 1h 30m")
        #expect(!coming.isNew(now: now))
        e.progress = TitleProgress(done: 8, next: 9)
        #expect(Broadcast(entry: e, episode: 8, airs: aired.airs).watched)
    }

    @Test func aWeekRunsMondayToSunday() {
        // Wednesday 21 October 2026, at noon where the test runs.
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        let wednesday = calendar.date(from: DateComponents(year: 2026, month: 10, day: 21, hour: 12))!
        let days = weekDays(offset: 0, now: wednesday)
        #expect(days.count == 7)
        #expect(calendar.component(.weekday, from: days[0]) == 2, "Monday first")
        #expect(calendar.component(.day, from: days[0]) == 19)
        #expect(CalendarWords.range(days) == "19–25 October 2026")
        #expect(calendar.component(.day, from: weekDays(offset: -1, now: wednesday)[0]) == 12)
    }

    @Test func aTitleSaysWhatItsActionPlays() {
        let resuming = entry {
            $0.progress = TitleProgress(done: 6, next: 7)
            $0.resume = ResumePoint(position: 606, fraction: 0.42)
        }
        #expect(TitleAction(resuming).title == "Resume Episode 7 · 10:06")
        #expect(TitleAction(resuming).sentence == "Resume episode 7 · 10:06")
        let starting = entry { $0.progress = TitleProgress(done: 3, next: 4) }
        #expect(TitleAction(starting).title == "Start Episode 4")
        #expect(TitleAction(entry { _ in }).title == "Start Episode 1")
    }

    @Test func seasonsAreTheSeriesInWatchOrderAndTheRestIsRelated() {
        let first = entry {
            $0.related = [
                RelatedTitle(id: 900_004, title: "Stardust Wayfarer Season 2", relation: "sequel", format: "TV"),
                RelatedTitle(id: 900_010, title: "Wayfarer's Long Road", relation: "sequel", format: "MOVIE"),
            ]
        }
        let family = TitleFamily(first)
        #expect(family.seasons.map(\.id) == [900_001, 900_004])
        #expect(family.seasons.map(\.isThis) == [true, false])
        #expect(family.related.map(\.id) == [900_010])
        #expect(TitleFamily(entry { _ in }).seasons.isEmpty, "one season is no picker")
    }

    @Test func aTitlesPageSaysItsScore() {
        #expect(entry { _ in }.titleMetadata == "TV · 12 episodes · 2024 · Kujaku Animation Works · 82%")
        #expect(entry { _ in }.phoneMetadata == "TV · 12 ep · 2024 · 82%")
    }

    @MainActor
    @Test func thePaletteFindsTitlesAndTheFocusedTitlesCommands() {
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries([entry {
            $0.progress = TitleProgress(done: 6, next: 7)
        }])))
        let navigator = Navigator(route: LaunchRoute())
        var lists = paletteItems(query: "hoshi", store: store, navigator: navigator)
        #expect(lists.titles.map(\.name) == ["Hoshikuzu no Tabibito"])
        #expect(lists.commands.isEmpty, "no title is focused, and no command is called hoshi")
        navigator.focusedTitle = 900_001
        lists = paletteItems(query: "hoshi", store: store, navigator: navigator)
        #expect(lists.commands.map(\.name) == ["Mark Watched", "Hide from Continue", "Show Settings for This Show…"])
        lists = paletteItems(query: "cal", store: store, navigator: navigator)
        #expect(lists.titles.isEmpty)
        #expect(lists.commands.map(\.name).contains("Calendar"))
    }
}
