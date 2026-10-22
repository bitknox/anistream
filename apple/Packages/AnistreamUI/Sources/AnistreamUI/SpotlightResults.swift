// Opening a Spotlight result: the app comes forward on that title's page.

import AnistreamKit
import CoreSpotlight
import SwiftUI

/// Takes the Spotlight results the app is opened with to their titles' pages.
struct SpotlightResults: ViewModifier {
    @Environment(Navigator.self) private var navigator

    func body(content: Content) -> some View {
        content.onContinueUserActivity(CSSearchableItemActionType) { activity in
            guard let id = SpotlightItem.title(in: activity) else { return }
            navigator.reveal(id)
        }
    }
}

extension Navigator {
    /// A title's page, asked for from outside the app: opened over what is on screen, unless it
    /// is the page there already. Settings' iPhone sheet gives way to it.
    func reveal(_ id: AnilistId) {
        #if os(iOS)
        phoneSettings = nil
        #endif
        if titleOnMac == id || paths[section]?.last == id { return }
        openTitle(id)
    }
}
