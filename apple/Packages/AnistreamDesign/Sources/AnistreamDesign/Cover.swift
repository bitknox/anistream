// Covers and banners: their shapes, and the plate that stands where the art has not arrived.
// Never a shimmer, a grey skeleton or a broken-image glyph: a flat rect in a muted version of the
// cover's own colour, which the art fades in over.

import SwiftUI

/// The corner radii of art, by the brief: a cover rounds 6 up to 40 points wide and 8 above, a
/// banner 16.
public enum ArtShape {
    public static func coverRadius(width: CGFloat) -> CGFloat {
        width <= 40 ? 6 : 8
    }

    public static let bannerRadius: CGFloat = 16
}

/// The placeholder for a cover or banner: the cover's own colour, as AniList gives it, muted
/// into the ground; the raised colour when there is none.
public struct CoverPlate: View {
    public let color: Color?

    public init(color: Color?) {
        self.color = color
    }

    /// `hex` as AniList writes it (`#e4a15d`), or `nil`.
    public init(hex: String?) {
        self.color = hex.flatMap(Color.init(hex:))
    }

    public var body: some View {
        Rectangle()
            .fill(Self.muted(color))
            .accessibilityHidden(true)
    }

    /// How much of the cover's colour shows through the ground: enough to tell covers apart,
    /// little enough that a column of plates reads as quiet.
    public static let strength = 0.3

    /// `color` muted into the ground.
    public static func muted(_ color: Color?) -> Color {
        guard let color else { return Palette.raised }
        return Palette.ground.mix(with: color, by: strength, in: .perceptual)
    }
}

struct CoverPlatePreviews: PreviewProvider {
    static var previews: some View {
        HStack(spacing: 12) {
            CoverPlate(hex: "#e4a15d").frame(width: 36, height: 54).clipShape(.rect(cornerRadius: 6))
            CoverPlate(hex: "#5d9be4").frame(width: 36, height: 54).clipShape(.rect(cornerRadius: 6))
            CoverPlate(hex: nil).frame(width: 36, height: 54).clipShape(.rect(cornerRadius: 6))
        }
        .padding(24)
        .background(.ground)
    }
}
