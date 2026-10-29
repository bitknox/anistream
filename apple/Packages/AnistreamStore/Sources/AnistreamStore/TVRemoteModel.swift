// Play on TV: the TVs on the network and the ones paired, pairing with a code or a scanned QR
// link, and the now-playing card of the TV this device is driving.
//
// The engine's sender (`FfiTvSender`) holds every rule: which rows the picker lists and in what
// order, what a row says under its name, what a failed pairing tells the viewer, and what the
// card reads. This model only keeps its latest answers on the main actor for the screens, and
// hands the platform's DNS-SD browser to it (`BonjourTVBrowser`, or the Fixtures app's made-up
// TVs).

import AnistreamKit
import AnistreamStrings
import Foundation
import Network
import Observation
#if canImport(UIKit)
import UIKit
#endif

/// What Play on TV plays: the episode, what this device calls the title, and where this device
/// was in it, which the TV starts from.
public struct TVPlayRequest: Equatable, Sendable {
    public var id: AnilistId
    public var title: String
    public var episode: String
    public var from: Double?
    /// Asked from the player: once the TV plays it, the player here closes.
    public var fromPlayer: Bool

    public init(id: AnilistId, title: String, episode: String, from: Double? = nil, fromPlayer: Bool = false) {
        self.id = id
        self.title = title
        self.episode = episode
        self.from = from
        self.fromPlayer = fromPlayer
    }
}

/// A pairing under way: the TV, whether a code is to be typed, and how the last try went.
public struct TVPairingStep: Equatable, Sendable {
    public var tvName: String
    /// The TV shows six digits to type here. False for a scanned link, which carried them.
    public var needsCode: Bool
    /// A try is out, or the TV is being asked for its code.
    public var working: Bool
    /// The engine's words for the last try that failed.
    public var problem: String?
    /// The code on the TV ran out, or took too many wrong tries: nothing more can be typed
    /// until a new one is asked for.
    public var expired = false

    public init(tvName: String, needsCode: Bool, working: Bool, problem: String? = nil, expired: Bool = false) {
        self.tvName = tvName
        self.needsCode = needsCode
        self.working = working
        self.problem = problem
        self.expired = expired
    }
}

@Observable @MainActor
public final class TVRemoteModel {
    /// What the TV sheet shows.
    public enum Sheet: Equatable, Sendable {
        /// The TVs to choose from, and pairing with one.
        case picker
        /// The now-playing card of the TV connected.
        case remote
    }

    /// The sheet up, if any.
    public var sheet: Sheet?
    /// What choosing a TV plays. Nil when the picker was opened to pair or connect only.
    public private(set) var request: TVPlayRequest?
    public private(set) var rows: [FfiTvRow] = []
    public private(set) var paired: [FfiPairedTv] = []
    public private(set) var browsing = false
    public private(set) var pairing: TVPairingStep?
    /// The TV this device drives, by fingerprint, while a link to it is open.
    public private(set) var connected: String?
    /// The card as the TV last said it, and when, for the playhead to move on from.
    public private(set) var card: FfiRemoteCard?
    public private(set) var cardAt = Date.now
    /// Something the picker or the card has to say: a command that did not get through.
    public private(set) var problem: String?
    /// This device's fingerprint in short, as a TV's settings list it.
    public private(set) var shortFingerprint = ""
    /// What Play on TV asked the TV to play, until the TV's card names it: the card shows it
    /// starting meanwhile, rather than the TV's last episode or nothing.
    public private(set) var starting: TVPlayRequest?
    /// When the link to the TV was lost, while it is: the playhead stops there.
    public private(set) var lostAt: Date?

    /// Called once the TV plays what the player here was playing: the app closes its player.
    @ObservationIgnored public var handedOff: (@MainActor () -> Void)?

    @ObservationIgnored private(set) var sender: FfiTvSender?
    @ObservationIgnored private(set) var browser: (any FfiTvBrowser)?
    @ObservationIgnored private var steps: FfiTvPairing?
    @ObservationIgnored private var link: FfiTvLink?
    @ObservationIgnored private var watcher: Task<Void, Never>?
    @ObservationIgnored private var cards: Task<Void, Never>?
    @ObservationIgnored private var path: NWPathMonitor?
    @ObservationIgnored private var browsers = 0

    public init() {}

    /// Whether Play on TV is there: the engine was built with the remote and gave a sender.
    public var available: Bool { sender != nil }

    /// Take the engine's sender, browsing with `browser` while the picker is up.
    public func attach(_ sender: FfiTvSender, browser: any FfiTvBrowser) {
        self.sender = sender
        self.browser = browser
        shortFingerprint = sender.shortFingerprint()
        refresh()
        watcher?.cancel()
        watcher = Task { [weak self, sender] in
            while !Task.isCancelled {
                await sender.changed()
                guard let self else { return }
                self.refresh()
            }
        }
    }

