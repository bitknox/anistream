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
    /// New-episode notifications: the centre, their actions, the background refresh.
    private(set) var notices: AppNotifications!
    /// The Fixtures app's directory for this launch, removed at quit.
    private var temporary: URL?
    /// The episode playing, handed off to your other devices; it pauses here when one takes it.
    private let playing: PlayingActivity
    #if os(macOS)
    /// Now Playing and the media keys for a TV this Mac drives, while its own player is down.
    private let tvNowPlaying: TVNowPlaying
    #endif

    override init() {
        let store = AppStore()
        self.store = store
        let player = PlayerModel(send: { [weak store] command in store?.send(command) })
        self.player = player
        phases = SystemPhases(
            send: { [weak store] command in store?.send(command) },
            playing: { [weak player] in player.map { $0.isPresented && !$0.paused } ?? false },
            downloading: { [weak store] in store?.continued.stage == .running })
        playing = player.playingActivity { [weak store] id in store?.titles[id]?.showTitle }
        #if os(macOS)
        tvNowPlaying = TVNowPlaying(tv: store.tv, player: player)
        #endif
        super.init()
        store.onPlayback = { [weak player = self.player] event in player?.apply(event) }
        // Play on TV from the player: the TV picker over it, and the player closed once the TV
        // plays the episode.
        player.playOnTVHandler = { [weak store] request in store?.tv.offer(request) }
        // What a watch party says goes over the video while the player is up.
        store.party.playerUp = { [weak player] in player?.isPresented ?? false }
        store.playerSays = { [weak player] text, kind in player?.say(text, kind: kind) }
        #if os(iOS)
        // The playback's Live Activity, told what plays as Now Playing is, and its Play and Pause.
        player.onLiveActivity = { [weak store] playing in store?.playbackChanged(playing) }
        store.playbackActivity.takeControls(
            play: { [weak player = self.player] in if player?.paused == true { player?.togglePause() } },
            pause: { [weak player = self.player] in player?.pause() })
        #endif
        store.tv.handedOff = { [weak player = self.player, weak store] in
            player?.viewerClosed()
            #if os(iOS)
            // The card sits on the player's cover, so it leaves with it, in one animation, rather
            // than going down with the cover and coming up again over the window. The
            // now-playing bar leads back to it.
            store?.tv.sheetClosed()
            #endif
        }
        // The App Intents reach the app here, from its first moment: an intent can be what
        // launched it, before any window has started the engine.
        let intents = IntentsBridge.shared
        intents.store = store
        intents.navigator = navigator
        intents.start = { [weak self] in Task { await self?.start() } }
        #if !FIXTURES
        // The Fixtures app's home is made at its start, a new one every launch.
        intents.shelfFile = Shelf.file(in: EngineHome.resolve())
        #endif
        notices = AppNotifications(
            store: store,
            navigator: navigator,
            storeStarted: { [weak self] in self?.started ?? false },
            backgroundLaunch: { Self.backgroundLaunch() }
        )
        // Before launch ends, as the system asks of a notification's delegate and the background
        // task's handler.
        notices.launch()
        #if os(iOS)
        // Downloads go on after the app leaves, in the system's window for them. What the window
        // means for the engine, SystemPhases says, with the scene and the player: one teller of
        // where the app is, so a window opening or closing away is said there.
        store.continued.playing = { [weak player = self.player] in
            guard let player else { return false }
            return player.isPresented && !player.paused
        }
        store.continued.send = { [weak phases = self.phases] _ in phases?.downloadsWindowChanged() }
        BackgroundDownloads.install(in: store)
        #endif
        #if os(macOS)
        // The Mac updates in place, when the viewer presses Update (MacUpdater). The Fixtures
        // app goes through the steps and installs nothing.
        #if FIXTURES
        store.updates.installer = RehearsedInstaller()
        #else
        store.updates.installer = MacUpdater()
        #endif
        #endif
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

    /// The engine a background refresh or a notification's Mark watched starts when the app has
    /// none running: the app's own, on its own directories and keychain. None for the Fixtures
    /// app, whose engine lives only as long as its window.
    static func backgroundLaunch() -> (EngineLaunch, AppHost)? {
        #if FIXTURES
        return nil
        #else
        return (EngineLaunch.app(platform: platform), AppHost(secrets: KeychainSecrets(), open: open))
        #endif
    }

    /// Start the engine, once, however many windows ask.
    func start() async {
        guard !started else { return }
        started = true
        // Said from the start, queued for the engine until it is up.
        phases.watchNetwork()
        // An engine a background refresh started stops first: one engine on the files at a time.
        await notices.yieldBackgroundEngine()
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
            // The titles "Open … in anistream" is offered for follow the shelf.
            spotlight.onUpdate = { AnistreamShortcuts.updateAppShortcutParameters() }
            spotlight.start()
            self.spotlight = spotlight
            IntentsBridge.shared.shelfFile = Shelf.file(in: launch.home)
        }
        #if FIXTURES
        store.applyWatchedArgument()
        #endif
        // What apple/scripts/shots.sh waits for before it takes a screenshot, flushed because
        // the Simulator writes the app's output to a file, which would hold it back.
        var ready = await store.settle()
        store.applyShowcase()
        #if FIXTURES
        // A launch sheet whose row the showcase puts up (sheet=proxy) opens only once it is
        // there: wait for the sheet to be taken, and for it to slide in.
        if navigator.launchSheet != nil {
            let deadline = ContinuousClock.now + .seconds(3)
            while navigator.launchSheet != nil, ContinuousClock.now < deadline {
                try? await Task.sleep(for: .milliseconds(50))
            }
            try? await Task.sleep(for: .milliseconds(600))
        }
        #endif
        // The week's notifications, scheduled, when there are any to schedule.
        await store.notifications.refreshPermission()
        if store.wantsWeek { store.refreshNotices() }
        #if FIXTURES
        // A screenshot of the player: play the Playing scene's episode to the state named.
        if let shot = PlayerShot.named(in: ProcessInfo.processInfo.arguments) {
            ready = await player.stage(shot, store: store)
        }
        // A screenshot of Play on TV: the scene the arguments name, on screen.
        if let scene = TVScene.named() {
            ready = await store.tv.reached(scene) && ready
        }
        // A screenshot of a watch party: the fake server's room, in the state named.
        if let scene = PartyScene.named() {
            ready = await player.stage(scene, store: store) && ready
        }
        #endif
        if ready {
            print("ANISTREAM_READY")
            fflush(stdout)
        }
        #if FIXTURES
        // apple/scripts/pip-check.sh: back out of Picture in Picture once in it, however it got
        // there (the shot's toggle, or going home while it plays).
        let arguments = ProcessInfo.processInfo.arguments
        if let index = arguments.firstIndex(of: "-AnistreamPictureInPictureReturn"),
            arguments.indices.contains(index + 1), let seconds = Int(arguments[index + 1])
        {
            let returned = await player.returnFromPictureInPicture(within: seconds)
            print("ANISTREAM_PIP_RETURNED \(returned)")
            fflush(stdout)
        }
        #endif
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
    /// Quitting in a watch party leaves it: asked first, with Stay the default.
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        store.party.asksBeforeQuitting() ? .terminateCancel : .terminateNow
    }

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
