// The player model on a scripted engine and a scripted mpv: ops to calls, events to reports,
// closing, and the cards' rules.

import AnistreamKit
import AnistreamStore
import AnistreamPlayer
import Foundation
import Testing

@testable import AnistreamPlayerUI

@MainActor
struct OpsTests {
    @Test func eachOpIsOneCallOnMpvInOrder() async throws {
        let rig = Rig()
        rig.ops(1, [
            .open(fullscreen: false, speed: 1.5, volume: 80),
            .load(url: "https://cdn.example/7.mkv", options: [
                MpvOption(name: "start", value: "600"), MpvOption(name: "alang", value: "ja,jpn"),
            ]),
            .addSubtitle(url: "/tmp/full.ass", title: "Full", language: "en"),
            .set(property: "aid", value: .int(2)),
            .set(property: "sub-delay", value: .double(0.5)),
            .set(property: "chapter-list", value: .node(#"[{"title":"Opening","time":3.5}]"#)),
            .cycle(property: "pause"),
            .add(property: "volume", delta: 5),
            .seek(seconds: 30, absolute: true),
            .seek(seconds: -10, absolute: false),
            .shaders(paths: ["/s/a.glsl", "/s/b.glsl"]),
            .audioFilter(add: true, filter: "@night:dynaudnorm"),
            .audioFilter(add: false, filter: "@night"),
            .stop,
        ])
        await rig.handedOver()
        #expect(rig.mpv.begun)
        #expect(rig.model.isPresented)
        #expect(rig.mpv.calls == [
            .set(property: "speed", value: .double(1.5)),
            .set(property: "volume", value: .double(80)),
            .load(url: "https://cdn.example/7.mkv", options: ["start": "600", "alang": "ja,jpn"]),
            .addSubtitle(url: "/tmp/full.ass", title: "Full", language: "en"),
            .set(property: "aid", value: .int(2)),
            .set(property: "sub-delay", value: .double(0.5)),
            .set(property: "chapter-list", value: [["title": "Opening", "time": 3.5]]),
            .command(["cycle", "pause"]),
            .command(["add", "volume", "5.0"]),
            .seek(seconds: 30, mode: .absolute),
            .seek(seconds: -10, mode: .relative),
            .shaders(["/s/a.glsl", "/s/b.glsl"]),
            .command(["af", "add", "@night:dynaudnorm"]),
            .command(["af", "remove", "@night"]),
            .command(["stop"]),
        ])
    }

    @Test func fullScreenIsTheWindowsNotMpvs() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.ops(1, [.cycle(property: "fullscreen")])
        #expect(rig.model.fullScreen)
        rig.ops(1, [.set(property: "fullscreen", value: .flag(false))])
        #expect(!rig.model.fullScreen)
        #expect(!rig.mpv.calls.contains { if case .command(let c) = $0 { c.contains("fullscreen") } else { false } })
    }

    @Test func theEnginesToggleFullscreenIsTheWindows() async throws {
        #expect(HostedStep(.toggleFullscreen) == .fullScreen(nil))
        let rig = Rig()
        try await rig.opened()
        rig.ops(1, [.toggleFullscreen])
        #expect(rig.model.fullScreen)
        rig.ops(1, [.toggleFullscreen])
        #expect(!rig.model.fullScreen)
        #expect(!rig.mpv.calls.contains { if case .command(let c) = $0 { c.contains("fullscreen") } else { false } })
    }

    @Test func restatingTheChaptersReportsThemAsTheyStand() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.answer("chapter-list", [["title": "Opening", "time": 5.0]])
        rig.ops(1, [.restateChapters])
        await rig.handedOver()
        #expect(rig.reports.contains { $0.1 == .property(name: "chapter-list", json: #"[{"time":5,"title":"Opening"}]"#) })
    }
}

@MainActor
struct ReportTests {
    @Test func mpvsEventsAreReportedForThePlaybackWhoseFileItHas() async throws {
        let rig = Rig()
        try await rig.opened(1)
        try await rig.playhead(12)
        #expect(rig.reports.last! == (1, .property(name: "time-pos", json: "12")))

        // The next episode: the file is stopped and the next loaded, which is the next
        // playback's from mpv's start-file on.
        rig.ops(1, [.stop])
        rig.ops(2, [.load(url: "/media/8.mkv", options: [])])
        await rig.handedOver()
        try #require(rig.mpv.calls.count == 3, "the next file loading")
        rig.mpv.emit(.endFile(MpvEnd(reason: "stop", error: nil)))
        rig.mpv.emit(.startFile)
        rig.mpv.emit(.fileLoaded)
        try await until("the next file reported") { rig.reports.filter { $0.1 == .fileLoaded }.count == 2 }
        let tail = rig.reports.suffix(2)
        #expect(tail.first! == (1, .ended(reason: .stop)))
        #expect(tail.last! == (2, .fileLoaded))
        #expect(rig.made.count == 1, "the same mpv")
    }

