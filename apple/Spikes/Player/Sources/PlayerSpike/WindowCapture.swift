import AppKit
import ImageIO
import UniformTypeIdentifiers

/// A picture of this process's own window, from the window server, so what is on screen can
/// be compared with what mpv thinks it drew (`screenshot-to-file … window` renders mpv's own
/// idea of the window again, so it cannot show a stale size).
///
/// `CGWindowListCreateImage` is unavailable in the macOS 15+ SDKs, so it is looked up at run
/// time. A process may capture its own windows without Screen Recording permission.
enum WindowCapture {
    private typealias CreateImage = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?

    private static let createImage: CreateImage? = {
        guard let symbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage") else {
            return nil
        }
        return unsafeBitCast(symbol, to: CreateImage.self)
    }()

    /// Writes the window's picture as PNG; returns its size in pixels, or nil if the window
    /// server would not give one.
    @MainActor
    static func capture(_ window: NSWindow, to path: String) -> [Int]? {
        guard let createImage else { return nil }
        // kCGWindowListOptionIncludingWindow; kCGWindowImageBoundsIgnoreFraming | BestResolution.
        guard let image = createImage(.null, 1 << 3, UInt32(window.windowNumber), (1 << 0) | (1 << 3))?
            .takeRetainedValue()
        else { return nil }
        let url = URL(fileURLWithPath: path)
        guard let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(dest, image, nil)
        guard CGImageDestinationFinalize(dest) else { return nil }
        return [image.width, image.height]
    }
}
