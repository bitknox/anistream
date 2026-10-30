// A title's wide art, drawn: the rule itself, `HeroArt`, is the store's, since the player's
// starting screen follows it too.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The wide art itself: the banner or the still filling the frame, faded in over the cover's
/// colour once it arrives (T8), or that colour alone while the episodes load. With neither, the
/// cover fills it (`CoverFill`). Nothing for `.compact`; the screen leaves the frame out.
///
/// With `holds`, the art shown last stays while the next is on its way, a moment at most: the Mac's
/// title page across a season switch, where the last season's banner crossfades to the next one's
/// rather than through a plate.
struct HeroArtView: View {
    @Environment(\.displayScale) private var scale
    let art: HeroArt
    let entry: Entry
    var holds = false
    /// Whether a blurred cover carries the sharp cover too: not where the screen shows the cover
    /// over the frame itself, as a title page does.
    var sharpCover = true

    /// What a page about to open asks its wide art at: wider than a page shows it on most screens,
    /// and never more than the file has.
    static let prefetchPixels = 2560

    var body: some View {
        GeometryReader { proxy in
            switch art {
            case .banner, .still, .waiting:
                ArtworkView(
                    url: art.url, color: entry.plateColor,
                    maxPixelSize: Int(max(proxy.size.width, proxy.size.height) * scale),
                    holds: holds
                )
            case .cover:
                CoverFill(entry: entry, sharpCover: sharpCover)
            case .compact:
                Color.clear
            }
        }
    }
}

/// The ground rising over the foot of a title's wide art, where a long title wraps up onto it:
/// opaque where it meets the ground, and still 70% of it `held` points above, at the top of the
/// first line's letters, which keeps silk text above 4.5:1 even over a pure white banner. Then
/// gone by `height`, low and light, so the art looks as it did; the title's shadow does the rest.
struct BannerScrim: View {
    var held: CGFloat = 34
    var height: CGFloat = 76

    var body: some View {
        // Stops by their distance above the foot, eased out from the held part to nothing.
        let fade = height - held
        let stops: [(above: CGFloat, opacity: Double)] = [
            (height, 0), (held + fade * 0.5, 0.28), (held, 0.7), (0, 1),
        ]
        LinearGradient(
            stops: stops.map { .init(color: Palette.ground.opacity($0.opacity), location: 1 - $0.above / height) },
            startPoint: .top, endPoint: .bottom
        )
        .frame(height: height)
        .allowsHitTesting(false)
    }
}

extension View {
    /// A title's own soft edge of the ground, so its letters hold on light or busy art.
    func titleOverArt() -> some View {
        shadow(color: Palette.ground.opacity(0.75), radius: 5, y: 1)
    }
}

/// A title's cover at its own 2:3, rounded and edged: what stands beside the title where there is
/// no wide art.
struct CoverArt: View {
    @Environment(\.displayScale) private var scale
    let entry: Entry
    let width: CGFloat

    var body: some View {
        ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: Int(width * 1.5 * scale))
            .frame(width: width, height: (width * 3 / 2).rounded())
            .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: width)))
            .overlay(
                RoundedRectangle(cornerRadius: ArtShape.coverRadius(width: width))
                    .strokeBorder(Palette.glassEdge.opacity(1.25), lineWidth: 1)
            )
    }
}
