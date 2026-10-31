// The flag beside a language's own name in the language picker. It is decoration: VoiceOver
// skips it, and the row's label stays the language's name.

import AnistreamDesign
import AnistreamStrings
import SwiftUI

/// A flag image, 20 by 15 pt with a corner radius of 2 and a hairline edge in the faint rule so a
/// white flag stays visible on a light surface.
struct LanguageFlag: View {
    let code: String

    static let size = CGSize(width: 20, height: 15)

    var body: some View {
        if let image = Self.image(code) {
            image
                .resizable()
                .interpolation(.high)
                .frame(width: Self.size.width, height: Self.size.height)
                .clipShape(.rect(cornerRadius: 2))
                .overlay(RoundedRectangle(cornerRadius: 2).strokeBorder(Palette.faintRule, lineWidth: 0.5))
                .accessibilityHidden(true)
        }
    }

    private static func image(_ code: String) -> Image? {
        guard let url = L.flagURL(code), let data = try? Data(contentsOf: url) else { return nil }
        #if os(macOS)
        return NSImage(data: data).map(Image.init(nsImage:))
        #else
        return UIImage(data: data).map(Image.init(uiImage:))
        #endif
    }
}

/// A language's flag and its name, the name read as the row's label.
struct LanguageNameLabel: View {
    let language: L.Language
    let text: String

    var body: some View {
        HStack(spacing: 8) {
            if let flag = language.flag { LanguageFlag(code: flag) }
            Text(text)
        }
    }
}
