// The app's sections, as the Mac sidebar and the iPhone tab bar list them.

import Foundation

/// Where the app can be. The Mac sidebar lists them all; the iPhone's tabs are Continue,
/// Calendar, Seasonal and Library, with Search on its own.
public enum AppSection: String, CaseIterable, Identifiable, Hashable, Sendable {
    case `continue`, calendar, seasonal, search, library, downloads
    case providers, pluginStores
    case settings

    public var id: String { rawValue }

    /// The name, in title case as the Mac sidebar and toolbar say it.
    public var title: String {
        switch self {
        case .continue: "Continue"
        case .calendar: "Calendar"
        case .seasonal: "Seasonal"
        case .search: "Search"
        case .library: "Library"
        case .downloads: "Downloads"
        case .providers: "Providers"
        case .pluginStores: "Plugin Stores"
        case .settings: "Settings"
        }
    }

    /// Its SF Symbol.
    public var symbol: String {
        switch self {
        case .continue: "play.circle"
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
    static let browse: [AppSection] = [.continue, .calendar, .seasonal, .search, .library, .downloads]
    /// The sidebar's Sources group.
    static let sources: [AppSection] = [.providers, .pluginStores]
}
