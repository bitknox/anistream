// Spotlight: what a title is indexed as, which titles are on the shelf, what goes to the index
// as the lists change, and which page a result opens.

@testable import AnistreamUI
import AnistreamKit
import AnistreamStore
import CoreGraphics
import CoreSpotlight
import Foundation
import Testing

@MainActor
struct SpotlightTests {
    static let prefix = "tests."

    // MARK: What a title is indexed as

    @Test func aTitleIsIndexedWithItsNamesListEpisodeAndKeywords() {
        let title = ShelfTitle(wayfarer(), list: .watching)
        #expect(title.title == "Hoshikuzu no Tabibito")
        #expect(title.otherTitles == ["Stardust Wayfarer", "星屑の旅人"])
        #expect(title.otherTitle == "Stardust Wayfarer")
        #expect(title.line == "Watching · Episode 7 of 24")
        #expect(title.keywords == ["Adventure", "Fantasy", "TV", "2026"], "its genres, format and year")

        let item = SpotlightItem(title, prefix: "").searchable
        #expect(item.uniqueIdentifier == "title/900001")
        #expect(item.domainIdentifier == "watching", "the list's name")
        #expect(item.attributeSet.title == "Hoshikuzu no Tabibito")
        #expect(item.attributeSet.displayName == "Hoshikuzu no Tabibito")
        #expect(item.attributeSet.alternateNames == ["Stardust Wayfarer", "星屑の旅人"])
        #expect(item.attributeSet.contentDescription == "Watching · Episode 7 of 24")
        #expect(item.attributeSet.keywords == ["Adventure", "Fantasy", "TV", "2026"])
        #expect(item.expirationDate == .distantFuture, "a title stays while it is on the shelf")
        #expect(SpotlightItem(title, prefix: "fixtures.").domain == "fixtures.watching")
    }

    @Test func aFinishedOrPlannedTitleSaysItsListAlone() {
        let movie = ShelfTitle(shelved(900_005, "The Steel Gardener") {
            $0.format = "MOVIE"
            $0.year = 2025
            $0.episodes = 1
            $0.progress = TitleProgress(done: 1, next: 2)
        }, list: .completed)
        #expect(movie.otherTitles.isEmpty)
        #expect(movie.line == "Completed")
        #expect(movie.keywords == ["Movie", "2025"])
        #expect(ShelfTitle(shelved(900_003, "Moonlit Ferry"), list: .planning).line == "Planning")
        let caughtUp = ShelfTitle(shelved(900_002, "The Library Above the Clouds") {
            $0.episodes = 12
            $0.progress = TitleProgress(done: 12, next: 13)
        }, list: .paused)
        #expect(caughtUp.line == "Paused", "no episode past the last")
        let continuing = ShelfTitle(shelved(900_006, "Summer Wind Relay") { $0.progress = TitleProgress(done: 7, next: 8) }, list: .continue)
        #expect(continuing.line == "Continue · Episode 8", "on Continue alone, its total not known")
    }

