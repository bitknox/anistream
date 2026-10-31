// The web view a trailer plays in, and what its page says: the hover preview and the Play trailer
// window or sheet. The views and their store last the run, and each one plays trailer after
// trailer, so a hover asks YouTube only for the video.

import AnistreamKit
import AnistreamStore
import os
import SwiftUI
import WebKit

/// A trailer playing in one of the pool's web views, made the way the trailers spec asks: the
/// engine's page loaded under the app's identifier so YouTube sees a Referer, nothing kept past the
/// run, inline and muted autoplay, no new windows and, for the preview, no pointer or touch input.
@MainActor
final class TrailerPlayer: NSObject, TrailerSurface {
    /// The app's identifier as YouTube knows it: the Referer and the player's `origin`.
    static let identity = AppIdentity(origin: "https://tv.anistream.app")

    /// A page to load in place of the engine's, for the Fixtures app's screenshots, where no
    /// video may be fetched: it says the same things through the same bridge.
    nonisolated(unsafe) static var standIn: (@MainActor (Trailer) -> String)?

    /// The page's view, or none when nothing is asked for.
    var webView: WKWebView? { page?.webView }
    private let page: TrailerPage?
    private let pool: TrailerPool
    private let trailer: Trailer
    private let onSignal: @MainActor (TrailerSignal) -> Void
    private let gate: TrailerGate
    private let started = ContinuousClock.now
    /// Whether the page was already up, and only the video was asked for.
    private let warm: Bool
    private var released = false
    /// The last thing the page said, for the log when the visit gives up.
    private var lastHeard = "nothing"

    /// Whether the page can play this trailer at all (YouTube's only).
    let playable: Bool

    /// `subtitles` is the playback setting's language, which captions follow.
    init(
        trailer: Trailer, style: EmbedStyle, subtitles: String, pool: TrailerPool = .shared,
        gate: TrailerGate = .shared, onSignal: @escaping @MainActor (TrailerSignal) -> Void
    ) {
        self.trailer = trailer
        self.pool = pool
        self.onSignal = onSignal
        self.gate = gate
        playable = trailerEmbeddable(trailer: trailer)
        let asks = playable && gate.mayAsk(trailer.id)
        let taken = Self.standIn.map { (pool.standIn($0(trailer), style: style), false) }
            ?? (asks ? pool.take(trailer, style: style, subtitles: subtitles) : (nil, false))
        page = taken.0
        warm = taken.1
        super.init()
        guard let page else {
            // Not YouTube's, or refused before this run: nothing is asked, and it says so as a
            // refusal, so Play trailer opens the watch page.
            let code = playable ? 150 : 0
            if playable { AppLog.debug("trailer", "not asking YouTube for \(trailer.id) again this run") }
            Task { @MainActor in self.onSignal(.failed(code)) }
            return
        }
        page.hear = { [weak self] text in self?.heard(text) }
        AppLog.debug("trailer", "loading \(trailer.id), \(warm ? "warm" : "cold") web view")
    }

    /// Tell the page to play, pause, mute or unmute.
    func control(_ control: TrailerControl) {
        guard !released, let page else { return }
        page.run(trailerScript(control: control))
    }

    /// Stop, and hand the view back to the pool, paused.
    func tearDown() {
        guard !released else { return }
        released = true
        page?.hear = nil
        page.map { pool.give($0) }
    }

    /// Nothing played in time: logged with what the page last said and YouTube's state now, read
    /// before the pool pauses it.
    func gaveUp(after: Duration) {
        let ms = Int(after.components.seconds * 1000 + after.components.attoseconds / 1_000_000_000_000_000)
        let line = "\(trailer.id) gave up after \(ms) ms, never playing; last heard \(lastHeard), \(warm ? "warm" : "cold")"
        guard let page else {
            AppLog.warn("trailer", line)
            return
        }
        page.webView.evaluateJavaScript(trailerStateScript()) { value, _ in
            let state = (value as? NSNumber).map { "\($0.intValue)" } ?? "none" // i18n-skip: a log value
            AppLog.warn("trailer", "\(line), player state \(state)")
        }
    }

