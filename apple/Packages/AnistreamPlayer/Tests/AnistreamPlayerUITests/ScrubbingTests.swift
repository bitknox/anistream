// Seek previews from the hidden mpv, the scrubbing pace, the subtitles' lift and the starting
// screen's line.

import AnistreamKit
import AnistreamPlayer
import Foundation
import Testing

@testable import AnistreamPlayerUI
@testable import AnistreamStore

@MainActor
struct PreviewTests {
    @Test func scrubbingOpensTheStreamInAHiddenMpvWithItsHeadersAndSeeksByKeyframes() async throws {
        let rig = Rig()
        try await rig.opened(url: "https://cdn.example/7.m3u8", options: [
            MpvOption(name: "http-header-fields", value: "Referer: https://site.example/"),
            MpvOption(name: "start", value: "600"),
            MpvOption(name: "force-media-title", value: "Stardust Wayfarer · Episode 7"),
        ])
        rig.model.scrub(to: 30)
        let preview = try #require(rig.previews.first)
        try await until("the preview loaded") { preview.calls.count >= 1 }
        #expect(preview.begun)
        #expect(preview.calls.first == .load(url: "https://cdn.example/7.m3u8", options: [
            "http-header-fields": "Referer: https://site.example/", "start": "30.0",
        ]))
        #expect(rig.model.preview.layer != nil)
        // While mpv works on one seek, only the newest waits.
        rig.model.scrub(to: 31)
        rig.model.scrub(to: 32)
        rig.model.scrub(to: 33)
        try await until("the newest seek") { preview.calls.last == .seek(seconds: 33, mode: .keyframe) }
        #expect(rig.previews.count == 1, "one preview for the file")
        #expect(!preview.calls.contains(.seek(seconds: 32, mode: .keyframe)), "\(preview.calls)")

        // The preview comes up when its frame does.
        preview.emit(.playbackRestart)
        try await until("the frame") { rig.model.preview.ready }

        // Another file is another preview's.
        rig.mpv.emit(.startFile)
        try await until("the preview closed") { preview.ended }
        #expect(rig.model.preview.layer == nil)
    }

    @Test func hoveringTheSeekLineOpensThePreviewAndLeavesThePlaybackAlone() async throws {
        let rig = Rig()
        try await rig.opened()
        let calls = rig.mpv.calls.count
        rig.model.hover(at: 20)
        #expect(rig.model.hovering == 20)
        let preview = try #require(rig.previews.first)
        try await until("the preview loaded") { !preview.calls.isEmpty }
        rig.model.hover(at: 25)
        try await until("the preview there") { preview.calls.last == .seek(seconds: 25, mode: .keyframe) }
        #expect(rig.mpv.calls.count == calls, "the playback stays where it is")
        #expect(rig.controls.isEmpty, "nothing for the engine")
        rig.model.hover(at: nil)
        #expect(rig.model.hovering == nil)
        #expect(!preview.ended, "kept for the next hover")
    }

    @Test func theEnginesThumbnailsAddressIsReadInsteadOfTheStream() async throws {
        let rig = Rig()
        try await rig.opened(url: "magnet:?xt=urn:btih:abc")
        rig.ui(1, .thumbnails(url: "http://127.0.0.1:7878/torrent/abc/0"))
        rig.model.scrub(to: 12)
        let preview = try #require(rig.previews.first)
        try await until("the preview loaded") { !preview.calls.isEmpty }
        #expect(preview.calls.first == .load(url: "http://127.0.0.1:7878/torrent/abc/0", options: ["start": "12.0"]))
    }

    @Test func lowPowerModeScrubsWithoutFrames() async throws {
        let rig = Rig()
        rig.lowPower = true
        try await rig.opened()
        rig.model.scrub(to: 30)
        rig.model.endScrub()
        #expect(rig.previews.isEmpty)
        #expect(!rig.model.previewsOn)
        #expect(rig.controls.last == .seekTo(30))
    }

    @Test func theyCloseWithThePlayerAndInLowPowerMode() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.scrub(to: 30)
        let preview = try #require(rig.previews.first)
        rig.lowPower = true
        rig.model.powerChanged()
        try await until("the preview closed") { preview.ended }
        rig.lowPower = false
        rig.model.scrub(to: 31)
        let second = try #require(rig.previews.last)
        rig.model.viewerClosed()
        try await until("the second closed") { second.ended }
    }

    @Test func theChapterAtATimeNamesTheBubble() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.mpv.emit(.property(name: "chapter-list", value: [
            ["title": "Prologue", "time": 0.0], ["title": "Opening", "time": 5.0], ["title": "Part A", "time": 20.0],
        ]))
        try await until("the chapters") { rig.model.chapters.count == 3 }
        #expect(rig.model.chapter(at: 4.9) == "Prologue")
        #expect(rig.model.chapter(at: 9) == "Opening")
        #expect(rig.model.chapter(at: 59) == "Part A")
    }
}

