// The Mac hover card's pointer logic: pointer paths played through it at the board's timings.

import AnistreamKit
@testable import AnistreamUI
import CoreGraphics
import Testing

struct HoverIntentTests {
    /// Eight covers a row on the board's 148 pt pitch, two rows, in a 1216 × 600 content area.
    let layout: HoverLayout = {
        var covers: [HoverCover] = []
        for row in 0..<2 {
            for column in 0..<8 {
                let id = AnilistId(900_100 + row * 8 + column)
                covers.append(HoverCover(id: id, rect: CGRect(x: 24 + column * 148, y: 20 + row * 280, width: 128, height: 192)))
            }
        }
        return HoverLayout(covers: covers, area: CGRect(x: 0, y: 0, width: 1216, height: 600))
    }()

    func centre(_ index: Int) -> CGPoint {
        let rect = layout.covers[index].rect
        return CGPoint(x: rect.midX, y: rect.midY)
    }

    /// Play the pointer resting at `point` from `from` to `to` seconds, stepping every 10 ms.
    func rest(_ intent: inout HoverIntent, at point: CGPoint, from: Double, to: Double) -> [(Double, HoverIntent.Event)] {
        var events: [(Double, HoverIntent.Event)] = []
        var time = from
        intent.move(point, at: time)
        while time <= to + 1e-9 {
            events += intent.step(at: time, layout: layout).map { (time, $0) }
            time += 0.01
        }
        return events
    }