    private func heard(_ text: String) {
        guard !released else { return }
        let word = text.trimmingCharacters(in: .whitespaces)
        lastHeard = word
        // The page's words that are not the player's state. i18n-skip: the page's words
        switch word {
        case "ending": onSignal(.ending); return // i18n-skip
        case "captions:on": onSignal(.captions(true)); return // i18n-skip
        case "captions:off": onSignal(.captions(false)); return // i18n-skip
        default: break
        }
        let ms = AppLog.ms(since: started)
        switch trailerMessage(message: text) {
        case .playing:
            gate.played()
            AppLog.debug("trailer", "\(trailer.id) playing, shown after \(ms) ms, \(warm ? "warm" : "cold")")
            page?.reveal()
            onSignal(.playing)
        case .ready:
            AppLog.debug("trailer", "\(trailer.id) player ready after \(ms) ms")
            onSignal(.quiet)
        case .paused:
            AppLog.debug("trailer", "\(trailer.id) paused after \(ms) ms")
            onSignal(.quiet)
        case .ended:
            AppLog.debug("trailer", "\(trailer.id) ended after \(ms) ms")
            onSignal(.ended)
        case .error(let code):
            let code = Int(code)
            AppLog.warn("trailer", "\(trailer.id) failed after \(ms) ms: error \(code), \(Self.meaning(code))")
            if gate.failed(trailer.id, code: code) {
                AppLog.warn("trailer", "YouTube refused two trailers in a row: no more trailers this run")
            }
            onSignal(.failed(code))
        case nil:
            break
        }
    }

    /// What YouTube's error code says, for the log.
    static func meaning(_ code: Int) -> String {
        switch code {
        case 0: "the page or the player API did not load"
        case 2: "a bad video id"
        case 5: "the HTML5 player failed"
        case 100: "the video is gone or private"
        case 101, 150: "embedding not allowed, or YouTube's sign-in check (a flagged network, such as a VPN exit)"
        case 152: "the embed's identity was refused"
        case 153: "the embed sent no identity (Referer)"
        default: "an unknown code"
        }
    }
}

/// The trailers' web views for the whole run, and the one store they share.
///
/// The store is in memory only: nothing is written to disk and nothing outlives the run. Within the
/// run, the player's scripts and YouTube's cookies are kept, and one Networking process serves
/// every view. A view handed back is paused and kept, its player loaded, to play the next trailer
/// through `loadVideoById`: two of each style, as a preview and Play trailer can be up together
/// with another on its way out.
@MainActor
final class TrailerPool {
    static let shared = TrailerPool()

    let store = WKWebsiteDataStore.nonPersistent()
    private var idle: [TrailerPage] = []
    private let keep = 2
    private var warmed = false

    /// Signposts round the WKWebView calls the trailers make on the main thread: creating a view,
    /// loading its page and running a script. The page's work runs in WebKit's own processes.
    static let signposts = OSSignposter(subsystem: "tv.anistream", category: "trailer") // i18n-skip: a log category

    /// Load the preview's player ahead of the first hover, with no video in it: once a run, and
    /// nothing after the first call. The page is built off the main actor; creating the web view
    /// and loading it are the main thread's part, and the first web view of the run starts
    /// WebKit's processes, so a caller picks a still moment.
    func prewarm(subtitles: String) async {
        guard !warmed, Self.standInFree else { return }
        warmed = true
        let identity = TrailerPlayer.identity
        let built = await Task.detached(priority: .utility) {
            (trailerPage(trailer: nil, style: .banner, app: identity, subtitleLanguage: subtitles), trailerBaseUrl(app: identity))
        }.value
        guard let html = built.0, let base = URL(string: built.1) else { return }
        let page = Self.signposts.withIntervalSignpost("create web view") { // i18n-skip: a signpost
            TrailerPage(style: .banner, store: store, pooled: true, subtitles: subtitles)
        }
        Self.signposts.withIntervalSignpost("load page") { // i18n-skip: a signpost
            _ = page.webView.loadHTMLString(html, baseURL: base)
        }
        idle.append(page)
        AppLog.debug("trailer", "the preview's player is loading ahead of the first hover")
    }

