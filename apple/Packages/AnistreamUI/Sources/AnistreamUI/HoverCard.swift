// The Mac's hover card, as logic: where a card sits, and when the pointer opens, hands off and
// closes one (the hover card board: timing, placement and neighbours). Pure, so the tests can play
// pointer paths through it; `HoverCards` runs it against the clock and the screen.

import AnistreamKit
import CoreGraphics
import Foundation

/// A cover the card can grow from, where it is in the layer's space.
struct HoverCover: Equatable {
    let id: AnilistId
    let rect: CGRect
}

/// The card's measurements (the board's Measurements).
enum HoverCardSize {
    static let width: CGFloat = 384
    /// The video or art band at the top, 16:9 and above YouTube's 356 × 200 floor.
    static let band: CGFloat = 216
    /// How far the card stays inside the content area.
    static let inset: CGFloat = 8
    /// The scrim over the band's foot.
    static let scrim: CGFloat = 88
    static let toggle: CGFloat = 28
    /// The toggle's distance from the band's trailing edge and foot.
    static let toggleInset: CGFloat = 12
    /// What stays with the open card round the toggle, never a flank: a pointer slowing onto the
    /// toggle comes to rest within about this of it. The action and the captions sit under the
    /// band, which is all a flank covers.
    static let toggleMargin: CGFloat = 24
    /// The synopsis's two lines at 19.
    static let synopsis: CGFloat = 38
    static let action: CGFloat = 28
    static let gap: CGFloat = 12
    static let foot: CGFloat = 16

    /// The card's height: 322 with the synopsis and the action, 282 without the action.
    static func height(synopsis: Bool, action: Bool) -> CGFloat {
        let parts = [synopsis ? Self.synopsis : nil, action ? Self.action : nil].compactMap { $0 }
        guard !parts.isEmpty else { return band }
        return band + parts.reduce(0) { $0 + gap + $1 } + foot
    }

    /// The card for `cover`: centred on it, then moved inward to stay `inset` inside `area`.
    static func frame(over cover: CGRect, height: CGFloat, in area: CGRect) -> CGRect {
        let x = min(max(cover.midX - width / 2, area.minX + inset), area.maxX - inset - width)
        let y = min(max(cover.midY - height / 2, area.minY + inset), area.maxY - inset - height)
        return CGRect(x: x, y: y, width: width, height: height)
    }

    /// The sound toggle in a card at `frame`, with the margin that stays with the card.
    static func toggleZone(in frame: CGRect) -> CGRect {
        let reach = toggle + toggleMargin * 2
        return CGRect(
            x: frame.minX + width - toggleInset - toggle - toggleMargin,
            y: frame.minY + band - toggleInset - toggle - toggleMargin,
            width: reach, height: reach
        )
    }
}

/// What the screen knows of the grid this instant: the covers on screen, the area the card stays
/// inside, and how tall each title's card is.
struct HoverLayout {
    var covers: [HoverCover]
    var area: CGRect
    var height: (AnilistId) -> CGFloat = { _ in HoverCardSize.height(synopsis: true, action: true) }
    /// Whether the title's card has a sound toggle, whose corner is never a flank.
    var toggle: (AnilistId) -> Bool = { _ in true }

    func cover(at point: CGPoint) -> HoverCover? {
        covers.first { $0.rect.contains(point) }
    }

    /// `id`'s cover, while it is on screen: the one nearest `card` when the title shows in more
    /// than one row, as Search's shelves can show it.
    func cover(_ id: AnilistId, near card: CGRect? = nil) -> HoverCover? {
        let all = covers.filter { $0.id == id }
        guard let card, all.count > 1 else { return all.first }
        func distance(_ cover: HoverCover) -> CGFloat {
            hypot(cover.rect.midX - card.midX, cover.rect.midY - card.midY)
        }
        return all.min { distance($0) < distance($1) }
    }

    /// The card `cover` grows into.
    func frame(of cover: HoverCover) -> CGRect {
        HoverCardSize.frame(over: cover.rect, height: height(cover.id), in: area)
    }

