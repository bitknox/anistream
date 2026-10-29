// The small widget's play button: an App Intent that opens the app into the player, since a small
// widget takes no links of its own. The medium and large ones link to the same place.

import AppIntents
import Foundation

/// Opens anistream and plays an episode.
///
/// It lives here, in a framework the app links as well as the widget extension, so it runs in
/// the app once the app is open: the app hands it to the router it registers at launch.
public struct PlayEpisodeIntent: AppIntent {
    public static let title: LocalizedStringResource = "Play episode" // i18n-skip
    public static let description = IntentDescription("Opens anistream and plays an episode.") // i18n-skip
    public static let supportedModes: IntentModes = .foreground
    /// The widget's own; Shortcuts offers the app's Continue watching instead.
    public static let isDiscoverable = false

    @Parameter(title: "Title") // i18n-skip
    public var titleID: Int

    @Parameter(title: "Episode") // i18n-skip
    public var episode: Int

    public init() {}

    public init(titleID: Int, episode: Int) {
        self.titleID = titleID
        self.episode = episode
    }

    @MainActor
    public func perform() async throws -> some IntentResult {
        WidgetLinkRouter.shared.open(.play(titleID, episode: episode))
        return .result()
    }
}

/// Where a widget's link goes in the app: the app registers a handler at launch, and a link that
/// comes before it waits for it.
@MainActor
public final class WidgetLinkRouter {
    public static let shared = WidgetLinkRouter()

    private var handler: (@MainActor (WidgetLink) -> Void)?
    private var waiting: WidgetLink?

    public init() {}

    /// Hand every link to `handler` from now on, and the one that came before it.
    public func register(_ handler: @escaping @MainActor (WidgetLink) -> Void) {
        self.handler = handler
        if let waiting {
            self.waiting = nil
            handler(waiting)
        }
    }

    public func open(_ link: WidgetLink) {
        if let handler { handler(link) } else { waiting = link }
    }
}

/// The intents this framework brings, for the app's and the extension's App Intents packages.
public struct AnistreamWidgetIntents: AppIntentsPackage {}
