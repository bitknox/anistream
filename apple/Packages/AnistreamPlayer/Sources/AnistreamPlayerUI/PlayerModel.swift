// The player the app hosts for the engine: its ops carried out on mpv, mpv's events reported
// back, and what the player's screens show.

import AnistreamKit
import AnistreamPlayer
import Foundation
import Observation
import os

/// The app's one player, on the main actor.
///
/// - The engine's `PlayerOps` run on mpv in order, one op to one call, on a task of their own
///   that awaits each: nothing waits on mpv from the main thread.
/// - mpv's events become the engine's `PlayerReport`s, for the playback whose file mpv has: a
///   `Load` belongs to its playback from mpv's `start-file` on, and what comes before it to the
///   playback before.
/// - `Quit`, and the viewer closing the player, report `Ended { Quit }` and then `Closed`; `Close`
///   says nothing. Either way mpv shuts down, and every op after it for those playbacks is
///   refused, until the engine opens a player again.
/// - The engine's `PlayerUi` is kept here until it says otherwise, with the cards' timing: the Up
///   Next countdown runs on the playhead from `ending_at`, and sends `PlayNext` at zero.
@MainActor @Observable
public final class PlayerModel {
    public typealias Send = @MainActor (Command) -> Void

    // MARK: Wiring

    @ObservationIgnored private let send: Send
    @ObservationIgnored private let makeBackend: @MainActor () -> any PlayerBackend
    @ObservationIgnored private var backend: (any PlayerBackend)?
    @ObservationIgnored private var work: AsyncStream<Work>.Continuation?
    @ObservationIgnored private var worker: Task<Void, Never>?
    @ObservationIgnored private var listener: Task<Void, Never>?
    @ObservationIgnored private var ticker: Task<Void, Never>?
    @ObservationIgnored private let log = Logger(subsystem: "tv.anistream", category: "player")
    @ObservationIgnored let nowPlaying = NowPlaying()
    @ObservationIgnored private let awake = KeepAwake()

    private enum Work: Sendable {
        case call(MpvCall)
        case display(DisplayInfo)
        case resized
    }

    /// `send` hands the engine a command; `backend` makes the mpv for each player opened.
    public init(
        send: @escaping Send,
        backend: @escaping @MainActor () -> any PlayerBackend = { PlayerModel.mpv() }
    ) {
        self.send = send
        self.makeBackend = backend
        nowPlaying.model = self
    }

    /// The app's mpv: audio and video as the platform plays them.
    public static func mpv() -> MpvClient {
        MpvClient(configuration: MpvConfiguration())
    }

    // MARK: The player

    /// Whether the player is up: from the playback staged, or the engine's Open, until it closes.
    public private(set) var isPresented = false
    /// The layer mpv draws into, while there is an mpv.
    public private(set) var layer: MpvLayer?
    /// The window fills the screen: the engine's Open and its cycles, and the viewer's.
    public var fullScreen = false

    /// The newest playback the engine has spoken of, whose file the player shows or is opening.
    public private(set) var playback: PlaybackId?
    /// The playback whose Open made this player.
    @ObservationIgnored private var opened: PlaybackId?
    /// Loads sent to mpv whose files have not started, oldest first.
    @ObservationIgnored private var loads: [PlaybackId] = []
    /// The playback whose file mpv has now.
    @ObservationIgnored private var current: PlaybackId?
    /// Every playback up to this one is over: its ops are refused.
    @ObservationIgnored private var closedThrough: PlaybackId?

    // MARK: What the engine says

    public private(set) var staged: StagedPlayback?
    public private(set) var starting: StartingScreen?
    public private(set) var episode: EpisodeOnScreen?
    public private(set) var menus: PlayerMenus?
    public private(set) var skip: SkipOffer?
    public private(set) var undo: UndoOffer?
    public private(set) var upNext: UpNextCard?
    public private(set) var stall: StallCard?
    public private(set) var resume: ResumeNote?
    public private(set) var alert: PlayerAlert?
    public private(set) var upscaler: String?
    public private(set) var hdr: String?
    /// "Still watching?": the episodes rolled on in a row with nobody there, which pauses.
    public private(set) var stillWatching = false
    /// Keep waiting was answered: no stall card until this stall ends.
    @ObservationIgnored private var stallWaived = false

    // MARK: What mpv says

