// A title's wide art, drawn: the rule itself, `HeroArt`, is the store's, since the player's
// starting screen follows it too.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The wide art itself: the banner or the still filling the frame, faded in over the cover's
/// colour once it arrives (T8), or that colour alone while the episodes load. Nothing for
/// `.compact`; the screen leaves the frame out.
struct HeroArtView: View {
    @Environment(\.displayScale) private var scale
    let art: HeroArt
    let entry: Entry

    var body: some View {
        GeometryReader { proxy in
            switch art {
            case .banner(let url), .still(let url):
                ArtworkView(
                    url: url, color: entry.plateColor,
                    maxPixelSize: Int(max(proxy.size.width, proxy.size.height) * scale)
                )
            case .waiting:
                CoverPlate(color: entry.plateColor)
            case .compact:
                Color.clear
            }
        }
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
