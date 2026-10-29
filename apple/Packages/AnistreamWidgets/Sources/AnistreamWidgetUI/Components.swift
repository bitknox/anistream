// The pieces both widgets are drawn with: the title face, progress, the marks, the play button
// over art, and the art itself, read from the snapshot's files.

import AnistreamDesign
import AnistreamWidgetData
import CoreText
import SwiftUI
import WidgetKit

// MARK: - Layouts

/// The layouts the widgets have: one per family. Its own type, since the shots draw the Lock
/// Screen's on the Mac, where WidgetKit has no accessory families.
public enum WidgetLayout: String, Sendable, CaseIterable {
    case small, medium, large, rectangular, inline

    public init(_ family: WidgetFamily) {
        switch family {
        case .systemSmall: self = .small
        case .systemLarge, .systemExtraLarge: self = .large
        #if os(iOS)
        case .accessoryRectangular, .accessoryCircular: self = .rectangular
        case .accessoryInline: self = .inline
        #endif
        default: self = .medium
        }
    }

    var isAccessory: Bool { self == .rectangular || self == .inline }
}

// MARK: - Type

enum WidgetType {
    /// A show title in Zen Kaku Gothic New 700: the extension's own copy of the face, or the
    /// design system's where the extension has none (the shots).
    static func title(_ size: CGFloat) -> Font {
        registerFace()
        return .custom(ShowTitleFace.postScriptName, size: size, relativeTo: .body)
    }

    /// Everything else: the system font, at the text style the app sets that size in.
    static func ui(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .ui(size, weight: weight)
    }

    /// The extension bundles the face itself, since its process may not read the app's frameworks.
    private static let registered: Bool = {
        if let url = Bundle.main.url(forResource: ShowTitleFace.postScriptName, withExtension: "ttf") {
            var error: Unmanaged<CFError>?
            if CTFontManagerRegisterFontsForURL(url as CFURL, .process, &error) { return true }
        }
        ShowTitleFace.register()
        return true
    }()

    static func registerFace() { _ = registered }
}

extension View {
    /// A fixed line height, as the boards give one.
    func lineBox(_ points: CGFloat) -> some View {
        fixedLineHeight(points)
    }
}

// MARK: - Clock

private struct WidgetNowKey: EnvironmentKey {
    static let defaultValue: Date? = nil
}

extension EnvironmentValues {
    /// The moment a shot is drawn at, so its countdowns read as of then. A widget leaves it nil
    /// and its countdowns run on their own.
    public var widgetNow: Date? {
        get { self[WidgetNowKey.self] }
        set { self[WidgetNowKey.self] = newValue }
    }
}

/// A countdown or a time since, that runs on its own in a widget: "1 hr, 30 min".
/// A line that carries a time which counts on its own: the message is worded, and the time is
/// drawn where its marker stood.
struct TimedLine: View {
    let date: Date
    let message: (String) -> String

    init(date: Date, message: @escaping (String) -> String) {
        self.date = date
        self.message = message
    }

    var body: some View {
        let marker = "\u{1}"
        let parts = message(marker).components(separatedBy: marker)
        HStack(spacing: 0) {
            Text(parts[0])
            RelativeTime(date: date)
            if parts.count > 1 { Text(parts[1]) }
        }
    }
}

struct RelativeTime: View {
    let date: Date
    @Environment(\.widgetNow) private var now

    var body: some View {
        if let now {
            Text(Self.fixed(from: now, to: date))
        } else {
            Text(date, style: .relative)
        }
    }

    /// What `.relative` says at `now`, for a shot.
    static func fixed(from now: Date, to date: Date) -> String {
        let formatter = DateComponentsFormatter()
        formatter.unitsStyle = .short
        formatter.allowedUnits = abs(date.timeIntervalSince(now)) >= 3_600 ? [.hour, .minute] : [.minute]
        formatter.maximumUnitCount = 2
        return formatter.string(from: abs(date.timeIntervalSince(now))) ?? ""
    }
}

// MARK: - Taps

private struct DrawsTapsKey: EnvironmentKey {
    static let defaultValue = true
}

extension EnvironmentValues {
    /// Whether links and buttons are drawn as such: off for the shots, which ImageRenderer draws
    /// and which has no links.
    public var widgetDrawsTaps: Bool {
        get { self[DrawsTapsKey.self] }
        set { self[DrawsTapsKey.self] = newValue }
    }
}

/// A region of a medium or large widget that opens `link`.
struct TapLink<Content: View>: View {
    let link: WidgetLink
    @ViewBuilder let content: () -> Content
    @Environment(\.widgetDrawsTaps) private var drawsTaps

    init(_ link: WidgetLink, @ViewBuilder content: @escaping () -> Content) {
        self.link = link
        self.content = content
    }

    var body: some View {
        if drawsTaps {
            Link(destination: link.url, label: content)
        } else {
            content()
        }
    }
}

// MARK: - Marks

