// The Mac's hover card, on screen: one layer over a grid's scroll view that finds each cover from
// its anchor, follows the pointer, and grows the card over the cover the pointer rests on (the
// hover card board). The grid never reflows and nothing in the scroll view clips the card.

#if os(macOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import AppKit
import SwiftUI

/// The covers a grid shows, each title's frame, for the layer over the grid. A title can show
/// more than once, as in Search's shelves.
struct HoverCoverAnchors: PreferenceKey {
    static let defaultValue: [HoverCoverAnchor] = []

    static func reduce(value: inout [HoverCoverAnchor], nextValue: () -> [HoverCoverAnchor]) {
        value += nextValue()
    }
}

struct HoverCoverAnchor {
    let id: AnilistId
    let bounds: Anchor<CGRect>
}

extension View {
    /// A grid's cover the hover card can grow from, where a layer above asks for one: the cell's
    /// top, at 2:3 across its width. Rows of a list, past `.xxxLarge`, have no card.
    func hoverCardSource(_ entry: Entry) -> some View {
        modifier(HoverCardSource(entry: entry))
    }

    /// The hover card layer, over a scroll view of covers (Continue's, Seasonal, Search and the
    /// Library). `trailing` is what a panel at the trailing edge covers, as Search's filters do:
    /// the card stays clear of it, and a cover under it opens nothing.
    func hoverCards(trailing: CGFloat = 0) -> some View {
        modifier(HoverCardLayer(trailing: trailing))
    }
}

private struct HoverCardSource: ViewModifier {
    @Environment(\.posterAsRow) private var asRow
    let entry: Entry

    func body(content: Content) -> some View {
        content
            .anchorPreference(key: HoverCoverAnchors.self, value: .bounds) { bounds in
                asRow ? [] : [HoverCoverAnchor(id: entry.id, bounds: bounds)]
            }
            // What the card shows, for VoiceOver, which never opens it.
            .accessibilityCustomContent(
                AccessibilityCustomContentKey(Text(L.accessHoverFacts), id: "hover-facts"),
                entry.hoverFacts.isEmpty ? nil : Text(entry.hoverFacts)
            )
            .accessibilityCustomContent(
                AccessibilityCustomContentKey(Text(L.accessHoverSynopsis), id: "hover-synopsis"),
                entry.shownSynopsis.isEmpty ? nil : Text(entry.shownSynopsis)
            )
    }
}

/// The card that is open, and the pointer logic behind it, against the clock.
@MainActor @Observable
final class HoverCards {
    struct Card: Equatable {
        /// One per card opened: a hand-off keeps it, so the card slides rather than regrows.
        let session: Int
        var id: AnilistId
        /// The cover it grew from and collapses to.
        var cover: CGRect
        /// Where it lands: the hit area, from its first frame.
        var frame: CGRect
        var closing = false
        /// Whether it got here by a hand-off.
        var slid = false
    }

    private(set) var card: Card?
    /// Counts the pointer's moves, for the captions' rule: the title fades after 2 s of stillness.
    private(set) var moves = 0

    /// The grid as last laid out, in the layer's space.
    @ObservationIgnored var layout = HoverLayout(covers: [], area: .zero)
    /// Where the layer's space starts in the window.
    @ObservationIgnored var origin: CGPoint = .zero
    /// Whether a card may open: the window is key, no sheet or player is over it.
    @ObservationIgnored var allowed = true {
        didSet { if !allowed, oldValue { dismiss(holding: false) } }
    }
    /// Called when the pointer has slowed on a cover: cue its trailer, ask for its details.
    @ObservationIgnored var cue: (AnilistId) -> Void = { _ in }
    /// Called when the pointer left a cued cover before its card opened.
    @ObservationIgnored var uncue: (AnilistId) -> Void = { _ in }
    /// Called as a card opens, hands off, or goes.
    @ObservationIgnored var changed: (Card?) -> Void = { _ in }
    /// A scripted pointer drives the layer instead of the real one: the motion demo and shots.
    @ObservationIgnored var scripted = false

    @ObservationIgnored private var intent = HoverIntent()
    @ObservationIgnored private var ticker: Task<Void, Never>?
    @ObservationIgnored private var inside = false
    @ObservationIgnored private var sessions = 0
    /// The last scroll, key press or close: what the warm-up waits a still moment after.
    @ObservationIgnored private var stirred = ContinuousClock.now
    @ObservationIgnored private let start = ContinuousClock.now

