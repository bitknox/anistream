// The Mac's hover card, drawn: the cover growing into the card, the art or video band with the
// title over its foot, then the synopsis and the one amber action (the hover card board's At rest
// and Motion studies).

#if os(macOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// One card, from the moment it opens until it has collapsed. The hit area is its grown frame from
/// the first frame; what is drawn grows into it from the cover with `standard`, and collapses back
/// with `quick`. Under Reduce Motion it crossfades in place instead.
struct HoverCardView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let card: HoverCards.Card
    let entry: Entry
    let cards: HoverCards
    let trailer: HoverTrailer
    let reduceMotion: Bool

    @State private var grown = false
    /// The banner, still or blur over the growing cover: `fade-in` from the grow's start.
    @State private var artShown = false
    /// The title, facts, synopsis and action: `fade-in`, 100 ms in.
    @State private var detailsShown = false
    /// Whether the grow has ended: the video never fades in before.
    @State private var settled = false
    /// The title, facts, scrim and toggle over the band: they fade while captions show and the
    /// pointer is still.
    @State private var chrome = true

    var body: some View {
        let open = !card.closing
        // What is drawn: the cover's frame, growing to the card's, and back.
        let drawn = reduceMotion ? card.frame : (grown && open ? card.frame : card.cover)
        let layout = reduceMotion ? true : grown && open
        let opacity: Double = reduceMotion ? (grown && open ? 1 : 0) : (open ? 1 : 0)
        ZStack(alignment: .topLeading) {
            Color.clear
            HoverCardFace(
                entry: entry, grown: layout, size: drawn.size,
                artShown: artShown || card.slid, detailsShown: detailsShown || card.slid,
                videoShown: settled && trailer.id == entry.id && trailer.shown,
                chrome: chrome, trailer: trailer
            )
            .frame(width: drawn.width, height: drawn.height, alignment: .topLeading)
            .offset(x: drawn.minX - card.frame.minX, y: drawn.minY - card.frame.minY)
            .opacity(opacity)
        }
        .frame(width: card.frame.width, height: card.frame.height, alignment: .topLeading)
        // One click anywhere on it opens the title page; its buttons take their own.
        .contentShape(.rect)
        .onTapGesture { navigator.openTitle(entry.id) }
        .contextMenu { TitleMenu(entry: entry) }
        .pointingHand()
        .allowsHitTesting(open)
        // The cover is the element VoiceOver reads, with the card's details; with VoiceOver on
        // no trailer plays, so nothing here is new.
        .accessibilityHidden(true)
        .offset(x: card.frame.minX, y: card.frame.minY)
        .onAppear {
            withAnimation(.standard) { grown = true }
            withAnimation(.fadeIn) { artShown = true }
            withAnimation(.fadeIn.delay(0.1)) { detailsShown = true }
        }
        .task {
            try? await Task.sleep(for: .milliseconds(300))
            settled = true
        }
        .task(id: Chrome(captions: captions, moves: cards.moves)) {
            // With captions on screen, 2 s of stillness lets them read alone; a move brings the
            // title and toggle back.
            if !chrome { withAnimation(.fadeIn) { chrome = true } }
            guard captions else { return }
            try? await Task.sleep(for: .seconds(2))
            guard !Task.isCancelled else { return }
            withAnimation(.fadeOut) { chrome = false }
        }
    }

    /// Whether the trailer's captions are showing: with its sound on, over the video.
    private var captions: Bool {
        trailer.id == entry.id && trailer.shown && !trailer.muted && trailer.captions
    }

    private struct Chrome: Hashable {
        let captions: Bool
        let moves: Int
    }
}

/// The card's surface and what is on it, at `size`, laid out grown or as the cover.
private struct HoverCardFace: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let grown: Bool
    let size: CGSize
    let artShown: Bool
    let detailsShown: Bool
    let videoShown: Bool
    let chrome: Bool
    let trailer: HoverTrailer

    var body: some View {
        let radius: CGFloat = grown ? 12 : ArtShape.coverRadius(width: size.width)
        ZStack(alignment: .topLeading) {
            Palette.raised
            // A title handed over crossfades its art, title and details over the last one's.
            HoverCardContent(
                entry: entry, grown: grown, width: size.width, artShown: artShown,
                detailsShown: detailsShown, videoShown: videoShown, chrome: chrome, trailer: trailer
            )
            .id(entry.id)
            .transition(.opacity.animation(.fadeIn))
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading)
        .clipShape(.rect(cornerRadius: radius))
        .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(Palette.silk.opacity(grown ? 0.08 : 0), lineWidth: 1))
        // The overlay elevation the canvas gives menus: 0 12 32 at 45%.
        .shadow(color: Color(red: 6 / 255, green: 7 / 255, blue: 13 / 255).opacity(grown ? 0.45 : 0), radius: 16, y: 12)
    }
}

