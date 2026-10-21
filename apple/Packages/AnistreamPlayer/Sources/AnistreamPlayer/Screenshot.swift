// mpv's picture of what it drew, as a CGImage, for tests and the app's screenshots.

import CoreGraphics
import Foundation
import Mpv

public enum ScreenshotContent: String, Sendable {
    /// What the window shows: scaled, after shaders, with subtitles.
    case window
    /// The video at its own size, with subtitles.
    case subtitles
    /// The video at its own size alone.
    case video
}

extension MpvClient {
    /// mpv renders the picture again offscreen and hands back its pixels (`screenshot-raw`):
    /// no Screen Recording permission, and no encoder, which MPVKit's FFmpeg has none of.
    ///
    /// Not in the Simulator: the read-back asks for a host-pointer buffer, which the Simulator's
    /// Metal traps on, and the app dies. Take those with `xcrun simctl io … screenshot`.
    public func screenshot(_ content: ScreenshotContent = .window) throws -> CGImage {
        #if targetEnvironment(simulator)
        throw MpvError.unsupported("mpv's screenshots crash the iOS Simulator; use simctl io screenshot")
        #else
        let raw = try command("screenshot-raw", ["flags": .string(content.rawValue), "format": "rgba"])
        guard let width = raw["w"].int, let height = raw["h"].int, let stride = raw["stride"].int,
            case .bytes(let pixels) = raw["data"], width > 0, height > 0
        else { throw MpvError.unsupported("screenshot-raw returned no picture") }
        guard let provider = CGDataProvider(data: pixels as CFData),
            let image = CGImage(
                width: Int(width), height: Int(height), bitsPerComponent: 8, bitsPerPixel: 32,
                bytesPerRow: Int(stride), space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.noneSkipLast.rawValue),
                provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent)
        else { throw MpvError.unsupported("could not make a \(width)×\(height) image") }
        return image
        #endif
    }
}
