// The Mac dev harness: the app's own window, on the testing engine, from `swift run`.

import AnistreamDesign
import AnistreamPlayerUI
import AnistreamStore
import AnistreamUI
import AppKit
import SwiftUI

/// The options, read once. A bad command line ends the process before any window.
@MainActor
let options: Options = {
    do {
        return try Options(arguments: CommandLine.arguments)
    } catch {
        FileHandle.standardError.write(Data("AnistreamDev: \(error)\n\(Options.usage)\n".utf8))
        exit(2)
    }
}()

@main
struct AnistreamDev: App {
    @NSApplicationDelegateAdaptor(Delegate.self) private var delegate

    var body: some Scene {
        Window("anistream", id: "main") {
            RootView()
                .hostsPlayer(delegate.player)
                .environment(\.playerUp, delegate.player.isPresented)
                .environment(delegate.store)
                .environment(delegate.navigator)
                .frame(minWidth: 960, minHeight: 600)
                .focusedSceneValue(\.playerInFront, delegate.player.inMainWindow)
                .task { await delegate.run() }
        }
        .defaultSize(width: options.width, height: options.height)
        // A frame AppKit saved from an earlier run would win over --size otherwise.
        .restorationBehavior(.disabled)
        .windowResizability(.contentMinSize)
        .commands {
            AnistreamCommands(
                store: delegate.store, navigator: delegate.navigator,
                playerBack: .init(
                    isUp: { [player = delegate.player] in player.inMainWindow },
                    back: { [player = delegate.player] in player.requestBack() })
            ) {
                PlaybackMenu(model: delegate.player)
            } playerWindowItems: {
                MiniPlayerMenuItem(model: delegate.player)
                PictureInPictureMenuItem(model: delegate.player)
            }
        }
    }
}

@MainActor
final class Delegate: NSObject, NSApplicationDelegate {
    let store = AppStore()
    let navigator = Navigator()
    lazy var player: PlayerModel = {
        let player = PlayerModel(send: { [weak store] command in store?.send(command) })
        store.onPlayback = { [weak player] event in player?.apply(event) }
        return player
    }()
    private var temporary: URL?

