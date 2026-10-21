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
        guard let url = Bundle.module.url(
            forResource: "ZenKakuGothicNew-Bold", withExtension: "ttf", subdirectory: "Fonts"
        ) else { return false }
        var error: Unmanaged<CFError>?
        let done = CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error)
        // Already registered, by another copy of the package in the same process, is as good.
        return done || CTFontCreateWithName(postScriptName as CFString, 12, nil)
            .postScriptName == postScriptName
    }()
}

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

extension View {
    /// A group header: 11 pt medium capitals spaced 0.08 em, in haze, as the Mac and iPhone lists
    /// head their groups ("IN PROGRESS", "NEW THIS WEEK").
    public func groupHeaderStyle() -> some View {
        font(.system(size: 11, weight: .medium))
            .kerning(11 * 0.08)
            .textCase(.uppercase)
            .foregroundStyle(.haze)
    }
}
