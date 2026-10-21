// The settings rules as Swift ports them, against the engine's own: every option a menu offers,
// set on the testing engine, comes back with the words the port gave it.

import AnistreamKit
import Foundation
import Testing

/// The next settings the engine says.
func nextSettings(from engine: FfiEngine) async throws -> SettingsView {
    while true {
        let batch = await engine.nextEvents()
        try #require(!batch.isEmpty, "the engine stopped before it said the settings")
        for event in batch {
            if case .settings(let view) = event { return view }
        }
    }
}

struct SettingsPortTests {
    @Test func theLaddersAreTheEnginesInOrder() {
        #expect(SettingKey.quality.options.map(\.label) == ["480p", "720p", "1080p", "2160p"])
        #expect(SettingKey.commitThreshold.options.map(\.label) == ["70%", "80%", "85%", "90%", "95%"])
        #expect(SettingKey.upscaling.options.map(\.label) == ["off", "natural · ArtCNN", "sharp · Anime4K"])
        #expect(SettingKey.seedRatio.options.map(\.label)
            == ["off", "until ratio 0.5", "until ratio 1.0", "until ratio 1.5", "until ratio 2.0"])
        #expect(SettingKey.seedHours.options.map(\.label) == ["1 h", "6 h", "12 h", "24 h", "72 h"])
        #expect(SettingKey.uploadLimit.options.map(\.label)
            == ["unlimited", "262 kB/s", "524 kB/s", "1.0 MB/s", "2.1 MB/s", "5.2 MB/s"])
        #expect(SettingKey.autoNext.options.map(\.value) == [.bool(false), .bool(true)])
        #expect(SettingKey.subtitles.options.isEmpty, "typed")
        #expect(SettingKey.vpnMode.options.isEmpty, "the file's to change")
    }

    @Test func aSizeReadsAsTheTerminalSaysIt() {
        #expect(Wording.bytes(0) == "0 B")
        #expect(Wording.bytes(999) == "999 B")
        #expect(Wording.bytes(9_400_000_000) == "9.4 GB")
        #expect(Wording.bytes(140_300_000) == "140 MB")
        #expect(Wording.bytes(1_500) == "1.5 kB")
    }

    @Test func aRowStandsOnTheSavedChoiceWhenTheSessionDiffers() {
        let row = SettingRow(key: .translation, value: "sub · dub this session", change: .steps(back: false, forward: true))
        #expect(row.option?.value == .str("sub"))
        let quality = SettingRow(key: .quality, value: "1080p", change: .steps(back: true, forward: true))
        #expect(quality.option?.value == .int(1080))
        let typed = SettingRow(key: .quality, value: "1440p", change: .steps(back: true, forward: true))
        #expect(typed.option == nil, "a value typed into the file is on no rung")
    }

    @Test(.timeLimit(.minutes(1)))
    func everyOptionSetOnTheEngineComesBackInThePortsWords() async throws {
        let (engine, root) = try startedEngine()
        defer { try? FileManager.default.removeItem(at: root) }
        defer { engine.shutdown() }
        _ = try await nextSettings(from: engine)

        let keys: [SettingKey] = [
            .quality, .commitThreshold, .upscaling, .seedRatio, .seedHours, .uploadLimit,
            .translation, .playerUi, .autoNext, .skipOpening, .nightMode, .keepSeeding,
        ]
        for key in keys {
            // Last to first, so each one is a change from the default the file starts with.
            for option in key.options.reversed() {
                _ = engine.send(command: .setSetting(key: key, value: option.value))
                let view = try await nextSettings(from: engine)
                let row = try #require(view.rows.first { $0.key == key })
                #expect(row.value == option.label, "\(key) set to \(option.value)")
                #expect(row.option == option)
            }
        }
    }
}

struct SourceOrderTests {
    func row(_ id: String, _ kind: String) -> ProviderRow {
        ProviderRow(id: id, kind: kind, state: "ready", latencyMs: nil, lastError: nil, usable: true, heldBack: false,
                    name: id, version: nil, hosts: [], origin: kind == "plugin" ? .byHand : .builtIn, disabled: false,
                    update: nil)
    }

    @Test func movingAPluginNamesThePluginsOnScreenAndKeepsPluginsAfterThem() {
        let rows = [row("torrent", "torrent"), row("alpha", "plugin"), row("beta", "plugin")]
        let order = ["torrent", "plugins"]
        #expect(SourceOrder.moved(order, rows: rows, id: "beta", step: -1) == ["torrent", "beta", "alpha", "plugins"])
        #expect(SourceOrder.moved(order, rows: rows, id: "alpha", step: -1) == ["alpha", "torrent", "beta", "plugins"])
        #expect(SourceOrder.moved(order, rows: rows, id: "torrent", step: 1) == ["alpha", "torrent", "beta", "plugins"])
    }

    @Test func aMoveStepsOverEntriesWithNoRowAndKeepsThem() {
        let rows = [row("remote", "remote"), row("alpha", "plugin")]
        #expect(SourceOrder.moved(["torrent", "remote", "gamma", "plugins"], rows: rows, id: "alpha", step: -1)
            == ["torrent", "alpha", "remote", "gamma", "plugins"])
        #expect(SourceOrder.moved(["alpha", "plugins", "remote"], rows: rows, id: "alpha", step: 1)
            == ["plugins", "remote", "alpha"])
    }

    @Test func aMovePastEitherEndOrOfNoRowChangesNothing() {
        let rows = [row("torrent", "torrent"), row("alpha", "plugin")]
        let order = ["torrent", "plugins", "remote"]
        #expect(SourceOrder.moved(order, rows: rows, id: "torrent", step: -1) == nil)
        #expect(SourceOrder.moved(order, rows: rows, id: "alpha", step: 1) == nil)
        #expect(SourceOrder.moved(order, rows: rows, id: "nothing", step: 1) == nil)
    }

    @Test func withoutPluginsInTheOrderAPluginOnScreenIsNamedAtTheEnd() {
        let rows = [row("torrent", "torrent"), row("alpha", "plugin")]
        #expect(SourceOrder.moved(["torrent"], rows: rows, id: "alpha", step: -1) == ["alpha", "torrent"])
    }

    @Test func aDragMovesAsManyPlacesAsItCrosses() {
        let rows = [row("torrent", "torrent"), row("alpha", "plugin"), row("beta", "plugin")]
        #expect(SourceOrder.moved(["torrent", "plugins"], rows: rows, id: "beta", to: 0)
            == ["beta", "torrent", "alpha", "plugins"])
        #expect(SourceOrder.moved(["torrent", "plugins"], rows: rows, id: "beta", to: 2) == nil, "where it is")
    }

    @Test func theRowsAreShownInTheOrderFailoverTriesThem() {
        let rows = [row("torrent", "torrent"), row("alpha", "plugin"), row("beta", "plugin")]
        #expect(SourceOrder.sorted(rows, by: ["beta", "torrent", "plugins"]).map(\.id) == ["beta", "torrent", "alpha"])
    }
}