    func refresh() {
        guard let sender else { return }
        rows = sender.rows()
        paired = sender.paired()
    }

    // MARK: Play on TV

    /// Play on TV, from a title, an episode or the player: straight to the TV driven already,
    /// otherwise the picker to choose one.
    public func offer(_ request: TVPlayRequest) {
        guard available else { return }
        problem = nil
        self.request = request
        if let connected, card?.phase == .connected {
            play(on: connected)
        } else {
            sheet = .picker
        }
    }

    /// The picker, to pair or connect, with nothing to play.
    public func openPicker() {
        guard available else { return }
        request = nil
        problem = nil
        sheet = .picker
    }

    /// The card of the TV driven, or the picker when there is none.
    public func openRemote() {
        sheet = connected == nil ? .picker : .remote
    }

    /// The sheet closed. A pairing under way is let go; the link stays, for the card to come
    /// back to.
    public func sheetClosed() {
        sheet = nil
        request = nil
        cancelPairing()
        // A session the TV ended while its card was up ends here as the card goes.
        if card?.over == true { disconnect() }
    }

    /// Look for TVs while a picker is up. Counted, as two can be up at once (a Mac's Settings
    /// and the sheet).
    public func startBrowsing() {
        guard let sender, let browser else { return }
        browsers += 1
        guard browsers == 1 else { return }
        browsing = true
        sender.browse(browser: browser)
        refresh()
    }

    public func stopBrowsing() {
        guard let sender, browsers > 0 else { return }
        browsers -= 1
        guard browsers == 0 else { return }
        browsing = false
        sender.stopBrowsing()
        refresh()
    }

    /// A row of the picker chosen: a paired TV is connected to and plays what was asked; a new
    /// one starts pairing.
    public func choose(_ row: FfiTvRow) {
        switch row.kind {
        case .paired, .pairedAway:
            if let fingerprint = row.fingerprint { use(fingerprint) }
        case .new:
            begin(row.key)
        case .needsUpdate:
            break
        }
    }

    /// Pair from `target`: a row's key, a scanned `anistream-remote://pair` link or an address
    /// typed in. A TV asked for a code shows one; a link that carried the code pairs at once.
    public func begin(_ target: String) {
        guard let sender else { return }
        cancelPairing()
        sheet = .picker
        pairing = TVPairingStep(tvName: "", needsCode: true, working: true, problem: nil)
        Task {
            do {
                let steps = try await sender.pair(target: target)
                guard pairing != nil else { return }
                self.steps = steps
                pairing = TVPairingStep(tvName: steps.tvName(), needsCode: steps.needsCode(), working: true)
                if steps.needsCode() {
                    try await steps.requestCode()
                    pairing?.working = false
                } else {
                    try await finish(steps, code: nil)
                }
            } catch {
                fail(error)
            }
        }
    }

    /// A link the app was opened with: the QR code on a TV's Pair a phone screen, read by the
    /// system's camera.
    @discardableResult
    public func open(_ url: URL) -> Bool {
        guard url.scheme == "anistream-remote", available else { return false }
        begin(url.absoluteString)
        return true
    }

    /// The six digits typed.
    public func submit(code: String) {
        guard let steps, pairing?.working == false, pairing?.expired == false else { return }
        pairing?.working = true
        pairing?.problem = nil
        Task {
            do {
                try await finish(steps, code: code)
            } catch {
                fail(error)
            }
        }
    }

    /// Ask the TV for a code again, as after one ran out.
    public func requestCodeAgain() {
        guard let steps else { return }
        pairing?.working = true
        pairing?.problem = nil
        pairing?.expired = false
        Task {
            do {
                try await steps.requestCode()
                pairing?.working = false
            } catch {
                fail(error)
            }
        }
    }

    public func cancelPairing() {
        pairing = nil
        steps = nil
    }

    private func finish(_ steps: FfiTvPairing, code: String?) async throws {
        let tv = try await steps.pair(code: code)
        guard pairing != nil else { return }
        cancelPairing()
        refresh()
        use(tv.fingerprint)
    }

    private func fail(_ error: Error) {
        guard pairing != nil else { return }
        pairing?.working = false
        pairing?.problem = Self.words(error)
        if case .Failed(.noCode, _) = error as? FfiTvError { pairing?.expired = true }
    }

    /// The engine's words for a failure, or the system's.
    static func words(_ error: Error) -> String {
        if case .Failed(_, let message) = error as? FfiTvError { return message }
        return error.localizedDescription
    }

    // MARK: The link

    /// Connect to a paired TV, then play what was asked on it, or show its card.
    func use(_ fingerprint: String) {
        connect(fingerprint)
        if request != nil {
            play(on: fingerprint)
        } else {
            sheet = .remote
        }
    }

