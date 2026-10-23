// The player is the size its window gives it, whatever the art it shows: a banner as wide as
// AniList's (4.75:1) once laid the Mac player out 3420 pt wide in a 1512 pt screen.

#if os(macOS)
import AppKit
import CoreGraphics
import SwiftUI
import Testing

@testable import AnistreamPlayerUI

@MainActor
struct LayoutTests {
    /// A banner of AniList's shape, 1900 × 400.
    static let banner: CGImage = {
        let context = CGContext(
            data: nil, width: 1900, height: 400, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.setFillColor(CGColor(red: 0.2, green: 0.3, blue: 0.6, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: 1900, height: 400))
        return context.makeImage()!
    }()

    func size(of view: some View, in proposal: CGSize) -> CGSize {
        NSHostingController(rootView: view).sizeThatFits(in: proposal)
    }

    @Test func aWideBannerFillsThePlayerWithoutWideningIt() {
        let player = CGSize(width: 1280, height: 720)
        // What the starting screen did: the art's own size, filled, became the player's.
        let filled = size(of: ZStack { Color.black; Image(decorative: Self.banner, scale: 1).resizable().scaledToFill() }, in: player)
        #expect(filled.width > 3000, "the old way is the bug: \(filled)")
        // What it does now.
        #expect(size(of: ZStack { Color.black; FilledArt(image: Self.banner) }, in: player) == player)
        let phone = CGSize(width: 874, height: 402)
        #expect(size(of: ZStack { Color.black; FilledArt(image: Self.banner) }, in: phone) == phone)
    }

    /// A list of `rows` menu rows in a scroll view, as the Mac's panels hold them.
    func panel(rows: Int) -> some View {
        MenuPanelFrame {
            ScrollView {
                VStack(spacing: 0) { ForEach(0..<rows, id: \.self) { _ in Color.clear.frame(height: 24) } }
            }
        }
        .frame(width: 264)
    }

    /// A subtitle menu of 21 rows (Off and 20 tracks) once ran past its panel, over the title
    /// strip and under the control row: the list took its rows' height whatever the window's.
    @Test func aLongMenuFitsTheRoomItIsGiven() {
        #expect(size(of: panel(rows: 21), in: CGSize(width: 264, height: 300)).height == 300)
        #expect(size(of: panel(rows: 21), in: CGSize(width: 264, height: 900)).height == MenuPanelFrame.cap)
        // A short one is its rows' height.
        #expect(size(of: panel(rows: 3), in: CGSize(width: 264, height: 900)).height == 72)
    }

    /// The fade shows at an edge only while there are rows past it.
    @Test func aMenuFadesOnlyWhereItScrollsOn() {
        #expect(ScrollRoom(top: 0, bottom: 420, height: 600) == { var r = ScrollRoom(); r.below = true; return r }())
        #expect(ScrollRoom(top: 180, bottom: 600, height: 600) == { var r = ScrollRoom(); r.above = true; return r }())
        let middle = ScrollRoom(top: 90, bottom: 510, height: 600)
        #expect(middle.above && middle.below)
        let all = ScrollRoom(top: 0, bottom: 72, height: 72)
        #expect(!all.above && !all.below)
    }

    @Test func aMenuPanelIsItsRowsUpToTheRoomAndTheCap() {
        #expect(MenuPanelFrame.height(rows: 1000, room: 240) == 240)
        #expect(MenuPanelFrame.height(rows: 1000, room: 2000) == MenuPanelFrame.cap)
        #expect(MenuPanelFrame.height(rows: 1000, room: nil) == MenuPanelFrame.cap)
        #expect(MenuPanelFrame.height(rows: 60, room: 240) == 60)
        // In a window too short for more, still a few rows.
        #expect(MenuPanelFrame.height(rows: 500, room: 20) == MenuPanelFrame.least)
    }
}
#endif
