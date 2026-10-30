// Play trailer: the trailer watched properly, in a window on the Mac and a sheet on iPhone and iPad,
// with YouTube's own controls and the sound on.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// Which trailer the window or sheet plays: a YouTube id and the title it belongs to.
public struct TrailerRequest: Hashable, Codable, Sendable, Identifiable {
    public var title: String
    public var titleId: AnilistId
    public var videoId: String

    public var id: String { "\(titleId)-\(videoId)" }

    var trailer: Trailer { Trailer(host: .youTube, id: videoId) }

    init?(_ entry: Entry) {
        guard let trailer = entry.trailer, trailerEmbeddable(trailer: trailer) else { return nil }
        title = entry.showTitle
        titleId = entry.id
        videoId = trailer.id
    }
}

extension Navigator {
    /// Play trailer: the window or sheet where the player plays it, and where it does not (a
    /// trailer on another host) the watch page in the browser, as before.
    func playTrailer(_ entry: Entry, store: AppStore) {
        if let request = TrailerRequest(entry) {
            trailerRequest = request
        } else if let url = entry.trailerUrl {
            store.send(.playTrailer(url: url, title: entry.title))
        }
    }
}

/// One watch of a trailer in the window or sheet.
@MainActor @Observable
final class TrailerSession {
    enum Phase: Equatable {
        case loading
        case playing
        /// Nothing played within the wait, or the page failed: Try again.
        case failed
        /// The owner does not allow the embed: the browser takes over.
        case refused
    }

    private(set) var phase = Phase.loading
    private(set) var surface: (any TrailerSurface)?
    /// Whether the first moment of loading has passed, and the system's spinner may show.
    private(set) var waiting = false
    /// Whether the player is showing captions.
    private(set) var captions = false
    private var timers: [Task<Void, Never>] = []
    private let timeout: Duration

    init(timeout: Duration = .seconds(8)) {
        self.timeout = timeout
    }

    /// Play `trailer`, its captions in `subtitles`, the playback setting's language.
    func start(_ trailer: Trailer, subtitles: String) {
        stop()
        phase = .loading
        waiting = false
        captions = false
        surface = TrailerPlayer(trailer: trailer, style: .full, subtitles: subtitles) { [weak self] in
            self?.hear($0)
        }
        timers = [
            Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(600))
                guard !Task.isCancelled else { return }
                self?.waiting = true
            },
            Task { [weak self, timeout] in
                try? await Task.sleep(for: timeout)
                guard !Task.isCancelled else { return }
                self?.timedOut()
            },
        ]
    }

    private func timedOut() {
        guard phase == .loading else { return }
        stop()
        phase = .failed
    }

    func hear(_ signal: TrailerSignal) {
        switch signal {
        case .playing:
            guard phase == .loading else { return }
            phase = .playing
            timers.forEach { $0.cancel() }
        case .failed(let code):
            guard phase != .failed, phase != .refused else { return }
            stop()
            // Gone, private or embedding off: the owner's choice, so the watch page opens instead.
            phase = [2, 5, 100, 101, 150].contains(code) ? .refused : .failed
        case .captions(let on):
            captions = on
        case .quiet, .ending, .ended:
            break
        }
    }

    /// Let go of the web view.
    func stop() {
        timers.forEach { $0.cancel() }
        timers = []
        surface?.tearDown()
        surface = nil
    }
}

