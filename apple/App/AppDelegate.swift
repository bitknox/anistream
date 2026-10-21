// What the app does at launch and at quit, around the store.

import AnistreamKit
import AnistreamPlayerUI
import AnistreamStore
import AnistreamUI
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

@MainActor
final class AppDelegate: NSObject {
    let store: AppStore
    let navigator = Navigator()
    /// The player the app hosts for the engine, fed the store's playback events.
    let player: PlayerModel
    private var started = false
    /// The Fixtures app's directory for this launch, removed at quit.
    private var temporary: URL?

    override init() {
        let store = AppStore()
        self.store = store
        player = PlayerModel(send: { [weak store] command in store?.send(command) })
        super.init()
        store.onPlayback = { [weak player = self.player] event in player?.apply(event) }
    }

    /// Start the engine, once, however many windows ask.
    func start() async {
        guard !started else { return }
        started = true
        #if FIXTURES
        let (launch, temporary) = EngineLaunch.fixtures(platform: Self.platform)
        self.temporary = temporary
        // The testing engine keeps tokens in files of its own; nothing reaches the keychain.
        let secrets: any SecretBackend = MemorySecrets()
        #else
        let launch = EngineLaunch.app(platform: Self.platform)
        let secrets: any SecretBackend = KeychainSecrets()
        #endif
        await store.start(launch, host: AppHost(secrets: secrets, open: Self.open))
        // What apple/scripts/shots.sh waits for before it takes a screenshot, flushed because
        // the Simulator writes the app's output to a file, which would hold it back.
        var ready = await store.settle()
        store.applyShowcase()
        #if FIXTURES
        // A screenshot of the player: play the Playing scene's episode to the state named.
        if let shot = PlayerShot.named(in: ProcessInfo.processInfo.arguments) {
            ready = await player.stage(shot, store: store)
        }
        #endif
        if ready {
            print("ANISTREAM_READY")
            fflush(stdout)
        }
        // A transition for the motion audit's recordings, two seconds after the screen is up.
        if let motion = MotionDemo.from() {
            try? await Task.sleep(for: .seconds(2))
            print("ANISTREAM_MOTION")
            fflush(stdout)
            motion.run(navigator: navigator, store: store)
        }
        #if FIXTURES
        // What the screens cost against AniList's rate limit, step by step.
        if RequestAudit.requested {
            await RequestAudit.run(navigator: navigator, store: store)
        }
        #endif
    }

    private func quit() {
        store.shutdown()
        if let temporary { try? FileManager.default.removeItem(at: temporary) }
    }

    /// This device, as the engine is told it.
    static var platform: FfiPlatform {
        #if os(macOS)
        .macOs
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? .ipadOs : .ios
        #endif
    }

    /// Open a link in the browser or the app for it. Called on the engine's threads.
    nonisolated static func open(_ url: URL) -> Bool {
        #if os(macOS)
        return NSWorkspace.shared.open(url)
        #else
        Task { @MainActor in UIApplication.shared.open(url) }
        return true
        #endif
    }
}

#if os(macOS)
extension AppDelegate: NSApplicationDelegate {
    func applicationWillTerminate(_ notification: Notification) {
        quit()
    }
}
#else
extension AppDelegate: UIApplicationDelegate {
    func applicationWillTerminate(_ application: UIApplication) {
        quit()
    }

    /// Landscape while the iPhone's player is up.
    func application(
        _ application: UIApplication, supportedInterfaceOrientationsFor window: UIWindow?
    ) -> UIInterfaceOrientationMask {
        PlayerOrientation.mask
    }
}
#endif