    /// The playhead, to a quarter of a second: what the screens read.
    public private(set) var position: Double = 0
    /// The playhead as mpv last said it.
    @ObservationIgnored public private(set) var playhead: Double = 0
    public private(set) var duration: Double?
    public private(set) var paused = false
    public private(set) var speed: Double = 1
    public private(set) var volume: Double = 100
    public private(set) var muted = false
    public private(set) var chapters: [Chapter] = []
    public private(set) var tracks: [Track] = []
    public private(set) var audioTrack: Int64?
    public private(set) var subtitleTrack: Int64?
    /// mpv is waiting for data, since then.
    public private(set) var bufferingSince: ContinuousClock.Instant?
    /// The file playing has put a picture up.
    public private(set) var hasPicture = false

    // MARK: The viewer

    /// Now, a few times a second while the player is up: what the cards' timing reads.
    public private(set) var clock = ContinuousClock.now
    /// The last tap, pointer movement or key: the controls hide a while after it.
    public private(set) var lastInput = ContinuousClock.now
    /// What keeps the controls up: scrubbing, a menu open, the pointer over the bar.
    public private(set) var holds: Set<String> = []
    /// Up for a screenshot, whatever else says.
    public var pinnedControls = false
    /// The starting screen held over the picture, for a screenshot of it.
    public var holdsCurtain = false
    /// Scrubbing, at this time: the seek line follows the finger, not mpv.
    public private(set) var scrubbing: Double?
    @ObservationIgnored private var scrubSent: Double?
    /// Episodes rolled on in a row since the viewer last did anything.
    @ObservationIgnored private(set) var rolledOn = 0
    @ObservationIgnored private var rollPending = false

    #if os(macOS)
    public static let controlsHide = PlayerTiming.controlsHideMac
    #else
    public static let controlsHide = PlayerTiming.controlsHideTouch
    #endif

    // MARK: The engine's events

    /// One of the engine's events: the player's ops and interface, and the playback around them.
    public func apply(_ event: Event) {
        switch event {
        case .playerOps(let playback, let ops):
            for op in ops { apply(op, for: playback) }
        case .playerUi(let playback, let ui):
            apply(ui, for: playback)
        case .playbackStaged(let id, let title, let episode, let episodeTitle, _):
            staged = StagedPlayback(id: id, title: title, episode: episode, episodeTitle: episodeTitle)
            present()
        case .playbackEnded(_, _, let rollingOn):
            if rollingOn {
                rollPending = true
            } else if backend == nil {
                // Nothing was opened for it: no stream found, or it played elsewhere.
                dismiss()
            }
        case .playbackKept:
            // The playback staged found nothing; the one before plays on.
            starting = nil
        default:
            break
        }
    }

    func apply(_ op: PlayerOp, for playback: PlaybackId) {
        guard accepts(playback) else {
            log.debug("refused \(String(describing: op)) for playback \(playback), which is over")
            return
        }
        enter(playback)
        switch HostedStep(op) {
        case .open(let fullScreen, let speed, let volume):
            open(for: playback, fullScreen: fullScreen, speed: speed, volume: volume)
        case .call(let call):
            guard backend != nil else {
                log.error("refused \(String(describing: op)) with no player open")
                return
            }
            if case .load = call {
                loads.append(playback)
                hasPicture = false
            }
            enqueue(.call(call))
        case .fullScreen(let on):
            fullScreen = on ?? !fullScreen
        case .quit:
            quit()
        case .close:
            close(reporting: false)
        }
    }

    func apply(_ ui: PlayerUi, for playback: PlaybackId) {
        guard accepts(playback) else { return }
        enter(playback)
        let now = ContinuousClock.now
        switch ui {
        case .starting(let title, let episode, let episodeTitle):
            starting = StartingScreen(
                playback: playback, title: title, episode: episode, episodeTitle: episodeTitle, since: now)
            present()
        case .episode(let number, let label, let title, let show, let translation):
            episode = EpisodeOnScreen(number: number, label: label, title: title, show: show, translation: translation)
            nowPlaying.update()
        case .menus(let episodes, let sources, let translation, let upscaling, let toneMapping, let nightMode):
            menus = PlayerMenus(
                episodes: episodes, sources: sources, translation: translation, upscaling: upscaling,
                toneMapping: toneMapping, nightMode: nightMode)
        case .skip(let kind, let to):
            skip = SkipOffer(kind: kind, to: to, since: now)
        case .skipCleared:
            skip = nil
        case .autoSkipped(let kind, let undoTo):
            undo = UndoOffer(kind: kind, undoTo: undoTo, from: nil, since: now)
        case .upNext(let next, let auto, let endingAt):
            var card = UpNextCard(next: next, auto: auto, endingAt: endingAt)
            if let before = upNext, before.next.number == next.number {
                card.cancelled = before.cancelled
                card.played = before.played
            }
            upNext = card
        case .upNextCleared:
            upNext = nil
        case .stall(let seconds):
            if !stallWaived { stall = StallCard(seconds: seconds, since: now) }
        case .stallCleared:
            stall = nil
            stallWaived = false
        case .resumed(let position):
            resume = ResumeNote(position: position)
        case .picture(let upscaler, let hdr):
            self.upscaler = upscaler
            self.hdr = hdr
        case .thumbnails:
            // Seek previews come with a second, hidden mpv, which the player does not have yet.
            break
        case .alert(let text, let kind):
            alert = PlayerAlert(text: text, kind: kind, since: now)
        }
    }

