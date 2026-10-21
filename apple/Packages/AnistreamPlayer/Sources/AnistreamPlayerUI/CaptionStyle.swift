// The system's caption style, for the subtitles that have no style of their own.
//
// SRT, WebVTT and the other plain-text kinds take their look from mpv's `sub-*` options. With
// "Subtitles follow your system caption style" on (Settings › Playback, on by default), those
// come from the style the viewer set for the whole system: Settings › Accessibility › Subtitles &
// Captioning on iPhone and iPad, System Settings › Accessibility › Captions on the Mac, read
// through MediaAccessibility. Font, size, colours, the background behind the text, the box behind
// all of it, and the edge.
//
// ASS subtitles keep the look their authors gave them. mpv applies the `sub-*` style options to
// them only with `sub-ass-override` at `force`; the player leaves it at mpv's default, `scale`,
// under which ASS styling stands and `sub-pos` still lifts ASS dialogue above the controls (`no`
// would stop that too).

import CoreGraphics
import CoreText
import Foundation
import MediaAccessibility

/// A caption style, and the mpv options that draw plain-text subtitles in it.
public struct CaptionStyle: Equatable, Sendable {
    /// A colour with its opacity, each 0 to 1.
    public struct Colour: Equatable, Sendable {
        public var red: Double
        public var green: Double
        public var blue: Double
        public var alpha: Double

        public init(red: Double, green: Double, blue: Double, alpha: Double) {
            self.red = red
            self.green = green
            self.blue = blue
            self.alpha = alpha
        }

        public static let black = Colour(red: 0, green: 0, blue: 0, alpha: 1)
        public static let white = Colour(red: 1, green: 1, blue: 1, alpha: 1)

        /// mpv's form: `#AARRGGBB`.
        public var mpv: String {
            func byte(_ value: Double) -> String { String(format: "%02X", Int((min(1, max(0, value)) * 255).rounded())) }
            return "#" + byte(alpha) + byte(red) + byte(green) + byte(blue)
        }

        func with(alpha: Double) -> Colour { Colour(red: red, green: green, blue: blue, alpha: alpha) }
    }

    /// How the letters' edges are drawn.
    public enum Edge: Equatable, Sendable {
        case none, raised, depressed, uniform, dropShadow
    }

    /// The font's family; nil for the system's own interface font, which libass cannot find by
    /// name, and then the player's own default stays.
    public var font: String?
    /// The text's size against the normal one: 0.5 to 2.
    public var size: Double
    public var text: Colour
    /// Behind each line's letters.
    public var background: Colour
    /// One box behind all the lines.
    public var window: Colour
    public var edge: Edge

    public init(font: String?, size: Double, text: Colour, background: Colour, window: Colour, edge: Edge) {
        self.font = font
        self.size = size
        self.text = text
        self.background = background
        self.window = window
        self.edge = edge
    }

    /// mpv's own look for plain-text subtitles, where the style does not follow the system.
    public static let mpvDefaults: [String: String] = [
        "sub-font-size": "38",
        "sub-color": "#FFFFFFFF",
        "sub-border-style": "outline-and-shadow",
        "sub-outline-size": "1.65",
        "sub-outline-color": "#FF000000",
        "sub-shadow-offset": "0",
        "sub-back-color": "#AF000000",
    ]

    /// The options that draw plain-text subtitles in this style. `font` is what `sub-font` is
    /// when the style names none: the player's own default.
    public func mpvOptions(defaultFont: String) -> [String: String] {
        var options: [String: String] = [
            "sub-font": font ?? defaultFont,
            // mpv's 38 is the size at a 720-line window, as its own scaling goes.
            "sub-font-size": String(format: "%.1f", 38 * min(2, max(0.5, size))),
            "sub-color": text.mpv,
        ]
        // The outline and the shadow for the edge; a box below, where the style has one.
        var outline = 0.0
        var outlineColour = Colour.black
        var shadow = 0.0
        var shadowColour = Colour.black.with(alpha: 0.75)
        switch edge {
        case .none:
            break
        case .uniform:
            outline = 1.65
        case .dropShadow:
            shadow = 2
        case .raised:
            // Lit from above: a dark edge below and to the right.
            shadow = 1
        case .depressed:
            // Pressed in: a light edge below and to the right.
            shadow = 1
            shadowColour = Colour.white.with(alpha: 0.6)
        }
        if background.alpha > 0 {
            // A box tight around each line, in the background's colour: libass's opaque box is
            // drawn by the outline, so the edge gives way to it.
            options["sub-border-style"] = "opaque-box"
            outline = 3
            outlineColour = background
            shadow = 0
        } else if window.alpha > 0 {
            // One box behind all the lines, the shadow's offset its margin; the outline as the
            // edge has it.
            options["sub-border-style"] = "background-box"
            shadow = 6
            shadowColour = window
        } else {
            options["sub-border-style"] = "outline-and-shadow"
        }
        options["sub-outline-size"] = String(format: "%.2f", outline)
        options["sub-outline-color"] = outlineColour.mpv
        options["sub-shadow-offset"] = String(format: "%.2f", shadow)
        options["sub-back-color"] = shadowColour.mpv
        return options
    }
}

extension CaptionStyle {
    /// The style the viewer set for the whole system, now.
    public static func system() -> CaptionStyle {
        let domain = MACaptionAppearanceDomain.user
        let descriptor = MACaptionAppearanceCopyFontDescriptorForStyle(domain, nil, .default).takeRetainedValue()
        let family = CTFontDescriptorCopyAttribute(descriptor, kCTFontFamilyNameAttribute) as? String
        func colour(_ color: CGColor, _ opacity: CGFloat) -> Colour {
            let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
            let c = color.converted(to: srgb, intent: .defaultIntent, options: nil)?.components ?? [0, 0, 0, 1]
            let rgb = c.count >= 3 ? Array(c.prefix(3)) : [c[0], c[0], c[0]]
            return Colour(red: Double(rgb[0]), green: Double(rgb[1]), blue: Double(rgb[2]), alpha: Double(opacity))
        }
        let edge: Edge =
            switch MACaptionAppearanceGetTextEdgeStyle(domain, nil) {
            case .raised: .raised
            case .depressed: .depressed
            case .uniform: .uniform
            case .dropShadow: .dropShadow
            default: .none
            }
        return CaptionStyle(
            // The system's interface font has a hidden name (".AppleSystemUIFont").
            font: family.flatMap { $0.hasPrefix(".") ? nil : $0 },
            size: Double(MACaptionAppearanceGetRelativeCharacterSize(domain, nil)),
            text: colour(
                MACaptionAppearanceCopyForegroundColor(domain, nil).takeRetainedValue(),
                MACaptionAppearanceGetForegroundOpacity(domain, nil)),
            background: colour(
                MACaptionAppearanceCopyBackgroundColor(domain, nil).takeRetainedValue(),
                MACaptionAppearanceGetBackgroundOpacity(domain, nil)),
            window: colour(
                MACaptionAppearanceCopyWindowColor(domain, nil).takeRetainedValue(),
                MACaptionAppearanceGetWindowOpacity(domain, nil)),
            edge: edge)
    }

    /// Where the system says the viewer changed the style.
    public static var changed: Notification.Name {
        Notification.Name(kMACaptionAppearanceSettingsChangedNotification as String)
    }
}
