// What the app does at launch and at quit, around the store.

import AnistreamKit
import AnistreamPlayerUI
import AnistreamStore
import AnistreamUI
import Foundation
import Observation
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
    /// Where the app is and the network the device uses, told to the engine.
    let phases: SystemPhases
    private var started = false
    /// Spotlight's titles, kept the shelf's once the engine is up.
    private var spotlight: SpotlightIndexer?
    /// The Fixtures app's directory for this launch, removed at quit.
    private var temporary: URL?

    override init() {
        let store = AppStore()
        self.store = store
        let player = PlayerModel(send: { [weak store] command in store?.send(command) })
        self.player = player
        phases = SystemPhases(
            send: { [weak store] command in store?.send(command) },
            playing: { [weak player] in player.map { $0.isPresented && !$0.paused } ?? false })
        super.init()
        store.onPlayback = { [weak player = self.player] event in player?.apply(event) }
        watchPlaying()
    }

    /// In the background, what waits follows whether the player plays: watched here rather than
    /// by a view, which the system does not update while the app is away.
    private func watchPlaying() {
        withObservationTracking {
            _ = player.isPresented
            _ = player.paused
        } onChange: { [weak self] in
            Task { @MainActor in
                self?.phases.playingChanged()
                self?.watchPlaying()
            }
        }
    }

    /// Start the engine, once, however many windows ask.
    func start() async {
        guard !started else { return }
        started = true
        // Said from the start, queued for the engine until it is up.
        phases.watchNetwork()
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
        if store.phase == .running {
            // The Fixtures app's titles go under its own domain, and its shelf in its own home.
            let spotlight = SpotlightIndexer(store: store, file: Shelf.file(in: launch.home))
            spotlight.start()
            self.spotlight = spotlight
        }
        #if FIXTURES
        store.applyWatchedArgument()
        #endif
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
