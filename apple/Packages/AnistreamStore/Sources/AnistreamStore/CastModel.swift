// Casting to a TV with Chromecast built in: the Play on TV picker's Chromecast group, and the
// card of the cast running.
//
// The engine's sender holds the rules, as for Play on TV: which TVs the group lists, how a cast
// starts, plays and ends, and in what words a failure is told (`FfiTvSender.castRows()`,
// `cast(key:…)` and `FfiCastSession`). This model keeps its latest answers on the main actor
// and adds what only the screens decide: how long a TV that left the network stays listed, the
// seconds counted on a slow start, and which buttons a failure offers.

import AnistreamKit
import AnistreamStrings
import Foundation
import Network
import Observation
import dnssd

/// A row of the picker's Chromecast group, and whether its TV has just left the network: the
/// row says so for a few seconds before it goes.
public struct CastRowShown: Identifiable, Equatable, Sendable {
    public let row: FfiCastRow
    public var gone: Bool

    public var id: String { row.key }

    public init(row: FfiCastRow, gone: Bool = false) {
        self.row = row
        self.gone = gone
    }
}

/// How the cast's card reads.
public enum CastStage: Equatable, Sendable {
    /// From the pick until the TV plays, and again while it loads the next episode.
    case starting
    case playing
    /// It never played: the engine's reason is the card's notice.
    case failed
    /// It played, then stopped without being asked to: the TV dropped off.
    case stopped
}

/// What a failed or stopped card offers, in its order: the first is the one in amber.
public enum CastAction: Equatable, Sendable {
    case chooseSource, playHere, tryAgain, chooseAnotherTV
}

/// The cast as the card shows it: the TV, what plays, and how it goes.
@Observable @MainActor
public final class CastModel {
    /// How long a TV that left the network stays listed, saying so.
    public static let goneFor: Duration = .seconds(4)
    /// How long a start goes before its line counts the seconds.
    public static let countFrom: TimeInterval = 4

    /// The picker's Chromecast rows, by name, with any that just left.
    public private(set) var rows: [CastRowShown] = []
    /// The system won't let the app look on the local network: neither group can find a TV.
    public private(set) var localNetworkDenied = false
    /// Whether the picker lists TVs with Chromecast built in at all.
    public var shown = true

    /// The cast running, from the pick until it is stopped or its ending dismissed.
    public private(set) var session: (any FfiCastSessionProtocol)?
    /// The card as the engine last read it, and when, for the playhead to move on from.
    public private(set) var card: FfiCastCard?
    public private(set) var cardAt = Date.now
    /// What the cast plays, as asked: the card's title and episode until the engine names them,
    /// and what Try again casts.
    public private(set) var request: TVPlayRequest?
    /// The row picked, for Try again.
    public private(set) var key: String?
    public private(set) var tvName = ""
    /// When the start the card shows began: the pick, or the next episode's load.
    public private(set) var startedAt = Date.now
    /// The TV has played something in this cast.
    public private(set) var played = false
    /// Where the TV last was, and in what episode, for Play here once the cast has ended.
    public private(set) var lastPosition: Double?
    /// Something the picker has to say about a pick that didn't start.
    public private(set) var problem: String?

    /// The cast's card closed by itself: someone else took the TV.
    @ObservationIgnored var closed: (@MainActor () -> Void)?
    /// Post a notice: someone else took the TV.
    @ObservationIgnored public var say: (@MainActor (Notice) -> Void)?
    /// Play an episode here, from a position when there is one: Play here.
    @ObservationIgnored public var playLocally: (@MainActor (AnilistId, String, Double?) -> Void)?
    /// What a pick starts, on the engine's sender; the Fixtures app scripts it instead.
    @ObservationIgnored var opener: (@MainActor (String, TVPlayRequest) throws -> any FfiCastSessionProtocol)?

    @ObservationIgnored private var cards: Task<Void, Never>?
    @ObservationIgnored private var leaving: [String: Task<Void, Never>] = [:]
    @ObservationIgnored private var browsing = false
    #if os(macOS)
    @ObservationIgnored private var awake: NSObjectProtocol?
    #endif

    public init() {}

    /// A cast is up: its card shows in place of Play on TV's.
    public var active: Bool { session != nil }

    // MARK: The picker

    func attach(_ sender: FfiTvSender) {
        opener = { [sender] key, request in
            try sender.cast(key: key, id: request.id, episode: request.episode, position: request.from)
        }
        if let running = sender.castSession() { follow(running, request: nil) }
    }