    private var now: Double {
        let elapsed = ContinuousClock.now - start
        return Double(elapsed.components.seconds) + Double(elapsed.components.attoseconds) / 1e18
    }

    /// The real pointer, in the window's space, or nil when it left the grid.
    func hover(_ location: CGPoint?) {
        guard !scripted else { return }
        point(location.map { CGPoint(x: $0.x - origin.x, y: $0.y - origin.y) })
    }

    /// The pointer, in the layer's space.
    func point(_ point: CGPoint?) {
        intent.move(point, at: now)
        inside = point != nil
        if point != nil { moves &+= 1 }
        tick()
        run()
    }

    /// A scroll, a key press, the window losing key status: the card closes at once.
    func dismiss(holding: Bool = true) {
        stirred = .now
        apply(intent.dismiss(holding: holding))
    }

    /// Wait for a still moment: no card open or closing, and nothing scrolled for half a second.
    /// False when cancelled.
    func still() async -> Bool {
        while card != nil || ContinuousClock.now - stirred < .milliseconds(500) {
            try? await Task.sleep(for: .milliseconds(200))
            if Task.isCancelled { return false }
        }
        return !Task.isCancelled
    }

    private func run() {
        guard ticker == nil, inside || card != nil else { return }
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(16))
                guard let self else { return }
                self.tick()
                if !self.inside, self.card == nil { break }
            }
            self?.ticker = nil
        }
    }

    private func tick() {
        guard allowed else { return }
        apply(intent.step(at: now, layout: layout))
    }

    private func apply(_ events: [HoverIntent.Event]) {
        for event in events {
            switch event {
            case .cue(let id):
                cue(id)
            case .uncue(let id):
                uncue(id)
            case .open(let id, let frame):
                guard let cover = layout.cover(id, near: frame) else { continue }
                sessions += 1
                card = Card(session: sessions, id: id, cover: cover.rect, frame: frame)
                changed(card)
            case .handOff(let id, let frame):
                guard let cover = layout.cover(id, near: frame) else { continue }
                withAnimation(.quick) {
                    card?.id = id
                    card?.cover = cover.rect
                    card?.frame = frame
                    card?.slid = true
                }
                changed(card)
            case .moved(let frame):
                withAnimation(.standard) { card?.frame = frame }
            case .close:
                guard let closing = card, !closing.closing else { continue }
                withAnimation(.quick) { card?.closing = true }
                changed(nil)
                Task { [weak self] in
                    // The collapse's 0.2 s, and a frame for it to land.
                    try? await Task.sleep(for: .milliseconds(240))
                    guard let self, self.card?.session == closing.session, self.card?.closing == true else { return }
                    self.card = nil
                }
            }
        }
    }
}