    /// Whether ops and interface for `playback` are still taken: not once it is over.
    private func accepts(_ playback: PlaybackId) -> Bool {
        guard let closedThrough else { return true }
        return playback > closedThrough
    }

    /// The engine speaks of `playback`: a newer one starts with none of the last one's cards.
    private func enter(_ playback: PlaybackId) {
        guard self.playback.map({ playback > $0 }) ?? true else { return }
        self.playback = playback
        skip = nil
        undo = nil
        upNext = nil
        stall = nil
        stallWaived = false
        resume = nil
        alert = nil
        if rollPending {
            rollPending = false
            rolledOn += 1
        }
    }

    // MARK: Opening and closing

    private func present() {
        guard !isPresented else { return }
        isPresented = true
        clock = .now
        lastInput = .now
        startTicker()
    }

    private func open(for playback: PlaybackId, fullScreen: Bool, speed: Double?, volume: Double?) {
        if backend == nil {
            let backend = makeBackend()
            let layer = MpvLayer()
            // Until the view lays it out: mpv's first swapchain needs a size.
            layer.resize(to: CGSize(width: 640, height: 360), scale: 2)
            self.backend = backend
            self.layer = layer
            opened = playback
            // Opened before mpv starts, so nothing it says is missed.
            let events = backend.events()
            listener = Task { @MainActor [weak self] in
                for await event in events {
                    self?.handle(event, from: backend)
                }
                self?.ended(backend)
            }
            let (stream, continuation) = AsyncStream.makeStream(of: Work.self)
            work = continuation
            worker = Task { @MainActor [weak self, log] in
                do {
                    try await backend.begin(on: layer)
                } catch {
                    log.error("mpv did not start: \(String(describing: error))")
                    self?.failed(backend)
                    return
                }
                for await item in stream {
                    do {
                        switch item {
                        case .call(let call):
                            if let report = try await backend.perform(call) { self?.report(report) }
                        case .display(let display):
                            await backend.display(display)
                        case .resized:
                            await backend.resized()
                        }
                    } catch {
                        log.error("mpv refused \(String(describing: item)): \(String(describing: error))")
                    }
                }
            }
            self.fullScreen = fullScreen
            reportPower()
            PlaybackSession.begin()
        }
        if let speed { enqueue(.call(.set(property: "speed", value: .double(speed)))) }
        if let volume { enqueue(.call(.set(property: "volume", value: .double(volume)))) }
        present()
    }

    /// The engine's Quit: the viewer stopped, so the file ended by quitting and the player closes.
    private func quit() {
        close(reporting: true)
    }

    /// The viewer closed the player: its close button, the swipe down, the window's close.
    public func viewerClosed() {
        input()
        guard backend != nil else {
            if isPresented {
                // Nothing opened yet: stop what is being found.
                send(.player(.stop))
                dismiss()
            }
            return
        }
        close(reporting: true)
    }

    /// Close mpv and the player. `reporting`: say the file ended by quitting, and that the
    /// player closed, first.
    private func close(reporting: Bool) {
        if reporting {
            report(.ended(reason: .quit))
            report(.closed)
        }
        teardown()
    }

    /// mpv went away by itself.
    private func ended(_ backend: any PlayerBackend) {
        guard backend === self.backend else { return }
        report(.closed)
        teardown()
    }

    /// mpv would not start: the player is gone before it was there.
    private func failed(_ backend: any PlayerBackend) {
        guard backend === self.backend else { return }
        report(.closed)
        teardown()
    }

