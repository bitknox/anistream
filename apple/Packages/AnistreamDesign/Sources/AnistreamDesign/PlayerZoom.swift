// Where the iPhone's and iPad's player zooms out of (T2): the banner a play started from.

import SwiftUI

extension EnvironmentValues {
    /// The namespace the player's full-screen cover zooms in, which the app's shell provides.
    @Entry public var playerZoom: Namespace.ID? = nil
}

extension View {
    /// This banner is where the player zooms out of when `id`, the title's, plays.
    public func playerZoomSource<ID: Hashable>(_ id: ID) -> some View {
        modifier(PlayerZoomSource(id: id))
    }
}

struct PlayerZoomSource<ID: Hashable>: ViewModifier {
    @Environment(\.playerZoom) private var zoom
    let id: ID

    func body(content: Content) -> some View {
        if let zoom {
            content.matchedTransitionSource(id: id, in: zoom)
        } else {
            content
        }
    }
}