    @Test func onlyTheEnginesPropertiesAreReported() async throws {
        let rig = Rig()
        try await rig.opened()
        let before = rig.reports.count
        rig.mpv.emit(.property(name: "estimated-vf-fps", value: .double(23.976)))
        rig.mpv.emit(.property(name: "paused-for-cache", value: .flag(true)))
        try await until("the cache report") { rig.reports.count == before + 1 }
        #expect(rig.reports.last! == (1, .property(name: "paused-for-cache", json: "true")))
        #expect(rig.model.bufferingSince != nil)
    }

    @Test func theDisplayIsToldToMpvAndReported() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.surfaceDisplay(
            DisplayInfo(width: 3024, height: 1964, scale: 2, refreshRate: 120, edrHeadroom: 4, edrPotential: 16))
        await rig.handedOver()
        #expect(!rig.mpv.state.withLock { $0.displays.isEmpty }, "mpv told")
        #expect(rig.reports.last! == (1, .display(width: 3024, height: 1964, scale: 2, refreshHz: 120, edrHeadroom: 4, compute: true)))
    }

    @Test func theFirstReportsSayWhatTheDeviceRunsOn() async throws {
        let rig = Rig()
        try await rig.opened()
        #expect(rig.reports.contains { if case .power = $0.1 { true } else { false } })
    }
}

@MainActor
struct ClosingTests {
    @Test func theViewerClosingReportsQuitThenClosedAndRefusesWhatFollows() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.viewerClosed()
        #expect(rig.reports.suffix(2).map(\.1) == [.ended(reason: .quit), .closed])
        #expect(!rig.model.isPresented)
        #expect(rig.model.layer == nil)
        try await until("mpv shut down") { rig.mpv.ended }

