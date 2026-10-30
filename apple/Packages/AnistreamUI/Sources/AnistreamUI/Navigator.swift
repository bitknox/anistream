// Where the app is: the section on screen, the title page over it, the palette, and what the
// menus act on.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The app's place, which the shells show and the menus, the palette and the screens move.
@Observable @MainActor
public final class Navigator {
    /// The section on screen: the Mac sidebar's selection, the iPhone's tab.
    public var section: AppSection {
        didSet { if section != oldValue { titleOnMac = nil } }
    }
    /// The title page open over the section on the Mac. On iPhone and iPad a page is pushed on
    /// the tab's own stack, `paths`.
    public private(set) var titleOnMac: AnilistId?
    /// Where the Mac's title page was opened from, which decides how it comes in: from the
    /// Continue preview its banner grows into the page's (M3).
    public private(set) var titleFromPreview = false
    /// Each tab's pushed title pages, on iPhone and iPad.
    public var paths: [AppSection: [AnilistId]] = [:]
    /// The title the menus' Episode items and the palette's commands act on: the one selected
    /// in a list, or the page that is open.
    public var focusedTitle: AnilistId?
    /// Whether the ⌘K palette is up.
    public var paletteShown = false
    /// Whether Messages is up.
    public var messagesShown = false
    /// Whether Kitsu's sign-in sheet is up, as a notice's Sign in asks.
    public var kitsuShown = false
    /// What Search's field holds.
    public var query = ""
    /// Whether every filter is up: a panel on the Mac, the inspector on iPad, a sheet on iPhone.
    public var searchFiltersShown = false
    /// The search the launch arguments ask for, for a screenshot, until Search runs it.
    public var launchSearch: SearchQuery?
    /// Whether Downloads is pushed over the Library, as its first row does on iPhone.
    public var downloadsPushed = false
    /// Counts the viewer's own marks of an episode as watched, for the light impact on iPhone:
    /// a mark the engine makes, from sync or the player closing, has none (Haptics rule 1).
    public var watchedByHand = 0
    /// The Library's list on screen.
    public var librarySegment: LibrarySegment = .watching
    /// The calendar's week, as weeks from this one.
    public var calendarWeek = 0
    /// The day the calendar opens on, as days after today, until it has: the launch route's.
    public var calendarOpensOn = 0
    /// Whether the calendar shows only the titles on your lists.
    public var calendarMine = true
    /// The Settings pane on screen, on the Mac and iPad.
    public var settingsPane: SettingsPane = .general
    /// What Settings' own search field holds.
    public var settingsQuery = ""
    /// The iPhone's Settings sheet, open at a page when it is up.
    public var phoneSettings: PhoneSettingsPage?
    /// An invite link the app was opened with, for the join sheet.
    public var invite: String?
    /// The sheet the launch arguments ask for, for a screenshot, until a screen shows it.
    public var launchSheet: String?
    /// The title whose own settings are up in a sheet.
    public var showSettings: AnilistId?
    /// A selection asked of Continue's list from outside it, as the list itself would move it.
    public var continueSelection: AnilistId?
    /// Whether the title search field is up for typing. The launch route's "typing" sheet sets it.
    public var searchPresented = false
    /// The title whose "More about this show" sheet is open.
    public var moreAbout: AnilistId?
    /// The title whose source picker is open.
    public var sourcePicker: AnilistId?
    /// The title whose match sheet is open.
    public var matchPicker: MatchPickerRequest?
    /// The trailer Play trailer opened: a window on the Mac, a sheet on iPhone and iPad.
    var trailerRequest: TrailerRequest?
    /// The titles whose watched episodes are open in their episode lists, for the session.
    var watchedOpen: Set<AnilistId> = []
    /// An episode handed over from another device, which its title's page offers to resume.
    public var handedOff: HandedOff?

    public init(route: LaunchRoute = .current) {
        section = route.section
        titleOnMac = route.title
        focusedTitle = route.title ?? route.selected
        query = route.query ?? ""
        librarySegment = route.library ?? .watching
        paletteShown = route.palette != nil
        settingsPane = route.pane.flatMap(SettingsPane.init(rawValue:)).flatMap { SettingsPane.shown.contains($0) ? $0 : nil }
            ?? .general
        launchSheet = route.sheet
        if let genre = route.genre { launchSearch = SearchQuery(text: route.query ?? "", genres: [genre]) }
        if route.sheet == "join" { invite = "https://plugins.hotaru.example/join/fixture#key" }
        if route.sheet == "show" { showSettings = route.title }
        if route.sheet == "about" { moreAbout = route.title }
        if route.sheet == "source" { sourcePicker = route.title }
        if let shape = AppStore.matchSheetOpens(route.sheet), let title = route.title {
            matchPicker = MatchPickerRequest(id: title, fromMenu: shape == .fromMenu)
        }
        if route.sheet == "messages" { messagesShown = true }
        if route.fold == "open", let title = route.title { watchedOpen.insert(title) }
        if let day = route.day {
            calendarOpensOn = day
            calendarWeek = weekOffset(ofDaysAhead: day)
        }
        #if os(iOS)
        if UIDevice.current.userInterfaceIdiom == .phone, route.section == .settings {
            phoneSettings = route.pane.flatMap(PhoneSettingsPage.named) ?? .root
        }
        #endif
        if let title = route.title {
            paths[route.section] = [title]
        }
    }

    /// Open a section, as the sidebar, a tab or ⌘1 to ⌘7 does.
    public func open(_ section: AppSection) {
        self.section = section
    }

    /// Providers, where a source is added: its section on the Mac and iPad, its page of the
    /// Settings sheet on iPhone.
    public func openProviders() {
        #if os(iOS)
        if UIDevice.current.userInterfaceIdiom == .phone {
            phoneSettings = .providers
            return
        }
        #endif
        open(.providers)
    }

    /// Open Settings at `pane`: the section on the Mac and iPad.
    public func openSettings(_ pane: SettingsPane) {
        settingsPane = pane
        section = .settings
    }

    /// Open a title's page over the section on screen.
    public func openTitle(_ id: AnilistId, fromPreview: Bool = false) {
        focusedTitle = id
        titleFromPreview = fromPreview
        // On the Mac the page comes in over the section with the standard spring: the banner
        // morphs from Continue's preview (M3), and the rest fades in.
        withAnimation(.standard) { titleOnMac = id }
        paths[section, default: []].append(id)
    }

    /// Back from the title page, as its back button, ⌘[ and Escape do.
    public func back() {
        withAnimation(.standard) { titleOnMac = nil }
        if var path = paths[section], !path.isEmpty {
            path.removeLast()
            paths[section] = path
        }
    }

    /// Whether Back has anywhere to go.
    public var canGoBack: Bool {
        titleOnMac != nil || !(paths[section] ?? []).isEmpty
    }

    /// Search for `query` now, in the Search section: a recent search, a rail's See All, a
    /// genre.
    public func search(_ query: SearchQuery, in store: AppStore) {
        section = .search
        store.search(query)
        // After the store has it, so the field's change finds nothing new to search for.
        self.query = query.text
    }

    /// Search for these words, with the filters already set.
    public func search(text: String, in store: AppStore) {
        var query = store.search.query
        query.text = text
        search(query, in: store)
    }
}
