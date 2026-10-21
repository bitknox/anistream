// Where the player shows: in the main window on the Mac, where the browsing was (M4), or in the
// system's Picture in Picture window; a full-screen cover zooming out of the banner on iPhone and
// iPad (T2).

import AnistreamDesign
import AnistreamStore
import SwiftUI
#if os(macOS)
import AppKit
#endif

extension View {
    /// This view, the app's main window's content, hosts the player. On the Mac the player takes
    /// the window's content while it plays, the browsing kept beneath it as it was, and moves to
    /// the system's Picture in Picture window and back; on iPhone and iPad it covers the window.
    public func hostsPlayer(_ model: PlayerModel) -> some View {
        modifier(HostsPlayer(model: model))
    }
}

struct HostsPlayer: ViewModifier {
    @Environment(AppStore.self) private var store
    let model: PlayerModel
    @AppStorage(DevicePreferences.followSystemCaptions) private var followCaptions = true
    @Namespace private var zoom
    #if os(macOS)
    @Environment(\.openWindow) private var openWindow
    @State private var pip: PictureInPicture?

    /// The player has the main window: up, and not in Picture in Picture.
    private var inWindow: Bool { model.inMainWindow }
    #endif

    func body(content: Content) -> some View {
        players(content)
            // Plain-text subtitles in the system's caption style, as the setting says, and as the
            // viewer changes the style.
            .onChange(of: followCaptions, initial: true) { _, follow in model.followsSystemCaptions = follow }
            .onReceive(NotificationCenter.default.publisher(for: CaptionStyle.changed)) { _ in
                model.captionStyleChanged()
            }
    }

    @ViewBuilder
    private func players(_ content: Content) -> some View {
        #if os(macOS)
        content
            // The browsing stays where it was, under the player, for Back to return to.
            .opacity(inWindow ? 0 : 1)
            .allowsHitTesting(!inWindow)
            .accessibilityHidden(inWindow)
            .overlay {
                if inWindow {
                    MacPlayerView(model: model)
                        .transition(.opacity)
                }
            }
            .animation(.standard, value: inWindow)
            .environment(\.playerZoom, zoom)
            .onAppear {
                guard pip == nil else { return }
                let pip = PictureInPicture(model: model)
                pip.bringBackWindow = { [openWindow] in
                    // Out of the Dock and out of hiding, in front.
                    NSApp.unhide(nil)
                    for window in NSApp.windows where window.isMiniaturized { window.deminiaturize(nil) }
                    openWindow(id: "main")
                    NSApp.activate()
                }
                self.pip = pip
            }
            // Leaving the window while it plays takes the picture into Picture in Picture.
            .onReceive(NotificationCenter.default.publisher(for: NSApplication.willHideNotification)) { _ in
                pip?.leaving()
            }
            .onReceive(NotificationCenter.default.publisher(for: NSWindow.willMiniaturizeNotification)) { _ in
                guard model.inMainWindow else { return }
                pip?.leaving()
            }
            .onChange(of: store.home?.cache, initial: true) { _, cache in
                model.shaderCache = cache?.appending(path: "mpv-shaders", directoryHint: .isDirectory)
            }
            .onReceive(powerChanges) { _ in model.powerChanged() }
        #else
        content
            .environment(\.playerZoom, zoom)
            .onChange(of: store.home?.cache, initial: true) { _, cache in
                model.shaderCache = cache?.appending(path: "mpv-shaders", directoryHint: .isDirectory)
            }
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