    /// The browse started or stopped. Stopping forgets what it found at once, with no row
    /// saying it left.
    func browsing(_ on: Bool) {
        browsing = on
        problem = nil
        if !on {
            leaving.values.forEach { $0.cancel() }
            leaving = [:]
            rows = []
        }
    }

    /// The engine's rows as they stand. A row the engine no longer lists while the browse runs
    /// stays, saying its TV left, for `goneFor`; a TV that comes back takes its row back.
    func refresh(_ listed: [FfiCastRow]) {
        guard browsing, shown else {
            rows = []
            return
        }
        let keys = Set(listed.map(\.key))
        var next = listed.map { CastRowShown(row: $0) }
        for row in listed { leaving.removeValue(forKey: row.key)?.cancel() }
        for old in rows where !keys.contains(old.id) {
            next.append(CastRowShown(row: old.row, gone: true))
            guard leaving[old.id] == nil else { continue }
            leaving[old.id] = Task { [weak self] in
                try? await Task.sleep(for: Self.goneFor)
                guard !Task.isCancelled, let self else { return }
                self.leaving[old.id] = nil
                self.rows.removeAll { $0.id == old.id && $0.gone }
            }
        }
        rows = next.sorted { $0.row.name.localizedStandardCompare($1.row.name) == .orderedAscending }
    }

    /// What the browse said of the local network: denied, or allowed after all.
    public func localNetwork(denied: Bool) {
        if localNetworkDenied != denied { localNetworkDenied = denied }
    }

    // MARK: Casting

    /// Cast `request` to the row `key`. The card shows at once, starting.
    public func cast(_ key: String, name: String, request: TVPlayRequest) {
        guard let opener else { return }
        problem = nil
        let session: any FfiCastSessionProtocol
        do {
            session = try opener(key, request)
        } catch {
            problem = TVRemoteModel.words(error)
            return
        }
        self.key = key
        tvName = name
        follow(session, request: request)
    }

    /// Read `session`'s cards until it ends.
    private func follow(_ session: any FfiCastSessionProtocol, request: TVPlayRequest?) {
        cards?.cancel()
        self.session = session
        self.request = request
        if tvName.isEmpty || request == nil { tvName = session.deviceName() }
        played = false
        lastPosition = request?.from
        startedAt = .now
        take(session.card())
        keepAwake(true)
        cards = Task { [weak self, session] in
            while let card = await session.nextCard() {
                guard let self, self.session === session else { return }
                self.take(card)
            }
        }
    }

    /// The card the engine read. Someone else taking the TV closes it, with a notice that
    /// offers to play on here.
    func take(_ next: FfiCastCard) {
        let before = stage
        let same = card.map { $0.card.now?.position == next.card.now?.position && $0.card.now?.paused == next.card.now?.paused } ?? false
        card = next
        if !same { cardAt = .now }
        if let now = next.card.now, !now.loading { lastPosition = now.position }
        let after = stage
        if after == .playing { played = true }
        if after == .starting, before == .playing { startedAt = .now }
        if after != before, after == .stopped || after == .failed { keepAwake(false) }
        if after == .stopped, next.notice == .noticeCastTakenOver {
            let carried = endedPlay
            end()
            closed?()
            say?(Notice(kind: .info, text: Msg.noticeCastTakenOver.text, hint: nil, msg: .noticeCastTakenOver))
            takenOver = carried
        }
    }

    /// What Play here plays once someone else took the TV: offered by the notice.
    public private(set) var takenOver: TVPlayRequest?

    /// What the cast plays, from where the TV is: for another TV.
    public var carried: TVPlayRequest? {
        guard var play = endedPlay else { return nil }
        if let at = playhead() { play.from = at }
        return play
    }

    /// The episode the cast was in and where, for Play here once it has ended.
    private var endedPlay: TVPlayRequest? {
        let now = card?.card.now
        guard let id = now?.id ?? request?.id, let episode = now?.episode ?? request?.episode else { return nil }
        return TVPlayRequest(id: id, title: now?.title ?? request?.title ?? "", episode: episode, from: lastPosition)
    }

    public var stage: CastStage {
        guard let card else { return .starting }
        if card.card.over || card.notice != nil { return played ? .stopped : .failed }
        if card.card.phase == .connected, let now = card.card.now, !now.loading { return .playing }
        return .starting
    }

