// Where the app opens, as the launch arguments say: for screenshots and for coming back to a
// screen while working on it.

import AnistreamKit
import Foundation

/// The screen to open at launch.
///
///     -AnistreamSection calendar    a section by its name: continue, calendar, seasonal, search,
///                                   library, downloads, providers, pluginStores, settings
///     -AnistreamSelect 900005       the title selected in a list
///     -AnistreamTitle 900001        a title's page, over the section
///     -AnistreamQuery hoshikuzu     what Search searches for
///     -AnistreamLibrary planning    the Library's list
///     -AnistreamPalette hoshi       the ⌘K palette, open on these words
public struct LaunchRoute: Equatable, Sendable {
    public var section: AppSection = .continue
    public var selected: AnilistId?
    public var title: AnilistId?
    public var query: String?
    public var library: LibrarySegment?
    public var palette: String?

    public init() {}

    public init(arguments: [String]) {
        func value(_ name: String) -> String? {
            guard let index = arguments.firstIndex(of: name), arguments.indices.contains(index + 1)
            else { return nil }
            return arguments[index + 1]
        }
        section = value("-AnistreamSection").flatMap(AppSection.init(rawValue:)) ?? .continue
        selected = value("-AnistreamSelect").flatMap { AnilistId($0) }
        title = value("-AnistreamTitle").flatMap { AnilistId($0) }
        query = value("-AnistreamQuery")
        palette = value("-AnistreamPalette")
        library = value("-AnistreamLibrary").flatMap { name in
            LibraryModelOrder.segments.first { "\($0)" == name }
        }
    }

    /// This process's.
    public static let current = LaunchRoute(arguments: ProcessInfo.processInfo.arguments)
}

/// The tracker's lists by the names the arguments use.
enum LibraryModelOrder {
    static let segments: [LibrarySegment] = [.watching, .planning, .completed, .paused, .dropped]
}
