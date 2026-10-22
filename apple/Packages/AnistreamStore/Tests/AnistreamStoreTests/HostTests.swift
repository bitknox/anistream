// The host the engine calls, on secrets in memory, and the directories the engine is handed. Never
// the real keychain or the real home: the keychain's own backend is not exercised here.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

struct HostTests {
    @Test func theHostKeepsAndForgetsSecretsByID() {
        let secrets = MemorySecrets()
        let host = AppHost(secrets: secrets)
        #expect(host.secretGet(id: "anilist") == nil)
        #expect(host.secretSet(id: "anilist", value: "token-1"))
        #expect(host.secretSet(id: "anilist", value: "token-2"))
        #expect(host.secretGet(id: "anilist") == "token-2")
        #expect(host.secretDelete(id: "anilist"))
        // Signing out twice is no failure.
        #expect(host.secretDelete(id: "anilist"))
        #expect(secrets.all.isEmpty)
    }

    @Test func theHostOpensWhatItIsGivenAndRefusesWhatIsNoLink() {
        let opened = OpenedURLs()
        let host = AppHost(secrets: MemorySecrets(), open: { opened.add($0); return true })
        #expect(host.openUrl(url: "https://anilist.co/settings/developer"))
        #expect(opened.all == [URL(string: "https://anilist.co/settings/developer")!])
        #expect(!host.openUrl(url: ""))
    }

    @Test func theKeychainIsTheTerminalsService() {
        #expect(KeychainSecrets().service == "anistream")
    }
}

/// The URLs a host opened.
final class OpenedURLs: @unchecked Sendable {
    private let lock = NSLock()
    private var urls: [URL] = []
    var all: [URL] { lock.withLock { urls } }
    func add(_ url: URL) { lock.withLock { urls.append(url) } }
}

struct HomeTests {
    @Test func theMacSharesTheTerminalsDirectories() {
        let home = EngineHome.shared(home: URL(filePath: "/Users/mika", directoryHint: .isDirectory))
        #expect(home.config.path == "/Users/mika/Library/Application Support/anistream")
        #expect(home.data.path == "/Users/mika/Library/Application Support/anistream")
        #expect(home.cache.path == "/Users/mika/Library/Caches/anistream")
        #expect(!home.isOverride)
    }

    @Test func iOSKeepsToItsContainer() {
        let home = EngineHome.container(
            applicationSupport: URL(filePath: "/c/Library/Application Support"),
            caches: URL(filePath: "/c/Library/Caches")
        )
        #expect(home.dirs == FfiDirs(
            config: "/c/Library/Application Support",
            data: "/c/Library/Application Support",
            cache: "/c/Library/Caches"
        ))
    }

    @Test func theOverrideComesFromTheArgumentsAheadOfTheEnvironment() {
        let environment = [EngineHome.overrideVariable: "/tmp/from-env"]
        let fromEnv = EngineHome.resolve(environment: environment, arguments: ["app"])
        #expect(fromEnv.isOverride)
        #expect(fromEnv.data.path == "/tmp/from-env/data")
        let fromArgs = EngineHome.resolve(
            environment: environment,
            arguments: ["app", EngineHome.overrideArgument, "/tmp/from-args"]
        )
        #expect(fromArgs.config.path == "/tmp/from-args/config")
        #expect(fromArgs.cache.path == "/tmp/from-args/cache")
        #expect(fromArgs.dirs == FfiDirs(root: URL(filePath: "/tmp/from-args", directoryHint: .isDirectory)))
    }

    @Test func anEmptyOverrideIsNone() {
        #expect(EngineHome.overrideRoot(environment: [EngineHome.overrideVariable: ""], arguments: []) == nil)
        #expect(EngineHome.overrideRoot(environment: [:], arguments: [EngineHome.overrideArgument]) == nil)
    }

    @Test func theStartRecordSaysWhatTheAppIs() {
        var launch = EngineLaunch.app(
            home: .override(root: URL(filePath: "/tmp/anistream-home", directoryHint: .isDirectory)),
            platform: .macOs
        )
        // The registered clients, on every platform.
        #expect(launch.record.anilistClientId == "54537")
        #expect(launch.record.malClientId == "0ccc4725381aca9513f26d32107a94cd")
        #expect(launch.record.simklClientId == "016a3721d77c54d04f5aa6f5a213fe76f047d83d3ae35b2c892074859b88be0b")
        #expect(launch.record.traktClientId == "p2yWSSGe7aVAlnHEMiQQh0RhiL6ivZfJNaubOJi4DZE")
        #expect(launch.record.traktClientSecret == nil, "Trakt's client has no secret")
        #expect(launch.record.anilistRedirectUri == nil && launch.record.malRedirectUri == nil,
                "the app's own scheme")
        let phone = EngineLaunch.app(home: launch.home, platform: .ios)
        #expect(phone.record.anilistClientId == "54537" && phone.record.malClientId == EngineLaunch.malClient)
        launch.anilistClientID = "4242"
        launch.malClientID = nil
        let record = launch.record
        #expect(record.platform == .macOs)
        #expect(record.secrets == .followConfig)
        #expect(record.anilistClientId == "4242")
        #expect(record.malClientId == nil)
        #expect(record.theme?.ground == "#161A2E")
        #expect(record.theme?.font == "Zen Kaku Gothic New")
    }

    @Test func theMacAppFollowsTheConfigsTokenStorageAndIOSTheKeychain() {
        let root = URL(filePath: "/tmp/anistream-home", directoryHint: .isDirectory)
        #expect(EngineLaunch.app(home: .override(root: root), platform: .macOs).secrets == .followConfig)
        #expect(EngineLaunch.app(home: .override(root: root), platform: .ios).secrets == .host)
    }
}

struct FixturesLaunchTests {
    @Test func theSceneIsWatchingUnlessTheArgumentsSayOtherwise() {
        #expect(EngineLaunch.scene(arguments: ["app"]) == .watching)
        #expect(EngineLaunch.scene(arguments: ["app", "-AnistreamScene", "fresh"]) == .fresh)
        #expect(EngineLaunch.scene(arguments: ["app", "-AnistreamScene", "elsewhere"]) == nil)
        let playing = EngineLaunch.scene(arguments: ["app", "-AnistreamScene", "playing", "-AnistreamMedia", "/m.mkv"])
        #expect(playing == .playing(media: "/m.mkv"))
        let fixture = EngineLaunch.scene(arguments: ["app", "-AnistreamScene", "playing"])
        #expect(fixture == .playing(media: EngineLaunch.fixtureEpisode.path(percentEncoded: false)))
        #expect(EngineLaunch.fixtureEpisode.path().hasSuffix("apple/.mpv/media/episode.mkv"))
    }

    @Test func aFixturesLaunchNeverStartsOnTheSharedDirectories() {
        let (launch, temporary) = EngineLaunch.fixtures(platform: .macOs, arguments: ["app"], environment: [:])
        let root = try! #require(temporary)
        #expect(launch.home == .override(root: root))
        #expect(root.path.hasPrefix(FileManager.default.temporaryDirectory.path))
        let (overridden, none) = EngineLaunch.fixtures(
            platform: .macOs, arguments: ["app"], environment: [EngineHome.overrideVariable: "/tmp/shots"]
        )
        #expect(none == nil)
        #expect(overridden.home.data.path == "/tmp/shots/data")
    }
}