        // The engine's answer to a player that is gone, a Close among it: refused, said to nothing.
        let sent = rig.sent.count
        rig.ops(1, [.cycle(property: "pause"), .quit, .close])
        #expect(rig.sent.count == sent)
        #expect(!rig.model.isPresented)
    }

    @Test func backFromOutsideThePlayerAsksItsViewOrStopsPictureInPicture() async throws {
        let rig = Rig()
        try await rig.opened()
        // In the window: the view answers, shrinking into the banner before it stops.
        rig.model.requestBack()
        #expect(rig.model.backRequests == 1)
        #expect(rig.model.isPresented)
        // In Picture in Picture there is no banner to go back into: it stops at once.
        rig.model.pictureInPicture = true
        rig.model.requestBack()
        #expect(!rig.model.isPresented)
        #expect(rig.reports.suffix(2).map(\.1) == [.ended(reason: .quit), .closed])
    }

    @Test func theEnginesQuitReportsHowTheFileEnded() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.seek(by: 10)
        rig.ops(1, [.quit])
        #expect(rig.reports.suffix(2).map(\.1) == [.ended(reason: .quit), .closed])
        #expect(!rig.model.isPresented)
    }

    @Test func theEnginesCloseSaysNothingMore() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.emit(.playbackRestart)
        rig.mpv.emit(.endFile(MpvEnd(reason: "eof", error: nil)))
        try await until("the end reported") { rig.reports.last?.1 == .ended(reason: .eof) }
        let sent = rig.sent.count
        rig.ops(1, [.close])
        #expect(rig.sent.count == sent)
        #expect(!rig.model.isPresented)
        try await until("mpv shut down") { rig.mpv.ended }
    }

    @Test func aNewPlaybackOpensANewPlayerAfterOneClosed() async throws {
        let rig = Rig()
        try await rig.opened(1)
        rig.ops(1, [.close])
        try await rig.opened(2)
        #expect(rig.made.count == 2)
        #expect(rig.model.isPresented)
        #expect(rig.reports.last! == (2, .fileLoaded))
    }

    @Test func mpvGoingAwayByItselfClosesThePlayer() async throws {
        let rig = Rig()
        try await rig.opened()
        await rig.mpv.end()
        try await until("the player closed") { !rig.model.isPresented }
        #expect(rig.reports.last?.1 == .closed)
    }

    func staged(_ rig: Rig, _ episode: String) {
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: episode, episodeTitle: nil, speed: 1))
    }

    /// What the engine says to a player it opens for `playback`.
    func opening(_ rig: Rig, _ playback: PlaybackId, episode: String) {
        rig.ui(playback, .starting(id: 900_001, title: "Stardust Wayfarer", episode: episode, episodeTitle: nil, coverUrl: nil, bannerUrl: nil))
        rig.ops(playback, [.open(fullscreen: false, speed: nil, volume: nil), .load(url: "/media/\(episode).mkv", options: [])])
    }

    @Test func closingWhileThePlayIsFoundStopsItAndItsPlayerNeverComesUp() async throws {
        let rig = Rig()
        // The playback the player showed before, long over: what it last heard of.
        try await rig.opened(1)
        rig.model.viewerClosed()
        staged(rig, "7")
        #expect(rig.model.isPresented, "the starting screen, finding a source")
        rig.model.viewerClosed()
        #expect(!rig.model.isPresented)
        #expect(rig.sent.last == .player(.stop), "for the play being found, not the playback before")

        // Found all the same: its player is refused, and the engine told it closed, once.
        opening(rig, 2, episode: "7")
        rig.ops(2, [.set(property: "pause", value: .flag(false))])
        #expect(!rig.model.isPresented)
        #expect(rig.made.count == 1)
        #expect(rig.reports.filter { $0 == (2, .closed) }.count == 1)

        // A play after it is the viewer's again.
        staged(rig, "8")
        try await rig.opened(3)
        #expect(rig.model.isPresented)
        #expect(rig.made.count == 2)
    }

    /// The viewer closes the player on the next episode's starting screen, while it is found.
    func closedWhileTheNextIsFound(_ ask: (PlayerModel) -> Void) async throws {
        let rig = Rig()
        try await rig.opened(1)
        rig.mpv.emit(.playbackRestart)
        try await until("the picture") { rig.model.hasPicture }
        rig.ui(1, .upNext(next: NextEpisode(number: "8", title: nil, thumbnail: nil), auto: true, endingAt: 42, countdown: 10, stillWatchingAfter: 3))
        ask(rig.model)
        staged(rig, "8")
        // The window says what comes, as the playback driving it.
        rig.ui(1, .starting(id: 900_001, title: "Stardust Wayfarer", episode: "8", episodeTitle: nil, coverUrl: nil, bannerUrl: nil))
        rig.model.viewerClosed()
        #expect(rig.reports.suffix(2).map(\.1) == [.ended(reason: .quit), .closed])
        #expect(rig.sent.last == .player(.stop), "the next episode, still being found, stops too")
        #expect(!rig.model.isPresented)

        opening(rig, 2, episode: "8")
        #expect(!rig.model.isPresented, "no player comes back up for it")
        #expect(rig.made.count == 1)
        #expect(rig.reports.last! == (2, .closed))
    }

    @Test func closingAfterPlayNextBringsNoPlayerBack() async throws {
        try await closedWhileTheNextIsFound { $0.playNext() }
    }

    @Test func closingAfterAnEpisodeIsPickedBringsNoPlayerBack() async throws {
        try await closedWhileTheNextIsFound { $0.choose(.episode("8")) }
    }

    @Test func aPlaybackThatNeverOpenedComesDownWhenItPlaysElsewhere() {
        let rig = Rig()
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1))
        #expect(rig.model.isPresented)
        rig.model.apply(.playbackEnded(watched: false, ending: .elsewhere, rollingOn: false))
        #expect(!rig.model.isPresented)
        #expect(rig.made.isEmpty)
    }
}

@MainActor
struct CardTests {
    func upNext(_ rig: Rig, auto: Bool = true, endingAt: Double? = 42) {
        rig.ui(1, .upNext(next: NextEpisode(number: "8", title: "A Town Without Night", thumbnail: nil), auto: auto, endingAt: endingAt, countdown: 10, stillWatchingAfter: 3))
    }

    @Test func upNextCountsDownFromTheEndingAndPlaysNextAtZero() async throws {
        let rig = Rig()
        try await rig.opened()
        upNext(rig)
        try await rig.playhead(41)
        #expect(!rig.model.upNextShown)
        try await rig.playhead(42)
        #expect(rig.model.upNextShown)
        #expect(rig.model.corner == .upNext)
        #expect(rig.model.countdown?.seconds == 10)
        try await rig.playhead(47)
        #expect(rig.model.countdown?.seconds == 5)
        #expect(!rig.controls.contains(.playNext))
        try await rig.playhead(52)
        #expect(rig.controls.filter { $0 == .playNext }.count == 1)
        try await rig.playhead(53)
        #expect(rig.controls.filter { $0 == .playNext }.count == 1, "once")
    }

