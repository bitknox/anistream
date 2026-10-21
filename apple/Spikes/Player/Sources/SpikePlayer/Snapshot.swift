import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// mpv's picture of its own output, saved as PNG.
///
/// `screenshot-to-file` cannot be used: MPVKit builds FFmpeg with no image encoders (no PNG,
/// MJPEG or WebP), so mpv cannot write the file. `screenshot-raw window rgba` hands back the
/// same render — after shaders, with subtitles, at the window's size — as pixels, and ImageIO
/// writes them. It needs no Screen Recording permission, on the Mac or in the Simulator.
public enum Snapshot {
    public struct Failure: Error, CustomStringConvertible {
        public let description: String
    }

    /// Returns the picture's size in pixels.
    @discardableResult
    public static func save(_ mpv: MPVClient, to path: String, flags: String = "window") throws -> (Int, Int) {
        let (status, raw) = mpv.commandResult(["screenshot-raw", flags, "rgba"])
        guard status >= 0 else { throw Failure(description: "screenshot-raw failed (\(status))") }
        guard let w = raw["w"].int, let h = raw["h"].int, let stride = raw["stride"].int,
            case .bytes(let data) = raw["data"]
        else { throw Failure(description: "screenshot-raw returned no image") }
        guard let provider = CGDataProvider(data: data as CFData),
            let image = CGImage(
                width: Int(w), height: Int(h), bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: Int(stride),
                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
                provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
        else { throw Failure(description: "could not make an image of \(w)×\(h)") }
        let url = URL(fileURLWithPath: path)
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        guard let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)
        else { throw Failure(description: "cannot write \(path)") }
        CGImageDestinationAddImage(dest, image, nil)
        guard CGImageDestinationFinalize(dest) else { throw Failure(description: "cannot write \(path)") }
        return (Int(w), Int(h))
    }
}