struct PaceTests {
    @Test func movingTheFingerUpSlowsTheScrubFromWhereTheThumbIs() {
        var pace = ScrubPace()
        // A 1000 pt line over a minute: 1 pt is 0.06 s.
        #expect(pace.start(at: 100, width: 1000, duration: 60) == 6)
        #expect(pace.time(at: 200, lifted: 0, option: false, width: 1000, duration: 60, current: 6) == 12)
        // Half speed from 48 pt up, starting where the thumb is.
        let half = pace.time(at: 200, lifted: 60, option: false, width: 1000, duration: 60, current: 12)
        #expect(half == 12)
        #expect(pace.factor == 0.5)
        #expect(pace.time(at: 300, lifted: 60, option: false, width: 1000, duration: 60, current: 12) == 15)
        // A quarter past 96 pt, or with ⌥.
        #expect(ScrubPace.factor(lifted: 100, option: false) == 0.25)
        #expect(ScrubPace.factor(lifted: 0, option: true) == 0.25)
        #expect(ScrubPace.label(0.5) == "Half-speed scrubbing")
        #expect(ScrubPace.label(0.25) == "Quarter-speed scrubbing")
        #expect(ScrubPace.label(1) == nil)
        // Never past either end.
        #expect(pace.time(at: 5000, lifted: 60, option: false, width: 1000, duration: 60, current: 15) == 60)
    }
}

@MainActor
struct LiftTests {
    @Test func theSubtitlesLiftWithTheControlsAndComeBackDown() async throws {
        let rig = Rig()
        try await rig.opened()
        let before = rig.mpv.calls.count
        rig.model.liftSubtitles(60.0 / 400, animated: false)
        rig.model.liftSubtitles(60.0 / 400, animated: false)
        try await until("the lift") { rig.mpv.calls.count == before + 1 }
        #expect(rig.mpv.calls.last == .set(property: "sub-pos", value: .double(85)))

        // Eased down over the controls' fade, ending where it started.
        rig.model.liftSubtitles(0, animated: true)
        try await until("back down", timeout: .seconds(3)) { rig.mpv.calls.count == before + 9 }
        #expect(rig.mpv.calls.last == .set(property: "sub-pos", value: .double(100)))
        let steps = rig.mpv.calls.suffix(from: before + 1)
        #expect(steps.count == 8, "\(Array(steps))")
        let values = steps.compactMap { call -> Double? in
            if case .set("sub-pos", .double(let value)) = call { value } else { nil }
        }
        #expect(values == values.sorted(), "it only goes down: \(values)")
    }
}

struct StartingLineTests {
    @Test func theWaitIsAdmittedAfterThreeSecondsWithTheEnginesNote() {
        let now = ContinuousClock.now
        let screen = StartingScreen(playback: 1, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, since: now)
        #expect(screen.line(at: now + .seconds(2), note: "waiting for the provider") == "Starting episode 7")
        #expect(screen.line(at: now + .seconds(4), note: nil) == "Finding a source · 4 s")
        #expect(screen.line(at: now + .seconds(5), note: "waiting for lantern-archive · rate limited")
            == "Waiting for lantern-archive · rate limited · 5 s")
    }

    @Test @MainActor func theNoteIsTheOneOnTheJobStartingTheEpisode() {
        let jobs = JobsModel(schedule: false)
        jobs.started(1, activity: "loading continue watching")
        jobs.started(2, activity: "starting ep 7")
        jobs.note(1, note: "waiting for anilist", wait: nil)
        jobs.note(2, note: "waiting for lantern-archive", wait: 4)
        #expect(PlayerModel.startingNote(in: jobs.running, episode: "7") == "waiting for lantern-archive")
        #expect(PlayerModel.startingNote(in: jobs.running, episode: "8") == nil)
    }
}
