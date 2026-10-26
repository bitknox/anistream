// The Mac's recorder for apple.sh: one app's window, filmed with ScreenCaptureKit, without the
// pointer. Nothing else on the screen is ever captured, so the recording needs no crop and shows
// nothing of the desktop; the encode draws a pointer of its own from the flow's clicks.
//
//   mac-capture <bundle id> <out.mov>
//
// It waits up to a minute for the app's window (an app opens it a moment after launch), then
// films it at its native pixels, up to 60 frames a second, into an H.264 .mov. It prints
// "ANISTREAM_CAPTURE began <seconds since 1970>" at the first frame, which the flow's marks are
// measured from, and finishes the file on SIGINT or when the window goes away.
//
// It needs Screen Recording allowed for the terminal that runs it.

import AppKit
import AVFoundation
import CoreMedia
import Foundation
import ScreenCaptureKit

setvbuf(stdout, nil, _IOLBF, 0)
// The window server's connection, which ScreenCaptureKit needs and a plain tool lacks.
_ = NSApplication.shared

let arguments = CommandLine.arguments
guard arguments.count == 3 else {
    FileHandle.standardError.write("usage: mac-capture <bundle id> <out.mov>\n".data(using: .utf8)!)
    exit(2)
}
let bundle = arguments[1]
let out = URL(fileURLWithPath: arguments[2])

func fail(_ message: String) -> Never {
    FileHandle.standardError.write("mac-capture: \(message)\n".data(using: .utf8)!)
    exit(1)
}

/// The app's main window, on screen at the normal level and the largest of its windows, the
/// display it is on, and every other app, to leave out.
func window() async -> (SCWindow, SCDisplay, [SCRunningApplication])? {
    guard let content = try? await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true) else { return nil }
    let window = content.windows
        .filter { $0.owningApplication?.bundleIdentifier == bundle && $0.windowLayer == 0 && $0.frame.width >= 400 }
        .max { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height }
    guard let window, let display = content.displays.first(where: { $0.frame.intersects(window.frame) }) else { return nil }
    return (window, display, content.applications.filter { $0.bundleIdentifier != bundle })
}

final class Recorder: NSObject, SCStreamOutput, SCStreamDelegate, @unchecked Sendable {
    private let writer: AVAssetWriter
    private let input: AVAssetWriterInput
    private let queue = DispatchQueue(label: "mac-capture")
    private var started = false
    private var finished = false
    /// Seconds since 1970 less the host clock's seconds, for the first frame's wall time.
    private let wall = Date().timeIntervalSince1970 - CMClockGetTime(CMClockGetHostTimeClock()).seconds
    var stream: SCStream?

    init(width: Int, height: Int) throws {
        try? FileManager.default.removeItem(at: out)
        writer = try AVAssetWriter(outputURL: out, fileType: .mov)
        input = AVAssetWriterInput(mediaType: .video, outputSettings: [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: width,
            AVVideoHeightKey: height,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: 60_000_000,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                AVVideoAllowFrameReorderingKey: false,
            ],
        ])
        input.expectsMediaDataInRealTime = true
        writer.add(input)
        super.init()
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer buffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, buffer.isValid, !finished else { return }
        // Only frames with new content: the stream also sends idle ones, without an image.
        guard let attachments = CMSampleBufferGetSampleAttachmentsArray(buffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let raw = attachments.first?[.status] as? Int, SCFrameStatus(rawValue: raw) == .complete,
              buffer.imageBuffer != nil else { return }
        let time = buffer.presentationTimeStamp
        if !started {
            guard writer.startWriting() else { fail("could not write \(out.path): \(String(describing: writer.error))") }
            writer.startSession(atSourceTime: time)
            started = true
            print("ANISTREAM_CAPTURE began \(String(format: "%.3f", wall + time.seconds))")
        }
        if input.isReadyForMoreMediaData { input.append(buffer) }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        // The window closed as the flow ended its app.
        queue.async { self.finish() }
    }

    /// The file finished and the process ended.
    func finish() {
        guard !finished else { return }
        finished = true
        let done = DispatchSemaphore(value: 0)
        if let stream { stream.stopCapture { _ in done.signal() }; _ = done.wait(timeout: .now() + 2) }
        guard started else { fail("no frame was filmed") }
        input.markAsFinished()
        writer.finishWriting {
            if self.writer.status != .completed { fail("could not finish \(out.path): \(String(describing: self.writer.error))") }
            exit(0)
        }
    }

    var sampleQueue: DispatchQueue { queue }
}

signal(SIGINT, SIG_IGN)
signal(SIGTERM, SIG_IGN)
var recorder: Recorder?
let stop: () -> Void = {
    if let recorder { recorder.sampleQueue.async { recorder.finish() } } else { exit(1) }
}
let interrupt = DispatchSource.makeSignalSource(signal: SIGINT, queue: .main)
interrupt.setEventHandler(handler: stop)
interrupt.resume()
let terminate = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
terminate.setEventHandler(handler: stop)
terminate.resume()

Task {
    let deadline = Date().addingTimeInterval(60)
    var found: (SCWindow, SCDisplay, [SCRunningApplication])?
    // Up, and done opening: the same frame twice, 0.3 s apart.
    var last: CGRect?
    while Date() < deadline {
        let now = await window()
        if let now, now.0.frame == last { found = now; break }
        last = now?.0.frame
        try? await Task.sleep(for: .milliseconds(now == nil ? 100 : 300))
    }
    guard let (window, display, others) = found else { fail("no window of \(bundle) came up") }
    // The display without every other app, cut to the window. A window filmed on its own, or
    // the display with only its window included, leaves out the title bar's buttons, which the
    // window server draws.
    let filter = SCContentFilter(display: display, excludingApplications: others, exceptingWindows: [])
    let scale = CGFloat(filter.pointPixelScale)
    let frame = window.frame.offsetBy(dx: -display.frame.minX, dy: -display.frame.minY)
    let width = Int(frame.width * scale), height = Int(frame.height * scale)
    let configuration = SCStreamConfiguration()
    configuration.width = width
    configuration.height = height
    configuration.sourceRect = frame
    configuration.showsCursor = false
    configuration.minimumFrameInterval = CMTime(value: 1, timescale: 60)
    configuration.pixelFormat = kCVPixelFormatType_32BGRA
    configuration.queueDepth = 8
    configuration.capturesAudio = false
    do {
        let made = try Recorder(width: width, height: height)
        let stream = SCStream(filter: filter, configuration: configuration, delegate: made)
        made.stream = stream
        try stream.addStreamOutput(made, type: .screen, sampleHandlerQueue: made.sampleQueue)
        recorder = made
        try await stream.startCapture()
        print("ANISTREAM_CAPTURE window \(Int(frame.minX)) \(Int(frame.minY)) \(Int(frame.width)) \(Int(frame.height)) at \(width)x\(height)")
    } catch {
        fail("could not film the window: \(error)")
    }
}

dispatchMain()