/// The layer itself, over the scroll view it modifies.
private struct HoverCardLayer: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.controlActiveState) private var activeState
    @Environment(\.playerUp) private var playerUp
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dynamicTypeSize) private var typeSize

    /// What a panel at the trailing edge covers.
    let trailing: CGFloat

    @State private var cards = HoverCards()
    @State private var trailer = HoverTrailer.shared
    @State private var monitor: Any?

    private var allowed: Bool {
        let key = activeState == .key || cards.scripted
        let covered = playerUp || navigator.trailerRequest != nil || navigator.moreAbout != nil
            || navigator.sourcePicker != nil || navigator.showSettings != nil || navigator.messagesShown
        return store.hoverCardsOn && key && !covered && typeSize <= .xxxLarge
    }

    func body(content: Content) -> some View {
        content
            .overlayPreferenceValue(HoverCoverAnchors.self) { anchors in
                GeometryReader { proxy in
                    // The part no panel covers: the card stays inside it.
                    let visible = CGRect(x: 0, y: 0, width: max(0, proxy.size.width - trailing), height: proxy.size.height)
                    // Only the covers on screen: a lazy stack keeps some laid out past the edge.
                    let covers = anchors.map { anchor in
                        let cell = proxy[anchor.bounds]
                        return HoverCover(id: anchor.id, rect: CGRect(x: cell.minX, y: cell.minY, width: cell.width, height: cell.width * 3 / 2))
                    }
                    .filter { $0.rect.intersects(visible) }
                    let place = Place(covers: covers.sorted { ($0.rect.minY, $0.rect.minX) < ($1.rect.minY, $1.rect.minX) },
                                      area: visible, origin: proxy.frame(in: .global).origin)
                    ZStack(alignment: .topLeading) {
                        Color.clear
                        if let page = trailer.priming {
                            // The warm page while it is primed: in the window, so it plays, and
                            // never seen or heard (muted, and transparent until a card shows it).
                            TrailerWebView(view: page.webView)
                                .frame(width: HoverCardSize.width, height: HoverCardSize.band)
                                .opacity(0)
                                .allowsHitTesting(false)
                                .accessibilityHidden(true)
                        }
                        if let card = cards.card, let entry = store.titles[card.id] {
                            HoverCardView(card: card, entry: entry, cards: cards, trailer: trailer, reduceMotion: reduceMotion)
                                .id(card.session)
                        }
                    }
                    .frame(width: proxy.size.width, height: proxy.size.height, alignment: .topLeading)
                    .onChange(of: place, initial: true) { _, place in
                        cards.origin = place.origin
                        cards.layout = HoverLayout(
                            covers: place.covers,
                            area: place.area,
                            height: { [store, navigator] id in store.hoverCardHeight(id, handedOff: navigator.handedOff, onHome: navigator.section == .home) },
                            toggle: { [store] id in store.titles[id].map { store.trailerMayAutoplay($0) } ?? false }
                        )
                    }
                    .task(id: !place.covers.isEmpty && store.hoverCardsOn && store.trailersMayAutoplay) {
                        // The warm player loads once a grid that can show cards is here (decision 3),
                        // and only while cards are on and trailers may play: 2 s after, so the first
                        // screen has drawn and the engine has started, and at a still moment, as
                        // the run's first web view starts WebKit's processes on the main thread.
                        guard !place.covers.isEmpty, store.hoverCardsOn, store.trailersMayAutoplay else { return }
                        try? await Task.sleep(for: .seconds(2))
                        guard await cards.still(), store.hoverCardsOn, store.trailersMayAutoplay else { return }
                        let first = cards.layout.covers.lazy
                            .compactMap { store.titles[$0.id] }
                            .first { store.trailerMayAutoplay($0) }?.trailer
                        await trailer.warmUp(subtitles: store.settings.value(.subtitles) ?? "", first: first)
                    }
                    .task(id: place.covers.first?.id) {
                        guard place.covers.first != nil else { return }
                        await runScript()
                    }
                }
            }
            .onContinuousHover(coordinateSpace: .global) { phase in
                switch phase {
                case .active(let location): cards.hover(location)
                case .ended: cards.hover(nil)
                }
            }
            .onAppear {
                cards.cue = { [store, trailer] id in
                    guard let entry = store.titles[id] else { return }
                    if !store.titles.detailed.contains(id) { store.loadDetail(id) }
                    if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
                    if let url = entry.coverUrl, let image = store.artwork.image(url) {
                        CoverFocus.shared.prepare(url, image: image)
                    }
                    if store.trailerMayAutoplay(entry) { trailer.cue(entry, subtitles: store.settings.value(.subtitles) ?? "") }
                }
                cards.uncue = { [trailer] id in trailer.uncue(id) }
                cards.changed = { [store, trailer] card in
                    guard let card, let entry = store.titles[card.id], store.trailerMayAutoplay(entry) else {
                        trailer.stopSoon()
                        return
                    }
                    trailer.play(entry, subtitles: store.settings.value(.subtitles) ?? "")
                }
                cards.allowed = allowed
                monitor = NSEvent.addLocalMonitorForEvents(matching: [.scrollWheel, .keyDown]) { [cards] event in
                    cards.dismiss()
                    return event
                }
            }
            .onDisappear {
                if let monitor { NSEvent.removeMonitor(monitor) }
                monitor = nil
                cards.dismiss(holding: false)
                trailer.stop()
            }
            .onChange(of: allowed) { _, now in cards.allowed = now }
    }

    /// The grid as one value, so the layer hears of it only when it moved.
    private struct Place: Equatable {
        let covers: [HoverCover]
        let area: CGRect
        let origin: CGPoint
    }

    /// The scripted pointer, when the launch asked for one: rest on a cover for a shot
    /// (sheet=preview, select=<id>), or the motion demo's path (-AnistreamMotion hover).
    private func runScript() async {
        let route = LaunchRoute.current
        let shot = route.sheet == "preview"
        let demo = MotionDemo.from() == .hover
        guard shot || demo, !HoverCardScript.ran else { return }
        // The demo starts once the launch has settled, which can be after the grid appears: its
        // frames count from then.
        for _ in 0..<300 where demo && !HoverCardScript.requested {
            try? await Task.sleep(for: .milliseconds(50))
        }
        try? await Task.sleep(for: .milliseconds(600))
        // Other sections' grids stay mounted out of sight: the one the script is for has the
        // selected cover, or a row of three for the demo's path.
        let covers = cards.layout.covers
        let mine = shot
            ? route.selected.map { id in covers.contains { $0.id == id } } ?? !covers.isEmpty
            : HoverCardScript.row(in: covers).count >= 3
        guard !Task.isCancelled, mine, !HoverCardScript.ran else { return }
        HoverCardScript.ran = true
        cards.scripted = true
        cards.allowed = allowed
        if shot {
            let cover = covers.first { $0.id == route.selected } ?? covers[min(1, covers.count - 1)]
            cards.point(CGPoint(x: cover.rect.midX, y: cover.rect.midY))
            return
        }
        await HoverCardScript.play(cards: cards)
    }
}

