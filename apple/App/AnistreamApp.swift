// The app: one window over the store, on the Mac, iPad and iPhone.
//
// Built twice. Anistream starts the app's own engine on the real sources and AniList. Anistream
// Fixtures, compiled with FIXTURES against the testing engine, starts on a scene of the test kit's
// fixture titles in a new temporary directory, for tests and screenshots only.

import AnistreamDesign
import AnistreamKit
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

    var body: some Scene {
        #if os(macOS)
        Window("anistream", id: "main") {
            content
        }
        .defaultSize(width: 1280, height: 800)
        .windowResizability(.contentMinSize)
        #else
        WindowGroup {
            content
        }
        #endif
    }

    private var content: some View {
        RootView()
            .environment(delegate.store)
            #if os(macOS)
            .frame(minWidth: 960, minHeight: 600)
            #endif
            .task { await delegate.start() }
    }
}
