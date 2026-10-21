// The snapshot's art as the widgets read it: small JPEGs, no larger than the widgets draw them.

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

public enum ArtThumbnail {
    /// `image` scaled down to just cover `kind`'s pixel size (never up), uncropped, as a JPEG:
    /// each layout crops it to its own shape, a 16:9 card more of a banner's middle and the large
    /// widget's hero more of its width.
    public static func jpeg(from image: CGImage, kind: ArtKind, quality: Double = 0.82) -> Data? {
        let (width, height) = kind.pixelSize
        let scale = min(1, max(Double(width) / Double(image.width), Double(height) / Double(image.height)))
        let outWidth = max(1, Int((Double(image.width) * scale).rounded()))
        let outHeight = max(1, Int((Double(image.height) * scale).rounded()))
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let context = CGContext(
                data: nil, width: outWidth, height: outHeight, bitsPerComponent: 8, bytesPerRow: 0,
                space: space, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
        else { return nil }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: outWidth, height: outHeight))
        guard let scaled = context.makeImage() else { return nil }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(
            data, UTType.jpeg.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(
            destination, scaled, [kCGImageDestinationLossyCompressionQuality: quality] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return data as Data
    }

    /// The image in the file at `url`, decoded at no more than `maxPixelSize` on its long side.
    public static func image(at url: URL, maxPixelSize: Int = 1_200) -> CGImage? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, [kCGImageSourceShouldCache: false] as CFDictionary)
        else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
        ]
        return CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary)
    }
}
