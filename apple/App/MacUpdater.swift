// The Mac's updater: Sparkle 2, driven by the app's own Settings row and notice instead of its
// windows. The app is signed ad hoc, with no Developer ID, so what Sparkle trusts is the EdDSA
// signature each release's appcast gives its .dmg, checked against SUPublicEDKey in Info.plist
// before the image is opened (SUVerifyUpdateBeforeExtraction). The feed itself is signed too
// (SURequireSignedFeed).
//
// Sparkle never looks on its own (SUEnableAutomaticChecks is off): the engine's daily check
// says when there is a release, and Sparkle reads its feed only when the viewer presses Update.
// Even then it installs only what the viewer asked for (UpdateGate). Its installer swaps the
// bundle in place, takes the new one out of quarantine so macOS does not ask for Open Anyway
// again, and relaunches the app.

#if os(macOS) && !FIXTURES
import AnistreamKit
import AnistreamStore
import Foundation
import Sparkle

@MainActor
final class MacUpdater: NSObject, UpdateInstaller, SPUUserDriver {
    private var updater: SPUUpdater?
    private var report: (@MainActor (UpdateProgress) -> Void)?
    private var gate = UpdateGate()
    private var expected: UInt64?
    private var received: UInt64 = 0
    private var cancelDownload: (() -> Void)?

    override init() {
        super.init()
        let updater = SPUUpdater(hostBundle: .main, applicationBundle: .main, userDriver: self, delegate: nil)
        updater.automaticallyChecksForUpdates = false
        updater.automaticallyDownloadsUpdates = false
        do {
            try updater.start()
            self.updater = updater
        } catch {
            AppLog.error("updates", "Sparkle did not start: \(error.localizedDescription)")
        }
    }

    // MARK: UpdateInstaller

    func install(_ release: Release, progress: @escaping @MainActor (UpdateProgress) -> Void) {
        report = progress
        guard let updater, updater.canCheckForUpdates else {
            progress(.failed("The updater is not ready. Try again in a moment."))
            return
        }
        gate.arm(release.version)
        AppLog.info("updates", "updating to \(release.version)")
        updater.checkForUpdates()
    }

    func cancel() {
        cancelDownload?()
        cancelDownload = nil
        gate.disarm()
    }

    private func say(_ progress: UpdateProgress) {
        report?(progress)
    }

    private func failed(_ error: any Error) {
        AppLog.error("updates", "the update stopped: \(error.localizedDescription)")
        gate.disarm()
        say(.failed(error.localizedDescription))
    }

    // MARK: SPUUserDriver

    func show(_ request: SPUUpdatePermissionRequest, reply: @escaping (SUUpdatePermissionResponse) -> Void) {
        // Sparkle's own scheduled checks stay off: the engine's check is the one that runs.
        reply(SUUpdatePermissionResponse(automaticUpdateChecks: false, sendSystemProfile: false))
    }

    func showUserInitiatedUpdateCheck(cancellation: @escaping () -> Void) {
        say(.preparing)
    }

    func showUpdateFound(with appcastItem: SUAppcastItem, state: SPUUserUpdateState, reply: @escaping (SPUUserUpdateChoice) -> Void) {
        let choice = gate.choice(found: appcastItem.displayVersionString, informationOnly: appcastItem.isInformationOnlyUpdate)
        AppLog.info("updates", "the feed has \(appcastItem.displayVersionString): \(choice)")
        switch choice {
        case .install:
            reply(.install)
        case .dismiss:
            reply(.dismiss)
            if gate.armed != nil {
                gate.disarm()
                say(.failed("The update feed does not have this release yet."))
            }
        }
    }

    func showUpdateReleaseNotes(with downloadData: SPUDownloadData) {}

    func showUpdateReleaseNotesFailedToDownloadWithError(_ error: any Error) {}

    func showUpdateNotFoundWithError(_ error: any Error, acknowledgement: @escaping () -> Void) {
        acknowledgement()
        gate.disarm()
        say(.failed("The update feed does not have this release yet."))
    }

    func showUpdaterError(_ error: any Error, acknowledgement: @escaping () -> Void) {
        acknowledgement()
        failed(error)
    }

    func showDownloadInitiated(cancellation: @escaping () -> Void) {
        cancelDownload = cancellation
        expected = nil
        received = 0
        say(.downloading(received: 0, expected: nil))
    }

    func showDownloadDidReceiveExpectedContentLength(_ expectedContentLength: UInt64) {
        expected = expectedContentLength > 0 ? expectedContentLength : nil
        say(.downloading(received: received, expected: expected))
    }

    func showDownloadDidReceiveData(ofLength length: UInt64) {
        received += length
        say(.downloading(received: received, expected: expected))
    }

    func showDownloadDidStartExtractingUpdate() {
        cancelDownload = nil
        say(.unpacking(0))
    }

    func showExtractionReceivedProgress(_ progress: Double) {
        say(.unpacking(progress))
    }

    func showReady(toInstallAndRelaunch reply: @escaping (SPUUserUpdateChoice) -> Void) {
        // Only the update the viewer pressed Update for gets this far, and they asked for it now.
        guard gate.armed != nil else {
            reply(.dismiss)
            return
        }
        say(.relaunching)
        reply(.install)
    }

    func showInstallingUpdate(withApplicationTerminated applicationTerminated: Bool, retryTerminatingApplication: @escaping () -> Void) {
        say(.relaunching)
    }

    func showUpdateInstalledAndRelaunched(_ relaunched: Bool, acknowledgement: @escaping () -> Void) {
        acknowledgement()
    }

    func showUpdateInFocus() {}

    func dismissUpdateInstallation() {
        cancelDownload = nil
        let wasArmed = gate.armed != nil
        gate.disarm()
        // The session ended without installing, as after Cancel: back to the offer.
        if wasArmed { say(.idle) }
    }
}
#endif