    private func teardown() {
        listener?.cancel()
        worker?.cancel()
        work?.finish()
        listener = nil
        worker = nil
        work = nil
        if let backend {
            Task.detached { await backend.end() }
        }
        backend = nil
        closedThrough = [playback, current, opened, loads.max()].compactMap { $0 }.max()
        opened = nil
        current = nil
        loads = []
        dismiss()
    }

    /// The player comes down, with everything it showed.
    private func dismiss() {
        isPresented = false
        layer = nil
        fullScreen = false
        staged = nil
        starting = nil
        episode = nil
        menus = nil
        skip = nil
        undo = nil
        upNext = nil
        stall = nil
        resume = nil
        alert = nil
        stillWatching = false
        resetFile()
        ticker?.cancel()
        ticker = nil
        holds = []
        scrubbing = nil
        nowPlaying.clear()
        awake.update(playing: false)
        PlaybackSession.end()
    }

    private func resetFile() {
        position = 0
        playhead = 0
        duration = nil
        paused = false
        chapters = []
        tracks = []
        audioTrack = nil
        subtitleTrack = nil
        bufferingSince = nil
        hasPicture = false
    }

    // MARK: mpv's events

    private func handle(_ event: MpvEvent, from backend: any PlayerBackend) {
        guard backend === self.backend else { return }
        switch event {
        case .startFile:
            if !loads.isEmpty { current = loads.removeFirst() }
            let keepPause = paused
            resetFile()
            paused = keepPause
        case .playbackRestart:
            if !hasPicture { firstFrame() }
        case .property(let name, let value):
            update(name, value)
        default:
            break
        }
        if let report = PlayerReport(event) { self.report(report) }
    }

    /// The file's first picture is up: the starting screen fades from it.
    private func firstFrame() {
        hasPicture = true
        if let starting, current.map({ $0 >= starting.playback }) ?? true {
            self.starting = nil
        }
        if rolledOn >= PlayerTiming.stillWatchingAfter, !stillWatching {
            stillWatching = true
            if !paused { send(.player(.togglePause)) }
        }
        nowPlaying.update()
    }

    private func update(_ name: String, _ value: AnistreamPlayer.MpvValue) {
        switch name {
        case "time-pos":
            guard let time = value.double else { return }
            playhead = time
            if scrubbing == nil, abs(time - position) >= 0.25 || time < position { position = time }
            played()
        case "duration":
            duration = value.double
            nowPlaying.update()
        case "pause":
            paused = value.bool ?? paused
            awake.update(playing: !paused)
            nowPlaying.update()
        case "speed":
            speed = value.double ?? speed
            nowPlaying.update()
        case "volume":
            volume = value.double ?? volume
        case "chapter-list":
            chapters = value.array.map { Chapter(title: $0["title"].string ?? "", time: $0["time"].double ?? 0) }
        case "track-list":
            tracks = value.array.compactMap(Track.init(mpv:))
        case "aid":
            audioTrack = value.int
        case "sid":
            subtitleTrack = value.int
        case "paused-for-cache":
            let waiting = value.bool ?? false
            bufferingSince = waiting ? (bufferingSince ?? .now) : nil
        case "seeking":
            if value.bool == false {
                position = playhead
                nowPlaying.update()
            }
        default:
            break
        }
    }

    /// The playhead moved: the cards that run on playback time.
    private func played() {
        if var card = upNext, !card.played,
            let left = card.remaining(at: playhead, duration: duration), left <= 0
        {
            card.played = true
            upNext = card
            rollPending = true
            send(.player(.playNext))
        }
        if var note = undo {
            if let from = note.from {
                if playhead - from >= PlayerTiming.undoPlayback || playhead < from - 1 { undo = nil }
            } else {
                note.from = playhead
                undo = note
            }
        }
        if let note = resume, playhead >= note.position + PlayerTiming.resumePlayback || playhead < note.position - 1 {
            resume = nil
        }
    }

    // MARK: Reports

    /// Tell the engine, for the playback whose file mpv has.
    private func report(_ report: PlayerReport) {
        guard let target = current ?? opened else { return }
        send(.playerReport(playback: target, report: report))
    }

    private func reportPower() {
        let power = PowerState.now()
        report(.power(onBattery: power.onBattery, lowPower: power.lowPower))
    }

