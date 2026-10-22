import Foundation
import Testing
@testable import AnistreamWidgetData

/// MacCalendar's Tuesday, with fixture titles: what the boards' Airing today shows at 19:30.
let tuesday: [SnapshotAiring] = [
    SnapshotAiring(id: 900004, title: "Hoshikuzu no Tabibito 2nd Season", episode: 3, airsAt: at(day: 20, 13),
               onList: true, watched: true),
    SnapshotAiring(id: 900006, title: "Natsukaze Relay", episode: 9, airsAt: at(day: 20, 17, 30), onList: true,
               translation: .sub),
    SnapshotAiring(id: 900005, title: "Hagane no Niwashi", episode: 5, airsAt: at(day: 20, 19)),
    SnapshotAiring(id: 900006, title: "Natsukaze Relay", episode: 9, airsAt: at(day: 20, 21), onList: true,
               translation: .dub),
    SnapshotAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: at(day: 20, 22), onList: true),
    SnapshotAiring(id: 900003, title: "Tsukiakari no Teishajo", episode: 2, airsAt: at(day: 20, 22, 30)),
    SnapshotAiring(id: 900001, title: "Hoshikuzu no Tabibito", episode: 8, airsAt: at(day: 20, 23, 45), onList: true),
]

let now = at(day: 20, 19, 30)
let format = WidgetFormat(calendar: copenhagen, locale: Locale(identifier: "en_GB"))

func signedIn(_ continuing: [SnapshotContinue] = [], airing: [SnapshotAiring] = tuesday, nextDays: [SnapshotAiring] = []) -> WidgetSnapshot {
    WidgetSnapshot(
        version: 2, generatedAt: at(day: 20, 19, 10), signedIn: true, tracker: "anilist",
        continueWatching: continuing, airingToday: airing, nextDays: nextDays)
}

@Suite("Continue watching's states")
struct ContinueStateTests {
    let item = SnapshotContinue(id: 900001, title: "Hoshikuzu no Tabibito", episode: 7, fraction: 0.42, resume: true, minutesLeft: 14)

    @Test("No snapshot, or signed out with nothing played here: connect AniList")
    func signedOut() {
        #expect(ContinuePresentation.content(nil, at: now) == .signedOut)
        var snapshot = signedIn()
        snapshot.signedIn = false
        #expect(ContinuePresentation.content(snapshot, at: now) == .signedOut)
    }

    @Test("Signed out with something played here shows it")
    func playedHere() {
        var snapshot = signedIn([item])
        snapshot.signedIn = false
        #expect(ContinuePresentation.content(snapshot, at: now) == .shows([item], updated: nil))
    }

