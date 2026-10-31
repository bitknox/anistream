// The app's sections, as the Mac sidebar and the iPhone tab bar list them.

import AnistreamStrings
import Foundation

/// Where the app can be. The Mac sidebar lists them all; the iPhone's tabs are Home,
/// Calendar, Seasonal and Library, with Search on its own.
public enum AppSection: String, CaseIterable, Identifiable, Hashable, Sendable {
    case home, calendar, seasonal, search, library, downloads
    case providers, pluginStores
    case settings

    public var id: String { rawValue }

    /// The name, as the Mac sidebar, the toolbar and the iPhone's tabs say it.
    public var title: String {
        switch self {
        case .home: L.homeTitle
        case .calendar: L.navCalendar
        case .seasonal: L.navSeasonal
        case .search: L.navSearch
        case .library: L.navLibrary
        case .downloads: L.navDownloads
        case .providers: L.sourcesTitle
        case .pluginStores: L.navPluginStores
        case .settings: L.navSettings
        }
    }

    /// Its SF Symbol.
    public var symbol: String {
        switch self {
        case .home: "house"
        case .calendar: "calendar"
        case .seasonal: "leaf"
        case .search: "magnifyingglass"
        case .library: "books.vertical"
        case .downloads: "arrow.down.to.line"
        case .providers: "powerplug"
        case .pluginStores: "bag"
        case .settings: "gearshape"
        }
    }

    /// The sidebar's first group.
    static let browse: [AppSection] = [.home, .calendar, .seasonal, .search, .library, .downloads]
    /// The sidebar's Sources group.
    static let sources: [AppSection] = [.providers, .pluginStores]
}
