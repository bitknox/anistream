// The hover preview on the Mac: rest the pointer on a cover and a popover opens over it with the
// trailer, the title, its facts, two lines of synopsis and the title's main action. The video sits
// flush at the popover's top; nothing frames it (Trailers board, Hover preview).

#if os(macOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import AppKit
import SwiftUI

/// What the previews share: one open at a time, and when the last closed.
@MainActor @Observable
final class PreviewCenter {
    static let shared = PreviewCenter()

    /// The title whose preview is open.
    private(set) var open: AnilistId?
    @ObservationIgnored private var closedAt: ContinuousClock.Instant?

    /// How long a pointer rests before the next preview opens: the first waits, one for the cover
    /// next to it, soon after a close, does not wait as long.
    var dwell: Duration {
        if let closedAt, ContinuousClock.now - closedAt < .seconds(1) { return .milliseconds(300) }
        return .milliseconds(800)
    }

    func opened(_ id: AnilistId) { open = id }

    func closed(_ id: AnilistId) {
        guard open == id else { return }
        open = nil
        closedAt = .now
    }

    /// Pointer travel, in points, under which it is resting.
    static let rest: CGFloat = 4
    static let grace: Duration = .milliseconds(150)
    static let size = CGSize(width: 384, height: 216)
}

/// A cover that opens a preview when the pointer rests on it.
struct TrailerPreviewHost: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    @State private var showing = false
    @State private var overCover = false
    @State private var overPreview = false
    @State private var anchor: CGPoint?
    @State private var dwell: Task<Void, Never>?
    @State private var closing: Task<Void, Never>?

    func body(content: Content) -> some View {
        content
            .onContinuousHover { phase in
                switch phase {
                case .active(let point):
                    overCover = true
                    closing?.cancel()
                    if let anchor, hypot(point.x - anchor.x, point.y - anchor.y) < PreviewCenter.rest { return }
                    anchor = point
                    dwell?.cancel()
                    guard !showing else { return }
                    dwell = Task {
                        try? await Task.sleep(for: PreviewCenter.shared.dwell)
                        guard !Task.isCancelled, overCover, navigator.trailerRequest == nil else { return }
                        showing = true
                    }
                case .ended:
                    overCover = false
                    anchor = nil
                    dwell?.cancel()
                    scheduleClose()
                }
            }
            .popover(isPresented: $showing, arrowEdge: .trailing) {
                TrailerPreview(entry: entry) { inside in
                    overPreview = inside
                    if inside { closing?.cancel() } else { scheduleClose() }
                } close: {
                    showing = false
                }
            }
            .onChange(of: showing) { _, now in
                if now { PreviewCenter.shared.opened(entry.id) } else { PreviewCenter.shared.closed(entry.id) }
            }
            .onChange(of: PreviewCenter.shared.open) { _, id in
                // One at a time: another cover's preview closes this one.
                if let id, id != entry.id { showing = false }
            }
            .task(id: store.trailersMayAutoplay) {
                // The first grid of covers loads the preview's player, so the first hover starts sooner.
                guard store.trailersMayAutoplay else { return }
                TrailerPool.shared.prewarm(subtitles: store.settings.value(.subtitles) ?? "")
            }
            .task {
                // The Fixtures app can open one at launch, for a shot: sheet=preview, select=<id>.
                let route = LaunchRoute.current
                if route.sheet == "preview", route.selected == entry.id {
                    try? await Task.sleep(for: .seconds(1))
                    showing = true
                }
            }
    }

    private func scheduleClose() {
        guard showing else { return }
        closing?.cancel()
        closing = Task {
            try? await Task.sleep(for: PreviewCenter.grace)
            guard !Task.isCancelled, !overCover, !overPreview else { return }
            showing = false
        }
    }
}

extension View {
    func trailerPreview(_ entry: Entry) -> some View { modifier(TrailerPreviewHost(entry: entry)) }
}

/// The popover's content.
struct TrailerPreview: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let hover: (Bool) -> Void
    let close: () -> Void

    @State private var visit = TrailerVisit()
    @State private var monitor: Any?

    var body: some View {
        let entry = store.titles.page(entry.id)
        let art = store.heroArt(entry)
        let mayPlay = store.trailerMayAutoplay(entry)
        let action = TitleAction(entry, handedOff: navigator.handedOff, episodes: store.titles.episodes[entry.id])
        VStack(alignment: .leading, spacing: 0) {
            ZStack {
                if mayPlay { TrailerVideo(visit: visit, size: PreviewCenter.size, fillsWidth: true) }
                Group {
                    if art.isCompact {
                        Palette.raised.overlay { CoverArt(entry: entry, width: 96).padding(.vertical, 12) }
                    } else {
                        HeroArtView(art: art, entry: entry)
                    }
                }
                .opacity(visit.shown ? 0 : 1)
                .animation(visit.shown ? .reveal : .fadeOut, value: visit.shown)
            }
            .frame(width: PreviewCenter.size.width, height: PreviewCenter.size.height)
            .clipped()
            .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(entry.showTitle)
                        .font(.macShowTitle(size: 17))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    Spacer(minLength: 0)
                    if mayPlay { TrailerSoundButton(visit: visit, size: 24) }
                }
                if let facts = entry.factsLine() {
                    Text(facts).font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                }
                if !entry.synopsis.isEmpty {
                    Text(entry.synopsis)
                        .font(.mac(13))
                        .macLineHeight(18)
                        .foregroundStyle(.haze)
                        .lineLimit(2)
                        .padding(.top, 2)
                }
                if action.playable {
                    Button {
                        close()
                        action.play(entry.id, store: store, navigator: navigator)
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: "play.fill").font(.mac(10))
                            Text(action.title)
                        }
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 30, fontSize: 13))
                    .padding(.top, 8)
                }
            }
            .padding(EdgeInsets(top: 12, leading: 16, bottom: 16, trailing: 16))
            .frame(width: PreviewCenter.size.width, alignment: .leading)
        }
        .onHover { hover($0) }
        .task(id: entry.id) {
            // The banner and the synopsis come with the title's detail, which a grid has not asked for.
            if !store.titles.detailed.contains(entry.id) { store.loadDetail(entry.id) }
            if store.titles.episodes[entry.id] == nil { store.loadEpisodes(entry.id) }
        }
        .task(id: entry.id) {
            guard mayPlay, let trailer = entry.trailer else { return }
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled else { return }
            let subtitles = store.settings.value(.subtitles) ?? ""
            visit.begin { signal in
                TrailerPlayer(trailer: trailer, style: .banner, subtitles: subtitles, onSignal: signal)
            }
        }
        .onAppear {
            // A scroll or a key press closes it at once.
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.scrollWheel, .keyDown]) { event in
                close()
                return event
            }
        }
        .onDisappear {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
            visit.release()
        }
    }
}
#endif
