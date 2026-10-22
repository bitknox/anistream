import SpikePlayer
import SwiftUI
import UIKit

/// The spike on iPhone and iPad: the same SwiftUI player screen (a UIViewRepresentable over
/// the CAMetalLayer view, native controls on top) and the same mpv setup as the Mac.
///
/// Launch arguments, after the bundle id in `xcrun simctl launch` (no `--`), or the scheme:
///
///   <url-or-file> [--shaders off|natural|sharp] [--seconds N] [--shot out.png]
///   [--report out.json] [--hwdec MODE] [--opt name=value] [--paused] [--rotate]
///
/// In the Simulator the app can read and write the Mac's own paths, so the media and the
/// reports stay where the macOS runner keeps them.
@main
struct PlayerSpikeApp: App {
    @State private var player: Player = {
        let args = LaunchArgs.parse()
        var options = Player.Options()
        options.hwdec = args.hwdec
        options.logLevel = "v"
        options.cacheDir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appending(path: "mpv").path
        options.extra = args.options
        return try! Player(options: options)
    }()

    var body: some Scene {
        WindowGroup {
            PlayerScreen(player: player)
                .statusBarHidden()
                .task { await Script(player: player, args: LaunchArgs.parse()).run() }
        }
    }
}

struct LaunchArgs {
    var input: String?
    var look: Look = .off
    var seconds: Double?
    var shot: String?
    var report: String?
    var hwdec = "videotoolbox"
    var paused = false
    var rotate = false
    var options: [(String, String)] = []

    static func parse() -> LaunchArgs {
        var args = LaunchArgs()
        var it = ProcessInfo.processInfo.arguments.dropFirst().makeIterator()
        while let arg = it.next() {
            switch arg {
            case "--shaders": args.look = it.next().flatMap(Look.init(rawValue:)) ?? .off
            case "--seconds": args.seconds = it.next().flatMap(Double.init)
            case "--shot": args.shot = it.next()
            case "--report": args.report = it.next()
            case "--hwdec": args.hwdec = it.next() ?? args.hwdec
            case "--paused": args.paused = true
            case "--rotate": args.rotate = true
            case "--opt":
                if let pair = it.next()?.split(separator: "=", maxSplits: 1), pair.count == 2 {
                    args.options.append((String(pair[0]), String(pair[1])))
                }
            default:
                // Xcode and simctl add their own -Flag value pairs; skip them.
                if arg.hasPrefix("-") { _ = it.next() } else { args.input = arg }
            }
        }
        return args
    }
}

/// Plays the input, waits, then writes mpv's screenshot and a report, as the Mac runner does.
@MainActor
struct Script {
    let player: Player
    let args: LaunchArgs

