// How Settings, Accounts and Sources say what they show.

import AnistreamKit
import AnistreamStore
@testable import AnistreamUI
import Foundation
import Testing

@MainActor
struct SettingsWordsTests {
    @Test func aChoiceReadsWithACapitalAndSubOrDubAsTheBoardsSayIt() {
        #expect(SettingKey.upscaling.display("natural · ArtCNN") == "Natural · ArtCNN")
        #expect(SettingKey.seedRatio.display("until ratio 1.0") == "Until ratio 1.0")
        #expect(SettingKey.translation.display("sub") == "Subbed")
        #expect(SettingKey.translation.display("dub · sub this session") == "Dubbed")
        #expect(SettingKey.commitThreshold.title == "Counts as watched at")
    }

    @Test func titleLanguageIsInGeneralNotPlayback() {
        #expect(Navigator(route: LaunchRoute()).settingsPane == .general, "the first pane opens first")
        var titles = LaunchRoute()
        titles.pane = "titles"
        #expect(Navigator(route: titles).settingsPane == .general)
        #expect(PhoneSettingsPage.named("titles") == .general)
        let index = SettingsResults.index
        #expect(index.contains { $0.title == "Title language" && $0.pane == .general })
        #expect(index.contains { $0.title == "Show adult titles" && $0.pane == .general })
    }

    /// A settings view as an engine with or without a Discord presence sends it.
    private func settingsView(presence: Bool) -> SettingsView {
        SettingsView(
            rows: [], refused: nil, translation: .sub, quality: 1080, theme: .adaptive,
            sources: ConfiguredSources(order: [], disabled: [], torrent: false, remote: false, index: nil),
            speed: nil, presenceClient: true, presenceAvailable: presence, nativeRows: [], overridden: [],
            torrent: ProtectionView(protection: .none, proxy: nil, onFailure: .pause)
        )
    }

    @Test func theDiscordSettingsAreHiddenWhereTheEngineHasNoPresence() {
        let store = AppStore()
        #expect(!store.settings.hasPresence, "nothing is shown before the engine has said")
        store.apply(.settings(settingsView(presence: false)))
        #expect(!store.settings.hasPresence)
        let hidden = SettingsResults.rows(presence: store.settings.hasPresence).map(\.title)
        for title in ["Discord presence", "Presence shows the title", "Rich presence"] {
            #expect(!hidden.contains(title), "\(title) searched for on a phone")
        }
        store.apply(.settings(settingsView(presence: true)))
        #expect(store.settings.hasPresence)
        let shown = SettingsResults.rows(presence: store.settings.hasPresence).map(\.title)
        #expect(shown.contains("Discord presence") && shown.contains("Rich presence"))
    }

    @Test func aSettingsViewBuiltByHandHasAPresenceUnlessTold() {
        // The bindings' default, so a front end that does not say keeps the terminal's behaviour.
        let view = SettingsView(
            rows: [], refused: nil, translation: .sub, quality: 1080, theme: .adaptive,
            sources: ConfiguredSources(order: [], disabled: [], torrent: false, remote: false, index: nil),
            speed: nil, presenceClient: true, nativeRows: [], overridden: [],
            torrent: ProtectionView(protection: .none, proxy: nil, onFailure: .pause)
        )
        #expect(view.presenceAvailable)
    }

    @Test func thePreviewDrawsWhatTheThreeSwitchesSend() {
        let off = PresenceSketch(enabled: false, titled: true, rich: true, show: "Show", episode: 5, total: 12)
        #expect(off.mode == .off)
        let plain = PresenceSketch(enabled: true, titled: true, rich: false, show: "Show", episode: 5, total: 12)
        #expect(plain.mode == .plain && plain.detail == "Episode 5" && !plain.cover && plain.button == nil)
        let rich = PresenceSketch(enabled: true, titled: true, rich: true, show: "Show", episode: 5, total: 12)
        #expect(rich.mode == .rich && rich.detail == "Episode 5 of 12" && rich.cover && rich.bar)
        #expect(rich.button == "View on AniList")
        let beyond = PresenceSketch(enabled: true, titled: true, rich: true, show: "Show", episode: 13, total: 12)
        #expect(beyond.detail == "Episode 13")
        let untitled = PresenceSketch(enabled: true, titled: false, rich: true, show: "Show", episode: 5, total: 12)
        #expect(untitled.mode == .plain && untitled.heading == "Watching anime" && untitled.detail.isEmpty)
        #expect(!untitled.cover && untitled.button == nil, "the title off keeps the show off every part")
    }