    @Test func theFixturesAppsDomainsAreItsOwn() {
        #expect(SpotlightIndexer.prefix(bundleIdentifier: "tv.anistream.app") == "")
        #expect(SpotlightIndexer.prefix(bundleIdentifier: "tv.anistream.fixtures") == "fixtures.")
        #expect(SpotlightIndexer.prefix(bundleIdentifier: nil) == "")
        #expect(SpotlightItem.domains(prefix: "fixtures.") == [
            "fixtures.continue", "fixtures.watching", "fixtures.planning", "fixtures.completed",
            "fixtures.paused", "fixtures.dropped",
        ])
    }

    // MARK: Which page a result opens

    @Test func aResultOpensItsTitle() {
        let result = NSUserActivity(activityType: CSSearchableItemActionType)
        result.userInfo = [CSSearchableItemActivityIdentifier: "title/900001"]
        #expect(SpotlightItem.title(in: result) == 900_001)

        let other = NSUserActivity(activityType: CSSearchableItemActionType)
        other.userInfo = [CSSearchableItemActivityIdentifier: "episode/900001"]
        #expect(SpotlightItem.title(in: other) == nil)
        let handoff = NSUserActivity(activityType: "tv.anistream.title")
        handoff.userInfo = [CSSearchableItemActivityIdentifier: "title/900001"]
        #expect(SpotlightItem.title(in: handoff) == nil)
    }

    @Test func aResultOpensItsPageOnceOverWhatIsOnScreen() {
        let navigator = Navigator(route: LaunchRoute())
        navigator.reveal(900_002)
        navigator.reveal(900_002)
        #if os(macOS)
        #expect(navigator.titleOnMac == 900_002)
        #endif
        #expect(navigator.paths[.continue] == [900_002])
    }

    // MARK: The shelf

    @Test func theShelfIsContinueThenTheLibraryEachTitleOnce() {
        let store = watchingStore()
        let shelf = Shelf.current(store, over: Shelf())
        #expect(shelf.order == [900_001, 900_005, 900_002, 900_003])
        #expect(shelf.sources[.continue] == [900_001, 900_005])
        #expect(shelf.sources[.watching] == [900_005, 900_002])
        #expect(shelf.sources[.planning] == [900_003])
        #expect(shelf.sources[.completed] == [], "never shown, nothing saved")
        #expect(shelf.titles[900_001]?.list == .continue)
        #expect(shelf.titles[900_005]?.list == .watching, "a Library list before Continue")
        #expect(shelf.titles[900_003]?.list == .planning)
    }

    @Test func aListNotShownThisSessionKeepsWhatWasSaved() {
        var saved = Shelf()
        saved.sources[.completed] = [900_004]
        saved.titles[900_004] = ShelfTitle(id: 900_004, title: "Lanterns of the Old Quarter")
        let shelf = Shelf.current(watchingStore(), over: saved)
        #expect(shelf.order == [900_001, 900_005, 900_002, 900_003, 900_004])
        #expect(shelf.titles[900_004]?.title == "Lanterns of the Old Quarter")
    }

    @Test func signedOutThereIsNoLibrary() {
        let store = watchingStore()
        store.apply(.sync(SyncState(tracker: "anilist", connected: false, outbox: 0, deferred: 0)))
        #expect(Shelf.current(store, over: Shelf()).order == [900_001, 900_005])
    }

    @Test func untilTheTrackersSaySoTheLibraryIsKept() {
        // At launch, before the engine has said a tracker's state, nobody is signed out.
        let shelf = Shelf.current(watchingStore(), over: Shelf())
        #expect(shelf.order.contains(900_003))
    }

    @Test func theShelfIsSavedAndReadBack() throws {
        let file = temporaryFile()
        defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
        var shelf = Shelf.current(watchingStore(), over: Shelf())
        shelf.covered = [900_001]
        try shelf.save(file)
        #expect(Shelf.load(file) == shelf)
    }

    @Test func theShelfIsSavedInTheEnginesCache() {
        let home = EngineHome.override(root: URL(filePath: "/tmp/anistream-home", directoryHint: .isDirectory))
        #expect(Shelf.file(in: home).path == "/tmp/anistream-home/cache/apple/shelf.json")
    }

    // MARK: Keeping the index

    @Test func theFirstPassClearsTheDomainAndIndexesTheShelf() async {
        let file = temporaryFile()
        defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
        let index = RecordingIndex()
        let indexer = indexer(watchingStore(), index: index, file: file)
        await indexer.pass()

        let calls = await index.calls
        #expect(calls.count == 2)
        #expect(calls.first == .deleteAll(SpotlightItem.domains(prefix: Self.prefix)))
        #expect(calls.last?.indexed == ["title/900001", "title/900005", "title/900002", "title/900003"])
        #expect(Shelf.load(file)?.order == [900_001, 900_005, 900_002, 900_003])
    }

    @Test func aTitleThatLeavesIsRemovedAndTheRestAreLeftAlone() async {
        let store = watchingStore()
        let index = RecordingIndex()
        let indexer = indexer(store, index: index)
        await indexer.pass()
        await index.reset()

        // Finished and gone from Continue: 900001 is on no list now. 900005 is still Watching.
        store.apply(.content(view: .continue, content: .entries([steelGardener()])))
        await indexer.pass()
        #expect(await index.calls == [.delete(["title/900001"])])
    }

    @Test func aTitleTakenOffContinueLeavesUnlessALibraryListHasIt() async {
        let store = watchingStore()
        let index = RecordingIndex()
        let indexer = indexer(store, index: index)
        await indexer.pass()
        await index.reset()

        store.hideFromContinue(900_005)
        await indexer.pass()
        #expect(await index.calls.isEmpty, "900005 is on Watching")
        store.hideFromContinue(900_001)
        await indexer.pass()
        #expect(await index.calls == [.delete(["title/900001"])])
    }

    @Test func aTitleThatChangesIsIndexedAgain() async {
        let store = watchingStore()
        let index = RecordingIndex()
        let indexer = indexer(store, index: index)
        await indexer.pass()
        await index.reset()

        // Named another way: its title in another language first, as the engine names it once
        // Settings › Title language changes.
        store.apply(.content(view: .library(.planning), content: .entries([
            shelved(900_003, "Tsukiyo no Watashibune") { $0.secondary = "Moonlit Ferry" },
        ])))
        await indexer.pass()
        let calls = await index.calls
        #expect(calls.map(\.indexed) == [["title/900003"]])
        #expect(calls.first?.items.first?.title == "Tsukiyo no Watashibune")
    }

    @Test func aTitleThatMovesListIsIndexedAgainUnderItsNewList() async {
        let store = watchingStore()
        let index = RecordingIndex()
        let indexer = indexer(store, index: index)
        await indexer.pass()
        await index.reset()

        store.apply([
            .content(view: .library(.watching), content: .entries([
                shelved(900_002, "The Library Above the Clouds") { $0.coverUrl = "cover/900002.jpg" },
            ])),
            .content(view: .library(.completed), content: .entries([steelGardener()])),
        ])
        await indexer.pass()
        let items = await index.calls.flatMap(\.items)
        #expect(items.map(\.identifier) == ["title/900005"])
        #expect(items.first?.domain == "tests.completed")
        #expect(items.first?.line == "Completed")
    }

    @Test func aLaterLaunchTakesUpFromTheSavedShelf() async {
        let file = temporaryFile()
        defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
        await indexer(watchingStore(), index: RecordingIndex(), file: file).pass()

        // The next launch: only Continue has answered, and 900005 has left it but is still
        // saved as Watching; 900001 has left Continue and is on no list.
        let store = AppStore()
        store.apply(.content(view: .continue, content: .entries([steelGardener()])))
        let index = RecordingIndex()
        await indexer(store, index: index, file: file).pass()
        #expect(await index.calls == [.delete(["title/900001"])], "no clearing: the saved shelf says what is there")
    }

    @Test func onceStartedTheIndexFollowsTheListsByItself() async throws {
        let store = AppStore()
        let index = RecordingIndex()
        let indexer = SpotlightIndexer(
            store: store, index: index, prefix: Self.prefix, file: nil,
            defaults: Self.defaults, delay: .milliseconds(20), covers: FakeCovers()
        )
        indexer.start()
        store.apply(.content(view: .continue, content: .entries([wayfarer()])))
        try await until { await index.calls.count == 2 }
        #expect(await index.calls.last?.indexed == ["title/900001"])

        store.apply(.content(view: .continue, content: .empty))
        try await until { await index.calls.count == 3 }
        #expect(await index.calls.last == .delete(["title/900001"]))
    }

    /// Wait for `condition`, two seconds at most.
    func until(_ condition: () async -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(2)
        while await !condition() {
            guard ContinuousClock.now < deadline else {
                Issue.record("timed out")
                return
            }
            try await Task.sleep(for: .milliseconds(10))
        }
    }

    @Test func nothingIsToldBeforeAnyListHasAnswered() async {
        let index = RecordingIndex()
        await indexer(AppStore(), index: index).pass()
        #expect(await index.calls.isEmpty)
    }

    @Test func coversAreAskedForAndIndexedOnceTheyAreHere() async {
        let store = watchingStore()
        let index = RecordingIndex()
        let covers = FakeCovers()
        let indexer = indexer(store, index: index, covers: covers)
        await indexer.pass()
        #expect(covers.requested == ["cover/900001.jpg", "cover/900002.jpg"])
        #expect(await index.calls.last?.items.allSatisfy { $0.thumbnail == nil } == true)
        await index.reset()

        covers.images["cover/900001.jpg"] = FakeCovers.picture(width: 460, height: 690)
        await indexer.pass()
        let items = await index.calls.flatMap(\.items)
        #expect(items.map(\.identifier) == ["title/900001"])
        let thumbnail = try! #require(items.first?.thumbnail)
        let decoded = try! #require(ArtworkStore.decode(thumbnail, maxPixelSize: 1000))
        #expect(decoded.height == SpotlightIndexer.coverPixels)
        #expect(decoded.width == 120)
        #expect(indexer.shelf?.covered == [900_001])

        await index.reset()
        await indexer.pass()
        #expect(await index.calls.isEmpty, "a cover goes once")

        // Named another way, it is indexed again whole, its cover with it.
        store.apply(.content(view: .continue, content: .entries([
            shelved(900_001, "Stardust Wayfarer") { $0.coverUrl = "cover/900001.jpg" }, steelGardener(),
        ])))
        await indexer.pass()
        let again = await index.calls.flatMap(\.items)
        #expect(again.map(\.identifier) == ["title/900001"])
        #expect(again.first?.title == "Stardust Wayfarer")
        #expect(again.first?.thumbnail != nil)
    }

    @Test func aCoverThatDoesNotComeIsNotAskedForAgain() async {
        let store = watchingStore()
        let covers = FakeCovers()
        let indexer = indexer(store, index: RecordingIndex(), covers: covers)
        await indexer.pass()
        covers.pending = []
        await indexer.pass()
        await indexer.pass()
        #expect(covers.requested == ["cover/900001.jpg", "cover/900002.jpg"])
    }

    @Test func switchedOffTheDomainIsClearedAndNothingIndexed() async throws {
        let file = temporaryFile()
        defer { try? FileManager.default.removeItem(at: file.deletingLastPathComponent()) }
        let suite = "anistream.tests.spotlight.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let index = RecordingIndex()
        let store = watchingStore()
        let indexer = indexer(store, index: index, file: file, defaults: defaults)
        #expect(indexer.isOn, "on unless turned off")
        await indexer.pass()
        await index.reset()

        defaults.set(false, forKey: SpotlightIndexer.preference)
        await indexer.pass()
        #expect(await index.calls == [.deleteAll(SpotlightItem.domains(prefix: Self.prefix))])
        #expect(Shelf.load(file) == nil)
        await index.reset()
        store.apply(.content(view: .continue, content: .entries([steelGardener()])))
        await indexer.pass()
        #expect(await index.calls.isEmpty)

        defaults.set(true, forKey: SpotlightIndexer.preference)
        await indexer.pass()
        #expect(await index.calls.map(\.indexed) == [["title/900005", "title/900002", "title/900003"]])
    }

    // MARK: Support

    func indexer(
        _ store: AppStore, index: RecordingIndex, file: URL? = nil,
        defaults: UserDefaults? = nil, covers: FakeCovers = FakeCovers()
    ) -> SpotlightIndexer {
        SpotlightIndexer(
            store: store, index: index, prefix: Self.prefix, file: file,
            defaults: defaults ?? Self.defaults, delay: .zero, covers: covers
        )
    }

    /// Defaults of the tests' own, never the app's.
    static let defaults: UserDefaults = {
        let suite = "anistream.tests.spotlight"
        UserDefaults().removePersistentDomain(forName: suite)
        return UserDefaults(suiteName: suite)!
    }()

    func temporaryFile() -> URL {
        FileManager.default.temporaryDirectory
            .appending(path: "anistream-spotlight-\(UUID().uuidString)/shelf.json", directoryHint: .notDirectory)
    }

    /// Named as the engine names it with Settings › Title language on romaji: the romaji, with
    /// the English as the other form.
    func wayfarer() -> Entry {
        shelved(900_001, "Hoshikuzu no Tabibito") {
            $0.secondary = "Stardust Wayfarer"
            $0.titleRomaji = "Hoshikuzu no Tabibito"
            $0.titleEnglish = "Stardust Wayfarer"
            $0.titleNative = "星屑の旅人"
            $0.format = "TV"
            $0.year = 2026
            $0.genres = ["Adventure", "Fantasy"]
            $0.coverUrl = "cover/900001.jpg"
            $0.episodes = 24
            $0.progress = TitleProgress(done: 6, next: 7)
        }
    }

    func steelGardener() -> Entry {
        shelved(900_005, "The Steel Gardener") { $0.progress = TitleProgress(done: 1, next: 2) }
    }

    /// Continue with two titles, Watching with one of them and another, Planning with a third.
    func watchingStore() -> AppStore {
        let store = AppStore()
        store.apply([
            .content(view: .continue, content: .entries([wayfarer(), steelGardener()])),
            .content(view: .library(.watching), content: .entries([
                steelGardener(),
                shelved(900_002, "The Library Above the Clouds") { $0.coverUrl = "cover/900002.jpg" },
            ])),
            .content(view: .library(.planning), content: .entries([shelved(900_003, "Moonlit Ferry")])),
        ])
        return store
    }
}