    /// Move in a straight line over `seconds`, a sample every 10 ms.
    func glide(_ intent: inout HoverIntent, from a: CGPoint, to b: CGPoint, start: Double, seconds: Double) -> [(Double, HoverIntent.Event)] {
        var events: [(Double, HoverIntent.Event)] = []
        let steps = Int(seconds / 0.01)
        for step in 0...steps {
            let t = Double(step) / Double(steps)
            let time = start + Double(step) * 0.01
            intent.move(CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t), at: time)
            events += intent.step(at: time, layout: layout).map { (time, $0) }
        }
        return events
    }

    @Test func theCardSitsOnItsCoverInsideTheArea() {
        let middle = HoverCardSize.frame(over: layout.covers[3].rect, height: 322, in: layout.area)
        #expect(middle.midX == layout.covers[3].rect.midX)
        #expect(middle.size == CGSize(width: 384, height: 322))
        // At the leading edge and the window's foot it moves inward, 8 pt from each.
        #expect(HoverCardSize.frame(over: layout.covers[0].rect, height: 322, in: layout.area).minX == 8)
        let short = CGRect(x: 0, y: 0, width: 1216, height: 520)
        #expect(HoverCardSize.frame(over: layout.covers[15].rect, height: 322, in: short).maxY == 512)
        #expect(HoverCardSize.frame(over: layout.covers[7].rect, height: 322, in: layout.area).maxX == 1208)
        #expect(HoverCardSize.height(synopsis: true, action: true) == 322)
        #expect(HoverCardSize.height(synopsis: true, action: false) == 282)
    }

    @Test func aRestingPointerCuesAt150AndOpensAt450() throws {
        var intent = HoverIntent()
        let events = rest(&intent, at: centre(3), from: 0, to: 0.6)
        let cue = try #require(events.first { if case .cue = $0.1 { true } else { false } })
        let open = try #require(events.first { if case .open = $0.1 { true } else { false } })
        #expect(abs(cue.0 - 0.15) < 0.011)
        #expect(abs(open.0 - 0.45) < 0.011)
        #expect(events.count == 2)
        #expect(intent.open?.id == layout.covers[3].id)
    }

    @Test func aPassAcrossTheRowOpensNothing() {
        var intent = HoverIntent()
        // 300 pt/s, which is slow: never under 12 pt in 100 ms.
        let start = CGPoint(x: 10, y: centre(0).y), end = CGPoint(x: 1200, y: centre(0).y)
        let events = glide(&intent, from: start, to: end, start: 0, seconds: (end.x - start.x) / 300)
        #expect(events.isEmpty)
    }

    @Test func smallMovesInsideTheCoverDoNotRestartTheWait() throws {
        var intent = HoverIntent()
        var events = rest(&intent, at: centre(2), from: 0, to: 0.3)
        // A 6 pt nudge at 0.3 s: still slowed, and the wait counts from entering.
        events += glide(&intent, from: centre(2), to: CGPoint(x: centre(2).x + 6, y: centre(2).y), start: 0.31, seconds: 0.05)
        events += rest(&intent, at: CGPoint(x: centre(2).x + 6, y: centre(2).y), from: 0.37, to: 0.6)
        let open = try #require(events.first { if case .open = $0.1 { true } else { false } })
        #expect(open.0 < 0.5)
    }

    @Test func leavingTheCardClosesItAndTheNextCoverWaitsTheFullDwell() throws {
        var intent = HoverIntent()
        _ = rest(&intent, at: centre(3), from: 0, to: 0.5)
        let frame = try #require(intent.open?.frame)
        // Down past the card's foot onto the cover below it in the next row.
        let below = centre(11)
        #expect(!frame.contains(below))
        let events = rest(&intent, at: below, from: 0.6, to: 1.2)
        #expect(events.first?.1 == .close)
        #expect(events.first?.0 == 0.6)
        let open = try #require(events.first { if case .open = $0.1 { true } else { false } })
        #expect(open.0 >= 0.6 + 0.45 - 1e-9)
    }

    @Test func aRestInAFlankHandsTheCardToTheNeighbour() throws {
        var intent = HoverIntent()
        _ = rest(&intent, at: centre(3), from: 0, to: 0.5)
        let frame = try #require(intent.open?.frame)
        let flank = try #require(layout.flanks(of: layout.covers[3].id, frame: frame).first { $0.id == layout.covers[4].id })
        let into = CGPoint(x: flank.rect.midX, y: frame.minY + 60)
        var events = glide(&intent, from: centre(3), to: into, start: 0.5, seconds: 0.2)
        events += rest(&intent, at: into, from: 0.71, to: 1.1)
        let handed = try #require(events.first { if case .handOff = $0.1 { true } else { false } })
        // 150 ms of rest once slowed, from entering the flank.
        #expect(handed.0 < 1.0)
        #expect(intent.open?.id == layout.covers[4].id)
    }

    @Test func headingDownThroughAFlankWaitsTheFullDwell() throws {
        var intent = HoverIntent()
        _ = rest(&intent, at: centre(3), from: 0, to: 0.5)
        let frame = try #require(intent.open?.frame)
        let flank = try #require(layout.flanks(of: layout.covers[3].id, frame: frame).first { $0.id == layout.covers[4].id })
        // Up to the band's top over the open cover, then steeply down and across into the flank,
        // as on the way to the details.
        let top = CGPoint(x: layout.covers[3].rect.maxX - 4, y: frame.minY + 10)
        _ = glide(&intent, from: centre(3), to: top, start: 0.5, seconds: 0.1)
        let into = CGPoint(x: flank.rect.minX + 6, y: frame.minY + 120)
        var events = glide(&intent, from: top, to: into, start: 0.61, seconds: 0.1)
        events += rest(&intent, at: into, from: 0.72, to: 1.0)
        #expect(!events.contains { if case .handOff = $0.1 { true } else { false } })
        #expect(intent.open?.id == layout.covers[3].id)
        events = rest(&intent, at: into, from: 1.01, to: 1.4)
        #expect(events.contains { if case .handOff = $0.1 { true } else { false } })
    }

    @Test func theToggleCornerAndTheDetailsAreNeverAFlank() throws {
        var intent = HoverIntent()
        _ = rest(&intent, at: centre(3), from: 0, to: 0.5)
        let frame = try #require(intent.open?.frame)
        let toggle = HoverCardSize.toggleZone(in: frame)
        let corner = CGPoint(x: toggle.midX, y: toggle.midY)
        let details = CGPoint(x: frame.maxX - 30, y: frame.minY + 260)
        for point in [corner, details] {
            var copy = intent
            _ = glide(&copy, from: centre(3), to: point, start: 0.5, seconds: 0.2)
            let events = rest(&copy, at: point, from: 0.71, to: 1.5)
            #expect(events.isEmpty, "\(point)")
            #expect(copy.open?.id == layout.covers[3].id)
        }
    }

    @Test func aScrollClosesItAndNothingOpensUntilThePointerMoves() throws {
        var intent = HoverIntent()
        _ = rest(&intent, at: centre(3), from: 0, to: 0.5)
        #expect(intent.dismiss() == [.close])
        // The pointer stays put while the grid moves under it.
        var events = rest(&intent, at: centre(3), from: 0.5, to: 1.5)
        #expect(events.isEmpty)
        events = glide(&intent, from: centre(3), to: CGPoint(x: centre(3).x + 4, y: centre(3).y), start: 1.5, seconds: 0.02)
        events += rest(&intent, at: CGPoint(x: centre(3).x + 4, y: centre(3).y), from: 1.53, to: 2.1)
        #expect(events.contains { if case .open = $0.1 { true } else { false } })
    }

    @Test func aTitleInTwoRowsOpensAndStaysOnTheCoverUnderThePointer() throws {
        // Search's shelves can show one title twice: the second row repeats the first row's fourth.
        var covers = layout.covers
        covers[12] = HoverCover(id: covers[3].id, rect: covers[12].rect)
        let twice = HoverLayout(covers: covers, area: layout.area)
        var intent = HoverIntent()
        let point = CGPoint(x: covers[12].rect.midX, y: covers[12].rect.midY)
        intent.move(point, at: 0)
        var events: [HoverIntent.Event] = []
        for step in 0...60 { events += intent.step(at: Double(step) * 0.01, layout: twice) }
        let frame = try #require(intent.open?.frame)
        #expect(frame == HoverCardSize.frame(over: covers[12].rect, height: 322, in: layout.area))
        #expect(events.filter { $0 == .close }.isEmpty, "it never closes and opens again")
    }

    @Test func leavingTheGridClosesIt() {
        var intent = HoverIntent()
        _ = rest(&intent, at: centre(3), from: 0, to: 0.5)
        intent.move(nil, at: 0.6)
        #expect(intent.step(at: 0.6, layout: layout) == [.close])
    }

    @Test func aCoverCropKeepsItsFocusInTheMiddle() {
        // The upper third of a 384 pt wide cover in the 216 pt band: the top 84 pt cut.
        #expect(CoverFillMode.cropTop(width: 384, height: 216, focus: 1 / 3) == -84)
        // Never past either end.
        #expect(CoverFillMode.cropTop(width: 384, height: 216, focus: 0) == 0)
        #expect(CoverFillMode.cropTop(width: 384, height: 216, focus: 1) == -360)
    }

    @Test func aCoverTooSmallToCropIsBlurred() {
        // A 460 px cover: crops in the band at 2x (1.67 times), blurs across a title page.
        #expect(CoverFillMode.of(pixels: 768, source: 460) == .crop)
        #expect(CoverFillMode.of(pixels: 2200, source: 460) == .blur)
        #expect(CoverFillMode.of(pixels: 768, source: 230) == .blur)
        #expect(CoverFillMode.of(pixels: 280, source: 460) == .crop)
        #expect(CoverFillMode.of(pixels: 768, source: nil) == .crop)
    }
}
