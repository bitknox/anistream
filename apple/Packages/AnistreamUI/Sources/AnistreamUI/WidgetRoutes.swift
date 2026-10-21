// Where the widgets' taps land: a title's page, the player on an episode, the Calendar on today,
// and Settings › Accounts. They come as anistream:// links, and the small widget's play button as
// an App Intent that runs in the app.

import AnistreamKit
import AnistreamStore
import AnistreamWidgetData
import SwiftUI

extension View {
    /// Open the widgets' links: the ones the app is opened with, and the play intent's.
    public func opensWidgetLinks() -> some View {
        modifier(WidgetRoutes())
    }
}

struct WidgetRoutes: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    func body(content: Content) -> some View {
        content
            .onOpenURL { url in
                if let link = WidgetLink(url: url) { navigator.openWidgetLink(link, store: store) }
            }
            .task {
                WidgetLinkRouter.shared.register { [navigator, store] link in
                    navigator.openWidgetLink(link, store: store)
                }
            }
    }
}

extension Navigator {
    /// Go where a widget's tap says. A title opens over what is on screen, unless it is there
    /// already; an episode plays from its title's page, as its own Play would; Settings' iPhone
    /// sheet gives way to a title, and is where Accounts is on the iPhone.
    func openWidgetLink(_ link: WidgetLink, store: AppStore) {
        switch link {
        case .title(let id):
            guard let id = AnilistId(exactly: id) else { return }
            showWidgetTitle(id)
        case .play(let id, let episode):
            guard let id = AnilistId(exactly: id) else { return }
            showWidgetTitle(id)
            store.send(.play(id: id, episode: String(episode)))
        case .calendar:
            #if os(iOS)
            phoneSettings = nil
            #endif
            calendarWeek = 0
            open(.calendar)
        case .accounts:
            #if os(iOS)
            if UIDevice.current.userInterfaceIdiom == .phone {
                phoneSettings = .accounts
                return
            }
            #endif
            openSettings(.accounts)
        }
    }

    /// A title's page, over the section on screen, unless it is the page there.
    private func showWidgetTitle(_ id: AnilistId) {
        #if os(iOS)
        phoneSettings = nil
        #endif
        #if os(macOS)
        let onScreen = titleOnMac
        #else
        let onScreen = paths[section]?.last
        #endif
        if onScreen != id { openTitle(id) }
    }
}