/// The motion demo's pointer path, through the same logic the pointer uses.
@MainActor
enum HoverCardScript {
    /// Set by `-AnistreamMotion hover`, for the layer on screen to run.
    static var requested = false
    /// Whether a layer has run it: once a launch.
    static var ran = false

    /// Rest on the second cover of the top row until its card has grown and its trailer shown,
    /// move onto the band over the next cover and rest there for the hand-off, then leave the card
    /// downward, and later pass quickly across the row, which opens nothing.
    static func play(cards: HoverCards) async {
        let row = row(in: cards.layout.covers)
        guard row.count >= 3 else { return }
        let first = row[1], next = row[2]
        let start = CGPoint(x: first.rect.minX - 10, y: first.rect.minY + 40)
        await glide(cards, from: start, to: CGPoint(x: first.rect.midX, y: first.rect.midY), over: 0.3)
        try? await Task.sleep(for: .milliseconds(2600))
        // Onto the band over the next cover, sideways, and rest: 150 ms hands the card over.
        let frame = cards.card?.frame ?? first.rect
        let flank = CGPoint(x: (next.rect.minX + min(next.rect.maxX, frame.maxX)) / 2, y: frame.minY + 90)
        await glide(cards, from: CGPoint(x: first.rect.midX, y: first.rect.midY), to: flank, over: 0.25)
        try? await Task.sleep(for: .milliseconds(2600))
        // Down and out of the card: it collapses at once.
        let below = CGPoint(x: flank.x, y: (cards.card?.frame.maxY ?? frame.maxY) + 30)
        await glide(cards, from: flank, to: below, over: 0.3)
        try? await Task.sleep(for: .milliseconds(900))
        cards.point(nil)
    }

    /// The first row of covers.
    static func row(in covers: [HoverCover]) -> [HoverCover] {
        guard let first = covers.first else { return [] }
        return covers.filter { abs($0.rect.minY - first.rect.minY) < 1 }
    }

    private static func glide(_ cards: HoverCards, from: CGPoint, to: CGPoint, over seconds: Double) async {
        let steps = max(1, Int(seconds * 60))
        for step in 0...steps {
            let t = Double(step) / Double(steps)
            let eased = t * t * (3 - 2 * t)
            cards.point(CGPoint(x: from.x + (to.x - from.x) * eased, y: from.y + (to.y - from.y) * eased))
            try? await Task.sleep(for: .milliseconds(16))
        }
    }
}

extension AppStore {
    /// How tall `id`'s card is: room for the synopsis unless its details are here without one, and
    /// for the action when there is an episode to play.
    @MainActor
    func hoverCardHeight(_ id: AnilistId, handedOff: HandedOff?, onHome: Bool = false) -> CGFloat {
        guard let entry = titles[id] else { return HoverCardSize.height(synopsis: true, action: true) }
        let reason = onHome && HomeHover(self, id)?.line != nil
        let synopsis = !(titles.detailed.contains(id) && entry.shownSynopsis.isEmpty)
        let action = TitleAction(entry, handedOff: handedOff, episodes: titles.episodes[id]).playable
            || (onHome && HomeHover(self, id)?.action == .addToPlanning)
        return HoverCardSize.height(synopsis: synopsis, action: action, reason: reason)
    }
}
#endif
