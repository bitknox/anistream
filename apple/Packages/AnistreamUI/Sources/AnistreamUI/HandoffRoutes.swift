// Handoff: the title page open here is offered to your other devices, and what another device
// hands over opens here. An episode handed over opens its title's page, with "Resume Episode 7 ·
// 12:40" as its action: it never starts by itself.

import AnistreamKit
import AnistreamStore
import SwiftUI

extension View {
    /// Open what another device hands over: a title's page, or for an episode playing there,
    /// its page with the episode offered at the place it was.
    public func continuesHandoff(_ types: HandoffTypes = .app) -> some View {
        modifier(HandoffRoutes(types: types))
    }

    /// Offer this title's page to your other devices while it is the page on screen and nothing
    /// plays: the player hands over its episode instead.
    func handsOff(title entry: Entry) -> some View {
        modifier(TitleHandoff(entry: entry))
    }
}

struct HandoffRoutes: ViewModifier {
    @Environment(Navigator.self) private var navigator
    let types: HandoffTypes

    func body(content: Content) -> some View {
        content
            .onContinueUserActivity(types.title) { Self.take($0, types: types, navigator: navigator) }
            .onContinueUserActivity(types.playing) { Self.take($0, types: types, navigator: navigator) }
    }

    /// What `onContinueUserActivity` does with an activity: whether it was one to open.
    @discardableResult
    static func take(_ activity: NSUserActivity, types: HandoffTypes, navigator: Navigator) -> Bool {
        guard let handoff = Handoff(activity, types: types) else { return false }
        navigator.continueHandoff(handoff)
        return true
    }
}

struct TitleHandoff: ViewModifier {
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    /// Whether the title page is offered. It does not follow the player opening and closing:
    /// SwiftUI keeps a cycle of closures for every flip of `isActive`. The playing activity
    /// becomes current when it starts, so it takes the offer over by itself.
    static func offers(_ entry: Entry, onScreen: AnilistId?) -> Bool {
        !entry.title.isEmpty && onScreen == entry.id
    }

    func body(content: Content) -> some View {
        let active = Self.offers(entry, onScreen: navigator.titleOnScreen)
        content.userActivity(HandoffTypes.app.title, isActive: active) { activity in
            Handoff.title(entry.id).describe(activity, title: entry.showTitle)
        }
    }
}

extension Navigator {
    /// The title page on screen: the Mac's over its section, or the top of the tab's stack.
    var titleOnScreen: AnilistId? {
        #if os(macOS)
        titleOnMac
        #else
        paths[section]?.last
        #endif
    }

    /// Open what another device handed over: the title's page over what is on screen, unless it
    /// is there already, and for an episode the offer to resume it where it was. Settings'
    /// iPhone sheet gives way to it.
    public func continueHandoff(_ handoff: Handoff) {
        if case .playing(let id, let episode, let position, _) = handoff {
            handedOff = HandedOff(id: id, episode: episode, position: position)
        }
        #if os(iOS)
        phoneSettings = nil
        #endif
        if titleOnScreen != handoff.id { openTitle(handoff.id) }
    }
}