/// One title's band and details.
private struct HoverCardContent: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.displayScale) private var scale
    let entry: Entry
    let grown: Bool
    let width: CGFloat
    let artShown: Bool
    let detailsShown: Bool
    let videoShown: Bool
    let chrome: Bool
    let trailer: HoverTrailer

    private var bandHeight: CGFloat { grown ? HoverCardSize.band : width * 3 / 2 }
    /// Where the 384 × 216 band sits in the frame drawn now, centred.
    private var bandOffset: CGSize {
        CGSize(width: (width - HoverCardSize.width) / 2, height: (bandHeight - HoverCardSize.band) / 2)
    }

    var body: some View {
        let mayPlay = store.trailerMayAutoplay(entry)
        ZStack(alignment: .topLeading) {
            ZStack(alignment: .topLeading) {
                // The video, under the art until its first played frame.
                if mayPlay, trailer.id == entry.id {
                    TrailerVideo(visit: trailer.visit, size: CGSize(width: HoverCardSize.width, height: HoverCardSize.band))
                        .offset(bandOffset)
                }
                art
                    .opacity(videoShown ? 0 : 1)
                    .animation(videoShown ? .reveal : .fadeOut, value: videoShown)
                overBand(mayPlay: mayPlay)
                    .frame(width: HoverCardSize.width, height: HoverCardSize.band)
                    .offset(bandOffset)
                    .opacity(detailsShown && chrome ? 1 : 0)
            }
            .frame(width: width, height: bandHeight, alignment: .topLeading)
            .clipped()

            details
                .frame(width: HoverCardSize.width, alignment: .topLeading)
                .offset(x: bandOffset.width, y: bandHeight)
                .opacity(detailsShown ? 1 : 0)
        }
        .frame(width: width, alignment: .topLeading)
    }

    // MARK: The band's art

    /// The art by the No wide art rule: the banner or a seen still over the growing cover, else
    /// the cover itself cropped around its focus, or blurred behind itself where it is too small.
    @ViewBuilder
    private var art: some View {
        let hero = store.heroArt(entry)
        ZStack(alignment: .topLeading) {
            Palette.raised
            if let url = entry.coverUrl {
                let mode = CoverFillMode.of(pixels: HoverCardSize.width * scale, source: store.artwork.sourceWidth(url))
                if hero.url == nil, mode == .blur {
                    BlurredCoverGround(entry: entry, size: CGSize(width: HoverCardSize.width, height: HoverCardSize.band))
                        .offset(bandOffset)
                        .opacity(artShown ? 1 : 0)
                    sharpCover(url)
                } else {
                    growingCover(url)
                }
            }
            if let wide = hero.url {
                ArtworkView(url: wide, color: entry.plateColor, maxPixelSize: Int(HoverCardSize.width * scale))
                    .frame(width: HoverCardSize.width, height: HoverCardSize.band)
                    .offset(bandOffset)
                    .opacity(artShown ? 1 : 0)
            }
        }
    }

    /// The grid's cover growing into the band's crop: the same picture from the first frame.
    private func growingCover(_ url: String) -> some View {
        let focus = CoverFocus.shared.focus(url) ?? CoverFocus.fallback
        let top = CoverFillMode.cropTop(width: HoverCardSize.width, height: HoverCardSize.band, focus: focus)
        return ArtworkView(url: url, color: entry.plateColor, maxPixelSize: Int(HoverCardSize.width * 1.5 * scale))
            .frame(width: width, height: width * 3 / 2)
            .offset(y: grown ? top : 0)
    }

    /// The grid's cover growing into the sharp cover on the blur.
    private func sharpCover(_ url: String) -> some View {
        let band = CGSize(width: HoverCardSize.width, height: HoverCardSize.band)
        let frame = BlurredCover.sharpFrame(in: band)
        let coverWidth = grown ? frame.width : width
        return SharpCover(entry: entry, width: coverWidth, radius: ArtShape.coverRadius(width: 128))
            .offset(x: grown ? frame.minX : 0, y: grown ? frame.minY : 0)
    }

    // MARK: Over the band

    /// The scrim, the title and facts at the foot's leading corner, and the sound toggle at its
    /// trailing one, which comes with the video.
    private func overBand(mayPlay: Bool) -> some View {
        ZStack(alignment: .bottomLeading) {
            LinearGradient(
                stops: [
                    .init(color: Palette.ground.opacity(0), location: 0),
                    .init(color: Palette.ground.opacity(0.5), location: 0.5),
                    .init(color: Palette.ground.opacity(0.8), location: 1),
                ],
                startPoint: .top, endPoint: .bottom
            )
            .frame(height: HoverCardSize.scrim)
            .allowsHitTesting(false)

            VStack(alignment: .leading, spacing: 2) {
                Text(entry.showTitle)
                    .font(.macShowTitle(size: 15))
                    .macLineHeight(20)
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                // Dropped when it does not fit on its line beside the toggle.
                ViewThatFits(in: .horizontal) {
                    Text(entry.hoverFacts)
                        .font(.mac(11, weight: .medium))
                        .tracking(11 * 0.08)
                        .textCase(.uppercase)
                        .macLineHeight(16)
                        .foregroundStyle(Palette.silk.opacity(0.78))
                        .lineLimit(1)
                        .fixedSize()
                    Color.clear.frame(height: 0)
                }
            }
            .padding(EdgeInsets(top: 0, leading: 16, bottom: 12, trailing: 56))
            .frame(width: HoverCardSize.width, alignment: .leading)
            .allowsHitTesting(false)

            if mayPlay {
                HoverSoundToggle(trailer: trailer, shown: videoShown)
                    .padding(HoverCardSize.toggleInset)
                    .frame(width: HoverCardSize.width, alignment: .trailing)
            }
        }
        .frame(width: HoverCardSize.width, height: HoverCardSize.band, alignment: .bottomLeading)
    }

    // MARK: Under the band

    private var details: some View {
        let action = TitleAction(entry, handedOff: navigator.handedOff, episodes: store.titles.episodes[entry.id])
        let synopsis = entry.shownSynopsis
        let reserves = !(store.titles.detailed.contains(entry.id) && synopsis.isEmpty)
        return VStack(alignment: .leading, spacing: HoverCardSize.gap) {
            if reserves {
                Text(synopsis.isEmpty ? " " : synopsis)
                    .font(.mac(13))
                    .macLineHeight(19)
                    .foregroundStyle(.haze)
                    .lineLimit(2, reservesSpace: true)
                    .frame(height: HoverCardSize.synopsis, alignment: .topLeading)
            }
            if action.playable {
                Button {
                    action.play(entry.id, store: store, navigator: navigator)
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "play.fill").font(.mac(10))
                        Text(action.cardTitle)
                    }
                }
                .buttonStyle(PrimaryCapsuleStyle(height: HoverCardSize.action, fontSize: 13))
                .focusable(false)
            }
        }
        .padding(EdgeInsets(top: HoverCardSize.gap, leading: 16, bottom: HoverCardSize.foot, trailing: 16))
    }
}