    /// Hold a link to the TV, reading its card until it closes.
    public func connect(_ fingerprint: String) {
        guard let sender else { return }
        if connected == fingerprint, link != nil { return }
        disconnect()
        guard let link = sender.link(fingerprint: fingerprint) else { return }
        self.link = link
        connected = fingerprint
        take(link.card())
        cards = Task { [weak self, link] in
            while let card = await link.nextCard() {
                guard let self, self.link === link else { return }
                self.take(card)
                // The engine says when the session is over: the TV stopped playing, or stayed
                // away too long. The card stays while it is up, to say so.
                if card.over, self.sheet != .remote { self.disconnect() }
            }
        }
        watchNetwork()
    }

    /// The card the TV sent. The playhead moves on from where and when the TV last said it: a
    /// card that only says the link changed keeps that moment, so the line does not jump back to
    /// it, and stops where the link was lost.
    private func take(_ next: FfiRemoteCard) {
        let same = card.map { $0.rev == next.rev && $0.now == next.now } ?? false
        if next.phase == .connected {
            lostAt = nil
        } else if lostAt == nil, card?.phase == .connected {
            lostAt = .now
        }
        card = next
        if !same { cardAt = .now }
        if let request = starting, let now = next.now, now.id == request.id, now.episode == request.episode {
            starting = nil
        }
        if next.over { starting = nil }
    }

    /// What the TV plays, as the card shows it: what was asked of it while it starts, else what
    /// the TV last said.
    public var now: FfiNowCard? {
        guard let request = starting else { return card?.now }
        return FfiNowCard(
            id: request.id, title: request.title, episode: request.episode,
            episodeLine: L.episodeNumberString(number: request.episode), loading: true, position: request.from ?? 0,
            duration: nil, paused: false, speed: 1, volume: card?.now?.volume, audio: [], subtitles: [],
            subtitlesOff: false, skip: nil)
    }

    /// The link is up, and the card's controls reach the TV.
    public var live: Bool { card?.phase == .connected }

    /// Try the TV again now rather than after the link's wait: as the app comes back to the
    /// front, or the network back. A connected link stays as it is.
    public func retryNow() {
        link?.retryNow()
    }

    /// Ask for a try at once whenever the network comes back while a TV is driven.
    private func watchNetwork() {
        guard path == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            guard path.status == .satisfied else { return }
            Task { @MainActor in self?.retryNow() }
        }
        monitor.start(queue: DispatchQueue(label: "tv.anistream.tv-path")) // i18n-skip
        path = monitor
    }

    private func play(on fingerprint: String) {
        guard let request, let link, connected == fingerprint else { return }
        self.request = nil
        starting = request
        sheet = .remote
        Task {
            do {
                try await link.play(id: request.id, title: request.title, episode: request.episode, from: request.from)
                // The TV playing it already sends nothing new.
                if starting == request, let now = card?.now, now.id == request.id, now.episode == request.episode {
                    starting = nil
                }
                if request.fromPlayer { handedOff?() }
            } catch {
                if starting == request { starting = nil }
                problem = Self.words(error)
            }
        }
    }

    /// Stop driving the TV. What it plays plays on.
    public func disconnect() {
        cards?.cancel()
        cards = nil
        link?.hangUp()
        link = nil
        connected = nil
        card = nil
        starting = nil
        lostAt = nil
        path?.cancel()
        path = nil
    }

    /// Send the TV a command from the card.
    public func send(_ command: FfiTvCommand) {
        guard let link else { return }
        problem = nil
        Task {
            do {
                try await link.send(command: command)
            } catch {
                problem = Self.words(error)
            }
        }
    }

    /// Forget a paired TV, on both sides when it can be reached.
    public func forget(_ fingerprint: String) {
        guard let sender else { return }
        if connected == fingerprint {
            disconnect()
            if sheet == .remote { sheet = nil }
        }
        paired.removeAll { $0.fingerprint == fingerprint }
        Task {
            _ = await sender.forget(fingerprint: fingerprint)
            refresh()
        }
    }

    /// Where the TV's playhead is now: moved on from the card on this device's clock while it
    /// plays.
    public func playhead(at date: Date = .now) -> Double? {
        guard starting == nil, let now = card?.now else { return nil }
        let until = lostAt.map { min($0, date) } ?? date
        return tvPlayhead(now: now, elapsed: max(0, until.timeIntervalSince(cardAt)))
    }

    /// What a TV lists this device as: the kind of device, never the name it was given, which
    /// is often its owner's.
    public static var deviceName: String {
        #if os(macOS)
        "Mac" // i18n-skip
        #else
        UIDevice.current.model
        #endif
    }

    /// The app and its version, as a TV's settings show it: `anistream-ios/2.1.0`.
    public static var appName: String {
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0"
        #if os(macOS)
        return "anistream-macos/\(version)"
        #else
        return "anistream-ios/\(version)"
        #endif
    }

    /// The paired TV this device drives, for the card's header and Settings.
    public var connectedTV: FfiPairedTv? {
        paired.first { $0.fingerprint == connected }
    }
}
