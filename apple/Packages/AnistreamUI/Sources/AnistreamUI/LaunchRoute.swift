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
///     -AnistreamGenre Fantasy       Search browses a genre, with any query's words
///     -AnistreamLibrary planning    the Library's list
///     -AnistreamPalette hoshi       the ⌘K palette, open on these words
///     -AnistreamPane accounts       Settings' pane, or on iPhone the Settings sheet at its page
///     -AnistreamSheet torrent       a sheet over the screen: torrent, proxy (Edit proxy), join, kitsu, update, show,
///                                   source (the source picker), filters (Search's filters), typing (the search field focused),
///                                   welcome (first launch), messages
///     -AnistreamFold open           the title page's watched episodes, opened
///     -AnistreamDay 2               the calendar opened on the day this many days after today,
///                                   for a day with broadcasts: the fixtures' own day can have none
///     -AnistreamCalendarMine 1      the calendar on My list (it opens on All airing)
///     -AnistreamShowcase stores     invented rows beside the testing engine's: see `Showcase`
///     -AnistreamCatalogue           Search's browse with the test kit's eight more titles beside the six
public struct LaunchRoute: Equatable, Sendable {
    public var section: AppSection = .continue
    public var selected: AnilistId?
    public var title: AnilistId?
    public var query: String?
    public var genre: String?
    public var library: LibrarySegment?
    public var palette: String?
    public var pane: String?
    public var sheet: String?
    public var fold: String?
    /// Days after today the calendar opens on.
    public var day: Int?
    /// The calendar on My list.
    public var calendarMine = false

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
        genre = value("-AnistreamGenre")
        palette = value("-AnistreamPalette")
        pane = value("-AnistreamPane")
        sheet = value("-AnistreamSheet")
        fold = value("-AnistreamFold")
        day = value("-AnistreamDay").flatMap { Int($0) }
        calendarMine = value("-AnistreamCalendarMine") == "1"
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