    /// The device's power changed: on battery or off, low power mode.
    public func powerChanged() {
        guard backend != nil else { return }
        reportPower()
    }

    // MARK: The surface

    /// The screen the surface is on.
    public func surfaceDisplay(_ display: DisplayInfo) {
        guard backend != nil else { return }
        enqueue(.display(display))
        report(PlayerReport(display))
    }

    public func surfaceResized() {
        enqueue(.resized)
    }

    private func enqueue(_ item: Work) {
        work?.yield(item)
    }

    // MARK: The clock

    private func startTicker() {
        ticker?.cancel()
        ticker = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .milliseconds(250))
                self?.tick()
            }
        }
    }

    private func tick() {
        let now = ContinuousClock.now
        clock = now
        if let note = undo, now - note.since >= PlayerTiming.undoPaused { undo = nil }
        if let alert, now - alert.since >= PlayerTiming.alertStays { self.alert = nil }
    }

    // MARK: What the screens read

    /// Whether the controls are up: after input, while paused, while something holds them.
    public var controlsVisible: Bool {
        pinnedControls || paused || !holds.isEmpty || clock - lastInput < Self.controlsHide
    }

    /// The starting screen's line: "Starting episode 7", and after a while "Finding a source · 4 s".
    public var startingLine: String? {
        guard let starting else { return nil }
        let waited = clock - starting.since
        guard waited >= PlayerTiming.startingWaits else { return "Starting episode \(starting.episode)" }
        return "Finding a source · \(waited.components.seconds) s"
    }

    /// Buffering long enough to show a spinner.
    public var showsSpinner: Bool {
        guard let bufferingSince, starting == nil else { return false }
        return clock - bufferingSince >= PlayerTiming.spinnerAfter
    }

    /// The skip button has been up long enough to dim.
    public var skipDimmed: Bool {
        guard let skip else { return false }
        return clock - skip.since >= PlayerTiming.skipDims
    }

    /// Up Next is on screen: from the ending, unless cancelled.
    public var upNextShown: Bool {
        guard let card = upNext, !card.cancelled, let at = card.showsAt(duration: duration) else { return false }
        return position >= at
    }

    /// "Plays in 8 s", while the countdown runs, and how far it has run, 0 to 1.
    public var countdown: (seconds: Int, progress: Double)? {
        guard upNextShown, let left = upNext?.remaining(at: position, duration: duration) else { return nil }
        let progress = 1 - left / PlayerTiming.upNextCountdown
        return (Int(left.rounded(.up)), min(1, max(0, progress)))
    }

    /// The one card in the corner: the stall prompt, then Undo, then Up Next, then Skip.
    public enum Corner: Equatable { case stall, undo, upNext, skip }
    public var corner: Corner? {
        if stall != nil { return .stall }
        if undo != nil { return .undo }
        if upNextShown { return .upNext }
        if skip != nil { return .skip }
        return nil
    }

    /// The seek line's stretches, from the file's chapters.
    public var segments: [SeekSegment] { seekSegments(chapters: chapters, duration: duration) }

    public var audioTracks: [Track] { tracks.filter { $0.kind == .audio } }
    public var subtitleTracks: [Track] { tracks.filter { $0.kind == .sub } }

    /// The header's lines: the show, and the episode.
    public var showTitle: String { episode?.show ?? staged?.title ?? starting?.title ?? "" }
    public var episodeLine: String {
        if let episode { return episode.line }
        let number = staged?.episode ?? starting?.episode ?? ""
        let title = staged?.episodeTitle ?? starting?.episodeTitle
        return title.map { "Episode \(number) · \($0)" } ?? "Episode \(number)"
    }

    // MARK: The viewer's actions

    /// A tap, pointer movement or key: the controls come up.
    public func poke() {
        lastInput = .now
        clock = lastInput
    }

    /// Something the viewer did: no longer nobody there.
    private func input() {
        poke()
        rolledOn = 0
    }

    /// A tap on the video while the controls are up: they go now.
    public func hideControls() {
        lastInput = .now - .seconds(60)
        clock = .now
    }

    public func hold(_ reason: String, _ on: Bool) {
        if on { holds.insert(reason) } else { holds.remove(reason) }
        poke()
    }

    public func togglePause() {
        input()
        send(.player(.togglePause))
    }

    public func seek(by seconds: Double) {
        input()
        send(.player(.seek(seconds)))
    }

    public func seek(to seconds: Double) {
        input()
        send(.player(.seekTo(seconds)))
    }

    /// The finger or pointer on the seek line: keyframe seeks while it moves, an exact one on
    /// release.
    public func scrub(to seconds: Double) {
        if scrubbing == nil { hold("scrub", true) }
        scrubbing = seconds
        position = seconds
        // A keyframe seek each half second of movement: mpv drops the ones it cannot keep up with
        // anyway, and the line follows the finger meanwhile.
        if let sent = scrubSent, abs(sent - seconds) < 0.5 { return }
        scrubSent = seconds
        enqueue(.call(.seek(seconds: seconds, mode: .keyframe)))
    }

    public func endScrub() {
        guard let seconds = scrubbing else { return }
        scrubbing = nil
        scrubSent = nil
        hold("scrub", false)
        seek(to: seconds)
    }

    /// Play at `target` times normal speed.
    public func setSpeed(_ target: Double) {
        input()
        guard speed > 0, target > 0 else { return }
        send(.player(.speed(target / speed)))
    }

    public func stepVolume(by points: Double) {
        input()
        send(.player(.volume(points)))
    }

    public func toggleMute() {
        input()
        muted.toggle()
        enqueue(.call(.set(property: "mute", value: .flag(muted))))
    }

    /// An audio or subtitle track picked: mpv takes it, and the engine reads it back from
    /// `aid` and `sid` to remember it for the series.
    public func selectAudio(_ id: Int64) {
        input()
        enqueue(.call(.set(property: "aid", value: .int(id))))
    }

    public func selectSubtitle(_ id: Int64?) {
        input()
        enqueue(.call(.set(property: "sid", value: id.map(AnistreamPlayer.MpvValue.int) ?? .string("no"))))
    }

    public func takeSkip() {
        input()
        skip = nil
        send(.player(.takeSkip))
    }

    public func undoSkip() {
        input()
        guard let note = undo else { return }
        undo = nil
        send(.player(.undoSkip(to: note.undoTo)))
    }

    public func playNext() {
        input()
        guard var card = upNext, !card.played else { return }
        card.played = true
        upNext = card
        send(.player(.playNext))
    }

    public func cancelNext() {
        input()
        guard var card = upNext else { return }
        card.cancelled = true
        upNext = card
        send(.player(.cancelNext))
    }

    public func choose(_ choice: PlayerChoice) {
        input()
        send(.player(.choose(choice)))
    }

    public func setUpscaling(_ id: String) {
        input()
        send(.player(.upscaling(id)))
    }

    public func setNightMode(_ on: Bool) {
        input()
        send(.player(.nightMode(on)))
    }

    /// The next (`1`) or previous (`-1`) episode.
    public func step(_ by: Int64) {
        input()
        send(.player(.step(by)))
    }

    public func switchSource() {
        input()
        stall = nil
        send(.player(.stallSwitch))
    }

    public func keepWaiting() {
        input()
        stall = nil
        stallWaived = true
        send(.player(.stallWait))
    }

    public func startOver() {
        input()
        resume = nil
        send(.player(.seekTo(0)))
    }

    /// "Still watching?": Continue watching.
    public func continueWatching() {
        input()
        stillWatching = false
        if paused { send(.player(.togglePause)) }
    }

    /// "Still watching?": Stop.
    public func stopWatching() {
        stillWatching = false
        viewerClosed()
    }

    /// Pinch out to fill the screen, in to fit the picture: mpv's `panscan`.
    public func setFill(_ fill: Bool) {
        input()
        enqueue(.call(.set(property: "panscan", value: .double(fill ? 1 : 0))))
    }

    /// The app went to the background, or came back: the audio plays on, the video track is off
    /// meanwhile.
    public func backgrounded(_ background: Bool) {
        enqueue(.call(.set(property: "vid", value: .string(background ? "no" : "auto"))))
    }

    #if FIXTURES
    /// "Still watching?" now, as after three episodes in a row: for screenshots.
    public func askStillWatching() {
        stillWatching = true
    }
    #endif
}

extension Track {
    /// A `track-list` entry.
    init?(mpv entry: AnistreamPlayer.MpvValue) {
        guard let kind = entry["type"].string.flatMap(Kind.init(rawValue:)), let id = entry["id"].int else { return nil }
        self.init(
            kind: kind, id: id, language: entry["lang"].string, title: entry["title"].string,
            selected: entry["selected"].bool ?? false, external: entry["external"].bool ?? false)
    }
}
