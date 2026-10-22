// Handoff: what an activity carries for a title's page and for an episode playing, what is read
// back from one, and the handed-over episode's start at its own place.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
struct HandoffTests {
    let types = HandoffTypes(bundleIdentifier: "tv.anistream.app")

    @Test func theTypesAreTheAppsOwn() {
        #expect(types.title == "tv.anistream.title")
        #expect(types.playing == "tv.anistream.playing")
        let fixtures = HandoffTypes(bundleIdentifier: "tv.anistream.fixtures")
        #expect(fixtures.title == "tv.anistream.fixtures.title")
        #expect(fixtures.playing == "tv.anistream.fixtures.playing")
    }

    @Test func aTitlesPageIsHandedOverByItsId() {
        let activity = NSUserActivity(activityType: types.title)
        Handoff.title(900_001).describe(activity, title: "Hoshikuzu no Tabibito")
        #expect(activity.title == "Hoshikuzu no Tabibito")
        #expect(activity.isEligibleForHandoff)
        #expect(!activity.isEligibleForSearch, "Spotlight has its own")
        #expect(activity.requiredUserInfoKeys == ["title"])
        #expect(activity.userInfo?["title"] as? Int == 900_001)
        #expect(Handoff(activity, types: types) == .title(900_001))
    }

    @Test func anEpisodeIsHandedOverWithWhereItWas() {
        let activity = NSUserActivity(activityType: types.playing)
        Handoff.playing(900_001, episode: "7", position: 760, translation: .dub)
            .describe(activity, title: "Hoshikuzu no Tabibito · Episode 7")
        #expect(activity.requiredUserInfoKeys == ["title", "episode", "position"])
        #expect(activity.userInfo?["translation"] as? String == "dub")
        #expect(Handoff(activity, types: types) == .playing(900_001, episode: "7", position: 760, translation: .dub))
        #expect(Handoff.playing(900_001, episode: "7", position: 760).activityType(types) == types.playing)

        let unknown = NSUserActivity(activityType: types.playing)
        Handoff.playing(900_001, episode: "7", position: 760).describe(unknown, title: "Hoshikuzu no Tabibito · Episode 7")
        #expect(unknown.userInfo?["translation"] == nil, "said only when known")
        #expect(Handoff(unknown, types: types) == .playing(900_001, episode: "7", position: 760))
    }

    @Test func onlyAWholeActivityOfTheAppsOwnTypesIsRead() {
        let full: [AnyHashable: Any] = ["title": 900_001, "episode": "7", "position": 760.0]
        #expect(Handoff(activityType: "tv.anistream.fixtures.playing", userInfo: full, types: types) == nil,
                "the Fixtures app's titles never reach the app")
        #expect(Handoff(activityType: types.playing, userInfo: ["title": 900_001], types: types) == nil)
        #expect(Handoff(activityType: types.title, userInfo: [:], types: types) == nil)
        #expect(Handoff(activityType: types.title, userInfo: ["title": -1], types: types) == nil)
        #expect(Handoff(activityType: types.playing, userInfo: ["title": 900_001, "episode": "7", "position": -3.0], types: types) == nil)
        #expect(Handoff(activityType: types.playing, userInfo: ["title": 900_001, "episode": "7", "position": Double.nan], types: types) == nil)
        #expect(Handoff(activityType: types.playing, userInfo: ["title": 900_001, "episode": "", "position": 1.0], types: types) == nil)
    }

    @Test func theHandedOverPlaceReadsAsTheResumeClock() {
        #expect(HandedOff(id: 900_001, episode: "7", position: 760).clock == "12:40")
        #expect(HandedOff(id: 900_001, episode: "7", position: 3_725).clock == "1:02:05")
    }

    @Test func theMoveComesOnceThePlaysFileIsOpen() {
        var start = StartAt(id: 900_001, episode: "7", position: 760)
        #expect(start.after(.playerReport(playback: 1, report: .fileLoaded)) == nil, "not staged yet")
        let staged = start.staged(id: 900_001, episode: "7")
        #expect(staged)
        #expect(start.after(.playerReport(playback: 1, report: .closed)) == nil)
        #expect(start.after(.playerReport(playback: 1, report: .fileLoaded)) == .player(.seekTo(760)))

        var other = StartAt(id: 900_001, episode: "7", position: 760)
        let nextEpisode = other.staged(id: 900_001, episode: "8")
        let otherTitle = other.staged(id: 900_005, episode: "7")
        #expect(!nextEpisode, "another episode gives it up")
        #expect(!otherTitle)
        #expect(other.after(.playerReport(playback: 1, report: .fileLoaded)) == nil)
    }
}