    @Test func upNextCountsDownAsLongAsTheEngineSays() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.ui(1, .upNext(next: NextEpisode(number: "8", title: nil, thumbnail: nil), auto: true, endingAt: 42,
                          countdown: 5, stillWatchingAfter: 3))
        try await rig.playhead(42)
        #expect(rig.model.countdown?.seconds == 5)
        try await rig.playhead(46)
        #expect(!rig.controls.contains(.playNext))
        try await rig.playhead(47)
        #expect(rig.controls.filter { $0 == .playNext }.count == 1)
    }

    @Test func withNoCountdownUpNextWaitsForTheViewer() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.ui(1, .upNext(next: NextEpisode(number: "8", title: nil, thumbnail: nil), auto: true, endingAt: 42,
                          countdown: 0, stillWatchingAfter: 3))
        try await rig.playhead(60)
        #expect(rig.model.upNextShown)
        #expect(rig.model.countdown == nil)
        #expect(!rig.controls.contains(.playNext))
        rig.model.playNext()
        #expect(rig.controls.last == .playNext)
    }

    @Test func withoutEndingDataUpNextShowsNearTheEnd() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.emit(.property(name: "duration", value: .double(1440)))
        upNext(rig, endingAt: nil)
        try await rig.playhead(1409)
        #expect(!rig.model.upNextShown)
        try await rig.playhead(1410)
        #expect(rig.model.upNextShown)
    }

    @Test func aSeekPastTheEndingRunsTheCountdownAgainRatherThanPlayingNext() async throws {
        let rig = Rig()
        try await rig.opened()
        upNext(rig)
        try await rig.playhead(44)
        #expect(rig.model.countdown?.seconds == 8)
        // Well past where the countdown would have run out: it runs again, all ten seconds.
        rig.model.seek(to: 80)
        rig.ops(1, [.seek(seconds: 80, absolute: true)])
        try await rig.playhead(80)
        #expect(!rig.controls.contains(.playNext))
        #expect(rig.model.countdown?.seconds == 10)
        try await rig.playhead(89)
        #expect(!rig.controls.contains(.playNext))
        try await rig.playhead(90)
        #expect(rig.controls.filter { $0 == .playNext }.count == 1)
    }

    @Test func scrubbingPastTheEndingPlaysNothingUntilTheCountdownRunsAgain() async throws {
        let rig = Rig()
        try await rig.opened()
        upNext(rig)
        try await rig.playhead(43)
        // The keyframe seeks under the finger move the playhead past the countdown's end.
        rig.model.scrub(to: 70)
        try await rig.playhead(70)
        #expect(!rig.controls.contains(.playNext), "not while scrubbing")
        rig.model.endScrub()
        rig.ops(1, [.seek(seconds: 70, absolute: true)])
        try await rig.playhead(71)
        #expect(!rig.controls.contains(.playNext))
        #expect(rig.model.countdown?.seconds == 9)
        try await rig.playhead(80)
        #expect(rig.controls.filter { $0 == .playNext }.count == 1)
    }

    @Test func aCancelledUpNextNeverPlaysNext() async throws {
        let rig = Rig()
        try await rig.opened()
        upNext(rig)
        try await rig.playhead(43)
        rig.model.cancelNext()
        #expect(rig.controls.last == .cancelNext)
        #expect(!rig.model.upNextShown)
        try await rig.playhead(60)
        #expect(!rig.controls.contains(.playNext))
    }

    @Test func upNextWithAutoOffWaitsForTheViewer() async throws {
        let rig = Rig()
        try await rig.opened()
        upNext(rig, auto: false)
        try await rig.playhead(55)
        #expect(rig.model.upNextShown)
        #expect(rig.model.countdown == nil)
        #expect(!rig.controls.contains(.playNext))
        rig.model.playNext()
        #expect(rig.controls.last == .playNext)
    }

    @Test func theCornerHoldsOneCardStallThenUndoThenUpNextThenSkip() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.ui(1, .skip(kind: .ending, to: 56))
        #expect(rig.model.corner == .skip)
        upNext(rig)
        try await rig.playhead(43)
        #expect(rig.model.corner == .upNext)
        rig.ui(1, .autoSkipped(kind: .opening, undoTo: 5))
        #expect(rig.model.corner == .undo)
        rig.ui(1, .stall(seconds: 8))
        #expect(rig.model.corner == .stall)
        rig.model.keepWaiting()
        #expect(rig.controls.last == .stallWait)
        rig.ui(1, .stall(seconds: 9))
        #expect(rig.model.stall == nil, "not again until the stall ends")
        rig.ui(1, .stallCleared)
        rig.ui(1, .stall(seconds: 8))
        #expect(rig.model.stall != nil)
    }

    @Test func undoLastsSixSecondsOfPlayback() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.ui(1, .autoSkipped(kind: .opening, undoTo: 5))
        try await rig.playhead(20)
        try await rig.playhead(25)
        #expect(rig.model.undo != nil)
        try await rig.playhead(26)
        #expect(rig.model.undo == nil)
    }

    @Test func takingTheSkipAndUndoingAreTheEnginesToDo() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.ui(1, .skip(kind: .opening, to: 20))
        rig.model.takeSkip()
        #expect(rig.model.skip == nil)
        rig.ui(1, .autoSkipped(kind: .opening, undoTo: 5))
        rig.model.undoSkip()
        #expect(rig.controls == [.takeSkip, .undoSkip(to: 5)])
    }

    @Test func aNewPlaybackStartsWithNoneOfTheLastOnesCards() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.ui(1, .skip(kind: .opening, to: 20))
        rig.ui(1, .resumed(position: 600))
        rig.ui(2, .starting(id: 900_001, title: "Stardust Wayfarer", episode: "8", episodeTitle: nil, coverUrl: nil, bannerUrl: nil))
        #expect(rig.model.skip == nil)
        #expect(rig.model.resume == nil)
        #expect(rig.model.startingLine == "Starting episode 8")
    }

    @Test func theStartingScreenCarriesTheTitleAndItsArt() {
        let rig = Rig()
        rig.ui(1, .starting(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: "Rain over Kiriya Pass",
                            coverUrl: "http://art.example/cover/900001.png", bannerUrl: "http://art.example/banner/900001.png"))
        let starting = rig.model.starting
        #expect(starting?.id == 900_001)
        #expect(starting?.bannerUrl == "http://art.example/banner/900001.png")
        #expect(starting?.coverUrl == "http://art.example/cover/900001.png")
        #expect(rig.model.isPresented, "nothing remembered from PlaybackStaged is needed")
    }

    @Test func theStartingScreenStaysUntilTheNextFilesFirstFrame() async throws {
        let rig = Rig()
        try await rig.opened(1)
        rig.mpv.emit(.playbackRestart)
        try await until("the picture") { rig.model.hasPicture }
        rig.ui(2, .starting(id: 900_001, title: "Stardust Wayfarer", episode: "8", episodeTitle: nil, coverUrl: nil, bannerUrl: nil))
        // The old file's frames do not lift it.
        rig.mpv.emit(.playbackRestart)
        rig.ops(1, [.stop])
        rig.ops(2, [.load(url: "/media/8.mkv", options: [])])
        await rig.handedOver()
        try #require(rig.mpv.calls.count == 3, "the load")
        #expect(rig.model.starting != nil)
        rig.mpv.emit(.startFile)
        rig.mpv.emit(.playbackRestart)
        try await until("the next picture") { rig.model.starting == nil }
    }

    @Test func threeEpisodesInARowWithNobodyThereAskStillWatching() async throws {
        let rig = Rig()
        try await rig.opened(1)
        rig.mpv.emit(.playbackRestart)
        for playback: PlaybackId in 2...4 {
            rig.ui(playback - 1, .upNext(next: NextEpisode(number: "\(playback + 6)", title: nil, thumbnail: nil), auto: true, endingAt: 42, countdown: 10, stillWatchingAfter: 3))
            try await rig.playhead(52)
            rig.ops(playback - 1, [.stop])
            rig.ops(playback, [.load(url: "/media/\(playback).mkv", options: [])])
            await rig.handedOver()
            try #require(rig.mpv.calls.filter { if case .load = $0 { true } else { false } }.count == Int(playback), "the load")
            rig.mpv.emit(.startFile)
            rig.mpv.emit(.playbackRestart)
            try await rig.playhead(0.5)
            #expect(rig.model.stillWatching == (playback == 4), "after \(playback - 1) in a row")
        }
        #expect(rig.controls.last == .togglePause, "it pauses")
        rig.mpv.emit(.property(name: "pause", value: .flag(true)))
        try await until("paused") { rig.model.paused }
        rig.model.continueWatching()
        #expect(!rig.model.stillWatching)
        #expect(rig.controls.last == .togglePause)
    }

    @Test func stillWatchingAsksAfterAsManyAsTheEngineSays() async throws {
        let rig = Rig()
        try await rig.opened(1)
        rig.mpv.emit(.playbackRestart)
        for playback: PlaybackId in 2...3 {
            rig.ui(playback - 1, .upNext(next: NextEpisode(number: "\(playback + 6)", title: nil, thumbnail: nil), auto: true, endingAt: 42, countdown: 10, stillWatchingAfter: 2))
            try await rig.playhead(52)
            rig.ops(playback, [.load(url: "/media/\(playback).mkv", options: [])])
            await rig.handedOver()
            try #require(rig.mpv.calls.filter { if case .load = $0 { true } else { false } }.count == Int(playback), "the load")
            rig.mpv.emit(.startFile)
            rig.mpv.emit(.playbackRestart)
            try await rig.playhead(0.5)
            #expect(rig.model.stillWatching == (playback == 3), "after \(playback - 1) in a row")
        }
        #expect(rig.model.stillWatchingAfter == 2)
    }

    @Test func anEpisodePlayedNextAfterASeekIsNotOneInARowWithNobodyThere() async throws {
        let rig = Rig()
        try await rig.opened(1)
        for playback: PlaybackId in 2...4 {
            rig.ui(playback - 1, .upNext(next: NextEpisode(number: "\(playback + 6)", title: nil, thumbnail: nil), auto: true, endingAt: 42, countdown: 10, stillWatchingAfter: 2))
            if playback == 3 {
                // The viewer seeks into the ending, and the countdown runs out after it.
                rig.model.seek(to: 45)
                rig.ops(playback - 1, [.seek(seconds: 45, absolute: true)])
                try await rig.playhead(55)
            } else {
                try await rig.playhead(52)
            }
            rig.ops(playback, [.load(url: "/media/\(playback).mkv", options: [])])
            await rig.handedOver()
            try #require(rig.mpv.calls.filter { if case .load = $0 { true } else { false } }.count == Int(playback), "the load")
            rig.mpv.emit(.startFile)
            rig.mpv.emit(.playbackRestart)
            try await rig.playhead(0.5)
        }
        // Two rolled on with nobody there would ask; the one after the seek is not among them.
        #expect(rig.controls.filter { $0 == .playNext }.count == 3)
        #expect(!rig.model.stillWatching)
    }

    @Test func anyInputStartsTheCountAgain() async throws {
        let rig = Rig()
        try await rig.opened(1)
        for playback: PlaybackId in 2...4 {
            rig.ui(playback - 1, .upNext(next: NextEpisode(number: "\(playback + 6)", title: nil, thumbnail: nil), auto: true, endingAt: 42, countdown: 10, stillWatchingAfter: 3))
            try await rig.playhead(52)
            if playback == 3 { rig.model.seek(by: 10) }
            rig.ops(playback, [.load(url: "/media/\(playback).mkv", options: [])])
            await rig.handedOver()
            try #require(rig.mpv.calls.filter { if case .load = $0 { true } else { false } }.count == Int(playback), "the load")
            rig.mpv.emit(.startFile)
            rig.mpv.emit(.playbackRestart)
            try await rig.playhead(0.5)
        }
        #expect(!rig.model.stillWatching)
    }
}

