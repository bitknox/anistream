// The web view a trailer plays in, and what its page says: one view for the title page's banner,
// the hover preview and the Play trailer window or sheet.

import AnistreamKit
import SwiftUI
import WebKit

/// A trailer's `WKWebView`, made the way the trailers spec asks: the engine's page loaded under the
/// app's identifier so YouTube sees a Referer, no cookies or storage kept, inline and muted
/// autoplay, no new windows and, for the banner and the preview, no pointer or touch input at all.
@MainActor
final class TrailerPlayer: NSObject, TrailerSurface {
    /// The app's identifier as YouTube knows it: the Referer and the player's `origin`.
    static let identity = AppIdentity(origin: "https://tv.anistream.app")

    /// A page to load in place of the engine's, for the Fixtures app's screenshots, where no
    /// video may be fetched: it says the same things through the same bridge.
    nonisolated(unsafe) static var standIn: (@MainActor (Trailer) -> String)?

    let webView: WKWebView
    private let style: EmbedStyle
    private let onSignal: @MainActor (TrailerSignal) -> Void
    private var released = false

    /// Whether the page can play this trailer at all (YouTube's only).
    let playable: Bool

    init(trailer: Trailer, style: EmbedStyle, onSignal: @escaping @MainActor (TrailerSignal) -> Void) {
        self.style = style
        self.onSignal = onSignal
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.mediaTypesRequiringUserActionForPlayback = []
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        #if os(iOS)
        configuration.allowsInlineMediaPlayback = true
        configuration.allowsPictureInPictureMediaPlayback = false
        #endif
        configuration.allowsAirPlayForMediaPlayback = false
        let bridge = Bridge()
        configuration.userContentController.add(bridge, name: "anistream") // i18n-skip: the page's bridge
        #if os(macOS)
        let view: WKWebView = style == .banner ? PassiveWebView(frame: .zero, configuration: configuration)
            : WKWebView(frame: .zero, configuration: configuration)
        view.setValue(false, forKey: "drawsBackground") // i18n-skip: transparent until it plays
        #else
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.isOpaque = false
        view.backgroundColor = .clear
        view.scrollView.backgroundColor = .clear
        view.scrollView.isScrollEnabled = false
        view.scrollView.bounces = false
        if style == .banner { view.isUserInteractionEnabled = false }
        #endif
        webView = view
        let page: String? = Self.standIn.map { $0(trailer) }
            ?? trailerEmbedPage(trailer: trailer, style: style, app: Self.identity)
        playable = page != nil
        super.init()
        bridge.player = self
        view.navigationDelegate = self
        view.uiDelegate = self
        view.allowsLinkPreview = false
        if let page, let base = URL(string: trailerBaseUrl(app: Self.identity)) {
            view.loadHTMLString(page, baseURL: base)
        } else {
            Task { @MainActor in self.onSignal(.failed(0)) }
        }
    }

    /// Tell the page to play, pause, mute or unmute.
    func control(_ control: TrailerControl) {
        guard !released else { return }
        webView.evaluateJavaScript(trailerScript(control: control), completionHandler: nil)
    }

    /// Stop, and let go of the view's decoder and everything it kept.
    func tearDown() {
        guard !released else { return }
        released = true
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "anistream") // i18n-skip
        webView.stopLoading()
        webView.loadHTMLString("", baseURL: nil)
        webView.removeFromSuperview()
    }

    fileprivate func heard(_ text: String) {
        guard !released else { return }
        if text.trimmingCharacters(in: .whitespaces) == "ending" { // i18n-skip: the page's word
            onSignal(.ending)
            return
        }
        switch trailerMessage(message: text) {
        case .playing: onSignal(.playing)
        case .ready, .paused: onSignal(.quiet)
        case .ended: onSignal(.ended)
        case .error(let code): onSignal(.failed(Int(code)))
        case nil: break
        }
    }

    /// Hears the page without keeping the player alive.
    private final class Bridge: NSObject, WKScriptMessageHandler {
        weak var player: TrailerPlayer?

        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let text = message.body as? String else { return }
            Task { @MainActor in self.player?.heard(text) }
        }
    }
}