/// The window's or sheet's content.
public struct TrailerWindowView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let request: TrailerRequest
    @State private var session = TrailerSession()

    public init(request: TrailerRequest) {
        self.request = request
    }

    public var body: some View {
        let entry = store.titles.page(request.titleId)
        VStack(spacing: 0) {
            stage(entry)
            #if os(iOS)
            sheetFooter
            #endif
        }
        .background(.ground)
        .background(WindowShape())
        .navigationTitle(L.trailerWindowTitleApple(title: request.title))
        .onAppear { session.start(request.trailer, subtitles: store.settings.value(.subtitles) ?? "") }
        .onDisappear { session.stop() }
        .onChange(of: session.phase) { _, phase in
            guard phase == .refused else { return }
            if let url = entry.trailerUrl { store.send(.playTrailer(url: url, title: request.title)) }
            dismiss()
        }
        .modifier(EscapeCloses { dismiss() })
    }

    private func stage(_ entry: Entry) -> some View {
        ZStack {
            // The title's art fills the stage until the first frame, then fades out.
            HeroArtView(art: store.heroArt(entry), entry: entry)
                .opacity(session.phase == .playing ? 0 : 1)
                .animation(session.phase == .playing ? .fadeIn : .fadeOut, value: session.phase)
            if let player = session.surface as? TrailerPlayer, let view = player.webView {
                TrailerWebView(view: view)
            }
            switch session.phase {
            case .loading where session.waiting:
                ProgressView().controlSize(.regular)
            case .failed:
                VStack(spacing: 12) {
                    Text(L.trailerFailedApple).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                    Button(L.actionTryAgain) {
                        session.start(request.trailer, subtitles: store.settings.value(.subtitles) ?? "")
                    }
                        .buttonStyle(SecondaryCapsuleStyle(height: 34, fontSize: 13))
                }
                .padding(24)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.ground.opacity(0.85))
            default:
                EmptyView()
            }
        }
        .aspectRatio(16 / 9, contentMode: .fit)
        .frame(maxWidth: .infinity)
        .background(.black)
        .clipped()
    }

    #if os(iOS)
    private var sheetFooter: some View {
        HStack(spacing: 12) {
            Text(request.title)
                .font(.ui(17, weight: .semibold))
                .foregroundStyle(.silk)
                .lineLimit(2)
            Spacer(minLength: 8)
            Button(L.actionClose) { dismiss() }
                .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 15))
        }
        .padding(16)
    }
    #endif
}

#if os(macOS)
/// Keeps the Mac window's content 16:9 when it is resized, as the player window does.
private struct WindowShape: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            guard let window = view.window else { return }
            window.contentAspectRatio = NSSize(width: 16, height: 9)
            window.titlebarAppearsTransparent = false
            window.collectionBehavior.insert(.fullScreenNone)
        }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {}
}
#else
private struct WindowShape: View {
    var body: some View { EmptyView() }
}
#endif

/// Where Play trailer's request goes: a window of its own on the Mac, a sheet over the app on
/// iPhone and iPad.
struct TrailerPresentation: ViewModifier {
    @Environment(Navigator.self) private var navigator
    #if os(macOS)
    @Environment(\.openWindow) private var openWindow
    #endif

    func body(content: Content) -> some View {
        #if os(macOS)
        @Bindable var navigator = navigator
        if LaunchRoute.current.sheet == "trailer" {
            // A shot of the window: the screenshot harness has one window, so the trailer shows in
            // a sheet over it.
            content.sheet(item: $navigator.trailerRequest) { TrailerWindowView(request: $0).frame(width: 960) }
        } else {
            content.onChange(of: navigator.trailerRequest) { _, request in
                guard let request else { return }
                navigator.trailerRequest = nil
                openWindow(id: TrailerRequest.windowId, value: request)
            }
        }
        #else
        @Bindable var navigator = navigator
        content.sheet(item: $navigator.trailerRequest) { request in
            TrailerWindowView(request: request)
                .presentationDetents([.medium, .large])
                .presentationDragIndicator(.visible)
        }
        #endif
    }
}

public extension TrailerRequest {
    /// The id of the Mac window group that plays a request, which the app declares.
    static let windowId = "trailer" // i18n-skip: a scene id
}

/// Escape closes the Mac window; a sheet has its own swipe and Close button.
private struct EscapeCloses: ViewModifier {
    let close: () -> Void

    func body(content: Content) -> some View {
        #if os(macOS)
        content.onExitCommand(perform: close)
        #else
        content
        #endif
    }
}

extension View {
    /// The Fixtures app opens Play trailer at launch for a shot: sheet=trailer.
    func trailerLaunchSheet(_ entry: Entry) -> some View {
        modifier(TrailerLaunchSheet(entry: entry))
    }
}

private struct TrailerLaunchSheet: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    func body(content: Content) -> some View {
        content.task {
            guard navigator.launchSheet == "trailer" else { return } // i18n-skip: a shot's sheet name
            navigator.launchSheet = nil
            try? await Task.sleep(for: .seconds(2))
            navigator.playTrailer(store.titles.page(entry.id), store: store)
        }
    }
}
