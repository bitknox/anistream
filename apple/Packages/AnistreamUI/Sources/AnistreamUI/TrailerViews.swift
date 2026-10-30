// The trailer on a title page: its place in the banner, the sound button under it, and when a
// visit starts and lets go.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// The measurements of a title page that may play a trailer (Trailers board, Mac, iPad, iPhone).
enum TrailerLayout {
    /// The video, 16:9 and above YouTube's 356 × 200 floor.
    static let wide = CGSize(width: 448, height: 252)

    #if os(macOS)
    /// The banner band, from the window's top edge.
    static let band: CGFloat = 328
    /// Where the video starts under the toolbar.
    static let videoTop: CGFloat = 60
    static let soundSize: CGFloat = 28
    #else
    /// 16 pt taller than the Mac's so the title, which starts at the cover's top, clears the video.
    static let band: CGFloat = 366
    static let videoTop: CGFloat = 82
    static let soundSize: CGFloat = 44
    #endif
    /// How far the cover and head overlap the band's foot.
    static let overlap: CGFloat = 88

    /// The iPhone's banner when a trailer may play, and the video across it below Back and More.
    static let phoneBand: CGFloat = 334
    static let phoneVideo = CGSize(width: 390, height: 220)
    static let phoneVideoTop: CGFloat = 106

    /// How far the page moves before the trailer gives way to the banner.
    static let scrollAway: CGFloat = 8
}

/// The video itself, which sits under the banner art: the art fades away over it. The web view
/// takes no pointer or touch, and nothing is drawn over it.
struct TrailerVideo: View {
    let visit: TrailerVisit
    var size = TrailerLayout.wide
    /// Whether it takes the width it is given, edge to edge, as on the iPhone.
    var fillsWidth = false

    var body: some View {
        ZStack {
            if let player = visit.surface as? TrailerPlayer {
                TrailerWebView(player: player)
            }
        }
        .frame(width: fillsWidth ? nil : size.width, height: size.height)
        .frame(maxWidth: fillsWidth ? .infinity : nil)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

/// A plain icon button that turns the trailer's sound on or off. Off at every start.
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

/// When a page's visit starts and when it lets go (Behaviour 2, 8 and 9 of the trailers spec).
struct TrailerLifecycle: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.playerUp) private var playerUp
    #if os(macOS)
    @Environment(\.controlActiveState) private var activeState
    #endif

    let visit: TrailerVisit
    let entry: Entry
    let mayPlay: Bool
    /// The style of page: the banner's.
    var settle: Duration = .milliseconds(900)

    private var sheetOpen: Bool {
        navigator.trailerRequest != nil || navigator.moreAbout != nil || navigator.sourcePicker != nil
            || navigator.showSettings != nil || navigator.messagesShown
    }

    func body(content: Content) -> some View {
        content
            .task(id: Start(id: entry.id, mayPlay: mayPlay)) {
                guard mayPlay, let trailer = entry.trailer else {
                    visit.release()
                    return
                }
                try? await Task.sleep(for: settle)
                guard !Task.isCancelled, visit.surface == nil, !playerUp, !sheetOpen else { return }
                visit.begin { signal in
                    TrailerPlayer(trailer: trailer, style: .banner, onSignal: signal)
                }
            }
            .task {
                // The Fixtures app opens Play trailer at launch for a shot: sheet=trailer.
                guard navigator.launchSheet == "trailer" else { return }
                navigator.launchSheet = nil
                try? await Task.sleep(for: .seconds(2))
                navigator.playTrailer(store.titles.page(entry.id), store: store)
            }
            .onDisappear { visit.release() }
            .onChange(of: scenePhase) { _, phase in if phase != .active { visit.release() } }
            .onChange(of: playerUp) { _, up in if up { visit.release() } }
            .onChange(of: sheetOpen) { _, open in if open { visit.release() } }
            #if os(macOS)
            .onChange(of: activeState == .inactive) { _, inactive in if inactive, !TrailerStandIn.keepsWhenInactive { visit.release() } }
            #endif
    }

    private struct Start: Hashable {
        let id: AnilistId
        let mayPlay: Bool
    }
}

extension View {
    func trailerLifecycle(visit: TrailerVisit, entry: Entry, mayPlay: Bool) -> some View {
        modifier(TrailerLifecycle(visit: visit, entry: entry, mayPlay: mayPlay))
    }
}
