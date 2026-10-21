// Type. Show titles, and only show titles, are set in Zen Kaku Gothic New 700; everything else is
// the system's SF at the sizes the brief gives.

import CoreText
import SwiftUI

/// The show-title face, which ships inside this package and is registered for the process the
/// first time it is asked for.
public enum ShowTitleFace {
    /// Its PostScript name, which `Font.custom` takes.
    public static let postScriptName = "ZenKakuGothicNew-Bold"

    /// Register the bundled font with the process. Safe to call any number of times, from any
    /// thread; only the first registers.
    public static func register() {
        _ = registered
    }

    /// Whether the face is there to draw with.
    public static var isAvailable: Bool {
        register()
        return CTFontCreateWithName(postScriptName as CFString, 12, nil)
            .postScriptName == postScriptName
    }

    private static let registered: Bool = {
        guard let url = resources.url(
            forResource: "ZenKakuGothicNew-Bold", withExtension: "ttf", subdirectory: "Fonts"
        ) else { return false }
        var error: Unmanaged<CFError>?
        let done = CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error)
        // Already registered, by another copy of the package in the same process, is as good.
        return done || CTFontCreateWithName(postScriptName as CFString, 12, nil)
            .postScriptName == postScriptName
    }()
}

/// Where the font is: SwiftPM's resource bundle for the package, and the framework itself when
/// the Xcode project builds these sources as one.
#if SWIFT_PACKAGE
let resources = Bundle.module
#else
private final class BundleToken {}
let resources = Bundle(for: BundleToken.self)
#endif

private extension CTFont {
    var postScriptName: String { CTFontCopyPostScriptName(self) as String }
}

extension Font {
    /// A show title in Zen Kaku Gothic New 700 at `size`, scaled with Dynamic Type as `style` is.
    ///
    /// The brief's sizes: 15 in a Mac row, 30 for the Mac preview's title, 22 for the iPhone
    /// hero, 17 in an iPhone row.
    public static func showTitle(size: CGFloat, relativeTo style: Font.TextStyle = .body) -> Font {
        ShowTitleFace.register()
        return .custom(ShowTitleFace.postScriptName, size: size, relativeTo: style)
    }
}

extension Font {
    /// The system font for a control both boards share, at the size its caller gives: a Mac
    /// board's (14 and under, `mac`) or an iPhone board's (15 and up, `ui`).
    public static func board(_ size: CGFloat, weight: Weight = .regular) -> Font {
        size < 15 ? .mac(size, weight: weight) : .ui(size, weight: weight)
    }

    /// A show title at a size a Mac board gives, for a screen the Mac and the iPad share: the
    /// size as given on the Mac and iPhone, and on iPad the size iPadOS sets the same line at
    /// (`TypeScale.iPad`), scaled with Dynamic Type as `style` is.
    public static func macShowTitle(size: CGFloat, relativeTo style: Font.TextStyle = .body) -> Font {
        #if os(iOS)
        return .showTitle(size: TypeScale.isPad ? TypeScale.iPad(size) : size, relativeTo: style)
        #else
        return .showTitle(size: size, relativeTo: style)
        #endif
    }
}

/// A fixed line height, as the boards give one, scaled with Dynamic Type as `style` is.
struct FixedLineHeight: ViewModifier {
    @ScaledMetric private var points: CGFloat

    init(_ points: CGFloat, relativeTo style: Font.TextStyle) {
        _points = ScaledMetric(wrappedValue: points, relativeTo: style)
    }

    func body(content: Content) -> some View {
        content.lineHeight(.exact(points: points))
    }
}

extension View {
    /// Lines `points` apart, as the boards set them: 20 for a 15 pt show title in a Mac row, 38
    /// for the 30 pt one above a preview.
    public func fixedLineHeight(_ points: CGFloat, relativeTo style: Font.TextStyle = .body) -> some View {
        modifier(FixedLineHeight(points, relativeTo: style))
    }

    /// A Mac board's line height, for a screen the Mac and the iPad share: exact on the Mac and
    /// iPhone, and on iPad, where the type is set larger (`Font.mac`), the font's own.
    @ViewBuilder
    public func macLineHeight(_ points: CGFloat, relativeTo style: Font.TextStyle = .body) -> some View {
        #if os(iOS)
        if TypeScale.isPad { self } else { fixedLineHeight(points, relativeTo: style) }
        #else
        fixedLineHeight(points, relativeTo: style)
        #endif
    }

