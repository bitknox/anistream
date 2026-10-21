// Where the player grows out of: the banner a play started from. The iPhone's and iPad's cover
// zooms out of it (T2); on the Mac the banner morphs into the video in the main window (M4).

import SwiftUI

extension EnvironmentValues {
    /// The namespace the player grows in from a banner, which the player's host provides.
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
            #if os(macOS)
            content.matchedGeometryEffect(id: id, in: zoom, properties: .frame, isSource: true)
            #else
            content.matchedTransitionSource(id: id, in: zoom)
            #endif
        } else {
            content
        }
    }
}
