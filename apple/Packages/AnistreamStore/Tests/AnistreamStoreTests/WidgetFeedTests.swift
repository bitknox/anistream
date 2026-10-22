// The widgets' snapshot: the engine's record as the widgets read it, written with its art into a
// directory of the test's own, never an App Group.

@testable import AnistreamStore
import AnistreamKit
import AnistreamWidgetData
import CoreGraphics
import Foundation
import Testing

/// Art for any address: a plain image, and a record of what was asked.
@MainActor
final class FakeArt: WidgetArtSource {
    var asked: [String] = []
    var missing: Set<String> = []

    func images(for urls: [String], maxPixelSize: Int) async -> [String: CGImage] {
        asked += urls
        var images: [String: CGImage] = [:]
        for url in urls where !missing.contains(url) {
            let context = CGContext(
                data: nil, width: 400, height: 225, bitsPerComponent: 8, bytesPerRow: 0,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)!
            context.setFillColor(red: 0.2, green: 0.3, blue: 0.5, alpha: 1)
            context.fill(CGRect(x: 0, y: 0, width: 400, height: 225))
            images[url] = context.makeImage()
        }
        return images
    }
}

@MainActor
final class Reloads {
    var count = 0
}

@MainActor
struct WidgetFeedTests {
    let data = WidgetData(
        version: 2, generatedAt: 1_792_516_200,
        continueWatching: [
            WidgetContinue(
                id: 900_001, title: "Hoshikuzu no Tabibito", episode: 7, watched: 6, episodes: 12, fraction: 0.42,
                coverUrl: "https://img.example/cover/900001.jpg", episodeTitle: "The Lantern Road", resume: true,
                positionSecs: 600, durationSecs: 1_440, minutesLeft: 14, bannerUrl: "https://img.example/banner/900001.jpg",
                coverColor: "#e4a15d"),
            WidgetContinue(
                id: 900_005, title: "Hagane no Niwashi", episode: 2, coverUrl: "https://img.example/cover/900005.jpg",
                nextAiring: 1_792_679_400, nextAiringEpisode: 3),
        ],
        airingToday: [
            WidgetAiring(id: 900_003, title: "Tsukiakari no Teishajo", episode: 2, airsAt: 1_792_528_200,
                         coverUrl: "https://img.example/cover/900003.jpg"),
            WidgetAiring(id: 900_006, title: "Natsukaze Relay", episode: 9, airsAt: 1_792_522_800, onList: true,
                         coverUrl: "https://img.example/cover/900006.jpg", watched: true, translation: .dub),
        ],
        signedIn: true, tracker: "anilist",
        nextAiring: WidgetNextAiring(id: 900_006, title: "Natsukaze Relay", episode: 9, airsAt: 1_792_522_800),
        nextDays: [
            WidgetAiring(id: 900_002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: 1_792_679_400, onList: true,
                         coverUrl: "https://img.example/cover/900002.jpg", bannerUrl: "https://img.example/banner/900002.jpg"),
            WidgetAiring(id: 900_004, title: "Hoshikuzu no Tabibito 2nd Season", episode: 1, airsAt: 1_792_679_500,
                         coverUrl: "https://img.example/cover/900004.jpg"),
        ]
    )

    @Test func theRecordIsWrittenAsTheWidgetsReadIt() {
        let snapshot = WidgetFeed.snapshot(data, art: ["https://img.example/banner/900001.jpg": "wide-a.jpg"])
        #expect(snapshot.version == WidgetSnapshot.newestVersion)
        #expect(snapshot.generatedAt == Date(timeIntervalSince1970: 1_792_516_200))
        #expect(snapshot.signedIn == true && snapshot.tracker == "anilist")
        let first = snapshot.continueWatching[0]
        #expect(first.id == 900_001 && first.episode == 7 && first.minutesLeft == 14 && first.resume == true)
        #expect(first.episodeTitle == "The Lantern Road")
        #expect(first.art == Art(wide: "wide-a.jpg", poster: nil, color: "#e4a15d"))
        #expect(snapshot.continueWatching[1].nextAiring == Date(timeIntervalSince1970: 1_792_679_400))
        #expect(snapshot.continueWatching[1].nextAiringEpisode == 3)
        #expect(snapshot.airingToday[1].translation == .dub)
        #expect(snapshot.airingToday[1].watched)
        #expect(snapshot.nextDays.count == 2)
        #expect(snapshot.nextAiring?.episode == 9)
    }

