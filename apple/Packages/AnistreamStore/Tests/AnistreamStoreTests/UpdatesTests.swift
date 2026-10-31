// The update check's word as the store keeps it, and the rule that nothing installs until the
// viewer presses Update.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

/// An installer that records what it was asked, and reports what the test says.
@MainActor
final class RecordingInstaller: UpdateInstaller {
    var installed: [String] = []
    var cancelled = 0
    var report: (@MainActor (UpdateProgress) -> Void)?

    func install(_ release: Release, progress: @escaping @MainActor (UpdateProgress) -> Void) {
        installed.append(release.version)
        report = progress
    }

    func cancel() {
        cancelled += 1
    }
}

func release(_ version: String, mac: Bool = true) -> Release {
    let file = { (name: String) in
        ReleaseAsset(name: name, url: "https://github.example/\(name)", sha256: String(repeating: "a", count: 64), size: 1000)
    }
    return Release(
        version: version, publishedAt: 1_792_664_639, pageUrl: "https://github.example/releases/tag/v\(version)",
        notes: "", macos: mac ? file("anistream-\(version)-macos.dmg") : nil,
        ios: file("anistream-\(version)-ios-unsigned.ipa"), android: nil)
}

@MainActor
struct UpdatesTests {
    @Test func aReleaseIsOfferedAndTheNoticeFollowsTheEnginesWord() {
        let store = AppStore()
        store.apply(.update(.available(release: release("2.0.3"), notify: false)))
        #expect(store.updates.latest?.version == "2.0.3")
        #expect(store.updates.notice == nil, "offered in Settings, not told")

        store.apply(.update(.available(release: release("2.0.4"), notify: true)))
        #expect(store.updates.notice?.version == "2.0.4")
        store.updates.dismissNotice()
        #expect(store.updates.notice == nil)
        #expect(store.updates.latest?.version == "2.0.4", "still on offer once the notice is gone")

        store.apply(.update(.upToDate(stableBehind: nil, runningBeta: false)))
        #expect(store.updates.latest == nil)
        #expect(store.updates.outcome == .upToDate)
    }

    @Test func checkNowAsksTheEngineOnceAndSaysWhatItFound() {
        let store = AppStore()
        var sent: [Command] = []
        store.updates.send = { sent.append($0) }
        store.updates.checkNow()
        store.updates.checkNow()
        #expect(sent == [.checkForUpdates], "not twice while one is under way")
        #expect(store.updates.checking)

        store.apply(.update(.checking))
        store.apply(.update(.failed(reason: "GitHub could not be reached")))
        #expect(!store.updates.checking)
        #expect(store.updates.outcome == .failed("GitHub could not be reached"))

        // A release found before stays on offer when a later check fails.
        store.apply(.update(.available(release: release("2.0.3"), notify: false)))
        store.updates.checkNow()
        store.apply(.update(.failed(reason: "GitHub answered 403")))
        #expect(store.updates.latest?.version == "2.0.3")
    }

    @Test func nothingInstallsUntilTheViewerPressesUpdate() {
        let store = AppStore()
        let installer = RecordingInstaller()
        store.updates.installer = installer
        // However the engine tells of it, and however often, nothing installs on its own.
        for notify in [true, false, true] {
            store.apply(.update(.available(release: release("2.0.3"), notify: notify)))
        }
        store.updates.checkNow()
        store.apply(.update(.available(release: release("2.0.3"), notify: false)))
        #expect(installer.installed.isEmpty)
        #expect(store.updates.installsHere)

        // Pressed: one install, of the release offered, and the notice goes.
        store.updates.update()
        #expect(installer.installed == ["2.0.3"])
        #expect(store.updates.notice == nil)
        #expect(store.updates.progress == .preparing)
        // Pressed again while it runs, and the engine telling of it again: still one.
        store.updates.update()
        store.apply(.update(.available(release: release("2.0.3"), notify: true)))
        #expect(installer.installed == ["2.0.3"])
        #expect(store.updates.notice == nil, "no notice over an update under way")
    }

    @Test func theInstallersProgressIsWhatSettingsShows() throws {
        let store = AppStore()
        let installer = RecordingInstaller()
        store.updates.installer = installer
        store.apply(.update(.available(release: release("2.0.3"), notify: false)))
        store.updates.update()
        let report = try #require(installer.report)
        report(.downloading(received: 250, expected: 1000))
        #expect(store.updates.progress.fraction == 0.25)
        #expect(store.updates.progress.isActive)

        store.updates.cancel()
        #expect(installer.cancelled == 1)
        #expect(store.updates.progress == .idle)

        // A failure ends it, and Update can be pressed again.
        store.updates.update()
        installer.report?(.failed("The update could not be verified."))
        #expect(!store.updates.progress.isActive)
        store.updates.update()
        #expect(installer.installed == ["2.0.3", "2.0.3", "2.0.3"])

        // Once it is installing, it cannot be stopped from here.
        installer.report?(.relaunching)
        store.updates.cancel()
        #expect(store.updates.progress == .relaunching)
        #expect(installer.cancelled == 1)
    }

    @Test func withoutAnInstallerOrTheMacsFileNothingInstalls() {
        let store = AppStore()
        store.apply(.update(.available(release: release("2.0.3"), notify: true)))
        // iPhone and iPad: no installer here; the viewer updates in SideStore.
        #expect(!store.updates.installsHere)
        store.updates.update()
        #expect(store.updates.progress == .idle)

        // A release without the Mac's file, as when its checksum did not read.
        let installer = RecordingInstaller()
        store.updates.installer = installer
        store.apply(.update(.available(release: release("2.0.4", mac: false), notify: false)))
        #expect(!store.updates.installsHere)
        store.updates.update()
        #expect(installer.installed.isEmpty)
    }

    @Test func theGateInstallsOnlyWhatTheViewerAskedFor() {
        var gate = UpdateGate()
        // Found by the updater on its own: never.
        #expect(gate.choice(found: "2.0.3", informationOnly: false) == .dismiss)

        gate.arm("2.0.3")
        #expect(gate.choice(found: "2.0.3", informationOnly: false) == .install)
        // A newer release published since is still what the viewer wants.
        #expect(gate.choice(found: "2.0.10", informationOnly: false) == .install)
        #expect(gate.choice(found: "2.1.0", informationOnly: false) == .install)
        // An older one, or one with nothing to install, is not.
        #expect(gate.choice(found: "2.0.2", informationOnly: false) == .dismiss)
        #expect(gate.choice(found: "2.0.3", informationOnly: true) == .dismiss)

        gate.disarm()
        #expect(gate.choice(found: "2.0.3", informationOnly: false) == .dismiss)
    }

    @Test func iPhoneAndIPadUpdateInSideStoreThenAltStoreThenThePage() throws {
        let links = UpdatesModel.storeLinks(for: release("2.0.3"))
        #expect(links.map(\.scheme) == ["sidestore", "altstore", "https"])
        let sidestore = try #require(URLComponents(url: links[0], resolvingAgainstBaseURL: false))
        #expect(sidestore.host == "source")
        #expect(sidestore.queryItems?.first { $0.name == "url" }?.value == UpdatesModel.sourceURL.absoluteString)
        #expect(UpdatesModel.sourceURL.absoluteString
            == "https://github.com/bitknox/anistream/releases/latest/download/sidestore.json")
        #expect(links[2].absoluteString == "https://github.example/releases/tag/v2.0.3")
    }
}