    /// The card `id`'s cover grows into, while the cover is on screen.
    func frame(_ id: AnilistId, near card: CGRect? = nil) -> CGRect? {
        cover(id, near: card).map(frame(of:))
    }

    /// The covers either side of `cover` in its row, the nearest each way within one cover's width.
    func neighbours(of cover: HoverCover) -> [HoverCover] {
        let row = covers.filter { $0 != cover && abs($0.rect.minY - cover.rect.minY) < 1 }
        let reach = cover.rect.width
        let left = row.filter { $0.rect.maxX <= cover.rect.minX + 0.5 && cover.rect.minX - $0.rect.maxX < reach }
            .max { $0.rect.minX < $1.rect.minX }
        let right = row.filter { $0.rect.minX >= cover.rect.maxX - 0.5 && $0.rect.minX - cover.rect.maxX < reach }
            .min { $0.rect.minX < $1.rect.minX }
        return [left, right].compactMap { $0 }
    }

    /// The flanks of `id`'s card at `frame`: the band's area over each neighbour's cover column.
    func flanks(of id: AnilistId, frame: CGRect) -> [(id: AnilistId, rect: CGRect, cover: HoverCover)] {
        guard let cover = cover(id, near: frame) else { return [] }
        return neighbours(of: cover).compactMap { neighbour in
            let x0 = max(neighbour.rect.minX, frame.minX), x1 = min(neighbour.rect.maxX, frame.maxX)
            guard x1 > x0 else { return nil }
            return (neighbour.id, CGRect(x: x0, y: frame.minY, width: x1 - x0, height: HoverCardSize.band), neighbour)
        }
    }
}

/// When the pointer opens, hands off and closes a card. Times are seconds on any one clock.
///
/// - A cover cues its trailer once the pointer has been on it 150 ms and opens its card at 450 ms,
///   each only while the pointer has slowed: under 12 pt of travel in the last 100 ms. Moves
///   inside the cover do not restart the wait.
/// - The card's hit area is its grown frame from its first frame. Leaving it closes it at once,
///   and the next cover waits the full 450 ms from there.
/// - A 150 ms rest in a flank hands the card to that neighbour; 450 ms when the pointer came in
///   heading down, for the details and the action.
/// - A scroll, a key press or the window losing key status closes it, and nothing opens again
///   until the pointer moves, so covers sliding under a still pointer stay closed and a card does
///   not come back under the keys.
struct HoverIntent {
    static let cue: Double = 0.15
    static let dwell: Double = 0.45
    static let handOff: Double = 0.15
    static let window: Double = 0.1
    static let slowTravel: CGFloat = 12

    enum Event: Equatable {
        /// Cue the title's trailer, hidden.
        case cue(AnilistId)
        /// The pointer left the cued cover before its card opened: let its trailer go.
        case uncue(AnilistId)
        /// Open the title's card, at the frame.
        case open(AnilistId, CGRect)
        /// Slide the card to the neighbour's frame.
        case handOff(AnilistId, CGRect)
        /// The card's frame changed where it is, as when its title's details land.
        case moved(CGRect)
        case close
    }

    private enum Target: Equatable {
        case card
        case cover(AnilistId)
    }

    private struct Sample {
        let time: Double
        let point: CGPoint
    }

    /// The open card's title and frame.
    private(set) var open: (id: AnilistId, frame: CGRect)?
    /// Whether nothing opens until the pointer moves: after a scroll.
    private(set) var suppressed = false

    private var pointer: CGPoint?
    private var trail: [Sample] = []
    private var target: Target?
    private var enteredAt: Double = 0
    private var cued: AnilistId?
    private var flank: (cover: HoverCover, since: Double, down: Bool)?

    /// The pointer is at `point`, or has left the grid.
    mutating func move(_ point: CGPoint?, at time: Double) {
        guard let point else {
            pointer = nil
            trail = []
            return
        }
        if let pointer, hypot(point.x - pointer.x, point.y - pointer.y) >= 1 { suppressed = false }
        if pointer == nil { suppressed = false }
        pointer = point
        trail.append(Sample(time: time, point: point))
        prune(time)
    }