    @Test func theWeeksFootnoteSaysWhatWaitsForItsDubAndWhatIsOff() {
        #expect(NoticeWords.leftOut(dubbed: [], off: []) == "A row opens that show's settings.")
        #expect(NoticeWords.leftOut(dubbed: ["Summer Wind Relay"], off: ["Amber Forest"])
            == "Summer Wind Relay notifies when its dub is out. Amber Forest is off in its own settings. A row opens that show's settings.")
        AirTime.$locale.withValue(Locale(identifier: "en_GB")) {
            #expect(NoticeWords.leftOut(dubbed: ["One", "Two", "Three"], off: [])
                == "One, Two and Three notify when their dubs are out. A row opens that show's settings.")
        }
    }

    @Test func aTitleLanguageIsNamedWithItsCapitalInEverySentence() {
        let note = TitleLanguageWords.note(.anilist, signedIn: true, viewer: .english, examples: [])
        #expect(note.hasPrefix("Follows your AniList account, which is set to English. "))
        #expect(TitleLanguageWords.line(.anilist, signedIn: true, viewer: .romaji) == "Romaji, from your AniList account")
        for language in [TitleLanguage.romaji, .english, .native] {
            #expect(TitleLanguageWords.name(language).first?.isUppercase == true)
        }
    }

    @Test func theProtectionIsNamedAsTheTurnOnSheetNamesIt() {
        #expect(ProtectionWords.label(TorrentProtection.proxy) == "Proxy")
        #expect(ProtectionWords.label(TorrentProtection.device) == "VPN")
        #expect(ProtectionWords.label(TorrentProtection.none) == "No protection")
        #expect(ProtectionWords.label(Optional<TorrentProtection>.none) == "–", "nothing said yet")
        #expect(ProtectionWords.failedChecks.map { ProtectionWords.label($0) }
                == ["Pause torrents", "Stop torrents", "Warn only"])
    }

    @Test func aProxyNeedsItsHostAndPortAndKeepsThePasswordItHasWhenLeftEmpty() {
        let store = AppStore()
        var draft = TorrentDraft(store)
        draft.protection = .proxy
        #expect(!draft.complete, "no host yet")
        draft.host = " proxy.example "
        draft.port = "1080"
        #expect(draft.complete)
        #expect(draft.proxy == TorrentProxy(host: "proxy.example", port: 1080, user: nil, password: nil),
                "an empty password keeps the one kept")
        draft.port = "eighty"
        #expect(!draft.complete)
        draft.protection = .device
        #expect(draft.complete && draft.proxy == nil)
    }

    @Test func aProxyIsCheckedBeforeItIsSavedAndAnythingElseIsSavedAtOnce() {
        let store = AppStore()
        var draft = TorrentDraft(store)
        draft.protection = .proxy
        draft.host = "proxy.example"
        draft.port = "1080"
        #expect(!store.save(draft, .turnOn), "a proxy waits for its check")
        #expect(store.sentLog == ["checkTorrentProtection"], "nothing set, torrents not turned on")
        #expect(store.providers.trial == .checking)

        draft.protection = .device
        #expect(store.save(draft, .change))
        #expect(store.sentLog.last == "setTorrentProtection")
    }

    @Test func aProxyNotSavedSaysWhyAndWhatStays() {
        #expect(ProtectionWords.notSaved(.leaking, .editProxy)
                == "Not saved. Torrent traffic left outside this proxy, so the saved proxy stays.")
        #expect(ProtectionWords.notSaved(.unreachable, .editProxy)
                == "Not saved. The check couldn't be made through this proxy, so the saved proxy stays.")
        #expect(ProtectionWords.notSaved(.unreachable, .turnOn)
                == "Torrents are still off. The check couldn't be made through this proxy.")
        #expect(ProtectionWords.notSaved(.leaking, .change)
                == "Not saved. Torrent traffic left outside this proxy, so the saved protection stays.")
    }

    @Test func aSavedSheetSaysWhatItSaved() {
        #expect(ProtectionWords.saved(.editProxy) == "Proxy saved")
        #expect(ProtectionWords.saved(.turnOn) == "Torrents are on")
        #expect(ProtectionWords.saved(.change) == "Torrent protection saved")
    }

    @Test func theSavedProxyIsNamedByWhereItIsAndWhoItLetsIn() {
        let proxy = ProxyView(host: "proxy.example", port: 1080, user: "mika")
        #expect(ProtectionWords.proxyLine(proxy) == "proxy.example:1080 · as mika")
        #expect(ProtectionWords.proxyLine(ProxyView(host: "proxy.example", port: 1080, user: nil)) == "proxy.example:1080")
        #expect(ProtectionView(protection: .proxy, proxy: proxy, onFailure: .pause).proxyLine == "proxy.example:1080 · as mika")
        #expect(ProtectionView(protection: .device, proxy: proxy, onFailure: .pause).proxyLine == nil,
                "no Edit proxy once torrents go another way")
    }

    @Test func theGuardSaysWhereTrafficLeftAndHowLongAgo() {
        let at = Date(timeIntervalSince1970: 1_000)
        let (note, status, _) = GuardWords.words(.protected(exit: "Lantern Networks"), at: at, now: at.addingTimeInterval(120))
        #expect(note == "Leaving through Lantern Networks · 2 min ago")
        #expect(status == "Passed")
        #expect(GuardWords.words(.leaking, at: at).1 == "Failed")
        #expect(GuardWords.words(nil, at: nil).0 == "Not checked yet")
    }

    @Test func anAccountSaysWhatWaitsOrWhenItLastSynced() {
        let store = AppStore()
        func state(outbox: UInt32 = 0, reauth: Bool = false, last: String? = nil) -> SyncState {
            SyncState(tracker: "anilist", connected: true, user: nil, storage: "file", storageDegraded: false,
                      outbox: outbox, deferred: 0, needsReauth: reauth, last: last, pullError: nil)
        }
        #expect(AccountWords.status(state(), store.accounts) == "Synced")
        #expect(AccountWords.status(state(outbox: 2), store.accounts) == "2 changes waiting to sync")
        #expect(AccountWords.status(state(reauth: true), store.accounts) == "Sign in again to keep syncing")
        store.apply(.sync(state(last: "pulled")))
        #expect(AccountWords.status(state(last: "pulled"), store.accounts) == "Synced just now")
        #expect(AccountWords.tokens(store.accounts).contains("in a file"))
    }

    @Test func aShowsRowSaysTheDefaultItFollowsOrWhatItSets() {
        let follows = ShowSettingRow(key: .skipOpening, own: nil, value: "on", default: "on")
        #expect(ShowWords.useDefault(follows) == "Use default · On")
        #expect(ShowWords.shown(.translation, "dub") == "English")
        #expect(ShowWords.shown(.translation, "sub · dub this session") == "Japanese")
        #expect(ShowWords.choices(.spoilerProtection).map(\.label) == ["On", "Off"])
    }

    @Test func aRememberedTrackIsNamedByItsLanguageAndItsOwnWords() {
        #expect(ShowWords.trackWords(RememberedTrack(language: "en", title: "english signs", forced: false,
                                                     hearingImpaired: false)) == "English (Signs)")
        #expect(ShowWords.trackWords(RememberedTrack(language: "ja", title: "", forced: false,
                                                     hearingImpaired: false)) == "Japanese")
        #expect(ShowWords.trackWords(RememberedTrack(language: nil, title: "", forced: false,
                                                     hearingImpaired: false)) == "Unnamed track")
    }

    @Test func aSettingSomeShowsSetSaysForHowMany() {
        let store = AppStore()
        #expect(store.settings.overriddenNote(.skipOpening) == nil)
    }
}