/// The trailer's sound, on or off for every trailer: a slashed speaker on over-video glass, or a
/// speaker with waves on the raised selected fill. Never amber. It comes with the video.
private struct HoverSoundToggle: View {
    let trailer: HoverTrailer
    let shown: Bool

    var body: some View {
        let on = !trailer.muted
        Button {
            trailer.toggleSound()
        } label: {
            ZStack {
                Circle().fill(Palette.ground.opacity(0.62))
                Circle().fill(Palette.raisedSelected).opacity(on ? 1 : 0)
                Circle().strokeBorder(Palette.silk.opacity(0.16), lineWidth: 1)
                Image(systemName: "speaker.slash").opacity(on ? 0 : 1)
                Image(systemName: "speaker.wave.2").opacity(on ? 1 : 0)
            }
            .font(.mac(12, weight: .medium))
            .foregroundStyle(.silk)
            .frame(width: HoverCardSize.toggle, height: HoverCardSize.toggle)
            .contentShape(.circle)
            .animation(.easeOut(duration: 0.15), value: on)
        }
        .buttonStyle(.plain)
        .focusable(false)
        .help(L.titleTrailerSound)
        .accessibilityLabel(L.titleTrailerSound)
        .accessibilityValue(on ? L.valueOn : L.valueOff)
        .opacity(shown ? 1 : 0)
        .allowsHitTesting(shown)
        .animation(shown ? .fadeIn : .fadeOut, value: shown)
    }
}

extension TitleAction {
    /// The card's words: "Resume E3 · 12:04", "Play E1" (D7).
    var cardTitle: String {
        if let resume { return L.continueRowResume(episode: Int(episode), clock: resume.clock) }
        return L.continueRowStart(episode: Int(episode))
    }
}
#endif
