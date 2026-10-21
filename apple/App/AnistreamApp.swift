// The app: one window over the store, on the Mac, iPad and iPhone.
//
// Built twice. Anistream starts the app's own engine on the real sources and AniList. Anistream
// Fixtures, compiled with FIXTURES against the testing engine, starts on a scene of the test kit's
// fixture titles in a new temporary directory, for tests and screenshots only.

import AnistreamDesign
import AnistreamKit
import AnistreamPlayerUI
import AnistreamStore
import AnistreamUI
import SwiftUI

@main
struct AnistreamApp: App {
    #if os(macOS)
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    #else
    @UIApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    #endif
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        #if os(macOS)
        // The one window: browsing, and the player in its place while it plays (M4).
        Window("anistream", id: "main") {
            content
                .focusedSceneValue(\.playerInFront, delegate.player.inMainWindow)
        }
        .defaultSize(width: 1280, height: 800)
        .windowResizability(.contentMinSize)
        .commands {
            AnistreamCommands(
                store: delegate.store, navigator: delegate.navigator,
                playerBack: .init(
                    isUp: { [player = delegate.player] in player.inMainWindow },
                    back: { [player = delegate.player] in player.requestBack() })
            ) {
                PlaybackMenu(model: delegate.player)
            } playerWindowItems: {
                PictureInPictureMenuItem(model: delegate.player)
            }
        }
        #else
        WindowGroup {
            content
                // The player covers the window it plays in, so it is in front while it shows.
                .focusedSceneValue(\.playerInFront, delegate.player.isPresented)
        }
        .commands {
            AnistreamCommands(store: delegate.store, navigator: delegate.navigator) {
                PlaybackMenu(model: delegate.player)
            } playerWindowItems: {
                EmptyView()
            }
        }
        #endif
    }

    private var content: some View {
        RootView()
            .hostsPlayer(delegate.player)
            .environment(\.playerUp, delegate.player.isPresented)
            .environment(delegate.store)
            .environment(delegate.navigator)
            #if os(macOS)
            .frame(minWidth: 960, minHeight: 600)
            #endif
            .task { await delegate.start() }
            // Where the app is, for the engine: in the background its torrents wait.
            .onChange(of: scenePhase, initial: true) { _, phase in
                switch phase {
                case .active: delegate.phases.scene(.active)
                case .background: delegate.phases.scene(.background)
                default: delegate.phases.scene(.inactive)
                }
            }
    }
}
