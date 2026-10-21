// Settings, accounts, sources, plugin stores and downloads: the store fed scripted events, as the
// engine would send them, and what each model then holds.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

func provider(_ id: String, _ kind: String = "plugin", state: String = "ready", usable: Bool = true,
              heldBack: Bool = false) -> ProviderRow {
    ProviderRow(id: id, kind: kind, state: state, latencyMs: 320, lastError: nil, usable: usable, heldBack: heldBack)
}

func plugin(_ id: String, _ status: StorePluginStatus, hosts: [String] = ["films.example"]) -> StorePluginRow {
    StorePluginRow(id: id, name: id.capitalized, version: "1.5.0", hosts: hosts, status: status)
}

func storeRow(_ id: String, _ plugins: [StorePluginRow], state: StoreState = .ready) -> StoreRow {
    StoreRow(id: id, name: id.capitalized, url: "https://\(id).example", fingerprint: "9f3c 1a2b 77de 0c41",
             member: "Mika", owner: false, state: state, plugins: plugins)
}

func download(_ id: Int64, title: AnilistId, episode: String, _ state: DownloadState, done: UInt64,
              of total: UInt64) -> DownloadRow {
    DownloadRow(id: id, anilistId: title, title: "Title \(title)", episode: episode, state: state,
                fraction: total > 0 ? Double(done) / Double(total) : 0, downloaded: done, total: total,
                error: nil, path: state == .done ? "/tmp/\(id).mkv" : nil)
}

func settingsView(order: [String], rows: [SettingRow] = []) -> SettingsView {
    SettingsView(
        rows: rows, refused: nil, translation: .sub, quality: 1080, theme: .adaptive,
        sources: ConfiguredSources(order: order, disabled: [], torrent: true, remote: false),
        speed: nil, presenceClient: false
    )
}

@MainActor
struct ProvidersTests {
    @Test func theProvidersComeInAndANoteGoesOnceThereAreSome() {
        let store = AppStore()
        #expect(!store.providers.loaded)
        store.apply(.providerNote("No sources yet: plugins are still compiling."))
        store.apply(.providers([]))
        #expect(store.providers.loaded)
        #expect(store.providers.note == "No sources yet: plugins are still compiling.")
        store.apply(.providers([provider("alpha")]))
        #expect(store.providers.note == nil)
    }

    @Test func aRebuildLastsUntilTheRebuiltList() {
        let store = AppStore()
        store.apply(.sourcesChanged)
        #expect(store.providers.rebuilding)
        store.apply(.providers([provider("torrent", "torrent")]))
        #expect(!store.providers.rebuilding)
    }

    @Test func theRowsFollowTheOrderTheConfigHasNow() {
        let store = AppStore()
        store.apply(.providers([provider("torrent", "torrent"), provider("alpha"), provider("beta")]))
        store.apply(.settings(settingsView(order: ["beta", "torrent", "plugins"])))
        #expect(store.orderedProviders.map(\.id) == ["beta", "torrent", "alpha"])
    }

    @Test func aProviderSaysHowItStands() {
        #expect(provider("a").standing == .working)
        #expect(provider("a", state: "degraded").standing == .slow)
        #expect(provider("a", state: "down", usable: false).standing == .failing)
        #expect(provider("a", state: "held back", usable: false, heldBack: true).standing == .heldBack)
    }

    @Test func theGuardAndThePinnedSourceAreKept() {
        let store = AppStore()
        store.apply(.vpn(.protected(exit: "Lantern Networks")))
        #expect(store.providers.guardView == .protected(exit: "Lantern Networks"))
        #expect(store.providers.guardAt != nil)
        #expect(store.providers.guardView?.passes == true)
        store.apply(.vpn(nil))
        #expect(store.providers.guardAt == nil)
        store.apply(.providerPreference("alpha"))
        #expect(store.providers.preference == "alpha")
    }
}

@MainActor
struct StoresTests {
    @Test func anInviteIsCheckedThenLeadsToAStoreOrToWhyNot() {
        let store = AppStore()
        store.previewStore("  https://plugins.example/join/abc#key \n")
        #expect(store.stores.preview == .checking(link: "https://plugins.example/join/abc#key"))
        let invited = InvitedStore(link: "https://plugins.example/join/abc#key", name: "Hotaru Plugins",
                                   url: "https://plugins.example", fingerprint: "9f3c 1a2b", alreadyJoined: false)
        store.apply(.storePreview(.store(invited)))
        #expect(store.stores.preview == .store(invited))
        store.apply(.storePreview(.failed(reason: "the invite code in that link is damaged")))
        #expect(store.stores.preview == .failed("the invite code in that link is damaged"))
        store.apply(.storeJoined(store: "hotaru"))
        #expect(store.stores.preview == nil)
        #expect(store.stores.joined == "hotaru")
    }

    @Test func aPluginComesFromTheStoreThatKeepsItUpdated() {
        let store = AppStore()
        store.apply(.stores([
            storeRow("mika", [plugin("kobunko", .otherStore("Hotaru")), plugin("machi", .addedByHand)]),
            storeRow("hotaru", [plugin("kobunko", .installed), plugin("mukashi", .update(from: "1.4.0", newHosts: ["media2.example"]))]),
        ]))
        #expect(store.stores.origin(of: "kobunko")?.store.id == "hotaru")
        #expect(store.stores.origin(of: "machi")?.store.id == "mika", "added by hand")
        #expect(store.stores.origin(of: "mukashi")?.row.installedVersion == "1.4.0")
        #expect(store.stores.origin(of: "nothing") == nil)
        #expect(store.stores.updates == 1)
    }

