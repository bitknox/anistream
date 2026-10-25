// Stats for nerds on the player model: the panel's command to the engine, its loading states,
// the rows as the engine words them, and Copy stats.

import AnistreamKit
import AnistreamPlayer
import Foundation
import Testing

@testable import AnistreamPlayerUI

@MainActor
struct StatsTests {
    @Test func theEngineIsAskedForFiguresOnlyWhileThePanelIsUp() async throws {
        let rig = Rig()
        try await rig.opened()
        #expect(!rig.controls.contains { if case .stats = $0 { true } else { false } })
        rig.model.toggleStats()
        #expect(rig.controls.last == .stats(true))
        #expect(rig.aims.last == 1)
        rig.model.toggleStats()
        #expect(rig.controls.last == .stats(false))
        #expect(rig.model.statsPanel == nil)
    }

    @Test func aSampleReadsWhatMpvHasAndQuietlyLeavesOutTheRest() async throws {
        #expect(HostedStep(.sample(properties: ["avsync"])) == .call(.sample(properties: ["avsync"])))
        #expect(reportedProperties.contains("hwdec-current"))
        let rig = Rig()
        try await rig.opened()
        rig.mpv.answer("avsync", .double(0.004))
        rig.mpv.answer("cache-speed", .int(2_400_000))
        rig.ops(1, [.sample(properties: ["avsync", "audio-params", "cache-speed"])])
        await rig.handedOver()
        try await until("the figures reported") {
            rig.reports.contains { $0.1 == .property(name: "cache-speed", json: "2400000") }
        }
        let names = rig.reports.compactMap { if case .property(let name, _) = $0.1 { name } else { nil } }
        #expect(names.contains("avsync"))
        #expect(!names.contains("audio-params"))
    }

    @Test func beforeTheFileOpensThePanelWaitsThenDashesThenSaysSo() async throws {
        let rig = Rig()
        rig.ops(1, [.open(fullscreen: false, speed: nil, volume: nil)])
        rig.model.toggleStats()
        // Nothing for a moment.
        #expect(rig.model.statsPanel == nil)
        try await until("the dashes", timeout: .seconds(3)) { rig.model.statsPanel != nil }
        let dashes = try #require(rig.model.statsPanel)
        #expect(!dashes.waitingLine)
        #expect(dashes.rows.allSatisfy { $0.value == "–" && !$0.warn })
        #expect(dashes.rows.first?.label == "Video")
        try await until("the waiting line", timeout: .seconds(6)) { rig.model.statsPanel?.waitingLine == true }

        // The figures arrive: the rows replace the dashes, and the waiting is over.
        let playing = playerStatsFixture(state: .playing)
        rig.model.apply(.playerStats(playback: 1, stats: playing))
        #expect(rig.model.statsPanel == StatsPanelState(rows: playing.rows, waitingLine: false))
    }

    @Test func theRowsAreTheEnginesAndOnlyItsFirstProblemIsAmber() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.toggleStats()
        rig.model.apply(.playerStats(playback: 1, stats: playerStatsFixture(state: .buffering)))
        let rows = try #require(rig.model.statsPanel?.rows)
        #expect(rows.filter(\.warn).map(\.label) == ["Buffer"])
        #expect(rows.first { $0.label == "Buffer" }?.value == "0 s ahead, waiting")

        // A local file has no Network row, and an SDR picture no HDR row.
        rig.model.apply(.playerStats(playback: 1, stats: playerStatsFixture(state: .local)))
        let local = try #require(rig.model.statsPanel?.rows).map(\.label)
        #expect(!local.contains("Network"))
        #expect(!local.contains("HDR"))
        rig.model.apply(.playerStats(playback: 1, stats: playerStatsFixture(state: .hdr)))
        #expect(rig.model.statsPanel?.rows.contains { $0.label == "HDR" } == true)

        // The summary said as the panel shuts is the TV's, not the Mac's.
        rig.model.toggleStats()
        rig.model.apply(.playerStats(playback: 1, stats: PlayerStats(rows: [], summary: ["720p"], waiting: false)))
        #expect(rig.model.statsPanel == nil)
    }

    @Test func copyStatsPutsTheEnginesLinesOnTheClipboardAndSaysCopied() async throws {
        let rig = Rig()
        try await rig.opened()
        rig.model.statsAbout = { StatsAbout(appVersion: "2.4.0", system: "macOS 26.1", device: "Mac15,3") }
        rig.model.toggleStats()
        rig.model.apply(.playerStats(playback: 1, stats: playerStatsFixture(state: .playing)))
        rig.model.copyStats()
        #expect(rig.controls.last == .copyStats(about: StatsAbout(appVersion: "2.4.0", system: "macOS 26.1", device: "Mac15,3")))
        rig.model.apply(.clipboard(text: "Stats for nerds\nVideo: 1920 × 1080"))
        #expect(rig.copied == ["Stats for nerds\nVideo: 1920 × 1080"])
        #expect(rig.model.statsCopiedAt != nil)
        try await until("Copied to go", timeout: .seconds(4)) { rig.model.statsCopiedAt == nil }
    }

    @Test func thePanelIsRememberedForTheNextPlaybackAndNotItsFigures() async throws {
        let rig = Rig()
        try await rig.opened(1)
        rig.model.toggleStats()
        rig.model.apply(.playerStats(playback: 1, stats: playerStatsFixture(state: .playing)))
        rig.model.viewerClosed()
        #expect(rig.model.statsShown)
        #expect(rig.model.stats == nil)

        // The next playback opens with the panel up, asking the engine again, waiting for its file.
        rig.model.apply(.playbackStaged(id: 900001, title: "Show", episode: "2", episodeTitle: nil, speed: 1))
        rig.ops(2, [.open(fullscreen: false, speed: nil, volume: nil)])
        #expect(rig.controls.last == .stats(true))
        #expect(rig.aims.last == 2)
        #expect(rig.model.statsWaitingSince != nil)
    }

    @Test func aboutNamesTheModelAndTheSystem() {
        let about = StatsAbout.here
        #expect(about.system.hasPrefix("macOS 26") || about.system.hasPrefix("macOS 27"))
        #expect(about.device.hasPrefix("Mac") || about.device.hasPrefix("Virtual"))
    }

    @Test func aValueWrapsBetweenItsPartsNotInsideThem() {
        let keep = "\u{00A0}"
        #expect(StatsPanel.wrapping("1920 × 1080 · HEVC 10-bit · 23.976 fps")
            == "1920\(keep)×\(keep)1080\(keep)· HEVC\(keep)10-bit\(keep)· 23.976\(keep)fps")
        #expect(StatsPanel.wrapping("42 s ahead · 61 MB") == "42\(keep)s ahead\(keep)· 61\(keep)MB")
        #expect(StatsPanel.wrapping("Software, 4 threads") == "Software, 4\(keep)threads")
    }
}