    /// The warm preview page, out of the pool, asked to play `trailer`: for priming, which gives
    /// it back. None when no page is warm.
    func takeWarm(_ trailer: Trailer, subtitles: String) -> TrailerPage? {
        guard let index = idle.firstIndex(where: { $0.style == .banner && !$0.broken && $0.subtitles == subtitles }),
              let script = trailerLoadScript(trailer: trailer) else { return nil }
        let page = idle.remove(at: index)
        page.run(script)
        return page
    }

    /// Whether the real page is in use, not the Fixtures app's stand-in.
    private static var standInFree: Bool { TrailerPlayer.standIn == nil }

    /// A page for `trailer`: one already up when there is one, which then only switches video.
    func take(_ trailer: Trailer, style: EmbedStyle, subtitles: String) -> (TrailerPage?, warm: Bool) {
        // A page made for another subtitle language is let go: its captions were set as it loaded.
        for page in idle where page.broken || page.subtitles != subtitles { page.close() }
        idle.removeAll { $0.broken || $0.subtitles != subtitles }
        if let index = idle.firstIndex(where: { $0.style == style }),
           let script = trailerLoadScript(trailer: trailer) {
            let page = idle.remove(at: index)
            page.run(script)
            return (page, true)
        }
        guard let html = trailerPage(trailer: trailer, style: style, app: TrailerPlayer.identity, subtitleLanguage: subtitles),
              let base = URL(string: trailerBaseUrl(app: TrailerPlayer.identity)) else { return (nil, false) }
        let page = TrailerPage(style: style, store: store, pooled: true, subtitles: subtitles)
        page.webView.loadHTMLString(html, baseURL: base)
        return (page, false)
    }

    /// The Fixtures app's page: never kept, so each shot starts as the first did.
    func standIn(_ html: String, style: EmbedStyle) -> TrailerPage {
        let page = TrailerPage(style: style, store: store, pooled: false, subtitles: "")
        page.webView.loadHTMLString(html, baseURL: nil)
        return page
    }

    /// Take a page back: hidden and out of the window first, then paused, so neither the player's
    /// pause screen nor its end screen is ever seen. Kept while there is room.
    func give(_ page: TrailerPage) {
        page.hide()
        page.webView.removeFromSuperview()
        page.run(trailerScript(control: .pause))
        guard page.pooled, !page.broken, idle.filter({ $0.style == page.style }).count < keep else {
            page.close()
            return
        }
        idle.append(page)
    }
}

/// One web view with the engine's page in it, and its bridge.
@MainActor
final class TrailerPage: NSObject {
    let webView: WKWebView
    let style: EmbedStyle
    let pooled: Bool
    /// The subtitle language its captions were set up for.
    let subtitles: String
    /// Who hears the page now: the trailer playing in it.
    var hear: ((String) -> Void)?
    /// The page failed to load or its process ended: not kept.
    private(set) var broken = false
    /// Whether the page has loaded, and its `window.trailer` is there to be told things.
    private var loaded = false
    private var queued: [String] = []

    init(style: EmbedStyle, store: WKWebsiteDataStore, pooled: Bool, subtitles: String) {
        self.style = style
        self.pooled = pooled
        self.subtitles = subtitles
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = store
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
        // In a preview, YouTube's own controls are hidden in its frame; Play trailer keeps them.
        if style == .banner {
            configuration.userContentController.addUserScript(
                WKUserScript(source: trailerChromeScript(), injectionTime: .atDocumentEnd, forMainFrameOnly: false)
            )
        }
        #endif
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
        super.init()
        bridge.page = self
        view.navigationDelegate = self
        view.uiDelegate = self
        view.allowsLinkPreview = false
        hide()
    }

    /// Run `script` in the page now, or once it has loaded: a hover can come before a page loaded
    /// ahead of it is up.
    func run(_ script: String) {
        guard loaded else {
            queued.append(script)
            return
        }
        TrailerPool.signposts.withIntervalSignpost("run script") { // i18n-skip: a signpost
            webView.evaluateJavaScript(script, completionHandler: nil)
        }
    }

    /// Unseen until the page says a frame is up, so YouTube's loading screen never shows.
    func hide() {
        #if os(macOS)
        webView.alphaValue = 0
        #else
        webView.alpha = 0
        #endif
    }

