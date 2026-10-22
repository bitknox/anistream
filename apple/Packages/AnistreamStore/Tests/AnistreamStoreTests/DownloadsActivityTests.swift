// The downloads' Live Activity, shown through a fake host: nothing reaches the system.

@testable import AnistreamStore
import AnistreamKit
import AnistreamWidgetData
import Foundation
import Testing

@MainActor
final class FakeActivityHost: DownloadsActivityHost {
    enum Call: Equatable {
        case start(DownloadsActivityState)
        case update(DownloadsActivityState)
        case end(DownloadsActivityState?, Date?)
    }

    var calls: [Call] = []
    var isShowing = false

    func start(_ state: DownloadsActivityState, staleAt: Date) -> Bool {
        calls.append(.start(state))
        isShowing = true
        return true
    }

    func update(_ state: DownloadsActivityState, staleAt: Date) {
        calls.append(.update(state))
    }

    func end(_ state: DownloadsActivityState?, dismissAt: Date?) {
        calls.append(.end(state, dismissAt))
        isShowing = false
    }
}

@MainActor
final class Clock {
    var now = Date(timeIntervalSince1970: 1_792_516_200)
}

func download(
    _ id: Int64, _ episode: String, _ state: DownloadState, done: UInt64, total: UInt64 = 1_100_000_000,
    path: String? = nil
) -> DownloadRow {
    DownloadRow(
        id: id, anilistId: 900_001, title: "Hoshikuzu no Tabibito", episode: episode, state: state,
        fraction: total > 0 ? Double(done) / Double(total) : 0, downloaded: done, total: total, error: nil, path: path)
}

@MainActor
struct DownloadsActivityFeedTests {
    let host = FakeActivityHost()
    let clock = Clock()
    let model = DownloadsModel()
    let titles = TitleCache()

    func feed() -> DownloadsActivityFeed {
        DownloadsActivityFeed(host: host, files: nil, now: { [clock] in clock.now })
    }

    func apply(_ feed: DownloadsActivityFeed, _ rows: [DownloadRow]) {
        model.apply(rows, at: clock.now)
        feed.downloadsChanged(model, titles: titles, artwork: nil)
    }

    @Test func itStartsWhenADownloadStartsAndNotForAQueueThatWaits() {
        let feed = feed()
        apply(feed, [download(1, "8", .queued, done: 0)])
        #expect(host.calls.isEmpty)
        apply(feed, [download(1, "8", .active, done: 0)])
        guard case .start(let state) = host.calls.first else {
            Issue.record("did not start: \(host.calls)")
            return
        }
        #expect(state.title == "Hoshikuzu no Tabibito" && state.episode == "8" && state.count == 1)
    }

    @Test func progressIsShownEveryFewSecondsAndAPauseAtOnce() {
        let feed = feed()
        apply(feed, [download(1, "8", .active, done: 100_000_000)])
        clock.now += 1
        apply(feed, [download(1, "8", .active, done: 102_400_000)])
        #expect(host.calls.count == 1, "within the interval, progress waits")
        clock.now += 3
        apply(feed, [download(1, "8", .active, done: 112_000_000)])
        guard case .update(let moved) = host.calls.last else {
            Issue.record("no update: \(host.calls)")
            return
        }
        #expect(moved.bytesDone == 112_000_000)
        #expect(moved.speed != nil && moved.secondsLeft != nil)
        clock.now += 0.5
        apply(feed, [download(1, "8", .paused, done: 112_000_000)])
        guard case .update(let paused) = host.calls.last else {
            Issue.record("the pause did not show: \(host.calls)")
            return
        }
        #expect(paused.paused && paused.secondsLeft == nil)
    }

    @Test func severalAreOneActivityAboutTheFirstRunning() {
        let feed = feed()
        apply(feed, [
            download(1, "8", .active, done: 400_000_000),
            download(2, "9", .queued, done: 0, total: 1_000_000_000),
            download(3, "10", .queued, done: 0, total: 1_000_000_000),
        ])
        guard case .start(let state) = host.calls.first else {
            Issue.record("did not start")
            return
        }
        #expect(state.count == 3 && state.episode == "8")
        #expect(state.bytesLeft == 700_000_000 + 2_000_000_000)
        #expect(DownloadsActivityText(state).title == "Downloading 3 episodes")
    }

    @Test func theLastOneFinishedStaysAFewMinutesWithPlay() {
        let feed = feed()
        apply(feed, [download(7, "8", .active, done: 1_000_000_000)])
        apply(feed, [download(7, "8", .done, done: 1_100_000_000, path: "/tmp/e8.mkv")])
        guard case .end(let state?, let dismissAt?) = host.calls.last else {
            Issue.record("did not end with the finished state: \(host.calls)")
            return
        }
        #expect(state.finished && state.downloadID == 7)
        #expect(dismissAt == clock.now.addingTimeInterval(DownloadsActivityFeed.finishedFor))
        #expect(!host.isShowing)
    }

    @Test func itEndsWhenTheSystemShowsTheProgressAndStaysAway() {
        let feed = feed()
        apply(feed, [download(1, "8", .active, done: 100_000_000)])
        feed.systemShowsProgress = true
        #expect(host.calls.last == .end(nil, nil))
        clock.now += 10
        apply(feed, [download(1, "8", .active, done: 200_000_000)])
        #expect(!host.isShowing, "never beside the system's own progress")
        feed.systemShowsProgress = false
        apply(feed, [download(1, "8", .active, done: 300_000_000)])
        #expect(host.isShowing, "back once the app runs the downloads again")
    }

    @Test func aQueueEmptiedWithoutFinishingEndsAtOnce() {
        let feed = feed()
        apply(feed, [download(1, "8", .active, done: 100_000_000)])
        apply(feed, [])
        #expect(host.calls.last == .end(nil, nil))
    }

    @Test func pauseAndResumeReachTheStore() {
        var said: [String] = []
        feed().takeControls(pause: { said.append("pause") }, resume: { said.append("resume") })
        DownloadControls.shared.pause?()
        DownloadControls.shared.resume?()
        #expect(said == ["pause", "resume"])
    }
}
