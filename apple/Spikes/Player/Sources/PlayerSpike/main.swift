import AppKit
import SpikePlayer
import SwiftUI

/// The spike's window: an NSWindow whose content is SwiftUI, with the player surface (an
/// NSViewRepresentable over the CAMetalLayer view) and the controls over it.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    let args: Args
    let player: Player
    let spike: Spike
    var window: NSWindow!

    init(args: Args) {
        self.args = args
        var options = Player.Options()
        options.hwdec = args.hwdec
        options.hdrHint = args.hdrHint
        options.logLevel = args.log
        options.cacheDir = Spike.cacheDir
        do { player = try Player(options: options) } catch { fail("\(error)") }
        spike = Spike(args: args, player: player)
        super.init()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: args.size),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false)
        window.title = "Player spike"
        window.titlebarAppearsTransparent = true
        window.collectionBehavior.insert(.fullScreenPrimary)
        window.contentView = NSHostingView(rootView: PlayerScreen(player: player))
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
        self.window = window
        spike.window = window
        let spike = self.spike
        Task.detached { await spike.run() }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

let args = Args.parse()
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = AppDelegate(args: args)
app.delegate = delegate
app.run()
