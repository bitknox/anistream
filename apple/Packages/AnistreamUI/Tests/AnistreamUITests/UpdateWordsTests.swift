// What Settings says of updates under this version, and the action it offers.

import AnistreamKit
import AnistreamStore
@testable import AnistreamUI
import Foundation
import Testing

@MainActor
struct UpdateWordsTests {
    let release = Release(
        version: "2.0.3", publishedAt: 1_792_664_639, pageUrl: "https://github.example/releases/tag/v2.0.3",
        notes: "", macos: nil, ios: nil, android: nil)

    func words(
        latest: Release? = nil, checking: Bool = false, outcome: UpdatesModel.Outcome? = nil,
        progress: UpdateProgress = .idle, installsHere: Bool = false, mobile: Bool = false
    ) -> UpdateWords {
        UpdateWords(latest: latest, checking: checking, outcome: outcome, progress: progress,
                    installsHere: installsHere, mobile: mobile)
    }

    @Test func withNothingNewItSaysSoAndOffersCheckNow() {
        #expect(words() == UpdateWords(note: nil, action: .checkNow))
        #expect(words(outcome: .upToDate) == UpdateWords(note: "Up to date", action: .checkNow))
        #expect(words(checking: true) == UpdateWords(note: "Checking…", action: .checkNow, disabled: true))
        let failed = words(outcome: .failed("GitHub could not be reached"))
        #expect(failed.note == "Could not check. GitHub could not be reached.")
        #expect(failed.problem)
        #expect(failed.label == "Check Now")
    }

    @Test func aReleaseIsOfferedTheWayThisDeviceTakesIt() {
        let mac = words(latest: release, installsHere: true)
        #expect(mac.note == "Version 2.0.3 is available")
        #expect(mac.label == "Update to v2.0.3")
        #expect(words(latest: release, mobile: true).label == "Update in SideStore")
        // A Mac release without its file: the page.
        #expect(words(latest: release).action == .releasePage)
    }

    @Test func anUpdateUnderWaySaysHowFarAndCanBeStoppedUntilItInstalls() {
        let downloading = words(latest: release, progress: .downloading(received: 12_000_000, expected: 56_208_258), installsHere: true)
        #expect(downloading.note == "Downloading 12 MB of 56 MB")
        #expect(downloading.action == .cancel)
        #expect(words(progress: .downloading(received: 1_500_000, expected: nil)).note == "Downloading 1.5 MB")
        #expect(words(progress: .preparing).action == .cancel)
        #expect(words(progress: .unpacking(0.5)).note == "Checking the download")
        let relaunching = words(progress: .relaunching)
        #expect(relaunching.action == .none)
        #expect(relaunching.label == nil)

        let failed = words(latest: release, progress: .failed("The update could not be verified."), installsHere: true)
        #expect(failed.note == "The update did not finish. The update could not be verified.")
        #expect(failed.problem)
        #expect(failed.label == "Update to v2.0.3", "pressed again, it tries again")
    }
}
