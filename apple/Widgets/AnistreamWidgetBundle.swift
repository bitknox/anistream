// The widget extension: Continue watching and Airing today, whose layouts, timelines and edit
// sheet are in apple/Packages/AnistreamWidgets (AnistreamWidgetUI), compiled into this target.

import AnistreamWidgetData
import AppIntents
import SwiftUI
import WidgetKit

@main
struct AnistreamWidgetBundle: WidgetBundle {
    var body: some Widget {
        ContinueWatchingWidget()
        AiringTodayWidget()
    }
}

/// The App Intents the extension brings: its own edit sheet's, and the play button's from
/// AnistreamWidgetData.
struct AnistreamWidgetsIntentsPackage: AppIntentsPackage {
    static var includedPackages: [any AppIntentsPackage.Type] {
        [AnistreamWidgetIntents.self]
    }
}