    func run() async {
        while !player.attached { try? await Task.sleep(for: .milliseconds(50)) }
        player.setLook(args.look)
        guard let input = args.input else { return }
        let mpv = player.mpv
        let log = LogLines()
        mpv.addHandler { event in
            if case .log(let line) = event {
                log.append(line)
                if ["fatal", "error", "warn"].contains(line.level) || line.prefix == "vd" || line.prefix == "cplayer" {
                    print("[\(line.prefix)] \(line.level): \(line.text)")
                }
            }
        }
        async let first = mpv.next(timeout: .seconds(30)) { event in
            switch event {
            case .playbackRestart: true
            case .endFile(let reason, _): reason == 4
            default: false
            }
        }
        let t0 = Date()
        print("PLAYER_SPIKE loading \(input)")
        player.load(input)
        let ticker = Task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                let osd = mpv.get("osd-dimensions")
                print("PLAYER_SPIKE t=\(mpv.get("time-pos").double ?? -1) frame=\(mpv.get("estimated-frame-number").int ?? -1) vo=\(mpv.get("current-vo").string ?? "none") drops=\(mpv.get("frame-drop-count").int ?? -1) osd=\(osd["w"].int ?? 0)x\(osd["h"].int ?? 0) layer=\(player.layer?.state ?? [:])")
            }
        }
        defer { ticker.cancel() }
        let started = await first
        print("PLAYER_SPIKE first frame: \(started.map { "\($0)" } ?? "none") after \(Int(Date().timeIntervalSince(t0) * 1000)) ms")
        if args.rotate {
            // S2 on iOS: turn to landscape and see whether mpv's idea of the size follows.
            try? await Task.sleep(for: .seconds(1.5))
            let osd0 = mpv.get("osd-dimensions")
            print("PLAYER_SPIKE portrait drawable=\(player.drawableSize) osd=\(osd0["w"].int ?? 0)x\(osd0["h"].int ?? 0)")
            let scene = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
            scene?.requestGeometryUpdate(.iOS(interfaceOrientations: .landscapeRight)) { error in
                print("PLAYER_SPIKE rotation refused: \(error.localizedDescription)")
            }
            try? await Task.sleep(for: .seconds(2.5))
            let osd1 = mpv.get("osd-dimensions")
            print("PLAYER_SPIKE landscape drawable=\(player.drawableSize) osd=\(osd1["w"].int ?? 0)x\(osd1["h"].int ?? 0)")
        }
        if args.paused {
            // Paused, the controls stay up: a picture of the native UI over real video.
            try? await Task.sleep(for: .seconds(2))
            player.togglePause()
        }
        var report: [String: Any] = [
            "input": input, "first_frame": started.map { "\($0)" } ?? "none",
            "first_frame_ms": Int(Date().timeIntervalSince(t0) * 1000),
        ]
        guard let seconds = args.seconds else { return }
        let before = mpv.get("frame-drop-count").int ?? 0
        try? await Task.sleep(for: .seconds(seconds))
        report["frame_drops"] = (mpv.get("frame-drop-count").int ?? 0) - before
        for name in [
            "current-vo", "current-gpu-context", "hwdec-current", "video-params", "video-target-params",
            "osd-dimensions", "estimated-frame-number", "glsl-shaders",
        ] {
            report[name] = mpv.get(name).json
        }
        report["chain"] = player.chain.label
        let passes = mpv.get("vo-passes")["fresh"].array.compactMap { $0["desc"].string }
        report["passes"] = ["count": passes.count, "shader_passes": passes.filter { $0.contains("Anime4K") || $0.contains("ArtCNN") || $0.contains("Krig") || $0.contains("SSim") }.count]
        report["layer"] = player.layer?.state ?? [:]
        report["edr"] = ["current": player.edrHeadroom, "potential": player.edrPotential]
        report["log_problems"] = Array(Set(log.problems())).sorted()
        // The Simulator's sandbox reads the Mac's paths but may not write them: save in the app's
        // own container too, and say where.
        let container = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        if let shot = args.shot {
            let name = URL(fileURLWithPath: shot).lastPathComponent
            let local = container.appending(path: name).path
            let result = await Task.detached { () -> String in
                do {
                    let (w, h) = try Snapshot.save(mpv, to: local)
                    return "\(local) \(w)x\(h)"
                } catch {
                    return "failed: \(error)"
                }
            }.value
            report["shot"] = result
            print("PLAYER_SPIKE shot \(result)")
            try? FileManager.default.removeItem(atPath: shot)
            do { try FileManager.default.copyItem(atPath: local, toPath: shot) } catch {
                print("PLAYER_SPIKE cannot write \(shot): \(error.localizedDescription)")
            }
        }
        if let path = args.report,
            let data = try? JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
        {
            let local = container.appending(path: URL(fileURLWithPath: path).lastPathComponent)
            try? data.write(to: local)
            do { try data.write(to: URL(fileURLWithPath: path)) } catch {
                print("PLAYER_SPIKE cannot write \(path): \(error.localizedDescription); kept \(local.path)")
            }
        }
        print("PLAYER_SPIKE_DONE")
    }
}

final class LogLines: @unchecked Sendable {
    private let lock = NSLock()
    private var lines: [MPVLogLine] = []
    func append(_ line: MPVLogLine) { lock.withLock { lines.append(line) } }
    func problems() -> [String] {
        lock.withLock {
            lines.filter { ["fatal", "error", "warn"].contains($0.level) }
                .map { "[\($0.prefix)] \($0.level): \($0.text)" }
        }
    }
}
