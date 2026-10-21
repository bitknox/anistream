import Foundation
import SpikePlayer

/// swift run PlayerSpike <url-or-file> [options]
///
///   --shaders off|natural|sharp   the look; `choose()` picks the chain for source and screen
///   --chain A|B|C[-HQ]            pin an Anime4K preset instead (rungs this Mac would not get)
///   --shot out.png                mpv's `screenshot-to-file out.png window` at the end
///   --seconds N                   play N seconds, report, quit (default: stay open)
///   --mode play|probe|s2          play (default), each shader alone (S3), resizes (S2)
///   --hwdec MODE                  default videotoolbox
///   --fullscreen                  go full screen before loading
///   --size WxH                    window size in points (default 1280x720)
///   --report out.json             write the report there too
///   --log LEVEL                   mpv log level kept (default info)
///   --no-hdr-hint                 target-colorspace-hint=no
///   --resize-fix none|vo          after a resize, nothing or re-create the VO (S2)
///   --start S                     start position
///   --opt name=value              any other mpv option, set before init
///   --log-grep TEXT               keep log lines whose prefix or text contains TEXT in the report
///   --speed X                     play X times as fast (headroom: 2 is 48 fps of 24p)
///   --paused                      pause once the first frame is up (S2 while paused)
///   --probe-file A.glsl[,B.glsl]  probe only these (one process per file keeps GPU timers)
///   --wake "CMD ARGS"             an mpv command sent after each S2 resize, to wake the VO
///
/// Inputs may be the aliases testsrc (1080p24 and a tone), testsrc720, testsrc480 and
/// testsrc4k, made by scripts/media.sh.
struct Args {
    var input = "testsrc"
    var look: Look = .off
    var chain: ShaderChain?
    var shot: String?
    var seconds: Double?
    var mode = "play"
    var hwdec = "videotoolbox"
    var fullscreen = false
    var size = CGSize(width: 1280, height: 720)
    var report: String?
    var log = "info"
    var hdrHint = true
    var resizeFix = "none"
    var start: Double?
    var paused = false
    var probeFiles: [String]?
    var speed: Double?
    var logGrep: String?
    var options: [(String, String)] = []
    var wake: [String]?

    static func parse(_ argv: [String] = Array(CommandLine.arguments.dropFirst())) -> Args {
        var args = Args()
        var it = argv.makeIterator()
        func value(_ flag: String) -> String {
            guard let v = it.next() else { fail("\(flag) needs a value") }
            return v
        }
        while let arg = it.next() {
            switch arg {
            case "--shaders":
                let v = value(arg)
                guard let look = Look(rawValue: v) else { fail("unknown look \(v)") }
                args.look = look
            case "--chain":
                let v = value(arg).split(separator: "-")
                guard let chain = Shaders.anime4K(String(v[0]), hq: v.count > 1 && v[1].uppercased() == "HQ")
                else { fail("unknown chain") }
                args.chain = chain
                args.look = .sharp
            case "--shot": args.shot = absolute(value(arg))
            case "--seconds": args.seconds = Double(value(arg))
            case "--mode": args.mode = value(arg)
            case "--hwdec": args.hwdec = value(arg)
            case "--fullscreen": args.fullscreen = true
            case "--size":
                let parts = value(arg).split(separator: "x").compactMap { Double($0) }
                guard parts.count == 2 else { fail("--size WxH") }
                args.size = CGSize(width: parts[0], height: parts[1])
            case "--report": args.report = absolute(value(arg))
            case "--log": args.log = value(arg)
            case "--no-hdr-hint": args.hdrHint = false
            case "--resize-fix": args.resizeFix = value(arg)
            case "--start": args.start = Double(value(arg))
            case "--paused": args.paused = true
            case "--speed": args.speed = Double(value(arg))
            case "--log-grep": args.logGrep = value(arg)
            case "--opt":
                let pair = value(arg).split(separator: "=", maxSplits: 1).map(String.init)
                guard pair.count == 2 else { fail("--opt name=value") }
                args.options.append((pair[0], pair[1]))
            case "--probe-file": args.probeFiles = value(arg).split(separator: ",").map(String.init)
            case "--wake": args.wake = value(arg).split(separator: " ").map(String.init)
            case "-h", "--help":
                print("swift run PlayerSpike <url-or-file> [--shaders off|natural|sharp] [--shot out.png] [--seconds N] …")
                exit(0)
            default:
                if arg.hasPrefix("--") { fail("unknown option \(arg)") }
                args.input = arg
            }
        }
        args.input = resolve(args.input)
        return args
    }

    /// The testsrc aliases are files from scripts/media.sh: MPVKit's FFmpeg has no lavfi
    /// device, so `av://lavfi:testsrc2=…` does not open.
    static func resolve(_ input: String) -> String {
        switch input {
        case "testsrc": return media("testsrc-1080p.mkv")
        case "testsrc720": return media("sdr-720p.mkv")
        case "testsrc480": return media("testsrc-480p.mkv")
        case "testsrc4k": return media("testsrc-2160p.mkv")
        default: return input.contains("://") ? input : absolute(input)
        }
    }

    static func media(_ name: String) -> String {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<3 { url.deleteLastPathComponent() }
        return url.appending(path: ".media/\(name)").path
    }

    static func absolute(_ path: String) -> String {
        URL(fileURLWithPath: path, relativeTo: URL(fileURLWithPath: FileManager.default.currentDirectoryPath))
            .standardizedFileURL.path
    }
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data("PlayerSpike: \(message)\n".utf8))
    exit(2)
}