/// Progress: amber on the rule, 3 points, round ends.
struct ProgressBar: View {
    let fraction: Double
    var height: CGFloat = 3
    var width: CGFloat?
    /// White on the Lock Screen, as the system draws it there.
    var monochrome = false
    @Environment(\.widgetRenderingMode) private var mode

    var body: some View {
        let amber = mode == .fullColor && !monochrome
        Capsule()
            .fill(amber ? Palette.rule : Color.primary.opacity(0.22))
            .overlay(alignment: .leading) {
                GeometryReader { proxy in
                    Capsule()
                        .fill(amber ? Palette.amber : Color.primary)
                        .frame(width: proxy.size.width * min(max(fraction, 0), 1))
                }
            }
            .frame(width: width, height: height)
            .widgetAccentable()
            .accessibilityHidden(true)
    }
}

/// The bookmark a show in your list carries.
struct Bookmark: View {
    var size: CGFloat = 9
    var color: Color = Palette.silk

    var body: some View {
        BookmarkShape()
            .fill(color)
            .frame(width: size, height: size)
            .widgetAccentable()
            .accessibilityHidden(true)
    }
}

struct BookmarkShape: Shape {
    func path(in rect: CGRect) -> Path {
        // The board's glyph, on a 10 × 10 grid.
        let x = rect.width / 10, y = rect.height / 10
        var path = Path()
        path.move(to: CGPoint(x: 2.5 * x, y: 0.8 * y))
        path.addLine(to: CGPoint(x: 7.5 * x, y: 0.8 * y))
        path.addLine(to: CGPoint(x: 7.5 * x, y: 9.2 * y))
        path.addLine(to: CGPoint(x: 5 * x, y: 7.2 * y))
        path.addLine(to: CGPoint(x: 2.5 * x, y: 9.2 * y))
        path.closeSubpath()
        return path
    }
}

/// The water dot: an episode out of a show in your list that you have not watched.
struct WaterDot: View {
    var size: CGFloat = 6

    var body: some View {
        Circle().fill(Palette.water).frame(width: size, height: size).accessibilityHidden(true)
    }
}

/// The haze check of a watched episode.
struct WatchedCheck: View {
    var size: CGFloat = 11

    var body: some View {
        Image(systemName: "checkmark")
            .font(.system(size: size * 0.82, weight: .semibold))
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

/// The dimmed glass circle with a play glyph, over art, that plays the episode: a link where the
/// widget takes links, and the play intent in the small size, which takes none.
struct PlayCircle: View {
    let id: Int
    let episode: Int
    let label: String
    var size: CGFloat = 32
    var intent = false

    @Environment(\.widgetDrawsTaps) private var drawsTaps

    var body: some View {
        if !drawsTaps {
            glyph
        } else if intent {
            Button(intent: PlayEpisodeIntent(titleID: id, episode: episode)) { glyph }
                .buttonStyle(.plain)
                .accessibilityLabel(label)
        } else {
            Link(destination: WidgetLink.play(id, episode: episode).url) { glyph }
                .accessibilityLabel(label)
        }
    }

    private var glyph: some View {
        Image(systemName: "play.fill")
            .font(.system(size: size * 0.34, weight: .regular))
            .foregroundStyle(Palette.silk)
            .frame(width: size, height: size)
            .background(Circle().fill(Palette.glassOverArt))
            .overlay(Circle().strokeBorder(Palette.glassOverArtEdge, lineWidth: 1))
    }
}

// MARK: - Art

/// Where a widget's art files are, for the views to read them.
struct ArtFilesKey: EnvironmentKey {
    static let defaultValue: WidgetFiles? = nil
}

extension EnvironmentValues {
    public var widgetFiles: WidgetFiles? {
        get { self[ArtFilesKey.self] }
        set { self[ArtFilesKey.self] = newValue }
    }
}

/// A row's art file, read whole: the files are thumbnails already.
struct ArtImage: View {
    let name: String?
    @Environment(\.widgetFiles) private var files
    @Environment(\.displayScale) private var scale

    var body: some View {
        if let image = cgImage {
            Image(decorative: image, scale: 1)
                .resizable()
                .widgetAccentedRenderingMode(.desaturated)
                .aspectRatio(contentMode: .fill)
        } else {
            Palette.artPlaceholder
        }
    }

    var cgImage: CGImage? {
        guard let name, let files else { return nil }
        return ArtThumbnail.image(at: files.art(name))
    }

    /// Whether the file is there to draw.
    static func exists(_ name: String?, in files: WidgetFiles?) -> Bool {
        guard let name, let files else { return false }
        return files.hasArt(name)
    }
}

/// Art cropped into a rounded frame.
struct ArtFrame: View {
    let name: String?
    let width: CGFloat?
    let height: CGFloat
    var radius: CGFloat = 0

    var body: some View {
        ArtImage(name: name)
            .frame(width: width, height: height)
            .frame(maxWidth: width == nil ? .infinity : nil)
            .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
            .accessibilityHidden(true)
    }
}
