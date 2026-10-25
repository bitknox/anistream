// The playback and airing Live Activities, shown through a fake host: nothing reaches the system.

@testable import AnistreamStore
import AnistreamKit
import AnistreamWidgetData
import Foundation
import Testing

@MainActor
final class FakeLiveActivityHost<State: Equatable>: LiveActivityHost {
    enum Call: Equatable {
        case start(State, Date?)
        case update(State, Date?)
        case end(State?, Date?)
    }

    var calls: [Call] = []
    var isShowing = false
    var isEnabled = true
    /// Whether a start is allowed: not in the background.
    var allows = true

    func start(_ state: State, staleAt: Date?) -> Bool {
        guard allows else { return false }
        calls.append(.start(state, staleAt))
        isShowing = true
        return true
    }

    func update(_ state: State, staleAt: Date?) {
        calls.append(.update(state, staleAt))
    }

    func end(_ state: State?, dismissAt: Date?) {
        calls.append(.end(state, dismissAt))
        isShowing = false
    }
}

@MainActor
struct PlaybackActivityFeedTests {
    let host = FakeLiveActivityHost<PlaybackActivityState>()
    let clock = Clock()
    let titles = TitleCache()

    func feed() -> PlaybackActivityFeed {
        PlaybackActivityFeed(host: host, files: nil, now: { [clock] in clock.now })
    }

    func playing(at position: Double, paused: Bool = false, episode: String = "7") -> PlaybackActivityFeed.Playing {
        .init(id: 900_001, title: "Hoshikuzu no Tabibito", episode: episode, position: position, duration: 1_400, rate: 1, paused: paused)
    }

    @Test func itStartsOnceThereIsALengthAndEndsWhenThePlayerCloses() {
        let feed = feed()
        feed.show(.init(id: 900_001, title: "Hoshikuzu no Tabibito", episode: "7", position: 0, duration: 0, rate: 1, paused: false),
                  titles: titles, artwork: nil)
        #expect(host.calls.isEmpty)
        feed.show(playing(at: 10), titles: titles, artwork: nil)
        guard case .start(let state, let stale) = host.calls.first else {
            Issue.record("did not start: \(host.calls)")
            return
        }
        #expect(state.title == "Hoshikuzu no Tabibito" && state.episode == "7" && !state.paused)
        // Stale a minute after the episode would end, had nobody said otherwise.
        #expect(stale == clock.now.addingTimeInterval(1_390 + 60))
        feed.show(nil, titles: titles, artwork: nil)
        #expect(host.calls.last == .end(nil, nil))
    }

    @Test func playingOnIsNoUpdateButAPauseASeekAndTheNextEpisodeAre() {
        let feed = feed()
        feed.show(playing(at: 10), titles: titles, artwork: nil)
        clock.now += 30
        feed.show(playing(at: 40.2), titles: titles, artwork: nil)
        #expect(host.calls.count == 1)
        feed.show(playing(at: 40.2, paused: true), titles: titles, artwork: nil)
        guard case .update(let paused, let stale) = host.calls.last else {
            Issue.record("no update: \(host.calls)")
            return
        }
        #expect(paused.paused && stale == nil)
        feed.show(playing(at: 600, paused: true), titles: titles, artwork: nil)
        #expect(host.calls.count == 3)
        feed.show(playing(at: 0, episode: "8"), titles: titles, artwork: nil)
        guard case .update(let next, _) = host.calls.last else {
            Issue.record("no update: \(host.calls)")
            return
        }
        #expect(next.episode == "8" && host.calls.count == 4)
    }

    @Test func aStartTheSystemRefusesIsTriedAgainAtTheNextChange() {
        let feed = feed()
        host.allows = false
        feed.show(playing(at: 10), titles: titles, artwork: nil)
        #expect(host.calls.isEmpty)
        host.allows = true
        feed.show(playing(at: 10, paused: true), titles: titles, artwork: nil)
        guard case .start(let state, _) = host.calls.first else {
            Issue.record("did not start: \(host.calls)")
            return
        }
        #expect(state.paused)
    }
}

@MainActor
struct AiringActivityFeedTests {
    let host = FakeLiveActivityHost<AiringActivityState>()
    let clock = Clock()