    /// A scroll, a key press, the window losing key status, a sheet or the player: the card
    /// closes, and with `holding` nothing opens again until the pointer moves.
    mutating func dismiss(holding: Bool = true) -> [Event] {
        if holding { suppressed = true }
        guard open != nil else {
            // A cue with no card yet: let its trailer go.
            guard let cued else { return [] }
            self.cued = nil
            return [.uncue(cued)]
        }
        open = nil
        flank = nil
        cued = nil
        return [.close]
    }

    /// Whether the pointer has travelled under `slowTravel` in the last `window`.
    private mutating func slowed(_ time: Double) -> Bool {
        prune(time)
        var travel: CGFloat = 0
        for (a, b) in zip(trail, trail.dropFirst()) {
            travel += hypot(b.point.x - a.point.x, b.point.y - a.point.y)
        }
        return travel < Self.slowTravel
    }

    private mutating func prune(_ time: Double) {
        while trail.count > 1, trail[0].time < time - Self.window { trail.removeFirst() }
        if let first = trail.first, first.time < time - Self.window, trail.count == 1 {
            // A lone sample older than the window is a pointer at rest: no travel either way.
            trail = [Sample(time: time, point: first.point)]
        }
    }

    /// Where things stand at `time`.
    mutating func step(at time: Double, layout: HoverLayout) -> [Event] {
        var events: [Event] = []
        let hit = pointer.flatMap { layout.cover(at: $0) }
        let now: Target? = pointer.flatMap { point in
            if let open, open.frame.contains(point) { return .card }
            return hit.map { .cover($0.id) }
        }
        if now != target {
            target = now
            enteredAt = time
        }

        if let current = open {
            guard let cover = layout.cover(current.id, near: current.frame) else {
                // Its cover went, as when the grid changed under it.
                return dismiss(holding: false)
            }
            if case let frame = layout.frame(of: cover), frame != current.frame {
                open = (current.id, frame)
                events.append(.moved(frame))
            }
            if now == .card {
                if let handed = flankStep(time, layout: layout) { events.append(handed) }
                return events
            }
            events += dismiss(holding: false)
        }

        if let cued, now != .cover(cued) {
            self.cued = nil
            events.append(.uncue(cued))
        }
        guard case .cover(let id) = now, !suppressed else { return events }
        let on = time - enteredAt
        guard on >= Self.cue, slowed(time) else { return events }
        if cued != id {
            cued = id
            events.append(.cue(id))
        }
        if on >= Self.dwell, let hit {
            let frame = layout.frame(of: hit)
            open = (id, frame)
            events.append(.open(id, frame))
        }
        return events
    }

    /// In the open card: a rest in a flank hands it over.
    private mutating func flankStep(_ time: Double, layout: HoverLayout) -> Event? {
        guard let open, let pointer else { return nil }
        let inToggle = layout.toggle(open.id) && HoverCardSize.toggleZone(in: open.frame).contains(pointer)
        let here = inToggle ? nil : layout.flanks(of: open.id, frame: open.frame).first { $0.rect.contains(pointer) }?.cover
        if here != flank?.cover {
            flank = here.map { cover in
                // Heading down on the way in: for the details and the action, so the full wait.
                prune(time)
                let from = trail.first?.point ?? pointer
                let dx = pointer.x - from.x, dy = pointer.y - from.y
                return (cover, time, dy > 2 && dy > abs(dx))
            }
        }
        guard let flank, time - flank.since >= (flank.down ? Self.dwell : Self.handOff), slowed(time)
        else { return nil }
        let frame = layout.frame(of: flank.cover)
        self.flank = nil
        self.open = (flank.cover.id, frame)
        cued = flank.cover.id
        // The pointer is now on the neighbour's card, and its wait starts again if it leaves.
        return .handOff(flank.cover.id, frame)
    }
}
