// The Mac dev harness: the app's own window, on the testing engine, from `swift run`.

import AnistreamDesign
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
                .environment(delegate.store)
                .frame(minWidth: 960, minHeight: 600)
                .task { await delegate.run() }
        }
        .defaultSize(width: options.width, height: options.height)
        .windowResizability(.contentMinSize)
    }
}

@MainActor
final class Delegate: NSObject, NSApplicationDelegate {
    let store = AppStore()
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

        let settled = await store.settle()
        print(settled ? "ANISTREAM_READY" : "AnistreamDev: did not settle; going on")
        guard let url = options.snapshot else { return }
        // The art's and the preview's fade-ins.
        try? await Task.sleep(for: .milliseconds(400))
        do {
            guard let window = Snapshot.window else { throw Snapshot.Problem.noWindow }
            try Snapshot.write(window, to: url)
            print("AnistreamDev: wrote \(url.path)")
        } catch {
            FileHandle.standardError.write(Data("AnistreamDev: \(error)\n".utf8))
        }
        NSApp.terminate(nil)
    }
}