@MainActor
struct ViewerTests {
    @Test func handedOffElsewhereThePlayingSessionPauses() async throws {
        let rig = Rig()
        let types = HandoffTypes(bundleIdentifier: "tv.anistream.app")
        let playing = rig.model.playingActivity(types: types) { _ in "Hoshikuzu no Tabibito" }
        #expect(playing.activity == nil, "nothing plays")

        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1))
        try await rig.opened(3)
        try await rig.playhead(760)
        try await until("the episode offered") { playing.activity != nil && rig.model.position == 760 }
        let offered = try #require(playing.activity)
        offered.delegate?.userActivityWillSave?(offered)
        #expect(offered.activityType == "tv.anistream.playing")
        #expect(offered.title == "Hoshikuzu no Tabibito · Episode 7", "named as its page heads it")
        #expect(Handoff(offered, types: types) == .playing(900_001, episode: "7", position: 760))

        // Another device took it over: Pause, to the playing session.
        let before = rig.sent.count
        offered.delegate?.userActivityWasContinued?(offered)
        try await until("paused here") { rig.sent.count > before }
        #expect(Array(rig.sent.dropFirst(before)) == [.control(playback: 3, command: .togglePause)])

        // Paused already, a toggle would play it: nothing goes.
        rig.mpv.emit(.property(name: "pause", value: .flag(true)))
        try await until("paused") { rig.model.paused }
        let paused = rig.sent.count
        rig.model.pause()
        offered.delegate?.userActivityWasContinued?(offered)
        try await Task.sleep(for: .milliseconds(50))
        #expect(rig.sent.count == paused)
    }

    @Test func pausingWithNothingPlayingSendsNothing() {
        let rig = Rig()
        rig.model.pause()
        #expect(rig.sent.isEmpty)
    }

    @Test func commandsAreAimedAtThePlaybackOnScreen() async throws {
        let rig = Rig()
        rig.model.togglePause()
        try await rig.opened(3)
        rig.model.togglePause()
        rig.model.takeSkip()
        #expect(rig.aims == [nil, 3, 3], "whatever plays until the engine names one")
    }

    @Test func picksAndTracksGoToTheEngine() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.emit(.property(name: "speed", value: .double(1)))
        rig.mpv.emit(.property(name: "track-list", value: [
            ["type": "audio", "id": 1, "lang": "jpn", "selected": true],
            ["type": "audio", "id": 2, "lang": "eng"],
            ["type": "sub", "id": 1, "lang": "eng", "title": "Full"],
        ]))
        try await until("the tracks") { rig.model.tracks.count == 3 }
        rig.model.selectAudio(2)
        rig.model.selectSubtitle(nil)
        rig.model.setSpeed(1.5)
        rig.model.choose(.episode("3"))
        rig.model.setUpscaling("sharp")
        rig.model.setNightMode(true)
        rig.model.step(1)
        // The engine selects the tracks, and keeps them for the series as the viewer's.
        #expect(!rig.mpv.calls.contains { if case .set(let property, _) = $0 { ["aid", "sid"].contains(property) } else { false } })
        #expect(rig.controls == [
            .chooseTrack(kind: .audio, id: 2), .chooseTrack(kind: .subtitle, id: nil),
            .speedTo(1.5), .choose(.episode("3")), .upscaling("sharp"), .nightMode(true), .step(1),
        ])
        #expect(rig.model.audioTracks.map(\.id) == [1, 2])
        #expect(rig.model.subtitleTracks.first?.title == "Full")
    }

    @Test func aSpeedIsSetOutrightWhateverPlays() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.emit(.property(name: "speed", value: .double(1.25)))
        try await until("the speed") { rig.model.speed == 1.25 }
        // 1.5× from 1.25×: the speed itself, not the factor between them, which the engine
        // multiplies by whatever plays then.
        rig.model.setSpeed(1.5)
        rig.model.setSpeed(1)
        #expect(rig.controls == [.speedTo(1.5), .speedTo(1)])
    }

    @Test func scrubbingSeeksByKeyframesThenExactlyOnRelease() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.scrub(to: 10)
        rig.model.scrub(to: 10.2)
        rig.model.scrub(to: 11)
        #expect(rig.model.controlsVisible)
        rig.model.endScrub()
        await rig.handedOver()
        #expect(rig.mpv.calls.count == 3, "the keyframe seeks")
        #expect(rig.mpv.calls.suffix(2) == [.seek(seconds: 10, mode: .keyframe), .seek(seconds: 11, mode: .keyframe)])
        #expect(rig.controls.last == .seekTo(11))
    }

    @Test func theBackgroundTurnsTheVideoTrackOff() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.backgrounded(true)
        rig.model.backgrounded(false)
        await rig.handedOver()
        #expect(rig.mpv.calls.count == 3, "vid set twice")
        #expect(rig.mpv.calls.suffix(2) == [.set(property: "vid", value: "no"), .set(property: "vid", value: "auto")])
    }

    @Test func theSegmentsComeFromTheChapters() {
        let segments = seekSegments(
            chapters: [
                Chapter(title: "Prologue", time: 0), Chapter(title: "Opening", time: 5),
                Chapter(title: "Part A", time: 20), Chapter(title: "Ending", time: 42),
                Chapter(title: "Preview", time: 56),
            ], duration: 60)
        #expect(segments == [
            SeekSegment(kind: .plain, start: 0, end: 5), SeekSegment(kind: .opening, start: 5, end: 20),
            SeekSegment(kind: .plain, start: 20, end: 42), SeekSegment(kind: .ending, start: 42, end: 56),
            SeekSegment(kind: .plain, start: 56, end: 60),
        ])
        #expect(PlayerClock.label(724) == "12:04")
        #expect(PlayerClock.remaining(149) == "−2:29")
        #expect(PlayerClock.label(3729) == "1:02:09")
    }

    @Test func theEngineMarksTakeOverFromTheChaptersAndTheBufferedRangesFollowTheFile() async throws {
        let rig = Rig()
        try await rig.opened(1)
        rig.mpv.emit(.property(name: "duration", value: .double(60)))
        rig.mpv.emit(.property(name: "chapter-list", value: .array([
            .map(["title": .string("Opening"), "time": .double(4)]),
            .map(["title": .string("Part A"), "time": .double(18)]),
        ])))
        try await until("the chapters taken") { rig.model.chapters.count == 2 && rig.model.duration == 60 }
        #expect(rig.model.segments.map(\.kind) == [.plain, .opening, .plain], "the chapters stand in")

        rig.model.apply(.playerTimeline(playback: 1, marks: [
            TimelineMark(kind: .ending, start: 42, end: 56), TimelineMark(kind: .opening, start: 5, end: 20),
        ]))
        #expect(rig.model.segments == [
            SeekSegment(kind: .plain, start: 0, end: 5), SeekSegment(kind: .opening, start: 5, end: 20),
            SeekSegment(kind: .plain, start: 20, end: 42), SeekSegment(kind: .ending, start: 42, end: 56),
            SeekSegment(kind: .plain, start: 56, end: 60),
        ])

        rig.model.apply(.playerBuffered(playback: 1, ranges: [TimeRange(start: 0, end: 30)]))
        #expect(rig.model.buffered == [TimeRange(start: 0, end: 30)])
        rig.model.apply(.playerBuffered(playback: 2, ranges: [TimeRange(start: 0, end: 60)]))
        #expect(rig.model.buffered == [TimeRange(start: 0, end: 30)], "another playback's is not this file's")
        let spans = bufferedSpans(rig.model.buffered, in: SeekSegment(kind: .opening, start: 5, end: 20))
        #expect(spans.count == 1 && spans[0].from == 0 && spans[0].length == 1)
        #expect(bufferedSpans(rig.model.buffered, in: SeekSegment(kind: .ending, start: 42, end: 56)).isEmpty)

        rig.mpv.emit(.startFile)
        try await until("the next file") { rig.model.buffered.isEmpty }
    }

    @Test func mpvsCacheIsReportedToTheEngine() async throws {
        #expect(reportedProperties.contains("demuxer-cache-state"))
        let rig = Rig()
        try await rig.opened(1)
        let cache = AnistreamPlayer.MpvValue.map([
            "seekable-ranges": .array([.map(["start": .double(0), "end": .double(30)])]),
        ])
        rig.mpv.emit(.property(name: "demuxer-cache-state", value: cache))
        try await until("the cache reported") {
            rig.reports.contains { $0.0 == 1 && $0.1 == .property(name: "demuxer-cache-state", json: cache.json) }
        }
    }
}