    @Test("Caught up names the next episode due, and its art from the schedule")
    func caughtUp() {
        var snapshot = signedIn(airing: [SnapshotAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 6,
                                                    airsAt: at(day: 20, 22), onList: true, art: Art(wide: "w.jpg"))])
        let next = SnapshotNextAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: at(day: 20, 22))
        snapshot.nextAiring = next
        #expect(ContinuePresentation.content(snapshot, at: now) == .caughtUp(next, art: Art(wide: "w.jpg")))
        // Once it has aired, it is no longer next.
        #expect(ContinuePresentation.content(snapshot, at: at(day: 20, 22, 1)) == .caughtUp(nil, art: Art()))
    }

    @Test("A version 1 file with nothing to continue reads as caught up")
    func versionOneEmpty() {
        let snapshot = WidgetSnapshot(version: 1, generatedAt: at(day: 20, 19, 10))
        #expect(ContinuePresentation.content(snapshot, at: now) == .caughtUp(nil, art: Art()))
    }

    @Test("Past a day, the content stays and says when it is from")
    func stale() {
        var snapshot = signedIn([item])
        snapshot.generatedAt = at(day: 18, 20)
        #expect(ContinuePresentation.content(snapshot, at: now) == .shows([item], updated: at(day: 18, 20)))
        #expect(format.updated(at(day: 18, 20), from: now) == "Updated Sunday")
        #expect(format.updated(at(day: 19, 20), from: now) == "Updated yesterday")
        #expect(format.updated(at(day: 1, 20), from: now) == "Updated 1 October")
    }

    @Test("What each item says")
    func lines() {
        #expect(ContinuePresentation.line(item) == "E7 · 14 min left")
        #expect(ContinuePresentation.action(item) == "Resume E7")
        #expect(ContinuePresentation.timeLeft(item) == "14 min left")
        let start = SnapshotContinue(id: 900002, title: "Kumo no Ue no Toshokan", episode: 4, resume: false, minutesLeft: 24)
        #expect(ContinuePresentation.line(start) == "Start E4")
        #expect(ContinuePresentation.action(start) == "Start E4")
        #expect(ContinuePresentation.timeLeft(start) == nil)
        #expect(start.progress == nil)
        #expect(ContinuePresentation.accessibilityLabel(item) == "Hoshikuzu no Tabibito, episode 7, 14 minutes left")
    }

    @Test("Caught up's day and time")
    func airs() {
        let next = SnapshotNextAiring(id: 900001, title: "Hoshikuzu no Tabibito", episode: 10, airsAt: at(day: 22, 16, 30))
        #expect(format.airs(next, from: now) == "Episode 10 airs Thursday 16:30")
        #expect(format.airs(next, from: at(day: 21, 9)) == "Episode 10 airs tomorrow 16:30")
        #expect(format.nextLine(next, from: now) == "Next: Thursday 16:30 · Hoshikuzu no Tabibito · Episode 10")
        #expect(format.nextLineShort(next, from: now) == "Next: Thursday 16:30, Hoshikuzu no Tabibito E10")
    }
}

@Suite("Airing today's states")
struct AiringStateTests {
    func rows(_ content: AiringContent) -> [AiringRow] {
        if case .rows(_, let rows, _, _) = content { return rows }
        Issue.record("not rows: \(content)")
        return []
    }

    @Test("All airing, your list's shows marked, each in its state")
    func marked() {
        let content = AiringPresentation.content(
            snapshot: signedIn(), publicSchedule: nil, filter: .all, at: now, calendar: copenhagen)
        let rows = rows(content)
        #expect(rows.count == 7)
        #expect(rows.map(\.marked) == [true, true, false, true, true, false, true])
        #expect(rows.map(\.status) == [.watched, .out, .aired, .upcoming, .upcoming, .upcoming, .upcoming])
        if case .rows(let day, _, let marks, let tomorrow) = content {
            #expect(day == at(day: 20, 0))
            #expect(marks)
            #expect(tomorrow == [])
        }
    }

    @Test("My list leaves the others out")
    func myList() {
        let content = AiringPresentation.content(
            snapshot: signedIn(), publicSchedule: nil, filter: .myList, at: now, calendar: copenhagen)
        #expect(rows(content).map(\.item.id) == [900004, 900006, 900006, 900002, 900001])
    }

    @Test("Signed out, nothing is marked and nothing is out")
    func signedOut() {
        var snapshot = signedIn()
        snapshot.signedIn = false
        let content = AiringPresentation.content(
            snapshot: snapshot, publicSchedule: nil, filter: .myList, at: now, calendar: copenhagen)
        let rows = rows(content)
        #expect(rows.count == 7)
        #expect(rows.allSatisfy { !$0.marked })
        #expect(!rows.contains { $0.status == .out || $0.status == .watched })
    }

    @Test("No snapshot: AniList's public schedule, unmarked; neither: no schedule")
    func publicSchedule() {
        let unmarked = tuesday.map { SnapshotAiring(id: $0.id, title: $0.title, episode: $0.episode, airsAt: $0.airsAt) }
        let content = AiringPresentation.content(
            snapshot: nil, publicSchedule: unmarked, filter: .all, at: now, calendar: copenhagen)
        #expect(rows(content).count == 7)
        #expect(rows(content).allSatisfy { !$0.marked })
        #expect(AiringPresentation.content(snapshot: nil, publicSchedule: nil, filter: .all, at: now) == .noSchedule)
    }

