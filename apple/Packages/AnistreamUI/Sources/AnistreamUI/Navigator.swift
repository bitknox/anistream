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
    /// What Search searches for.
    public var query = ""
    /// The Library's list on screen.
    public var librarySegment: LibrarySegment = .watching
    /// The calendar's week, as weeks from this one.
    public var calendarWeek = 0
    /// Whether the calendar shows only the titles on your lists.
    public var calendarMine = true
    /// The Settings pane on screen, on the Mac and iPad.
    public var settingsPane: SettingsPane = .playback
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
    /// The title whose "More about this show" sheet is open.
    public var moreAbout: AnilistId?
    /// The Playback pane's row the column beside it is about.
    public var playbackFocus: PlaybackFocus = .upscaling

    public init(route: LaunchRoute = .current) {
        section = route.section
        titleOnMac = route.title
        focusedTitle = route.title ?? route.selected
        query = route.query ?? ""
        librarySegment = route.library ?? .watching
        paletteShown = route.palette != nil
        settingsPane = route.pane.flatMap(SettingsPane.init(rawValue:)).flatMap { SettingsPane.shown.contains($0) ? $0 : nil }
            ?? .playback
        // "titles": Playback, its title-language row selected (MacSettingsTitles).
        if route.pane == "titles" { playbackFocus = .titleLanguage }
        launchSheet = route.sheet
        if route.sheet == "join" { invite = "https://plugins.hotaru.example/join/fixture#key" }
        if route.sheet == "show" { showSettings = route.title }
        if route.sheet == "about" { moreAbout = route.title }
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

    /// Search for `query`, in the Search section.
    public func search(_ query: String, in store: AppStore) {
        self.query = query
        section = .search
        if !query.isEmpty {
            store.search(query)
            RecentSearches.shared.add(query)
        }
    }
}

/// What was searched for lately, newest first, kept on this device.
@Observable @MainActor
public final class RecentSearches {
    public static let shared = RecentSearches()
    private static let key = "anistream.recentSearches"
    /// How many are kept.
    static let kept = 8

    public private(set) var queries: [String]
    @ObservationIgnored private let defaults: UserDefaults

    public init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        queries = defaults.stringArray(forKey: Self.key) ?? []
    }

    public func add(_ query: String) {
        let query = query.trimmingCharacters(in: .whitespaces)
        guard !query.isEmpty else { return }
        queries.removeAll { $0.caseInsensitiveCompare(query) == .orderedSame }
        queries.insert(query, at: 0)
        queries = Array(queries.prefix(Self.kept))
        defaults.set(queries, forKey: Self.key)
    }

    public func remove(_ query: String) {
        queries.removeAll { $0 == query }
        defaults.set(queries, forKey: Self.key)
    }

    public func clear() {
        queries = []
        defaults.removeObject(forKey: Self.key)
    }
}