    /// The card's line: starting, then from four seconds the engine's phase with the seconds
    /// counted; playing; or why it ended.
    public func line(at date: Date = .now) -> String {
        switch stage {
        case .playing:
            return L.tvStatusPlaying(name: tvName)
        case .failed:
            return L.castCouldntStart(name: tvName)
        case .stopped:
            return L.castStopped
        case .starting:
            let seconds = date.timeIntervalSince(startedAt)
            guard seconds >= Self.countFrom else { return L.tvStartingOn(name: tvName) }
            if let phase = card?.card.phaseLine, !phase.isEmpty {
                return L.castPhaseCounted(line: phase, seconds: Int(seconds))
            }
            return L.castStartingCounted(name: tvName, seconds: Int(seconds))
        }
    }

    /// The engine's reason a failed or stopped card gives, in the person's language.
    public var reason: String? {
        guard stage == .failed || stage == .stopped else { return nil }
        return (card?.notice ?? (stage == .stopped ? .noticeCastLost : .noticeCastRefused)).text
    }

    /// What a failed or stopped card offers, the first in amber.
    public var actions: [CastAction] {
        switch stage {
        case .playing, .starting:
            return []
        case .stopped:
            return [.playHere, .chooseAnotherTV]
        case .failed:
            switch card?.notice {
            case .noticeCastNeverFetches: return [.tryAgain, .playHere]
            case .noticeCastNotSameNetwork: return [.playHere, .chooseAnotherTV]
            case .noticeCastDeepLink: return [.chooseSource]
            default: return [.chooseSource, .playHere]
            }
        }
    }

    /// What plays on the TV, as the card shows it: what was asked while the engine hasn't named
    /// it yet.
    public var now: FfiNowCard? {
        if let now = card?.card.now { return now }
        guard let request else { return nil }
        return FfiNowCard(
            id: request.id, title: request.title, episode: request.episode,
            episodeLine: L.episodeNumberString(number: request.episode), loading: true,
            position: request.from ?? 0, duration: nil, paused: false, speed: 1, volume: nil,
            audio: [], subtitles: [], subtitlesOff: false, skip: nil)
    }

    /// The card's controls reach the TV: it plays this cast.
    public var live: Bool { stage == .playing }

    /// Whether the TV's volume is its own, with mute alone here.
    public var fixedVolume: Bool { card?.volumeControl == .muteOnly }
    public var muted: Bool { card?.muted ?? false }

    /// Where the TV's playhead is now: moved on from the card on this device's clock while it
    /// plays.
    public func playhead(at date: Date = .now) -> Double? {
        guard let now = card?.card.now, !now.loading else { return nil }
        guard live else { return now.position }
        return tvPlayhead(now: now, elapsed: max(0, date.timeIntervalSince(cardAt)))
    }

    /// Send the TV a command from the card.
    public func send(_ command: FfiTvCommand) {
        session?.command(command: command)
    }

    /// Stop casting and play on here, where the TV was.
    public func playHere() {
        let play = endedPlay
        let position = session?.playHere()
        end()
        guard let play else { return }
        playLocally?(play.id, play.episode, position ?? play.from)
    }

    /// Play here, from the notice that said someone else took the TV.
    public func playHereAfterTakeOver() {
        guard let play = takenOver else { return }
        takenOver = nil
        playLocally?(play.id, play.episode, play.from)
    }

    /// Cast the same episode to the same TV again.
    public func tryAgain() {
        guard let key, let request else { return }
        let name = tvName
        end()
        cast(key, name: name, request: request)
    }

    /// Stop casting: the TV goes back to its own screen, and plays open here again.
    public func stop() {
        session?.stop()
        end()
    }

    /// The card of a cast that ended, put away: the engine stops casting, so the next play opens
    /// here.
    public func dismissEnded() {
        guard stage == .failed || stage == .stopped else { return }
        stop()
    }

    private func end() {
        cards?.cancel()
        cards = nil
        session = nil
        card = nil
        request = nil
        played = false
        keepAwake(false)
    }

    /// The Mac stays awake while it serves the TV.
    private func keepAwake(_ on: Bool) {
        #if os(macOS)
        if on, awake == nil {
            awake = ProcessInfo.processInfo.beginActivity(
                options: [.idleSystemSleepDisabled], reason: "Casting to a TV") // i18n-skip
        } else if !on, let activity = awake {
            ProcessInfo.processInfo.endActivity(activity)
            awake = nil
        }
        #endif
    }
}

/// What a DNS-SD browse's state says of the local network: whether the system denies it.
enum LocalNetworkAccess {
    /// Nil while the state says nothing either way.
    static func denied(_ state: NWBrowser.State) -> Bool? {
        switch state {
        case .ready:
            return false
        case .waiting(let error), .failed(let error):
            if case .dns(let code) = error, code == DNSServiceErrorType(kDNSServiceErr_PolicyDenied) { return true }
            return nil
        default:
            return nil
        }
    }
}
