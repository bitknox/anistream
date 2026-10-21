// The Obi & Silk palette, from the native design brief. Dark first: every board is drawn on the
// ground colour, and the app shows itself in the dark appearance.

import SwiftUI
#if os(macOS)
import AppKit

private extension NSColor {
    convenience init(rgb: UInt32) {
        self.init(srgbRed: CGFloat((rgb >> 16) & 0xFF) / 255, green: CGFloat((rgb >> 8) & 0xFF) / 255,
                  blue: CGFloat(rgb & 0xFF) / 255, alpha: 1)
    }
}
#else
import UIKit

private extension UIColor {
    convenience init(rgb: UInt32) {
        self.init(red: CGFloat((rgb >> 16) & 0xFF) / 255, green: CGFloat((rgb >> 8) & 0xFF) / 255,
                  blue: CGFloat(rgb & 0xFF) / 255, alpha: 1)
    }
}
#endif

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

    /// A colour that is `rgb`, and `increased` when the viewer has asked for more contrast
    /// (Increase Contrast): the system says which as it draws, so it changes in place.
    public init(rgb: UInt32, increased: UInt32) {
        #if os(macOS)
        self.init(nsColor: NSColor(name: nil) { appearance in
            let high: [NSAppearance.Name] = [.accessibilityHighContrastDarkAqua, .accessibilityHighContrastAqua]
            let matched = appearance.bestMatch(from: [.darkAqua, .aqua] + high)
            return NSColor(rgb: matched.map(high.contains) == true ? increased : rgb)
        })
        #else
        self.init(uiColor: UIColor { traits in
            UIColor(rgb: traits.accessibilityContrast == .high ? increased : rgb)
        })
        #endif
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
    /// Where a related or recommended cover, a character's art or an unaired still has not come:
    /// flat, until the engine gives those a colour of their own.
    public static let artPlaceholder = Color(rgb: 0x1F2440)
    /// The segmented control's selection, and a secondary button on a raised surface.
    public static let raisedSelected = Color(rgb: 0x343A57)
    /// A progress track. Lifted to #5C6073 under Increase Contrast.
    public static let rule = Color(rgb: 0x434653, increased: 0x5C6073)
    /// Hairlines between rows, and a secondary button's edge on the ground. Lifted to the rule's
    /// own colour under Increase Contrast.
    public static let faintRule = Color(rgb: 0x2F3242, increased: 0x434653)
    /// 絹. Primary text and icons.
    public static let silk = Color(rgb: 0xE4E0D6)
    /// 霞. Secondary text. Lifted to #B8BDD6 under Increase Contrast.
    public static let haze = Color(rgb: 0x9298B5, increased: 0xB8BDD6)
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

/// What sits over video or art: the ground at `opacity`, or solid Raised when the viewer has asked
/// for less transparency (Reduce Transparency), read where it is drawn.
public struct OverVideoFill: ShapeStyle {
    let opacity: Double

    public func resolve(in environment: EnvironmentValues) -> Color {
        environment.accessibilityReduceTransparency ? Palette.raised : Palette.ground.opacity(opacity)
    }
}

/// Glass chrome's tint, or solid Raised under Reduce Transparency.
public struct GlassFill: ShapeStyle {
    public func resolve(in environment: EnvironmentValues) -> Color {
        environment.accessibilityReduceTransparency ? Palette.raised : Palette.glass
    }
}

extension ShapeStyle where Self == OverVideoFill {
    /// The ground at `opacity` over video, solid Raised under Reduce Transparency.
    public static func overVideo(_ opacity: Double) -> OverVideoFill { OverVideoFill(opacity: opacity) }
}

extension ShapeStyle where Self == GlassFill {
    /// Glass chrome's fill, solid Raised under Reduce Transparency.
    public static var glassFill: GlassFill { GlassFill() }
}

extension View {
    /// Glass chrome in `shape`: the system's glass tinted the brief's colour, or solid Raised
    /// under Reduce Transparency, where the notice capsule and the palette turn opaque.
    public func chromeGlass(in shape: some Shape) -> some View {
        modifier(ChromeGlass(shape: AnyShape(shape)))
    }
}

struct ChromeGlass: ViewModifier {
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    let shape: AnyShape

    func body(content: Content) -> some View {
        if reduceTransparency {
            content.background(Palette.raised, in: shape)
        } else {
            content.glassEffect(.regular.tint(Palette.glass), in: shape)
        }
    }
}
