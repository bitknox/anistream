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

    @Test func thePlayerWindowOpensAt16By9WithinTheScreen() {
        // A MacBook's built-in display, 1512 × 982 pt, under its menu bar.
        let visible = CGRect(x: 0, y: 0, width: 1512, height: 945)
        let frame = PlayerWindowFrame.initial(in: visible)
        #expect(frame.width <= visible.width * 0.7 && frame.height <= visible.height * 0.7)
        #expect(abs(frame.width / frame.height - 16 / 9) < 0.01)
        #expect(visible.contains(frame))
        #expect(frame == CGRect(x: 227, y: 175, width: 1058, height: 595))
        // A large screen: the video's usual size.
        #expect(PlayerWindowFrame.initial(in: CGRect(x: 0, y: 0, width: 3008, height: 1667)).size == CGSize(width: 1280, height: 720))
        // Last time's frame, when it still fits; a 3420 pt one never does.
        #expect(PlayerWindowFrame.restored(CGRect(x: 100, y: 100, width: 800, height: 450), on: visible) != nil)
        #expect(PlayerWindowFrame.restored(CGRect(x: 0, y: 0, width: 3420, height: 720), on: visible) == nil)
    }
}
#endif
