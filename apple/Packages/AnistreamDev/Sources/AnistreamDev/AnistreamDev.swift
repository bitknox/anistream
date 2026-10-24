// The Mac dev harness: the app's own window, on the testing engine, from `swift run`.

import AnistreamDesign
import AnistreamPlayer
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
        // Only an app in the Dock goes full screen, so `--full-screen` runs as one.
        let regular = options.snapshot == nil || options.fullScreenCycles > 0
        NSApp.setActivationPolicy(regular ? .regular : .accessory)
        if regular { NSApp.activate() }
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
        // The Mac's updater, rehearsed: it goes through a download and installs nothing.
        store.updates.installer = RehearsedInstaller()
        await store.start(launch, host: host)
        if let trouble = options.artTrouble {
            store.troubleArt(failFirst: trouble.failFirst, delay: trouble.delay)
        }
        store.applyWatchedArgument()

        // The saved frame can still win over the default size, so a snapshot sets its own.
        if options.snapshot != nil, let window = Snapshot.window {
            window.setContentSize(NSSize(width: options.width, height: options.height))
        }
        _ = player
        let settled = await store.settle()
        store.applyShowcase()
        // `-AnistreamUpdating YES`: Update pressed on the showcase's release, for a shot of the
        // download under way.
        if ProcessInfo.processInfo.arguments.contains("-AnistreamUpdating") {
            store.updates.update()
            try? await Task.sleep(for: .milliseconds(1800))
        }
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
        } else if options.artTrouble != nil, !options.frames.isEmpty, let window = Snapshot.window {
            // The window left alone while the troubled art comes in.
            await frames(of: nil, in: window, to: url)
        }
        if options.fullScreenCycles > 0, let window = Snapshot.window {
            await fullScreenCycles(window: window, url: url)
        }
        do {
            guard let window = Snapshot.window else { throw Snapshot.Problem.noWindow }
            try Snapshot.write(window, to: url)
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
    private func frames(of motion: MotionDemo?, in window: NSWindow, to url: URL) async {
        var taken: [(offset: Int, at: Duration, image: CGImage)] = []
        let start = ContinuousClock.now
        motion?.run(navigator: navigator, store: store)
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

    /// In and out of full screen `--full-screen` times, playing for the first half and paused for
    /// the rest, with the window shot at each end. Each step says where the picture and the
    /// window's content view are against the window, which they must fill.
    private func fullScreenCycles(window: NSWindow, url: URL) async {
        func waited(_ condition: @MainActor () -> Bool) async -> Bool {
            let deadline = ContinuousClock.now + .seconds(10)
            while !condition() {
                guard ContinuousClock.now < deadline else { return false }
                try? await Task.sleep(for: .milliseconds(50))
            }
            return true
        }
        let cycles = options.fullScreenCycles
        var misplaced = 0
        // The ways in and out a viewer mixes: the player's own toggle (its button, F), the
        // window's green button, which AppKit starts and the player hears of afterwards, a double
        // click on the video and one in the title bar strip, and the window resized by hand on
        // the way out.
        enum Way: String, CaseIterable { case player, window, doubleClick, doubleClickTop, resizedAfter }
        // ANISTREAM_FULL_SCREEN_WAYS=doubleClick,window picks some, in that order.
        let ways = ProcessInfo.processInfo.environment["ANISTREAM_FULL_SCREEN_WAYS"].map {
            $0.split(separator: ",").compactMap { Way(rawValue: String($0)) }
        } ?? Way.allCases
        func check(_ name: String, _ there: Bool) {
            let file = url.deletingPathExtension().appendingPathExtension("\(name).png")
            try? Snapshot.write(window, to: file)
            let fits = Self.pictureFillsContent(of: window)
            if !fits.fills { misplaced += 1 }
            print(
                "AnistreamDev: full screen \(name)\(there ? "" : " (did not get there)")\(player.paused ? ", paused" : ""): "
                    + "picture \(fits.picture), content \(fits.content)\(fits.fills ? "" : ", MISPLACED")")
        }
        for cycle in 1...cycles {
            if cycle > cycles / 2, cycles > 1, !player.paused { player.togglePause() }
            for on in [true, false] {
                let way = ways[((cycle - 1) * 2 + (on ? 0 : 1)) % ways.count]
                switch way {
                case .player, .resizedAfter: player.fullScreen = on
                case .window: window.toggleFullScreen(nil)
                case .doubleClick, .doubleClickTop:
                    // A double click on the video, as AppKit hands it to the window: posted to
                    // the app's own queue, so the pointer never moves.
                    let size = window.contentView?.bounds.size ?? .zero
                    let point = NSPoint(x: size.width / 2, y: way == .doubleClickTop ? size.height - 20 : size.height / 2)
                    for count in 1...2 {
                        for type in [NSEvent.EventType.leftMouseDown, .leftMouseUp] {
                            if let event = NSEvent.mouseEvent(
                                with: type, location: point, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: count, pressure: 1)
                            {
                                NSApp.postEvent(event, atStart: false)
                            }
                        }
                    }
                }
                // The pointer moving while the window changes: the controls, and the window's
                // own buttons with them, fade in and out again during the transition.
                player.poke()
                try? await Task.sleep(for: .milliseconds(150))
                player.hideControls()
                let there = await waited { window.styleMask.contains(.fullScreen) == on }
                if way == .resizedAfter, !on {
                    // Dragged a little larger as soon as the window is back.
                    var frame = window.frame
                    frame.size.width += 60
                    frame.size.height += 40
                    window.setFrame(frame, display: true)
                }
                // The transition's own animation, and the layout after it.
                try? await Task.sleep(for: .milliseconds(1200))
                check("\(on ? "in" : "out")-\(cycle)-\(way.rawValue)", there)
            }
        }
        print("AnistreamDev: full screen round trips: \(cycles), misplaced \(misplaced)")
    }

    /// The picture's frame in the window and the window's content size: the player fills the
    /// content, in full screen and out of it.
    private static func pictureFillsContent(of window: NSWindow) -> (fills: Bool, picture: String, content: String) {
        func host(in view: NSView) -> NSView? {
            if view is PlayerHostView { return view }
            for sub in view.subviews { if let found = host(in: sub) { return found } }
            return nil
        }
        guard let content = window.contentView else { return (false, "none", "none") }
        // Where the content should be: all of the window less its frame, in the window's
        // coordinates, which the content view's own bounds cannot say when it is the one off.
        let expected = CGRect(origin: .zero, size: window.contentRect(forFrameRect: window.frame).size)
        let describe = { (r: CGRect) in "\(Int(r.width))×\(Int(r.height)) at (\(Int(r.minX)), \(Int(r.minY)))" }
        func same(_ a: CGRect, _ b: CGRect) -> Bool {
            abs(a.minX - b.minX) < 1 && abs(a.minY - b.minY) < 1 && abs(a.width - b.width) < 1 && abs(a.height - b.height) < 1
        }
        let contentFrame = content.convert(content.bounds, to: nil)
        guard let picture = host(in: content) else { return (false, "none", describe(contentFrame)) }
        let frame = picture.convert(picture.bounds, to: nil)
        return (same(frame, expected) && same(contentFrame, expected), describe(frame), "\(describe(contentFrame)) of \(describe(expected))")
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