    func applicationDidFinishLaunching(_ notification: Notification) {
        // A process started from a terminal is not an app until it says so: no Dock icon, no
        // key window. A snapshot stays out of the Dock; macOS does not let either take focus from
        // what you are doing, so a snapshot shows the window as it looks when it is not key.
        NSApp.setActivationPolicy(options.snapshot == nil ? .regular : .accessory)
        if options.snapshot == nil { NSApp.activate() }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    func applicationWillTerminate(_ notification: Notification) {
        store.shutdown()
        if let temporary { try? FileManager.default.removeItem(at: temporary) }
    }

    /// Start the engine, and take the snapshot when one was asked for.
    func run() async {
        let (launch, temporary) = options.launch()
        self.temporary = temporary
        print("AnistreamDev: \(options.engine) in \(launch.home.data.deletingLastPathComponent().path)")
        let host = AppHost(
            // The real engine on the terminal's directories shares its keychain items; anything
            // else keeps tokens in memory, so the harness never writes to the keychain.
            secrets: options.shared ? KeychainSecrets() : MemorySecrets(),
            open: { NSWorkspace.shared.open($0) }
        )
        await store.start(launch, host: host)
        store.applyWatchedArgument()

        // The saved frame can still win over the default size, so a snapshot sets its own.
        if options.snapshot != nil, let window = Snapshot.window {
            window.setContentSize(NSSize(width: options.width, height: options.height))
        }
        _ = player
        let settled = await store.settle()
        store.applyShowcase()
        print(settled ? "ANISTREAM_READY" : "AnistreamDev: did not settle; going on")
        if let shot = options.playerShot {
            let there = await player.stage(shot, store: store)
            print(there ? "AnistreamDev: the player is at \(shot.rawValue)" : "AnistreamDev: the player did not reach \(shot.rawValue)")
        }
        guard let url = options.snapshot else { return }
        // The art's and the preview's fade-ins.
        try? await Task.sleep(for: .milliseconds(400))
        if let motion = MotionDemo.from(), !options.frames.isEmpty, let window = Snapshot.window {
            await frames(of: motion, in: window, to: url)
        }
        do {
            guard let window = Snapshot.window else { throw Snapshot.Problem.noWindow }
            if options.playerShot == .miniPlayer, let panel = Snapshot.miniPlayer {
                // The mini player over the main window, browsing, each where it is on screen.
                try Snapshot.write([window, panel], to: url)
            } else {
                try Snapshot.write(window, to: url)
            }
            if options.playerShot == .pictureInPicture {
                // The system's floating window is another process's: what it shows is the frame
                // the feed gave it, written beside the main window's shot.
                guard let frame = player.pictureInPictureFrame?() else { throw Snapshot.Problem.noWindow }
                let file = url.deletingPathExtension().appendingPathExtension("frame.png")
                let bitmap = NSBitmapImageRep(cgImage: frame)
                try bitmap.representation(using: .png, properties: [:])?.write(to: file)
                print("AnistreamDev: wrote \(file.path) (\(frame.width)×\(frame.height))")
                await pictureInPictureRoundTrips(window: window, url: url)
            }
            print("AnistreamDev: wrote \(url.path)")
        } catch {
            FileHandle.standardError.write(Data("AnistreamDev: \(error)\n".utf8))
        }
        if options.playerShot != nil {
            player.viewerClosed()
            try? await Task.sleep(for: .milliseconds(300))
        }
        exitAfterSnapshot()
    }

    /// Run `motion` and snapshot the window at each of the frames' offsets after it starts: taken
    /// into memory as it runs, and written once it has, so writing never delays the next frame.
    private func frames(of motion: MotionDemo, in window: NSWindow, to url: URL) async {
        var taken: [(offset: Int, at: Duration, image: CGImage)] = []
        let start = ContinuousClock.now
        motion.run(navigator: navigator, store: store)
        for offset in options.frames.sorted() {
            let at = start + .milliseconds(offset)
            if at > .now { try? await Task.sleep(until: at) }
            if let image = Snapshot.composited(window).flatMap({ Snapshot.withSheets($0, of: window) }) {
                taken.append((offset, ContinuousClock.now - start, image))
            }
        }
        for frame in taken {
            let name = url.deletingPathExtension().lastPathComponent + "-\(frame.offset)"
            let file = url.deletingLastPathComponent().appending(path: name).appendingPathExtension("png")
            let bitmap = NSBitmapImageRep(cgImage: frame.image)
            if let png = bitmap.representation(using: .png, properties: [:]) {
                try? png.write(to: file)
                let ms = frame.at.components.seconds * 1000 + frame.at.components.attoseconds / 1_000_000_000_000_000
                print("AnistreamDev: wrote \(file.path) at \(ms) ms")
            }
        }
    }

    /// Picture in Picture's ways back and in, for real on AVKit: the toggle back into the window,
    /// then the window minimised and the app hidden while it plays, each restored. Each step says
    /// whether it got there; the window is shot once the picture is back in it.
    private func pictureInPictureRoundTrips(window: NSWindow, url: URL) async {
        func waited(_ what: String, _ condition: @MainActor () -> Bool) async -> Bool {
            let deadline = ContinuousClock.now + .seconds(10)
            while !condition() {
                guard ContinuousClock.now < deadline else {
                    print("AnistreamDev: picture in picture did not \(what)")
                    return false
                }
                try? await Task.sleep(for: .milliseconds(50))
            }
            print("AnistreamDev: picture in picture did \(what)")
            return true
        }
        let before = player.position
        player.togglePictureInPicture()
        if await waited("return to the window", { self.player.pictureInPicturePhase == .inline && self.player.inMainWindow }) {
            try? await Task.sleep(for: .seconds(1))
            let back = url.deletingPathExtension().appendingPathExtension("restored.png")
            try? Snapshot.write(window, to: back)
            print("AnistreamDev: wrote \(back.path); played on from \(Int(before)) s to \(Int(player.position)) s")
        }
        window.miniaturize(nil)
        _ = await waited("start as the window was minimised", { self.player.pictureInPicturePhase == .active })
        player.togglePictureInPicture()
        _ = await waited("bring the window back from the Dock", {
            self.player.inMainWindow && !window.isMiniaturized && self.player.pictureInPicturePhase == .inline
        })
        // Only an active app in the Dock is hidden, so the snapshot's accessory app is one for
        // this step: its icon shows, and it takes the focus a moment.
        let policy = NSApp.activationPolicy()
        NSApp.setActivationPolicy(.regular)
        NSApp.activate()
        _ = await waited("become active to be hidden", { NSApp.isActive })
        NSApp.hide(nil)
        if await waited("start as the app was hidden", { self.player.pictureInPicturePhase == .active }) {
            player.togglePictureInPicture()
            _ = await waited("bring the app back from hiding", {
                self.player.inMainWindow && !NSApp.isHidden && self.player.pictureInPicturePhase == .inline
            })
        }
        NSApp.setActivationPolicy(policy)
        // Paused: in, then 10 s back as the floating window's button does it, each frame written.
        if !player.paused { player.togglePause() }
        if await waited("pause", { self.player.paused }) {
            player.togglePictureInPicture()
            if await waited("start while paused", { self.player.pictureInPicturePhase == .active }) {
                try? await Task.sleep(for: .milliseconds(500))
                writeFrame(url, "paused")
                player.seek(by: -10)
                try? await Task.sleep(for: .seconds(1.5))
                writeFrame(url, "paused-skipped")
                player.togglePictureInPicture()
                _ = await waited("return to the window from paused", {
                    self.player.inMainWindow && self.player.pictureInPicturePhase == .inline
                })
            }
        }
        print("AnistreamDev: one playback throughout: \(player.isPresented), at \(Int(player.position)) s")
    }

    /// The frame the floating window was last given, beside the main window's shot.
    private func writeFrame(_ url: URL, _ name: String) {
        guard let frame = player.pictureInPictureFrame?() else { return }
        let file = url.deletingPathExtension().appendingPathExtension("\(name).frame.png")
        guard let png = NSBitmapImageRep(cgImage: frame).representation(using: .png, properties: [:]) else { return }
        try? png.write(to: file)
        print("AnistreamDev: wrote \(file.path) at \(Int(player.position)) s")
    }

    private func exitAfterSnapshot() {
        NSApp.terminate(nil)
        Task { @MainActor in await self.end() }
    }

    private func end() async {
        // A sheet with a field in it can hold the quit back; a snapshot run ends regardless.
        try? await Task.sleep(for: .seconds(2))
        store.shutdown()
        if let temporary { try? FileManager.default.removeItem(at: temporary) }
        exit(0)
    }
}
