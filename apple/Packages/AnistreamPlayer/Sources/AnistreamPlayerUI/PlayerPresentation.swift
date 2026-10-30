// Where the player shows: in the main window on the Mac, where the browsing was (M4); a
// full-screen cover zooming out of the banner on iPhone and iPad (T2); or, on both, the system's
// Picture in Picture window.

import AnistreamDesign
import AnistreamStore
import SwiftUI
#if os(macOS)
import AppKit
#endif

extension View {
    /// This view, the app's main window's content, hosts the player. On the Mac the player takes
    /// the window's content while it plays, the browsing kept beneath it as it was; on iPhone and
    /// iPad it covers the window. On both it moves to the system's Picture in Picture window and
    /// back.
    public func hostsPlayer(_ model: PlayerModel) -> some View {
        modifier(HostsPlayer(model: model, over: EmptyModifier()))
    }

    /// The same, with `over` on the player on iPhone and iPad, where it covers the window: the
    /// sheets the player's own buttons open, Play on TV's among them, which the window cannot
    /// present while the cover is up.
    public func hostsPlayer<Over: ViewModifier>(_ model: PlayerModel, over: Over) -> some View {
        modifier(HostsPlayer(model: model, over: over))
    }
}

struct HostsPlayer<Over: ViewModifier>: ViewModifier {
    @Environment(AppStore.self) private var store
    let model: PlayerModel
    let over: Over
    @AppStorage(DevicePreferences.followSystemCaptions) private var followCaptions = true
    @Namespace private var zoom
    @State private var pip: PictureInPicture?
    #if os(macOS)
    @Environment(\.openWindow) private var openWindow

    /// The player has the main window: up, and not in Picture in Picture.
    private var inWindow: Bool { model.inMainWindow }
    #else
    @Environment(\.scenePhase) private var phase
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
        // Read here, so the cover follows it: a binding's getter is not read while the body is.
        let presented = model.inMainWindow
        content
            .environment(\.playerZoom, zoom)
            // The player's own `statusBarHidden` is not taken from a cover on iPad: the scene
            // asks the view beneath it, so that one hides the status bar while the player is up.
            .statusBarHidden(presented)
            .onChange(of: store.home?.cache, initial: true) { _, cache in
                model.shaderCache = cache?.appending(path: "mpv-shaders", directoryHint: .isDirectory)
            }
            // Down while the picture floats in Picture in Picture, and back up when it returns.
            .fullScreenCover(
                isPresented: Binding(
                    get: { presented },
                    // SwiftUI says so for its own dismissals too: the picture going into Picture
                    // in Picture, or the player already closed, are not the viewer closing it.
                    set: { up in if !up, model.inMainWindow { model.viewerClosed() } })
            ) {
                PhonePlayerView(model: model)
                    .modifier(over)
                    .environment(store)
                    .navigationTransition(.zoom(sourceID: model.staged?.id ?? 0, in: zoom))
                    .interactiveDismissDisabled()
                    .presentationBackground(.black)
            }
            .onAppear {
                guard pip == nil else { return }
                guard PictureInPicture.isSupported else {
                    PlayerModel.note(.info, "picture in picture: not on this device")
                    return
                }
                pip = PictureInPicture(model: model)
            }
            // Here rather than in the player, which is down while the picture floats: the audio
            // plays on in the background, and the video only for Picture in Picture.
            .onChange(of: phase) { _, phase in
                if phase == .background { model.backgrounded(true) }
                if phase == .active { model.backgrounded(false) }
            }
            .onReceive(powerChanges) { _ in model.powerChanged() }
        #endif
    }

    private var powerChanges: NotificationCenter.Publisher {
        NotificationCenter.default.publisher(for: .NSProcessInfoPowerStateDidChange)
    }
}
