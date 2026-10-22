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
        #expect(SettingKey.translation.display("sub") == "Subtitled")
        #expect(SettingKey.translation.display("dub · sub this session") == "Dubbed")
        #expect(SettingKey.commitThreshold.title == "Counts as watched at")
    }

    @Test func theWeeksFootnoteSaysWhatWaitsForItsDubAndWhatIsOff() {
        #expect(NoticeWords.leftOut(dubbed: [], off: []) == "A row opens that show’s settings.")
        #expect(NoticeWords.leftOut(dubbed: ["Summer Wind Relay"], off: ["Amber Forest"])
            == "Summer Wind Relay notifies when its dub is out. Amber Forest is off in its own settings. A row opens that show’s settings.")
        #expect(NoticeWords.leftOut(dubbed: ["One", "Two", "Three"], off: [])
            == "One, Two and Three notify when their dubs are out. A row opens that show’s settings.")
    }

    @Test func theProtectionIsNamedAsTheTurnOnSheetNamesIt() {
        #expect(ProtectionWords.label(TorrentProtection.proxy) == "SOCKS5 proxy")
        #expect(ProtectionWords.label(TorrentProtection.device) == "VPN on this device")
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

    @Test func theGuardSaysWhereTrafficLeftAndHowLongAgo() {
        let at = Date(timeIntervalSince1970: 1_000)
        let (note, status, _) = GuardWords.words(.protected(exit: "Lantern Networks"), at: at, now: at.addingTimeInterval(120))
        #expect(note == "Leaving through Lantern Networks · 2m ago")
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
        #expect(ShowWords.useDefault(follows, titleCase: true) == "Use Default · On")
        #expect(ShowWords.useDefault(follows, titleCase: false) == "Use default · On")
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