/// Buffering mid-episode: the amber line along the picture's top, after a second.
struct BufferingLineTests {
    @Test func theLineShowsAfterASecondOfBufferingAndGoesWithIt() {
        let since = ContinuousClock.now
        #expect(!PlayerTiming.showsBufferingLine(since: nil, at: since + .seconds(5), starting: false),
                "not buffering")
        #expect(!PlayerTiming.showsBufferingLine(since: since, at: since + .milliseconds(900), starting: false),
                "a moment's wait shows nothing")
        #expect(PlayerTiming.showsBufferingLine(since: since, at: since + .seconds(1), starting: false))
        #expect(PlayerTiming.showsBufferingLine(since: since, at: since + .seconds(9), starting: false),
                "still there under the stall prompt")
        #expect(!PlayerTiming.showsBufferingLine(since: since, at: since + .seconds(3), starting: true),
                "the starting screen has its own")
    }
}

extension BufferingLineTests {
    @Test func theLineRunsAlongThePicturesTopNotTheLetterbox() {
        // 16:9 in a 16:10 window: bars above and below.
        let wide = BufferingLine.picture(in: CGSize(width: 1600, height: 1000), aspect: 16.0 / 9)
        #expect(wide.minY == 50 && wide.width == 1600)
        // 4:3 in the same window: bars at the sides, the line as wide as the picture.
        let square = BufferingLine.picture(in: CGSize(width: 1600, height: 1000), aspect: 4.0 / 3)
        #expect(square.minY == 0 && abs(square.width - 1333.33) < 0.01)
        #expect(BufferingLine.picture(in: CGSize(width: 800, height: 600), aspect: nil) == CGRect(x: 0, y: 0, width: 800, height: 600))
    }
}

