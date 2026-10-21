// Nagare 流, the mascot: a hand-authored pixel sprite, 20 by 32, the same one the website draws
// (website/src/data/nagare.ts). Shown in empty states, on first launch and in About; never in
// navigation, the player, or beside a show's art. Static, on the plain ground, always at a whole
// number of points per pixel so the pixels stay crisp.

import SwiftUI

/// The sprite's pixels and colours.
public enum NagareSprite {
    public static let width = 20
    public static let height = 32

    /// A row per line, a character per pixel, `.` for none.
    public static let rows: [String] = [
        "........hH..........",
        ".........Hh.........",
        "........hH..........",
        "......hHHHHh........",
        ".....hHHHHHHh.......",
        "....hHHHHHHHHh......",
        "....HHHHHHHHHHh.....",
        "...hHFFFFhHHHHh.....",
        "...HFFFFFFhHHHh.....",
        "...HFEFFFFhHHHh.....",
        "...hFFFFFfhHHh......",
        ".ww.fFFFfhHHh.......",
        ".wWWAAAAAAHHh.......",
        ".wWaAAAAAAa..aAA....",
        "..w.aAAAa....aAAa...",
        "..SRCCCCCCc.........",
        "..SRCCCCCCCc........",
        "..SRCCCCCCCc........",
        "..SfFCCCCCCCc.......",
        "..SRCCCCCCCCc.......",
        "..SRCCCCCCCCc.......",
        "..SRCCCCCCCCcc......",
        "..SRCCCCCCCCCc......",
        "..SRCCCCCCCCCc......",
        "..SRCCCCCCCCCcc.....",
        "..SRCCCCCCCCCCc.....",
        "..SRCCCCCCCCCCc.....",
        "..SRCCCCCCCCCCcc....",
        "..SRCCCCCCCCCCCc....",
        "..S.BB......BB......",
        "....BB......BB......",
        "...BBB.....BBB......",
    ]

    /// Each character's colour.
    public static let colors: [Character: UInt32] = [
        "H": 0xE8E2D4, "h": 0xC6BEA9, "F": 0xEFCFAC, "f": 0xD0A883, "E": 0x232842,
        "C": 0x464E7C, "c": 0x2E345C, "R": 0x68729E, "A": 0xF2A64B, "a": 0xC57C2C,
        "S": 0x8A6844, "W": 0x7FC7D9, "w": 0xDFF5FB, "B": 0x232842,
    ]

    public static let accessibilityLabel =
        "Nagare, the anistream mascot: a figure with a silk bob, an amber scarf and an indigo cloak, holding a staff with a water charm"
}

/// Nagare at `cell` points per pixel: 4, 5, 6 or 8 on the boards, so 80 by 128 points to 160 by
/// 256.
public struct Nagare: View {
    public let cell: Int

    public init(cell: Int = 6) {
        self.cell = max(1, cell)
    }

    public var body: some View {
        let cell = CGFloat(self.cell)
        Canvas(rendersAsynchronously: false) { context, _ in
            for (y, row) in NagareSprite.rows.enumerated() {
                for (x, pixel) in row.enumerated() {
                    guard let rgb = NagareSprite.colors[pixel] else { continue }
                    let rect = CGRect(x: CGFloat(x) * cell, y: CGFloat(y) * cell, width: cell, height: cell)
                    context.fill(Path(rect), with: .color(Color(rgb: rgb)))
                }
            }
        }
        .frame(width: CGFloat(NagareSprite.width) * cell, height: CGFloat(NagareSprite.height) * cell)
        .accessibilityElement()
        .accessibilityLabel(NagareSprite.accessibilityLabel)
        .accessibilityAddTraits(.isImage)
    }
}

// A PreviewProvider rather than #Preview, whose macro only Xcode has: the package builds with
// the Command Line Tools alone too.
struct NagarePreviews: PreviewProvider {
    static var previews: some View {
        HStack(alignment: .bottom, spacing: 24) {
            Nagare(cell: 4)
            Nagare(cell: 6)
            Nagare(cell: 8)
        }
        .padding(24)
        .background(.ground)
    }
}
