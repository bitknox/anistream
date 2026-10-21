// Where the player shows: a window of its own on the Mac (M4), a full-screen cover zooming out
// of the banner on iPhone and iPad (T2).

import AnistreamDesign
import AnistreamStore
import SwiftUI

extension View {
    /// The app's main window hosts the player: it opens and closes with the model, the Mac's in
    /// the `Window` scene with id `PlayerWindow.id`, the iPhone's and iPad's over this view.
    public func hostsPlayer(_ model: PlayerModel) -> some View {
        modifier(HostsPlayer(model: model))
    }
}

struct HostsPlayer: ViewModifier {
    @Environment(AppStore.self) private var store
    let model: PlayerModel
    #if os(macOS)
    @Environment(\.openWindow) private var openWindow
    @Environment(\.dismissWindow) private var dismissWindow
    #else
    @Namespace private var zoom
    #endif

    func body(content: Content) -> some View {
        #if os(macOS)
        content
            .onChange(of: model.isPresented) { _, presented in
                if presented {
                    openWindow(id: PlayerWindow.id)
                } else {
                    dismissWindow(id: PlayerWindow.id)
                }
            }
            .onReceive(powerChanges) { _ in model.powerChanged() }
        #else
        content
            .environment(\.playerZoom, zoom)
            .fullScreenCover(
                isPresented: Binding(
                    get: { model.isPresented },
                    set: { presented in if !presented { model.viewerClosed() } })
            ) {
                PhonePlayerView(model: model)
                    .environment(store)
                    .navigationTransition(.zoom(sourceID: model.staged?.id ?? 0, in: zoom))
                    .interactiveDismissDisabled()
                    .presentationBackground(.black)
            }
            .onReceive(powerChanges) { _ in model.powerChanged() }
        #endif
    }

    private var powerChanges: NotificationCenter.Publisher {
        NotificationCenter.default.publisher(for: .NSProcessInfoPowerStateDidChange)
    }
}

#if os(macOS)
/// The Mac's player window's content: `Window("Player", id: PlayerWindow.id) { PlayerWindow(…) }`.
public struct PlayerWindow: View {
    public static let id = "player"
    let model: PlayerModel

    public init(model: PlayerModel) {
        self.model = model
    }

    public var body: some View {
        MacPlayerView(model: model)
            .frame(minWidth: 480, minHeight: 270)
    }
}
#endif