    func feed() -> AiringActivityFeed {
        AiringActivityFeed(host: host, files: nil, now: { [clock] in clock.now })
    }

    func week(_ rows: [UpcomingNotice]) -> UpcomingNotices {
        UpcomingNotices(enabled: false, generatedAt: Int64(clock.now.timeIntervalSince1970), airings: rows, watching: [], overrides: 0)
    }

    func airing(
        _ id: AnilistId = 900_002, episode: UInt32 = 6, in seconds: Int64, reason: NoticeReason = .watching,
        translation: Translation? = nil
    ) -> UpcomingNotice {
        UpcomingNotice(
            id: id, title: id == 900_006 ? "Natsukaze Relay" : "Kumo no Ue no Toshokan", episode: episode,
            airsAt: Int64(clock.now.timeIntervalSince1970) + seconds, reason: reason, nextToWatch: episode,
            coverUrl: "https://img.example/\(id).png", bannerUrl: nil, notifies: reason != .switchedOff,
            translation: translation)
    }

    @Test func anEpisodeWithinTheHourCountsDownToItsAirTime() {
        let feed = feed()
        feed.weekChanged(week([airing(in: 2 * 3600)]), artwork: nil)
        #expect(host.calls.isEmpty)
        feed.weekChanged(week([airing(in: 20 * 60)]), artwork: nil)
        guard case .start(let state, let stale) = host.calls.first else {
            Issue.record("did not start: \(host.calls)")
            return
        }
        let airs = clock.now.addingTimeInterval(20 * 60)
        #expect(state.title == "Kumo no Ue no Toshokan" && state.episode == 6 && !state.out)
        #expect(state.airsAt == airs && state.since == clock.now)
        // The air time is its stale date: it turns to "Out now" with the app away.
        #expect(stale == airs)
    }

    @Test func atTheAirTimeItSaysOutNowAndEndsHalfAnHourOn() {
        let feed = feed()
        let started = clock.now
        feed.weekChanged(week([airing(in: 20 * 60)]), artwork: nil)
        clock.now += 20 * 60
        feed.look()
        guard case .update(let out, let stale) = host.calls.last else {
            Issue.record("no update: \(host.calls)")
            return
        }
        #expect(out.out && stale == nil && out.since == started)
        // A week asked for now no longer has it: it stays out for its half hour.
        feed.weekChanged(week([]), artwork: nil)
        #expect(host.isShowing)
        clock.now += 30 * 60
        feed.look()
        #expect(host.calls.last == .end(nil, nil))
    }

    @Test func notAShowSwitchedOffNorOneWatchedDubbed() {
        let feed = feed()
        feed.weekChanged(week([
            airing(900_006, in: 10 * 60, reason: .switchedOff),
            airing(900_002, in: 15 * 60, translation: .dub),
        ]), artwork: nil)
        #expect(host.calls.isEmpty)
    }

    @Test func theSoonestLeadsAndAnotherReplacesItOnceItEnds() {
        let feed = feed()
        feed.weekChanged(week([airing(900_006, episode: 9, in: 40 * 60), airing(900_002, in: 10 * 60)]), artwork: nil)
        guard case .start(let first, _) = host.calls.first else {
            Issue.record("did not start: \(host.calls)")
            return
        }
        #expect(first.titleID == 900_002)
        // The first one is gone from the week before it airs, unfollowed: the next takes its place.
        feed.weekChanged(week([airing(900_006, episode: 9, in: 40 * 60)]), artwork: nil)
        #expect(host.calls.dropFirst().first == .end(nil, nil))
        guard case .start(let second, _) = host.calls.last else {
            Issue.record("did not start the next: \(host.calls)")
            return
        }
        #expect(second.titleID == 900_006 && second.episode == 9)
    }

    @Test func pickKeepsToTheHourAhead() {
        let rows = week([airing(in: 3_601), airing(900_006, in: -(31 * 60))])
        #expect(AiringActivityFeed.pick(rows, now: clock.now) == nil)
        let soon = week([airing(in: 3_600)])
        #expect(AiringActivityFeed.pick(soon, now: clock.now)?.id == 900_002)
    }
}