    /// A group header: 11 pt medium capitals spaced 0.08 em, in haze, as the Mac and iPhone lists
    /// head their groups ("IN PROGRESS", "NEW THIS WEEK").
    public func groupHeaderStyle() -> some View {
        modifier(GroupHeaderStyle())
    }
}

// MARK: - Dynamic Type

/// The text style each iPhone size is, at the default text size: a font set at one of these
/// sizes scales with Dynamic Type as the style does. The Mac has no Dynamic Type, and keeps the
/// size as given.
public enum TypeScale {
    /// iOS's text styles by their size at the default text size.
    static let styles: [CGFloat: Font.TextStyle] = [
        34: .largeTitle, 28: .title, 22: .title2, 20: .title3, 17: .body, 16: .callout,
        15: .subheadline, 13: .footnote, 12: .caption, 11: .caption2,
    ]

    /// A Mac size as iPadOS sets the same text: the Mac's text styles to the iPad's, 13 body to
    /// 17, 12 to 15, 11 to 13 and so on up the ladder.
    public static func iPad(_ mac: CGFloat) -> CGFloat {
        switch mac {
        case ...9: 11
        case ...10: 12
        case ...11: 13
        case ...12: 15
        case ...14: 17
        case ...15: 20
        case ...17: 22
        case ...20: 24
        case ...22: 28
        case ...26: 34
        default: mac
        }
    }

    /// The text style a size is, when it is one.
    public static func style(_ size: CGFloat) -> Font.TextStyle? {
        styles[size]
    }

    #if os(iOS)
    /// Whether this is an iPad, where the Mac's boards are drawn at iPadOS's sizes: by the
    /// model, or the model a Simulator stands in for, so that any thread may ask.
    static let isPad: Bool = {
        if let simulated = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] {
            return simulated.hasPrefix("iPad")
        }
        var system = utsname()
        uname(&system)
        let machine = withUnsafeBytes(of: &system.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
        return machine.hasPrefix("iPad")
    }()
    #endif
}

extension Font {
    /// The system font at a size an iPhone board gives. On iPhone and iPad it is the text style
    /// of that size (17 body, 15 subheadline, 13 footnote…), so it scales with Dynamic Type; a
    /// size no style has stays as it is. On the Mac, the size as given.
    public static func ui(_ size: CGFloat, weight: Weight = .regular, design: Design = .default) -> Font {
        #if os(iOS)
        if let style = TypeScale.style(size) { return .system(style, design: design, weight: weight) }
        #endif
        return .system(size: size, weight: weight, design: design)
    }

    /// The system font at a size a Mac board gives, for a screen the Mac and the iPad share. On the
    /// Mac, the size as given; on iPad, the size iPadOS sets the same text at (13 to 17, 12 to 15,
    /// 11 to 13), scaling with Dynamic Type; on iPhone, the size's own text style.
    public static func mac(_ size: CGFloat, weight: Weight = .regular, design: Design = .default) -> Font {
        #if os(iOS)
        return .ui(TypeScale.isPad ? TypeScale.iPad(size) : size, weight: weight, design: design)
        #else
        return .system(size: size, weight: weight, design: design)
        #endif
    }
}

/// `groupHeaderStyle`: as large as `.accessibility1` and no larger, its letters set close once
/// the text is that large (ux_part3, Dynamic Type: metadata lines).
struct GroupHeaderStyle: ViewModifier {
    @Environment(\.dynamicTypeSize) private var typeSize

    func body(content: Content) -> some View {
        content
            .font(.mac(11, weight: .medium))
            .kerning(typeSize.isAccessibilitySize ? 0 : 11 * 0.08)
            .textCase(.uppercase)
            .foregroundStyle(.haze)
            .dynamicTypeSize(...DynamicTypeSize.accessibility1)
    }
}

extension View {
    /// A show title in a list row: two lines, three at the accessibility sizes, and no larger
    /// than `.accessibility2` (ux_part3, Dynamic Type).
    public func rowTitle() -> some View {
        modifier(RowTitle())
    }
}

struct RowTitle: ViewModifier {
    @Environment(\.dynamicTypeSize) private var typeSize

    func body(content: Content) -> some View {
        content
            .lineLimit(typeSize.isAccessibilitySize ? 3 : 2)
            .dynamicTypeSize(...DynamicTypeSize.accessibility2)
    }
}