/// A title with only its id and name, and whatever `change` sets.
func shelved(_ id: AnilistId, _ title: String, _ change: (inout Entry) -> Void = { _ in }) -> Entry {
    var shelved = entry {
        $0.id = id
        $0.title = title
        $0.secondary = nil
        $0.format = nil
        $0.episodes = nil
        $0.year = nil
        $0.score = nil
        $0.studio = nil
    }
    change(&shelved)
    return shelved
}

/// What the indexer told Spotlight, in order.
actor RecordingIndex: SpotlightIndex {
    enum Call: Equatable {
        case index([SpotlightItem])
        case delete([String])
        case deleteAll([String])

        var items: [SpotlightItem] {
            if case .index(let items) = self { items } else { [] }
        }

        var indexed: [String] { items.map(\.identifier) }
    }

    private(set) var calls: [Call] = []

    func index(_ items: [SpotlightItem]) async throws { calls.append(.index(items)) }
    func delete(_ identifiers: [String]) async throws { calls.append(.delete(identifiers)) }
    func deleteAll(domains: [String]) async throws { calls.append(.deleteAll(domains)) }
    func reset() { calls = [] }
}

/// Covers as the tests have them: none until a test puts one here.
@MainActor
final class FakeCovers: SpotlightCovers {
    var images: [String: CGImage] = [:]
    var requested: [String] = []
    /// What is on its way: whatever was asked for, until a test says otherwise.
    var pending: Set<String>?

    func image(_ url: String) -> CGImage? { images[url] }
    func request(cover url: String) { requested.append(url) }
    func isPending(_ url: String) -> Bool { pending.map { $0.contains(url) } ?? requested.contains(url) }

    static func picture(width: Int, height: Int) -> CGImage {
        let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
        )!
        context.setFillColor(red: 0.9, green: 0.6, blue: 0.3, alpha: 1)
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        return context.makeImage()!
    }
}
