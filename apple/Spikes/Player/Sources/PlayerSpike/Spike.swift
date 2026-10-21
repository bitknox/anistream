import AppKit
import Foundation
import SpikePlayer

/// Log lines kept from mpv, numbered so a step can ask for the ones since it began.
final class LogBook: @unchecked Sendable {
    private let lock = NSLock()
    private var lines: [MPVLogLine] = []

    func append(_ line: MPVLogLine) { lock.withLock { lines.append(line) } }
    var count: Int { lock.withLock { lines.count } }
    func since(_ index: Int) -> [MPVLogLine] { lock.withLock { Array(lines[min(index, lines.count)...]) } }
    func all() -> [MPVLogLine] { lock.withLock { lines } }
}

/// The scripted runs: play and measure, probe each shader alone (S3), or resize and go full
/// screen (S2). It runs off the main thread and hops there only for AppKit and the model,
/// since mpv calls may wait on the render thread, which may wait on the main thread.
final class Spike: @unchecked Sendable {
    static let cacheDir: String = {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<3 { url.deleteLastPathComponent() }
        return url.appending(path: ".cache/mpv").path
    }()

    let args: Args
    let player: Player
    let mpv: MPVClient
    let log = LogBook()
    @MainActor var window: NSWindow!
    var report: [String: Any] = [:]

    @MainActor
    init(args: Args, player: Player) {
        self.args = args
        self.player = player
        self.mpv = player.mpv
        let log = self.log
        mpv.addHandler { event in
            if case .log(let line) = event {
                log.append(line)
                if line.level == "error" || line.level == "fatal" || line.level == "warn" {
                    FileHandle.standardError.write(Data("[\(line.prefix)] \(line.level): \(line.text)\n".utf8))
                }
            }
        }
    }

    func run() async {
        let started = Date()
        while await !MainActor.run(body: { player.attached }) {
            if Date().timeIntervalSince(started) > 10 { fail("mpv never attached to the surface") }
            try? await Task.sleep(for: .milliseconds(50))
        }
        report["attach_ms"] = Int(Date().timeIntervalSince(started) * 1000)
        if args.fullscreen { await setFullscreen(true) }
        await MainActor.run {
            player.pinnedChain = args.chain
            player.setLook(args.look)
        }
        if let start = args.start { mpv.set("start", String(start)) }

        switch args.mode {
        case "probe":
            await probe()
        case "s2":
            guard await load(args.input) else { return finish() }
            await s2()
        default:
            guard await load(args.input) else { return finish() }
            guard let seconds = args.seconds else {
                // Interactive: the window stays until it is closed.
                if let shot = args.shot {
                    try? await Task.sleep(for: .seconds(2))
                    screenshot(shot)
                }
                return
            }
            try? await Task.sleep(for: .seconds(2))  // shaders compile, the first frames settle
            await measure(seconds: seconds)
        }
        finish()
    }

    // MARK: Steps

    /// Loads `url` and waits for its first frame.
    func load(_ url: String) async -> Bool {
        let t0 = Date()
        let mpv = self.mpv
        async let restart = mpv.next(timeout: .seconds(30)) { event in
            switch event {
            case .playbackRestart, .endFile: true
            default: false
            }
        }
        mpv.command(["loadfile", url, "replace"])
        guard case .playbackRestart = await restart else {
            report["load_error"] = "no first frame from \(url)"
            return false
        }
        report["first_frame_ms"] = Int(Date().timeIntervalSince(t0) * 1000)
        return true
    }

    func measure(seconds: Double) async {
        let before = counters()
        let t0 = Date()
        try? await Task.sleep(for: .seconds(seconds))
        let after = counters()
        var delta: [String: Int64] = [:]
        for (key, value) in after { delta[key] = value - (before[key] ?? 0) }
        report["measured_seconds"] = Date().timeIntervalSince(t0)
        report["counters"] = delta
        report["passes"] = passes()
    }

    func counters() -> [String: Int64] {
        var out: [String: Int64] = [:]
        for name in [
            "frame-drop-count", "decoder-frame-drop-count", "vo-delayed-frame-count", "mistimed-frame-count",
            "estimated-frame-number",
        ] {
            out[name] = mpv.get(name).int ?? -1
        }
        return out
    }