    func reveal() {
        #if os(macOS)
        webView.alphaValue = 1
        #else
        webView.alpha = 1
        #endif
    }

    /// Let go of the view, its decoder and everything it kept.
    func close() {
        hear = nil
        webView.configuration.userContentController.removeScriptMessageHandler(forName: "anistream") // i18n-skip
        webView.stopLoading()
        webView.loadHTMLString("", baseURL: nil)
        webView.removeFromSuperview()
    }

    fileprivate func fail(_ reason: String) {
        broken = true
        AppLog.warn("trailer", "the trailer's web view failed: \(AppLog.redacting(reason))")
        hear?("error:0") // i18n-skip: the page's word
    }

    /// Hears the page without keeping it alive.
    private final class Bridge: NSObject, WKScriptMessageHandler {
        weak var page: TrailerPage?

        func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
            guard let text = message.body as? String else { return }
            Task { @MainActor in
                if text == trailerChromeMessage() {
                    AppLog.debug("trailer", "YouTube's controls hidden in the preview's player")
                    return
                }
                if text.hasPrefix("bytes:"), let bytes = Int(text.dropFirst(6)) { // i18n-skip: the page's word
                    AppLog.debug("trailer", "the preview's player fetched \(bytes / 1024) kB before the first hover")
                    return
                }
                self.page?.hear?(text)
            }
        }
    }
}

extension TrailerPage: WKNavigationDelegate, WKUIDelegate {
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

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        loaded = true
        let scripts = queued
        queued = []
        scripts.forEach { webView.evaluateJavaScript($0, completionHandler: nil) }
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: any Error) {
        fail("\(error)")
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) {
        fail("\(error)")
    }

    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        fail("its process ended")
    }
}

#if os(macOS)
/// A web view the pointer passes through: the preview takes no mouse input.
private final class PassiveWebView: WKWebView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var acceptsFirstResponder: Bool { false }
    override func menu(for event: NSEvent) -> NSMenu? { nil }
}

/// The page's view inside a holder of this place's own, so a view the pool moves on to another
/// place is never pulled out of it when this one goes.
struct TrailerWebView: NSViewRepresentable {
    let view: WKWebView

    func makeNSView(context: Context) -> NSView {
        let holder = Holder()
        holder.passive = view is PassiveWebView
        adopt(into: holder)
        return holder
    }

    func updateNSView(_ holder: NSView, context: Context) { adopt(into: holder) }

    static func dismantleNSView(_ holder: NSView, coordinator: ()) {
        holder.subviews.forEach { $0.removeFromSuperview() }
    }

    /// Passes the pointer through, as its web view does, for the preview's.
    final class Holder: NSView {
        var passive = false
        override func hitTest(_ point: NSPoint) -> NSView? { passive ? nil : super.hitTest(point) }
        override var acceptsFirstResponder: Bool { false }
    }

    private func adopt(into holder: NSView) {
        guard view.superview !== holder else { return }
        view.removeFromSuperview()
        view.frame = holder.bounds
        view.autoresizingMask = [.width, .height]
        holder.addSubview(view)
    }
}
#else
/// The page's view inside a holder of this place's own, so a view the pool moves on to another
/// place is never pulled out of it when this one goes.
struct TrailerWebView: UIViewRepresentable {
    let view: WKWebView

    func makeUIView(context: Context) -> UIView {
        let holder = UIView()
        holder.backgroundColor = .clear
        adopt(into: holder)
        return holder
    }

    func updateUIView(_ holder: UIView, context: Context) { adopt(into: holder) }

    static func dismantleUIView(_ holder: UIView, coordinator: ()) {
        holder.subviews.forEach { $0.removeFromSuperview() }
    }

    private func adopt(into holder: UIView) {
        guard view.superview !== holder else { return }
        view.removeFromSuperview()
        view.frame = holder.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        holder.addSubview(view)
    }
}
#endif

/// The Fixtures app's trailers: a page of flat grey shapes in place of the video, saying the same
/// things through the same bridge, so screenshots and click tests need no network and fetch
/// nothing. Only the Fixtures app installs it.
public enum TrailerStandIn {
    @MainActor public static func install() {
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
