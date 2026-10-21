// Pieces every screen uses: art over its plate, the meter, the buttons and the sync capsule.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// A cover or banner: the plate first, and the art fading in over it once it arrives.
struct ArtworkView: View {
    @Environment(AppStore.self) private var store
    let url: String?
    /// The cover's own colour, for the plate.
    var color: Color? = nil
    /// The most pixels the art needs on its long side.
    let maxPixelSize: Int

    var body: some View {
        let image = url.flatMap { store.artwork.image($0) }
        Color.clear
            .overlay {
                ZStack {
                    CoverPlate(color: color)
                    if let image {
                        Image(decorative: image, scale: 1)
                            .resizable()
                            .scaledToFill()
                            .transition(.opacity)
                    }
                }
                .animation(.fadeIn, value: image != nil)
            }
            .clipped()
            .task(id: url) {
                if let url { store.artwork.request(url, maxPixelSize: maxPixelSize) }
            }
    }
}

/// A small card's wide art, in a grid that needs every card's frame, as Calendar's and This
/// Season's are: the banner, else a seen still when the title's episodes are here already (see
/// `HeroArt`; a calendar asks for no episodes). Else the cover at its own aspect against the
/// leading edge of a plate in its colour: a portrait cover stretched across a banner's frame is
/// never the answer. The heroes drop the frame instead, with `HeroArt`.
struct BannerArt: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    let entry: Entry

    var body: some View {
        GeometryReader { proxy in
            let size = proxy.size
            let wide = entry.bannerUrl ?? store.titles.episodes[entry.id].flatMap {
                HeroArt.seenStill($0, done: entry.progress?.done ?? 0)
            }
            if let wide {
                ArtworkView(
                    url: wide, color: entry.plateColor,
                    maxPixelSize: Int(max(size.width, size.height) * scale)
                )
            } else {
                let inset = (size.height * 0.06).rounded()
                let height = max(size.height - inset * 2, 0)
                let width = (height * 2 / 3).rounded()
                ZStack(alignment: .leading) {
                    CoverPlate(color: entry.plateColor)
                    ArtworkView(
                        url: entry.coverUrl, color: entry.plateColor,
                        maxPixelSize: Int(height * scale)
                    )
                    .frame(width: width, height: height)
                    .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: width)))
                    .padding(.leading, inset)
                }
            }
        }
    }
}

/// Text cut to `lines`, with More to show the rest and Less to cut it again, only when there is
/// more to show.
struct ClampedText: View {
    let text: String
    var lines = 4
    @State private var expanded = false
    @State private var full: CGFloat = 0
    @State private var shown: CGFloat = 0

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(text)
                .lineLimit(expanded ? nil : lines)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { shown = $0 }
                .background(alignment: .topLeading) {
                    Text(text)
                        .fixedSize(horizontal: false, vertical: true)
                        .hidden()
                        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { full = $0 }
                }
            if expanded || full > shown + 1 {
                Button(expanded ? "Less" : "More") {
                    withAnimation(.standard) { expanded.toggle() }
                }
                .buttonStyle(.plain)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.silk)
            }
        }
    }
}

/// A progress meter: amber over the rule colour, rounded.
struct Meter: View {
    let fraction: Double
    var height: CGFloat = 3

    var body: some View {
        Capsule()
            .fill(.rule)
            .overlay(alignment: .leading) {
                GeometryReader { proxy in
                    Capsule()
                        .fill(.amber)
                        .frame(width: proxy.size.width * min(max(fraction, 0), 1))
                }
            }
            .frame(height: height)
            .animation(.standard, value: fraction)
            .accessibilityElement()
            .accessibilityLabel("Progress")
            .accessibilityValue(Text(fraction, format: .percent.precision(.fractionLength(0))))
    }
}

/// The one amber action on a screen: a capsule with ground-coloured text.
struct PrimaryCapsuleStyle: ButtonStyle {
    var height: CGFloat = 34
    var fontSize: CGFloat = 13

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: fontSize, weight: .semibold))
            .monospacedDigit()
            .foregroundStyle(.ground)
            .lineLimit(1)
            .padding(.horizontal, height < 34 ? 14 : 18)
            .frame(height: height)
            .background(.amber.opacity(configuration.isPressed ? 0.85 : 1), in: .capsule)
            .contentShape(.capsule)
    }
}

/// A secondary action: a raised capsule with a hairline edge on the ground, or the selected
/// raised colour on a raised surface.
struct SecondaryCapsuleStyle: ButtonStyle {
    var height: CGFloat = 28
    /// Whether it sits on a raised surface, such as the setup card.
    var raisedSurface = false
    var fontSize: CGFloat = 13

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: fontSize))
            .foregroundStyle(.silk)
            .lineLimit(1)
            .padding(.horizontal, 14)
            .frame(height: height)
            .background(background(pressed: configuration.isPressed), in: .capsule)
            .overlay(Capsule().strokeBorder(raisedSurface ? .clear : Palette.faintRule, lineWidth: 1))
            .contentShape(.capsule)
    }

    private func background(pressed: Bool) -> Color {
        if raisedSurface { return pressed ? Palette.rule : Palette.raisedSelected }
        return pressed ? Palette.raisedSelected : Palette.raised
    }
}

/// The dot a state is shown with.
struct StateDot: View {
    let state: SyncSummary.State

    var body: some View {
        Group {
            switch state {
            case .signedOut:
                Circle().strokeBorder(.haze, lineWidth: 1.5)
            case .synced:
                Circle().fill(.water)
            case .waiting:
                Circle().fill(.haze)
            case .problem:
                Circle().fill(.vermilion)
            }
        }
        .frame(width: 6, height: 6)
        .accessibilityHidden(true)
    }
}

/// The toolbar's sync capsule: where sync stands, as a button that opens Settings, Accounts.
struct SyncCapsule: View {
    @Environment(AppStore.self) private var store
    let open: () -> Void

    var body: some View {
        let summary = store.accounts.summary
        Button(action: open) {
            HStack(spacing: 6) {
                StateDot(state: summary.state)
                Text(summary.text)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(summary.state == .signedOut ? .haze : .silk)
            }
            .padding(.horizontal, 4)
        }
        .accessibilityLabel(accessibilityLabel(summary))
        .help("Opens Accounts in Settings")
    }

    private func accessibilityLabel(_ summary: SyncSummary) -> String {
        let text = summary.state == .signedOut
            ? "\(AccountsModel.name(summary.tracker ?? "anilist")) not connected"
            : summary.text
        return "\(text). Opens Settings, Accounts"
    }
}

/// A section that has no screen yet.
struct NotBuiltYet: View {
    let section: AppSection

    var body: some View {
        Text("\(section.title) comes in a later build.")
            .font(.system(size: 13))
            .foregroundStyle(.haze)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