    /// `vo-passes`: what each render pass costs on the GPU, per fresh frame.
    func passes() -> [String: Any] {
        let fresh = mpv.get("vo-passes")["fresh"].array
        let avg = fresh.reduce(0.0) { $0 + ($1["avg"].double ?? 0) }
        let peak = fresh.reduce(0.0) { $0 + ($1["peak"].double ?? 0) }
        var byDesc: [String: Double] = [:]
        for pass in fresh {
            let desc = (pass["desc"].string ?? "?")
                .replacingOccurrences(of: #"\s*\(\w+\)$"#, with: "", options: .regularExpression)
            byDesc[desc, default: 0] += pass["avg"].double ?? 0
        }
        let heaviest = byDesc.sorted { $0.value > $1.value }.prefix(8).map {
            ["desc": $0.key, "avg_ms": round($0.value / 1e4) / 100]
        }
        return [
            "count": fresh.count, "avg_ms": round(avg / 1e4) / 100, "peak_ms": round(peak / 1e4) / 100,
            "heaviest": heaviest,
        ]
    }

    @discardableResult
    func screenshot(_ path: String) -> Bool {
        try? FileManager.default.createDirectory(
            at: URL(fileURLWithPath: path).deletingLastPathComponent(), withIntermediateDirectories: true)
        var shots = report["shots"] as? [String] ?? []
        defer { report["shots"] = shots }
        do {
            let (w, h) = try Snapshot.save(mpv, to: path)
            shots.append("\(path) (\(w)×\(h))")
            return true
        } catch {
            shots.append("\(path) (failed: \(error))")
            return false
        }
    }

    func setFullscreen(_ on: Bool) async {
        await MainActor.run {
            if window.styleMask.contains(.fullScreen) != on { window.toggleFullScreen(nil) }
        }
        for _ in 0..<60 {
            try? await Task.sleep(for: .milliseconds(100))
            if await MainActor.run(body: { window.styleMask.contains(.fullScreen) == on }) { break }
        }
        try? await Task.sleep(for: .seconds(1))  // the animation
    }

    // MARK: S3: every shader alone

    /// Each file on a source its `//!WHEN` fires for, at full screen: libplacebo compiles a
    /// hook to SPIR-V only when it runs, so a skipped hook would prove nothing.
    func probe() async {
        var results: [[String: Any]] = []
        var current = ""
        for file in Shaders.all {
            let source =
                switch file {
                case "Anime4K_AutoDownscalePre_x4.glsl": "testsrc480"
                case "SSimDownscaler.glsl": "testsrc4k"
                default: "testsrc"
                }
            if source != current {
                mpv.set("glsl-shaders", strings: [])
                guard await load(Args.resolve(source)) else {
                    results.append(["file": file, "error": "source \(source) did not load"])
                    continue
                }
                current = source
                try? await Task.sleep(for: .seconds(1))
            }
            let path = Shaders.directory.appending(path: file).path
            let descs = Self.descs(path)
            let mark = log.count
            let t0 = Date()
            let status = mpv.set("glsl-shaders", strings: [path])
            try? await Task.sleep(for: .seconds(3))
            let fresh = mpv.get("vo-passes")["fresh"].array
            let mine = fresh.filter { pass in
                let desc = pass["desc"].string ?? ""
                return descs.contains { desc.contains($0) }
            }
            let problems = log.since(mark).filter { ["fatal", "error", "warn"].contains($0.level) }
            let ms = mine.reduce(0.0) { $0 + ($1["avg"].double ?? 0) } / 1e6
            results.append([
                "file": file, "source": source, "set_status": status,
                "hooks_in_file": descs.count, "passes_run": mine.count,
                "gpu_ms": round(ms * 100) / 100,
                "elapsed_ms": Int(Date().timeIntervalSince(t0) * 1000),
                "problems": problems.map { "[\($0.prefix)] \($0.level): \($0.text)" },
            ])
            FileHandle.standardError.write(
                Data("probe \(file): \(mine.count) passes, \(String(format: "%.2f", ms)) ms, \(problems.count) problems\n".utf8))
        }
        report["probe"] = results
    }

    static func descs(_ path: String) -> [String] {
        guard let text = try? String(contentsOfFile: path, encoding: .utf8) else { return [] }
        return Array(
            Set(
                text.split(separator: "\n")
                    .filter { $0.hasPrefix("//!DESC ") }
                    .map { String($0.dropFirst(8)).trimmingCharacters(in: .whitespaces) }))
    }

    // MARK: S2: resize and full screen

    func s2() async {
        let base = (args.shot ?? Args.absolute("s2.png")).replacingOccurrences(of: ".png", with: "")
        var steps: [[String: Any]] = []
        let plan: [(String, CGSize?, Bool)] = [
            ("initial", nil, false),
            ("1600x900", CGSize(width: 1600, height: 900), false),
            ("900x900", CGSize(width: 900, height: 900), false),
            ("fullscreen", nil, true),
            ("windowed", CGSize(width: 1280, height: 720), false),
        ]
        for (name, size, fullscreen) in plan {
            if fullscreen {
                await setFullscreen(true)
            } else {
                if await MainActor.run(body: { window.styleMask.contains(.fullScreen) }) {
                    await setFullscreen(false)
                }
                if let size {
                    await MainActor.run {
                        var frame = window.frame
                        frame.origin.y += frame.height - size.height
                        frame.size = size
                        window.setFrame(frame, display: true, animate: false)
                    }
                }
            }
            try? await Task.sleep(for: .seconds(1.5))
            steps.append(await snapshot(name: name, base: base))
            if args.resizeFix == "vo", name != "initial" {
                mpv.set("vo", "gpu-next")
                try? await Task.sleep(for: .seconds(2))
                steps.append(await snapshot(name: "\(name)-after-vo-reinit", base: base))
            }
        }
        report["s2"] = steps
    }

    func snapshot(name: String, base: String) async -> [String: Any] {
        let osd = mpv.get("osd-dimensions")
        let mpvShot = "\(base)-\(name)-mpv.png"
        let windowShot = "\(base)-\(name)-window.png"
        screenshot(mpvShot)
        let (layer, windowSize, captured) = await MainActor.run {
            (
                Unchecked(player.layer?.state ?? [:]),
                [Double(window.contentLayoutRect.width), Double(window.contentLayoutRect.height)],
                WindowCapture.capture(window, to: windowShot)
            )
        }
        return [
            "step": name, "window_points": windowSize, "layer": layer.value,
            "mpv_osd": ["w": osd["w"].int ?? 0, "h": osd["h"].int ?? 0],
            "mpv_shot": mpvShot,
            "window_capture": captured.map { _ in windowShot } ?? "unavailable",
            "window_capture_px": captured ?? [],
        ]
    }

    // MARK: Report

    func finish() {
        if let shot = args.shot, args.mode == "play" { screenshot(shot) }
        report["input"] = args.input
        report["mode"] = args.mode
        report["hwdec_requested"] = args.hwdec
        let props = [
            "mpv-version", "ffmpeg-version", "libass-version", "current-vo", "current-gpu-context", "hwdec-current",
            "hwdec-interop", "video-codec", "video-params", "video-out-params", "video-target-params",
            "osd-dimensions", "container-fps", "estimated-vf-fps", "display-fps", "display-width", "display-hidpi-scale",
            "glsl-shaders", "sub-font-provider", "track-list",
        ]
        var mpvState: [String: Any] = [:]
        for name in props { mpvState[name] = mpv.get(name).json }
        report["mpv"] = mpvState
        let ui = DispatchQueue.main.sync {
            MainActor.assumeIsolated {
                let screen = window.screen ?? NSScreen.main
                return Unchecked<[String: Any]>([
                    "look": player.look.rawValue,
                    "chain": ["label": player.chain.label, "files": player.chain.files],
                    "display_px": player.display.map { [$0.width, $0.height] } ?? [],
                    "layer": player.layer?.state ?? [:],
                    "screen": [
                        "name": screen?.localizedName ?? "?",
                        "maximumPotentialEDR": Double(screen?.maximumPotentialExtendedDynamicRangeColorComponentValue ?? 1),
                        "maximumEDR": Double(screen?.maximumExtendedDynamicRangeColorComponentValue ?? 1),
                        "maximumReferenceEDR": Double(screen?.maximumReferenceExtendedDynamicRangeColorComponentValue ?? 0),
                        "maximumFramesPerSecond": screen?.maximumFramesPerSecond ?? 0,
                    ],
                ])
            }
        }
        report["ui"] = ui.value
        let problems = log.all().filter { ["fatal", "error", "warn"].contains($0.level) }
        report["log_problems"] = Array(Set(problems.map { "[\($0.prefix)] \($0.level): \($0.text)" })).sorted()
        report["log_hdr"] = log.all()
            .filter { $0.text.localizedCaseInsensitiveContains("colorspace") || $0.text.contains("hdr") || $0.text.contains("swapchain") || $0.text.contains("Using hardware") || $0.text.contains("hwdec") }
            .prefix(40).map { "[\($0.prefix)] \($0.level): \($0.text)" }
        if let data = try? JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys]) {
            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data("\n".utf8))
            if let path = args.report {
                try? FileManager.default.createDirectory(
                    at: URL(fileURLWithPath: path).deletingLastPathComponent(), withIntermediateDirectories: true)
                try? data.write(to: URL(fileURLWithPath: path))
            }
        }
        mpv.shutdown()
        exit(0)
    }
}

/// Foundation values carried across an actor hop; they are built on one side and only read
/// on the other.
struct Unchecked<T>: @unchecked Sendable {
    let value: T
    init(_ value: T) { self.value = value }
}
