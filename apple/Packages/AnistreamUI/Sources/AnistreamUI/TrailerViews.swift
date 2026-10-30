// The trailer in a hover preview: the video under the art, and the sound button.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

#if os(macOS)
/// The video itself, which sits under the preview's art: the art fades away over it. The web view
/// takes no pointer or touch, and nothing is drawn over it.
struct TrailerVideo: View {
    let visit: TrailerVisit
    let size: CGSize
    /// Whether it takes the width it is given, edge to edge.
    var fillsWidth = false

    var body: some View {
        ZStack {
            if let player = visit.surface as? TrailerPlayer, let view = player.webView {
                TrailerWebView(view: view)
            }
        }
        .frame(width: fillsWidth ? nil : size.width, height: size.height)
        .frame(maxWidth: fillsWidth ? .infinity : nil)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// A plain icon button that turns the trailer's sound on or off, for this trailer and the next.
struct TrailerSoundButton: View {
    let visit: TrailerVisit
    var size: CGFloat

    var body: some View {
        Button {
            visit.toggleSound()
        } label: {
            Image(systemName: visit.muted ? "speaker.slash" : "speaker.wave.2")
                .font(.ui(size > 30 ? 17 : 13, weight: .medium))
                .foregroundStyle(.silk)
                .contentTransition(.symbolEffect(.replace))
                .frame(width: size, height: size)
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .animation(.quick, value: visit.muted)
        .help(L.titleTrailerSound)
        .accessibilityLabel(L.titleTrailerSound)
        .accessibilityAddTraits(visit.muted ? [] : .isSelected)
        .opacity(visit.shown ? 1 : 0)
        .allowsHitTesting(visit.shown)
        .animation(visit.shown ? .fadeIn : .fadeOut, value: visit.shown)
    }
}
#endif
