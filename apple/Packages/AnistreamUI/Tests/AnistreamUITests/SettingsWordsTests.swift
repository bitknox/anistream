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
        #expect(Protection.label("socks5") == "SOCKS5 proxy")
        #expect(Protection.label("external") == "VPN on this device")
        #expect(Protection.label("none") == "No protection")
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

    @Test func aShowsSourceSaysTheDefaultItFollows() {
        let store = AppStore()
        #expect(ShowDefaults.spoilers(true) == "Use Default · On")
        #expect(ShowDefaults.skipOpening(store) == "Use Default · Off", "a row it does not have reads off")
    }
}