extension TrailerPlayer: WKNavigationDelegate, WKUIDelegate {
    /// The page is the only page: nothing the player does navigates the view away.
    func webView(
        _ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        // The page's own load, and the player's frames inside it; a link or a redirect is not.
        let main = action.targetFrame?.isMainFrame ?? true
        decisionHandler(!main || action.navigationType == .other ? .allow : .cancel)
    }

    func webView(
        _ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
        for action: WKNavigationAction, windowFeatures: WKWindowFeatures
    ) -> WKWebView? { nil }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: any Error) {
        onSignal(.failed(0))
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) {
        onSignal(.failed(0))
    }
}

#if os(macOS)
/// A web view the pointer passes through: the banner and the preview take no mouse input.
private final class PassiveWebView: WKWebView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var acceptsFirstResponder: Bool { false }
    override func menu(for event: NSEvent) -> NSMenu? { nil }
}

struct TrailerWebView: NSViewRepresentable {
    let player: TrailerPlayer

    func makeNSView(context: Context) -> WKWebView { player.webView }
    func updateNSView(_ view: WKWebView, context: Context) {}
}
#else
struct TrailerWebView: UIViewRepresentable {
    let player: TrailerPlayer

    func makeUIView(context: Context) -> WKWebView { player.webView }
    func updateUIView(_ view: WKWebView, context: Context) {}
}
#endif

/// The Fixtures app's trailers: a page of flat grey shapes in place of the video, saying the same
/// things through the same bridge, so screenshots and click tests need no network and fetch
/// nothing. Only the Fixtures app installs it.
public enum TrailerStandIn {
    /// Whether a window that is not key keeps its trailer: a snapshot's window never is.
    nonisolated(unsafe) static var keepsWhenInactive = false

    @MainActor public static func install(keepsWhenInactive: Bool = false) {
        Self.keepsWhenInactive = keepsWhenInactive
        TrailerPlayer.standIn = { trailer in page(endsAfter: trailer.id.hasSuffix("-ends") ? 3 : nil) }
    }

    @MainActor static func page(endsAfter: Double?) -> String {
        let ending = endsAfter.map { "setTimeout(function(){say('ending');setTimeout(function(){say('ended')},500)}, \(Int($0 * 1000)));" } ?? ""
        return """
        <!doctype html><html><head><meta charset="utf-8"><style>
        html,body{margin:0;width:100%;height:100%;overflow:hidden;background:#4a4f63}
        .sun{position:absolute;left:6%;top:16%;width:15%;height:26.67vw;max-height:42%;border-radius:50%;background:#8c92a7;animation:sun 12s linear infinite}
        .hills{position:absolute;left:0;bottom:0;width:200%;height:46%;animation:drift 6s linear infinite}
        @keyframes sun{from{transform:translateX(0)}to{transform:translateX(480%)}}
        @keyframes drift{from{transform:translateX(0)}to{transform:translateX(-50%)}}
        </style></head><body><div class="sun"></div>
        <svg class="hills" viewBox="0 0 400 100" preserveAspectRatio="none"><path d="M0 60 L40 35 L80 55 L120 25 L160 50 L200 60 L240 35 L280 55 L320 25 L360 50 L400 60 V100 H0 Z" fill="#3b4054"/><path d="M0 80 L50 68 L100 82 L150 70 L200 80 L250 68 L300 82 L350 70 L400 80 V100 H0 Z" fill="#2f3345"/></svg>
        <script>
        function say(m){try{window.webkit.messageHandlers.anistream.postMessage(m)}catch(e){}}
        window.trailer={play:function(){say('playing')},pause:function(){say('paused')},mute:function(){},unmute:function(){}};
        say('ready');setTimeout(function(){say('playing')},1200);\(ending)
        </script></body></html>
        """
    }
}
