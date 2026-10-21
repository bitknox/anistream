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
                .environment(delegate.store)
                .environment(delegate.navigator)
                .frame(minWidth: 960, minHeight: 600)
                .task { await delegate.run() }
        }
        .defaultSize(width: options.width, height: options.height)
        // A frame AppKit saved from an earlier run would win over --size otherwise.
        .restorationBehavior(.disabled)
        .windowResizability(.contentMinSize)
        .commands { AnistreamCommands(store: delegate.store, navigator: delegate.navigator) }

        Window("Player", id: PlayerWindow.id) {
            PlayerWindow(model: delegate.player)
                .environment(delegate.store)
        }
        .windowStyle(.hiddenTitleBar)
        .defaultSize(width: 1280, height: 720)
        .windowResizability(.contentMinSize)
        .restorationBehavior(.disabled)
        .defaultLaunchBehavior(.suppressed)
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
        do {
            let window = options.playerShot == nil ? Snapshot.window : Snapshot.playerWindow
            guard let window else { throw Snapshot.Problem.noWindow }
            try Snapshot.write(window, to: url)
            print("AnistreamDev: wrote \(url.path)")
        } catch {
            FileHandle.standardError.write(Data("AnistreamDev: \(error)\n".utf8))
        }
        if options.playerShot != nil {
            player.viewerClosed()
            try? await Task.sleep(for: .milliseconds(300))
        }
        NSApp.terminate(nil)
        // A sheet with a field in it can hold the quit back; a snapshot run ends regardless.
        try? await Task.sleep(for: .seconds(2))
        store.shutdown()
        if let temporary { try? FileManager.default.removeItem(at: temporary) }
        exit(0)
    }
}
