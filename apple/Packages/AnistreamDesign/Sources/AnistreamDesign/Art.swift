// Drawings the app ships, in the same hand as the boards' covers: anistream's own, not any show's.

import CoreGraphics
import Foundation
import ImageIO

public enum DesignArt {
    /// A traveller under a crescent moon, 1600 × 900: what Settings shows the upscaling modes on.
    public static let upscalingPreview: CGImage? = image("upscaling-preview")

    private static func image(_ name: String) -> CGImage? {
        guard let url = resources.url(forResource: name, withExtension: "png", subdirectory: "Art"),
              let source = CGImageSourceCreateWithURL(url as CFURL, nil)
        else { return nil }
        return CGImageSourceCreateImageAtIndex(source, 0, nil)
    }
}