    @Test func whatIsUnderWayEndsWithItsJob() {
        let store = AppStore()
        store.stores.started(.install(store: "hotaru", plugin: "mukashi"), job: 7)
        #expect(store.stores.isWorking(.install(store: "hotaru", plugin: "mukashi")))
        store.apply(.jobDone(7))
        #expect(!store.stores.isWorking(.install(store: "hotaru", plugin: "mukashi")))
    }

    @Test func aPluginSaysWhatItMayContact() {
        #expect(plugin("a", .available, hosts: ["one.example"]).contactsLine == "Contacts one.example")
        #expect(plugin("a", .available, hosts: ["one.example", "two.example"]).contactsLine
            == "Contacts one.example and 1 more")
    }

    @Test func anInviteLinkIsReadFromTheStoresAddressOrTheAppsOwn() throws {
        let https = "https://plugins.example/join/abc#key"
        #expect(InviteLinks.invite(in: try #require(URL(string: https))) == https)
        #expect(InviteLinks.invite(in: try #require(URL(string: "anistream://plugins.example/join/abc#key"))) == https)
        let wrapped = "anistream://join?link=" + (https.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? "")
        #expect(InviteLinks.invite(in: try #require(URL(string: wrapped))) == https)
        #expect(InviteLinks.invite(in: try #require(URL(string: "anistream://auth/anilist#token=x"))) == nil, "a sign-in")
        #expect(InviteLinks.invite(in: try #require(URL(string: "https://plugins.example/about"))) == nil)
        #expect(InviteLinks.looksLikeInvite(" \(https)\n"))
    }
}

@MainActor
struct DownloadsTests {
    @Test func theQueueIsGroupedByTitleWhereEachFirstAppears() {
        let store = AppStore()
        store.apply(.downloads([
            download(1, title: 900_001, episode: "8", .done, done: 1_400, of: 1_400),
            download(2, title: 900_006, episode: "9", .queued, done: 0, of: 0),
            download(3, title: 900_001, episode: "9", .active, done: 300, of: 1_400),
        ]))
        #expect(store.downloads.groups.map(\.id) == [900_001, 900_006])
        #expect(store.downloads.groups[0].rows.map(\.episode) == ["8", "9"])
        #expect(store.downloads.bytesOnDisk == 1_700)
    }

    @Test func aRunningDownloadsSpeedIsWhatItMovedOverTheTime() {
        let model = DownloadsModel()
        let start = Date(timeIntervalSince1970: 1_000)
        model.apply([download(3, title: 1, episode: "9", .active, done: 300_000_000, of: 1_400_000_000)], at: start)
        #expect(model.rates[3] == nil, "one reading is no speed")
        model.apply([download(3, title: 1, episode: "9", .active, done: 308_400_000, of: 1_400_000_000)],
                    at: start.addingTimeInterval(1))
        #expect(model.rates[3] == 8_400_000)
        #expect(model.secondsLeft(model.rows[0]) == 130)
        // Paused, it has no speed.
        model.apply([download(3, title: 1, episode: "9", .paused, done: 308_400_000, of: 1_400_000_000)],
                    at: start.addingTimeInterval(2))
        #expect(model.rates[3] == nil)
    }

    @Test func aSeedingFilesSizeIsWhatWentUpOverItsRatio() {
        let row = SeedRow(name: "Hoshikuzu no Tabibito - 05.mkv", uploadedBytes: 360_000_000, ratio: 0.8, goal: 1, leftSecs: 21_600)
        #expect(row.size == 450_000_000)
    }
}

@MainActor
struct AccountsTests {
    func state(_ tracker: String, connected: Bool = true, outbox: UInt32 = 0, last: String? = "pulled") -> SyncState {
        SyncState(tracker: tracker, connected: connected, user: nil, storage: "keychain", storageDegraded: false,
                  outbox: outbox, deferred: 0, needsReauth: false, last: last, pullError: nil)
    }

    @Test func aTrackerIsSyncedOnceASyncWentThroughWithNothingLeft() {
        let store = AppStore()
        store.apply(.sync(state("anilist", last: nil)))
        #expect(store.accounts.syncedAt["anilist"] == nil, "at launch nothing has synced yet")
        store.apply(.sync(state("anilist", outbox: 2, last: "queued")))
        #expect(store.accounts.syncedAt["anilist"] == nil)
        store.apply(.sync(state("anilist")))
        #expect(store.accounts.syncedAt["anilist"] != nil)
        store.apply(.sync(state("anilist", connected: false)))
        #expect(store.accounts.syncedAt["anilist"] == nil)
    }

    @Test func aSignInIsUnderWayUntilItsJobEnds() {
        let store = AppStore()
        store.accounts.started("kitsu", job: 4)
        #expect(store.accounts.isWorking("kitsu"))
        store.apply(.jobDone(4))
        #expect(!store.accounts.isWorking("kitsu"))
    }

    @Test func aSettingsRowSaysItsSwitchAndItsChoice() {
        let store = AppStore()
        store.apply(.settings(settingsView(order: [], rows: [
            SettingRow(key: .autoNext, value: "on", change: .steps(back: true, forward: false)),
            SettingRow(key: .quality, value: "720p", change: .steps(back: true, forward: true)),
        ])))
        #expect(store.settings.isOn(.autoNext))
        #expect(!store.settings.isOn(.nightMode), "a row it does not have")
        #expect(store.settings.row(.quality)?.option?.value == .int(720))
    }
}