    @Test("Nothing today names the next from your list")
    func nothingToday() {
        let thursday = SnapshotAiring(id: 900001, title: "Hoshikuzu no Tabibito", episode: 10, airsAt: at(day: 22, 16, 30), onList: true)
        let other = SnapshotAiring(id: 900003, title: "Tsukiakari no Teishajo", episode: 3, airsAt: at(day: 21, 20))
        let snapshot = signedIn(airing: [], nextDays: [other, thursday])
        let content = AiringPresentation.content(
            snapshot: snapshot, publicSchedule: nil, filter: .all, at: now, calendar: copenhagen)
        #expect(content == .nothingToday(next: SnapshotNextAiring(id: 900001, title: "Hoshikuzu no Tabibito", episode: 10,
                                                          airsAt: at(day: 22, 16, 30))))
    }

    @Test("A week on, the schedule has run out")
    func stale() {
        let content = AiringPresentation.content(
            snapshot: signedIn(), publicSchedule: nil, filter: .all, at: at(day: 27, 9), calendar: copenhagen)
        #expect(content == .noSchedule)
    }

    @Test("The next day comes from the snapshot's next days, at midnight")
    func nextDay() {
        let wednesday = SnapshotAiring(id: 900003, title: "Tsukiakari no Teishajo", episode: 3, airsAt: at(day: 21, 20))
        let snapshot = signedIn(nextDays: [wednesday])
        let content = AiringPresentation.content(
            snapshot: snapshot, publicSchedule: nil, filter: .all, at: at(day: 21, 0), calendar: copenhagen)
        #expect(rows(content).map(\.item.id) == [900003])
        let today = AiringPresentation.content(
            snapshot: snapshot, publicSchedule: nil, filter: .all, at: now, calendar: copenhagen)
        if case .rows(_, _, _, let tomorrow) = today { #expect(tomorrow == [wednesday]) }
    }

    @Test("Medium's five: what is to come, then the list show that is out, as the board has it")
    func window() {
        let rows = rows(AiringPresentation.content(
            snapshot: signedIn(), publicSchedule: nil, filter: .all, at: now, calendar: copenhagen))
        let five = AiringPresentation.window(rows, count: 5)
        #expect(five.map { format.time($0.item.airsAt) } == ["17:30", "21:00", "22:00", "22:30", "23:45"])
        #expect(AiringPresentation.comingUp(rows, count: 4).map { format.time($0.item.airsAt) }
            == ["21:00", "22:00", "22:30", "23:45"])
        #expect(AiringPresentation.nextUp(rows)?.item.airsAt == at(day: 20, 21))
        // Late in the evening: the last of the day.
        let late = AiringPresentation.content(
            snapshot: signedIn(), publicSchedule: nil, filter: .all, at: at(day: 20, 23, 50), calendar: copenhagen)
        #expect(AiringPresentation.comingUp(self.rows(late), count: 3).map(\.item.id) == [900002, 900003, 900001])
    }

    @Test("A row's words")
    func words() {
        #expect(AiringPresentation.episodeShort(tuesday[3]) == "E9 · Dub")
        #expect(AiringPresentation.episodeLong(tuesday[1]) == "Episode 9 · Sub")
        #expect(AiringPresentation.episodeShort(tuesday[4]) == "E6")
        let row = AiringPresentation.row(for: tuesday[1], marks: true, at: now)
        #expect(AiringPresentation.accessibilityLabel(row, format: format)
            == "Natsukaze Relay, episode 9, 17:30, out, in your list")
        #expect(format.weekday(now) == "Tuesday")
        #expect(format.longDay(now) == "Tuesday 20 October")
    }
}
