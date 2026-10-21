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
private let resources = Bundle.module
#else
private final class BundleToken {}
private let resources = Bundle(for: BundleToken.self)
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

    /// A group header: 11 pt medium capitals spaced 0.08 em, in haze, as the Mac and iPhone lists
    /// head their groups ("IN PROGRESS", "NEW THIS WEEK").
    public func groupHeaderStyle() -> some View {
        font(.system(size: 11, weight: .medium))
            .kerning(11 * 0.08)
            .textCase(.uppercase)
            .foregroundStyle(.haze)
    }
}