    @Test func theArtAskedForIsContinuesThenTodaysListThenTheRest() {
        let wanted = WidgetFeed.art(in: data).map(\.url)
        #expect(wanted == [
            "https://img.example/banner/900001.jpg", "https://img.example/cover/900001.jpg",
            "https://img.example/cover/900005.jpg",
            // Today's, the list's first, a cover where there is no banner.
            "https://img.example/cover/900006.jpg", "https://img.example/cover/900003.jpg",
            // The next days' in the list only.
            "https://img.example/banner/900002.jpg",
        ])
    }

    @Test func aSnapshotIsWrittenWithItsArtAndTheWidgetsAreToldOnce() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let files = WidgetFiles(directory: root)
        let reloads = Reloads()
        let feed = WidgetFeed(files: files, reload: { reloads.count += 1 })
        let art = FakeArt()
        art.missing = ["https://img.example/cover/900003.jpg"]
        try await feed.write(data, art: art)
        let snapshot = try #require(files.snapshot())
        #expect(reloads.count == 1)
        let wide = try #require(snapshot.continueWatching[0].art.wide)
        #expect(files.hasArt(wide))
        #expect(snapshot.airingToday[0].art.poster == nil, "its art could not be had")
        #expect(snapshot.airingToday[1].art.poster != nil)

        // Written again, the art there already is not asked for again.
        art.asked = []
        try await feed.write(data, art: art)
        #expect(art.asked == ["https://img.example/cover/900003.jpg"])
        #expect(reloads.count == 2)
    }

    @Test func withNoAppGroupNothingIsWritten() async throws {
        let reloads = Reloads()
        let feed = WidgetFeed(files: nil, reload: { reloads.count += 1 })
        #expect(!feed.isEnabled)
        try await feed.write(data, art: FakeArt())
        #expect(reloads.count == 0)
        // The test runner's bundle names no App Group, so a store made here writes nothing.
        #expect(!AppStore().widgets.isEnabled)
    }
}

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct WidgetFeedEngineTests {
    @Test func continueLoadingAsksForTheSnapshotAndItIsWritten() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let files = WidgetFiles(directory: root.appending(path: "widgets", directoryHint: .isDirectory))
        let reloads = Reloads()
        let store = AppStore(widgets: WidgetFeed(files: files, reload: { reloads.count += 1 }))
        await store.start(
            EngineLaunch(engine: .fixtures(.watching), home: .override(root: root.appending(path: "home")),
                         platform: .macOs, secrets: .host),
            host: quietHost()
        )
        defer { store.shutdown() }
        store.loadContinue()
        try await waitFor({ reloads.count > 0 }, within: .seconds(30))
        await store.widgets.finished()
        let snapshot = try #require(files.snapshot())
        #expect(snapshot.version == 2)
        #expect(!snapshot.continueWatching.isEmpty)
        #expect(snapshot.continueWatching.allSatisfy { (900_001...900_006).contains($0.id) })
        // The fake AniList serves the fixtures' art, which is kept as the widgets' thumbnails.
        let art = snapshot.continueWatching.flatMap { [$0.art.wide, $0.art.poster] }.compactMap { $0 }
        #expect(!art.isEmpty)
        #expect(art.allSatisfy(files.hasArt))
        #expect(store.sentLog.filter { $0 == "widgetSnapshot" }.count == 1, "changes are gathered into one ask")
    }
}
