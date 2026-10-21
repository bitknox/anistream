// The Obi & Silk palette, from the native design brief. Dark first: every board is drawn on the
// ground colour, and the app shows itself in the dark appearance.

import SwiftUI

extension Color {
    /// A colour from `0xRRGGBB`, in sRGB.
    public init(rgb: UInt32, opacity: Double = 1) {
        self.init(
            .sRGB,
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255,
            opacity: opacity
        )
    }

    /// A colour from `#rrggbb`, as AniList writes a cover's colour, or `nil` for anything else.
    public init?(hex: String) {
        var digits = Substring(hex.trimmingCharacters(in: .whitespaces))
        if digits.hasPrefix("#") { digits = digits.dropFirst() }
        guard digits.count == 6, let value = UInt32(digits, radix: 16) else { return nil }
        self.init(rgb: value)
    }
}

/// The tokens, by the brief's names. Amber is the only accent: one primary action per screen,
/// progress, the current selection and switches that are on. Water marks states, vermilion
/// errors, and neither is ever text on a selected row.
public enum Palette {
    /// 藍. The window and every screen.
    public static let ground = Color(rgb: 0x161A2E)
    /// Secondary buttons, the segmented track, grouped rows.
    public static let raised = Color(rgb: 0x222638)
    /// The segmented control's selection, and a secondary button on a raised surface.
    public static let raisedSelected = Color(rgb: 0x343A57)
    /// A progress track.
    public static let rule = Color(rgb: 0x434653)
    /// Hairlines between rows, and a secondary button's edge on the ground.
    public static let faintRule = Color(rgb: 0x2F3242)
    /// 絹. Primary text and icons.
    public static let silk = Color(rgb: 0xE4E0D6)
    /// 霞. Secondary text.
    public static let haze = Color(rgb: 0x9298B5)
    /// 琥珀. The accent.
    public static let amber = Color(rgb: 0xF2A64B)
    /// 水. States: a new episode, airing, synced, downloading.
    public static let water = Color(rgb: 0x7FC7D9)
    /// 朱. Errors only.
    public static let vermilion = Color(rgb: 0xE06B54)

    /// Glass chrome over the ground: the sidebar, toolbars, the tab bar, floating controls.
    public static let glass = Color(rgb: 0x282D44, opacity: 0.72)
    /// The inside edge glass chrome draws over the ground.
    public static let glassEdge = Color(rgb: 0xE4E0D6, opacity: 0.08)
    /// Glass over video or art, which needs more of the ground behind it.
    public static let glassOverArt = Color(rgb: 0x161A2E, opacity: 0.68)
    /// The inside edge of glass over video or art.
    public static let glassOverArtEdge = Color(rgb: 0xE4E0D6, opacity: 0.12)
    /// The selection tint, under an amber label and icon.
    public static let selection = Color(rgb: 0xF2A64B, opacity: 0.14)
    /// A separator inside a menu or on glass.
    public static let glassRule = Color(rgb: 0xE4E0D6, opacity: 0.12)
    /// The dim player controls sit on: 35% of the ground.
    public static let controlsDim = Color(rgb: 0x161A2E, opacity: 0.35)
}

extension ShapeStyle where Self == Color {
    public static var ground: Color { Palette.ground }
    public static var raised: Color { Palette.raised }
    public static var raisedSelected: Color { Palette.raisedSelected }
    public static var rule: Color { Palette.rule }
    public static var faintRule: Color { Palette.faintRule }
    public static var silk: Color { Palette.silk }
    public static var haze: Color { Palette.haze }
    public static var amber: Color { Palette.amber }
    public static var water: Color { Palette.water }
    public static var vermilion: Color { Palette.vermilion }
}