/// The handed-over start through the store, over the testing engine.
@MainActor
@Suite(.timeLimit(.minutes(1)))
struct HandoffStartTests {
    func started(root: URL) async throws -> AppStore {
        let store = AppStore()
        let launch = EngineLaunch(engine: .fixtures(.fresh), home: .override(root: root), platform: .macOs, secrets: .host)
        await store.start(launch, host: quietHost())
        try #require(store.phase == .running, "\(store.phase)")
        return store
    }

    /// What was sent since `mark`, by its case's name.
    func sent(_ store: AppStore, since mark: Int) -> [String] {
        Array(store.sentLog.dropFirst(mark))
    }

    @Test func aHandedOverPlayIsMovedToItsPlaceOnceItsFileIsOpen() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }

        let mark = store.sentLog.count
        store.play(900_001, episode: "7", at: 760)
        store.apply(.playbackStaged(id: 900_001, title: "Hoshikuzu no Tabibito", episode: "7", episodeTitle: nil, speed: 1))
        store.send(.playerReport(playback: 1, report: .fileLoaded))
        store.send(.playerReport(playback: 1, report: .fileLoaded))
        #expect(sent(store, since: mark) == ["play", "playerReport", "player", "playerReport"], "moved once")
    }

    @Test func anotherPlayGivesThePlaceUp() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }

        let mark = store.sentLog.count
        store.play(900_001, episode: "7", at: 760)
        store.send(.play(id: 900_001, episode: "7"))
        store.apply(.playbackStaged(id: 900_001, title: "Hoshikuzu no Tabibito", episode: "7", episodeTitle: nil, speed: 1))
        store.send(.playerReport(playback: 1, report: .fileLoaded))
        #expect(sent(store, since: mark) == ["play", "play", "playerReport"])

        // And one staged for another episode does too.
        let next = store.sentLog.count
        store.play(900_001, episode: "7", at: 760)
        store.apply(.playbackStaged(id: 900_001, title: "Hoshikuzu no Tabibito", episode: "8", episodeTitle: nil, speed: 1))
        store.send(.playerReport(playback: 2, report: .fileLoaded))
        #expect(sent(store, since: next) == ["play", "playerReport"])
    }
}

/// The episode playing as an activity of the app's own: offered while it plays, read when the
/// system asks, and told when another device takes it over.
@MainActor
struct PlayingActivityTests {
    let types = HandoffTypes(bundleIdentifier: "tv.anistream.app")

    @MainActor
    final class Playback {
        var now: (handoff: Handoff, title: String)?
        var continued = 0

        func at(_ position: Double) {
            now = (.playing(900_001, episode: "7", position: position), "Hoshikuzu no Tabibito · Episode 7")
        }
    }

    @Test func whatPlaysIsOfferedAndItsPlaceReadWhenTheSystemAsks() throws {
        let playback = Playback()
        let playing = PlayingActivity(types: types, playing: { playback.now }, continued: { playback.continued += 1 })
        playing.update()
        #expect(playing.activity == nil, "nothing plays")

        playback.at(100)
        playing.update()
        let offered = try #require(playing.activity)
        #expect(offered.activityType == "tv.anistream.playing")
        #expect(offered.isEligibleForHandoff)
        #expect(offered.title == "Hoshikuzu no Tabibito · Episode 7")

        playback.at(104)
        playing.update()
        #expect(playing.activity === offered, "the same activity, marked again")
        offered.delegate?.userActivityWillSave?(offered)
        #expect(Handoff(offered, types: types) == .playing(900_001, episode: "7", position: 104), "as it is when asked")

        playback.now = nil
        playing.update()
        #expect(playing.activity == nil, "no longer offered once nothing plays")
    }

    @Test func takenOverElsewhereItSaysSo() async throws {
        let playback = Playback()
        playback.at(760)
        let playing = PlayingActivity(types: types, playing: { playback.now }, continued: { playback.continued += 1 })
        playing.update()
        let offered = try #require(playing.activity)
        offered.delegate?.userActivityWasContinued?(offered)
        let deadline = ContinuousClock.now + .seconds(2)
        while playback.continued == 0, ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(5)) }
        #expect(playback.continued == 1)
    }
}