struct LogLineTests {
    @Test func mpvsLinesAreLoggedWithoutTheTokensInTheirAddresses() {
        let line = MpvLogLine(
            prefix: "ffmpeg", level: "error",
            text: "https: HTTP error 403 for https://cdn.example/hls/7/seg-12.ts?token=s3cret&expires=1")
        #expect(PlayerModel.said(line) == "ffmpeg: https: HTTP error 403 for https://cdn.example")
    }
}

/// Load Subtitle File… and Load Audio File…: the file added and selected on mpv, the engine not
/// told, and a file mpv can't load said over the video.
@MainActor
struct TrackFileTests {
    @Test func aFileIsAddedAndSelectedOnMpvAlone() async {
        let rig = Rig()
        rig.ops(1, [.open(fullscreen: false, speed: nil, volume: nil)])
        rig.model.loadTrack(.sub, from: "/Users/viewer/Lantern Road.ass")
        rig.model.loadTrack(.audio, from: "/Users/viewer/commentary.mka")
        await rig.handedOver()
        #expect(rig.mpv.calls.suffix(2) == [
            .command(["sub-add", "/Users/viewer/Lantern Road.ass", "select"]),
            .command(["audio-add", "/Users/viewer/commentary.mka", "select"]),
        ])
        #expect(!rig.controls.contains { if case .chooseTrack = $0 { true } else { false } })
        #expect(rig.model.alert == nil)
    }

    @Test func aFileMpvCantLoadIsSaid() async {
        let rig = Rig()
        rig.ops(1, [.open(fullscreen: false, speed: nil, volume: nil)])
        rig.mpv.state.withLock { $0.refuses = ["sub-add"] }
        rig.model.loadTrack(.sub, from: "/Users/viewer/notes.txt")
        await rig.handedOver()
        #expect(rig.model.alert?.text == "Couldn't load that subtitle file")
        #expect(rig.model.alert?.kind == .alert)
    }
}
