// The player the app hosts for the engine: its ops carried out on mpv, mpv's events reported
// back, and what the player's screens show.

import AnistreamKit
import AnistreamPlayer
import AnistreamStore
import CoreGraphics
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
    @ObservationIgnored private let makeBackend: (@MainActor () -> any PlayerBackend)?
    @ObservationIgnored private var backend: (any PlayerBackend)?
    @ObservationIgnored private var work: AsyncStream<Work>.Continuation?
    @ObservationIgnored private var worker: Task<Void, Never>?
    @ObservationIgnored private var listener: Task<Void, Never>?
    @ObservationIgnored private var ticker: Task<Void, Never>?
    /// When the playback on screen was staged or started, for the log's timings.
    @ObservationIgnored private var since: ContinuousClock.Instant?
    /// mpvs shutting down, off the main thread: what `finished()` waits for.
    @ObservationIgnored private var ending: [Task<Void, Never>] = []
    /// What mpv has warned of for this file, said once each.
    @ObservationIgnored private var mpvSaid: Set<String> = []
    @ObservationIgnored let nowPlaying = NowPlaying()
    @ObservationIgnored private let awake = KeepAwake()
    @ObservationIgnored private let lowPower: @MainActor () -> Bool
    /// The frame under the scrubbing finger.
    public let preview: SeekPreview

    private enum Work: Sendable {
        case call(MpvCall)
        case display(DisplayInfo)
        case resized
    }

    /// `send` hands the engine a command; `backend` makes the mpv for each player opened, and
    /// `preview` the hidden one for seek previews, which are off while `lowPower` says so.
    public init(
        send: @escaping Send,
        backend: (@MainActor () -> any PlayerBackend)? = nil,
        preview: @escaping @MainActor () -> any PlayerBackend = { SeekPreview.mpv() },
        lowPower: @escaping @MainActor () -> Bool = { ProcessInfo.processInfo.isLowPowerModeEnabled }
    ) {
        self.send = send
        self.makeBackend = backend
        self.preview = SeekPreview(make: preview)
        self.lowPower = lowPower
        nowPlaying.model = self
    }

    /// The app's mpv: audio and video as the platform plays them, keeping the shaders libplacebo
    /// compiles in `shaderCache`, so upscaling does not compile its chain again on every start.
    public static func mpv(shaderCache: URL? = nil) -> MpvClient {
        var configuration = MpvConfiguration()
        if let shaderCache, (try? FileManager.default.createDirectory(at: shaderCache, withIntermediateDirectories: true)) != nil {
            configuration.shaderCache = shaderCache
        }
        return MpvClient(configuration: configuration)
    }

    /// Where mpv keeps its compiled shaders: `<cache>/mpv-shaders`, once the app's cache is known.
    @ObservationIgnored public var shaderCache: URL?


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
    /// What each playback's Load opened, for the seek preview to open too.
    @ObservationIgnored private var sources: [PlaybackId: PreviewSource] = [:]
    /// Where previews are read from instead of the stream, when the engine says so.
    public private(set) var thumbnails: String?
    /// Every playback up to this one is over: its ops are refused.
    @ObservationIgnored private var closedThrough: PlaybackId?
    /// The viewer closed the player: every op is refused until the engine stages a playback
    /// again, so one it was still finding cannot bring the player back up.
    @ObservationIgnored private var closedByViewer = false
    /// A playback has been staged that has said nothing yet: the one on screen, if any, is older,
    /// and stopping is for the staged one, which only a Stop for whatever plays reaches.
    @ObservationIgnored private var stagedAhead = false
    /// The playbacks the engine has been told this player closed for.
    @ObservationIgnored private var closedSaid: Set<PlaybackId> = []

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
    /// The playback never played: "Couldn't play episode 7", with Choose source and Close.
    public private(set) var failure: PlaybackFailure?
    /// The viewer asked for the episode's releases; `sourceChoices` is nil until they come.
    public private(set) var choosingSource = false
    public private(set) var sourceChoices: [SourceCandidate]?
    /// The player closed before a frame: waiting for the engine to say whether it failed.
    @ObservationIgnored private var verdictPending = false
    /// "Still watching?": the episodes rolled on in a row with nobody there, which pauses.
    public private(set) var stillWatching = false
    /// Skips the viewer has taken, for the haptic that answers each (T4).
    public private(set) var skipsTaken = 0
    /// After how many episodes in a row with nobody there it asks, as the engine last said: `0`
    /// is never.
    public private(set) var stillWatchingAfter = PlayerTiming.stillWatchingAfter
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
    /// The picture's width over its height, as mpv displays it.
    @ObservationIgnored public private(set) var videoAspect: Double = 16 / 9

    // MARK: The viewer

    /// Now, a few times a second while the player is up: what the cards' timing reads.
    public private(set) var clock = ContinuousClock.now
    /// The last tap, pointer movement or key: the controls hide a while after it.
    public private(set) var lastInput = ContinuousClock.now
    /// What keeps the controls up: scrubbing, a menu open, the pointer over the bar.
    public private(set) var holds: Set<String> = []
    /// Up for a screenshot, whatever else says.
    public var pinnedControls = false
    /// The Mac's Picture in Picture: the picture in the system's floating window while the main
    /// window browses. Set by `PictureInPicture` as the picture leaves the window and returns.
    public internal(set) var pictureInPicture = false
    /// Where Picture in Picture is, for the button and the menu.
    public internal(set) var pictureInPicturePhase: PictureInPicturePhase = .inline
    /// What toggles Picture in Picture, where the player has it (the Mac).
    @ObservationIgnored var pictureInPictureToggle: (@MainActor () -> Void)?
    /// The last frame Picture in Picture's window was given, for a capture of it.
    @ObservationIgnored public internal(set) var pictureInPictureFrame: (@MainActor () -> CGImage?)?

    /// The player has the main window: up, and not in Picture in Picture.
    public var inMainWindow: Bool { isPresented && !pictureInPicture }

    /// Picture in Picture can be asked for: there is a picture to put in it.
    public var pictureInPictureAvailable: Bool { pictureInPictureToggle != nil && isPresented && hasPicture }

    /// Into the system's floating window, or back out of it: the button, ⌥⌘P and the menu.
    public func togglePictureInPicture() {
        guard pictureInPictureAvailable || pictureInPicturePhase != .inline else { return }
        pictureInPictureToggle?()
    }
    /// The pointer over the seek line on the Mac, at this time: the bubble shows there.
    public private(set) var hovering: Double?
    /// How fast scrubbing moves now: 1, ½ or ¼.
    public private(set) var scrubPace: Double = 1
    /// Where `sub-pos` is headed, lifting the subtitles above the controls.
    @ObservationIgnored private(set) var subtitlePosition: Double = 100
    @ObservationIgnored private var subtitleTween: Task<Void, Never>?
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
            since = .now
            closedByViewer = false
            stagedAhead = true
            failure = nil
            choosingSource = false
            sourceChoices = nil
            verdictPending = false
            staged = StagedPlayback(id: id, title: title, episode: episode, episodeTitle: episodeTitle)
            present()
        case .playbackEnded(_, let ending, let rollingOn):
            if rollingOn {
                // Not when the viewer sought into the ending: someone was there.
                rollPending = !(upNext?.sought ?? false)
            } else if backend == nil, ending == .failed, isPresented, let episode = startingEpisode {
                // It never played: no stream was found, or none showed a frame. Say so where the
                // viewer is looking, with another source to choose, rather than vanish.
                verdictPending = false
                failure = PlaybackFailure(id: staged?.id, episode: episode)
                Self.note(.warn, "couldn't play episode \(episode)")
            } else if backend == nil {
                // Nothing was opened for it, or it played elsewhere, or it is simply over.
                verdictPending = false
                dismiss()
            }
        case .sources(let id, let episode, let candidates):
            guard choosingSource else { return }
            Self.note(.info, "\(candidates.count) sources for \(id) episode \(episode)")
            sourceChoices = candidates
        case .playbackKept:
            // The playback staged found nothing; the one before plays on.
            starting = nil
        default:
            break
        }
    }

    func apply(_ op: PlayerOp, for playback: PlaybackId) {
        guard accepts(playback) else {
            Self.note(.warn, "#\(playback) refused \(Self.describe(op)): \(closedByViewer ? "the viewer closed the player" : "that playback is over")")
            sayClosed(playback)
            return
        }
        enter(playback)
        Self.note(Self.loud(op) ? .info : .debug, "#\(playback) \(Self.describe(op))\(elapsed)")
        if case .seek(let seconds, let absolute) = op {
            restartCountdown(at: absolute ? seconds : playhead + seconds)
        }
        switch HostedStep(op) {
        case .open(let fullScreen, let speed, let volume):
            open(for: playback, fullScreen: fullScreen, speed: speed, volume: volume)
        case .call(let call):
            guard backend != nil else {
                Self.note(.warn, "#\(playback) refused \(Self.describe(op)): no player is open")
                return
            }
            if case .load(let url, let options) = call {
                loads.append(playback)
                sources[playback] = PreviewSource(url: url, options: PreviewSource.reaching(options))
                hasPicture = false
            }
            enqueue(.call(call))
        case .fullScreen(let on):
            fullScreen = on ?? !fullScreen
        case .quit:
            quit()
        case .close:
            // A file that never showed a frame may be a playback that failed: the starting screen
            // stays up for the engine's verdict, which follows the close.
            close(reporting: false, awaitingVerdict: !hasPicture)
        }
    }

    func apply(_ ui: PlayerUi, for playback: PlaybackId) {
        guard accepts(playback) else {
            sayClosed(playback)
            return
        }
        enter(playback)
        let now = ContinuousClock.now
        switch ui {
        case .starting(let id, let title, let episode, let episodeTitle, let coverUrl, let bannerUrl):
            if since == nil || self.playback.map({ playback > $0 }) == true { since = now }
            Self.note(.info, "#\(playback) starting episode \(episode)\(elapsed)")
            starting = StartingScreen(
                playback: playback, id: id, title: title, episode: episode, episodeTitle: episodeTitle,
                coverUrl: coverUrl, bannerUrl: bannerUrl, since: now)
            present()
        case .episode(let number, let label, let title, let show, let translation):
            episode = EpisodeOnScreen(number: number, label: label, title: title, show: show, translation: translation)
            nowPlaying.update()
        case .menus(let episodes, let sources, let translation, let upscaling, let toneMapping, let nightMode):
            menus = PlayerMenus(
                episodes: episodes, sources: sources, translation: translation, upscaling: upscaling,
                toneMapping: toneMapping, nightMode: nightMode)
        case .skip(let kind, let to):
            Self.note(.info, "#\(playback) skip \(kind) offered, to \(Int(to)) s")
            skip = SkipOffer(kind: kind, to: to, since: now)
        case .skipCleared:
            skip = nil
        case .autoSkipped(let kind, let undoTo):
            undo = UndoOffer(kind: kind, undoTo: undoTo, from: nil, since: now)
        case .upNext(let next, let auto, let endingAt, let countdown, let stillWatchingAfter):
            Self.note(.info, "#\(playback) up next: episode \(next.number), auto \(auto), from \(endingAt.map { "\(Int($0)) s" } ?? "near the end"), \(countdown) s")
            var card = UpNextCard(
                next: next, auto: auto, endingAt: endingAt, countdown: Double(countdown),
                stillWatchingAfter: Int(stillWatchingAfter))
            self.stillWatchingAfter = Int(stillWatchingAfter)
            if let before = upNext, before.next.number == next.number {
                card.cancelled = before.cancelled
                card.played = before.played
                card.restartedAt = before.restartedAt
                card.sought = before.sought
            }
            upNext = card
        case .upNextCleared:
            upNext = nil
        case .stall(let seconds):
            Self.note(.info, "#\(playback) stalled \(seconds) s\(stallWaived ? ", waived" : "")")
            if !stallWaived { stall = StallCard(seconds: seconds, since: now) }
        case .stallCleared:
            stall = nil
            stallWaived = false
        case .resumed(let position):
            resume = ResumeNote(position: position)
        case .picture(let upscaler, let hdr):
            self.upscaler = upscaler
            self.hdr = hdr
        case .thumbnails(let url):
            if url != thumbnails { preview.close() }
            thumbnails = url
        case .alert(let text, let kind):
            Self.note(kind == .alert ? .warn : .info, "#\(playback) alert: \(text)")
            alert = PlayerAlert(text: text, kind: kind, since: now)
        }
    }

    /// Whether ops and interface for `playback` are still taken: not once it is over, nor any
    /// at all once the viewer has closed the player, until a playback is staged again.
    private func accepts(_ playback: PlaybackId) -> Bool {
        guard !closedByViewer else { return false }
        guard let closedThrough else { return true }
        return playback > closedThrough
    }

    /// Tell the engine this player is closed for `playback`, once: what it says to a player that
    /// is gone is refused, and a playback that opened one here, as the next episode found after
    /// the viewer closed it can, ends as closed rather than waiting on it.
    private func sayClosed(_ playback: PlaybackId) {
        guard closedSaid.insert(playback).inserted else { return }
        Self.logReport(.closed, for: playback)
        send(.playerReport(playback: playback, report: .closed))
    }

    /// The engine speaks of `playback`: a newer one starts with none of the last one's cards.
    private func enter(_ playback: PlaybackId) {
        guard self.playback.map({ playback > $0 }) ?? true else { return }
        self.playback = playback
        stagedAhead = false
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
            let backend = makeBackend?() ?? Self.mpv(shaderCache: shaderCache)
            if makeBackend == nil {
                Self.note(.info, "mpv shader cache \(shaderCache.map { $0.path(percentEncoded: false) } ?? "none")")
            }
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
            let handed = HandedLayer(layer: layer)
            worker = Task { @MainActor [weak self] in
                do {
                    try await backend.begin(on: handed.layer)
                } catch {
                    Self.note(.error, "mpv did not start: \(error)")
                    self?.failed(backend)
                    return
                }
                if let self { Self.note(.info, "mpv started\(self.elapsed)") }
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
                        Self.note(.warn, "mpv refused \(Self.describe(item)): \(error)")
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

    /// Back from the player, asked from outside it (Library › Back): the Mac's player shrinks
    /// into its banner and stops. Counted, for the player's view to answer.
    public private(set) var backRequests = 0

    public func requestBack() {
        if pictureInPicture || !isPresented {
            viewerClosed()
        } else {
            backRequests += 1
        }
    }

    /// The viewer closed the player: its close button, the swipe down, the window's close.
    ///
    /// Nothing the engine says brings it back up until it stages another playback: a playback
    /// still being found, the first or the next after Play Now or an Episodes pick, is stopped,
    /// and should its player open all the same, it is told the player closed.
    public func viewerClosed() {
        input()
        closedByViewer = true
        if failure != nil {
            // Nothing is playing: the screen only said so.
            dismiss()
            return
        }
        guard backend != nil else {
            if isPresented {
                // Nothing opened yet: stop what is being found.
                stopFinding()
                dismiss()
            }
            return
        }
        close(reporting: true)
        // The file on screen is closed; the next one staged since is still being found.
        if stagedAhead { send(.player(.stop)) }
    }

    /// Stop what the engine is finding for this player: aimed at the playback on screen, unless
    /// one has been staged since that has said nothing yet. Aimed at the one before, a Stop would
    /// miss it, so it goes to whatever plays, which is the one staged last.
    private func stopFinding() {
        if stagedAhead || playback == nil {
            send(.player(.stop))
        } else {
            control(.stop)
        }
    }

    /// Close mpv and the player. `reporting`: say the file ended by quitting, and that the
    /// player closed, first.
    private func close(reporting: Bool, awaitingVerdict: Bool = false) {
        Self.note(.info, reporting ? "closing: the file ended by quitting" : "closing: the playback is over")
        if reporting {
            report(.ended(reason: .quit))
            report(.closed)
        }
        teardown(awaitingVerdict: awaitingVerdict)
    }

    /// mpv went away by itself.
    private func ended(_ backend: any PlayerBackend) {
        guard backend === self.backend else { return }
        Self.note(.warn, "mpv went away by itself")
        report(.closed)
        teardown()
    }

    /// mpv would not start: the player is gone before it was there.
    private func failed(_ backend: any PlayerBackend) {
        guard backend === self.backend else { return }
        report(.closed)
        teardown()
    }

    private func teardown(awaitingVerdict: Bool = false) {
        listener?.cancel()
        worker?.cancel()
        work?.finish()
        listener = nil
        worker = nil
        work = nil
        if let backend {
            ending.append(Task.detached { await backend.end() })
        }
        backend = nil
        preview.close()
        closedThrough = [playback, current, opened, loads.max()].compactMap { $0 }.max()
        opened = nil
        current = nil
        loads = []
        sources = [:]
        thumbnails = nil
        guard awaitingVerdict, isPresented else {
            dismiss()
            return
        }
        // mpv is gone; the screen waits a moment for how the playback ended.
        layer = nil
        verdictPending = true
        Task { @MainActor [weak self] in
            try? await Task.sleep(for: PlayerTiming.verdictWait)
            guard let self, self.verdictPending else { return }
            self.verdictPending = false
            self.dismiss()
        }
    }

    /// The player comes down, with everything it showed.
    private func dismiss() {
        isPresented = false
        failure = nil
        choosingSource = false
        sourceChoices = nil
        verdictPending = false
        layer = nil
        fullScreen = false
        pictureInPicture = false
        subtitleTween?.cancel()
        subtitlePosition = 100
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
        hovering = nil
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
            mpvSaid = []
            if !loads.isEmpty { current = loads.removeFirst() }
            Self.note(.info, "#\(current.map(String.init) ?? "?") mpv started the file\(elapsed)")
            // Another file: its previews are another file's too.
            preview.close()
            let keepPause = paused
            resetFile()
            paused = keepPause
        case .playbackRestart:
            if !hasPicture { firstFrame() }
        case .fileLoaded:
            Self.note(.info, "#\(current.map(String.init) ?? "?") file loaded\(elapsed)")
        case .endFile(let end):
            Self.note(end.reason == "error" ? .warn : .info,
                "#\(current.map(String.init) ?? "?") file ended: \(end.reason)\(end.error.map { ", \($0)" } ?? "")")
        case .log(let line) where ["fatal", "error", "warn"].contains(line.level):
            // mpv's warnings and errors, each said once a file: a stream can repeat one per segment.
            let text = "\(line.prefix): \(line.text)"
            if mpvSaid.insert(text).inserted {
                Self.note(line.level == "warn" ? .info : .warn, "mpv \(line.level) \(text)")
            }
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
        let size = layer.map { "\(Int($0.drawableSize.width))×\(Int($0.drawableSize.height))" } ?? "no layer"
        Self.note(.info, "#\(current.map(String.init) ?? "?") first frame, drawn at \(size)\(elapsed)")
        if let starting, current.map({ $0 >= starting.playback }) ?? true {
            self.starting = nil
        }
        if stillWatchingAfter > 0, rolledOn >= stillWatchingAfter, !stillWatching {
            stillWatching = true
            if !paused { control(.togglePause) }
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
            Self.note(.debug, "tracks: \(audioTracks.count) audio, \(subtitleTracks.count) subtitles")
        case "video-params":
            if value["w"].int != nil {
                Self.note(.info, "video \(value["w"].int ?? 0)×\(value["h"].int ?? 0) \(value["pixelformat"].string ?? "")")
            }
            if let w = value["dw"].int ?? value["w"].int, let h = value["dh"].int ?? value["h"].int, w > 0, h > 0 {
                videoAspect = Double(w) / Double(h)
            }
        case "aid":
            audioTrack = value.int
        case "sid":
            subtitleTrack = value.int
        case "paused-for-cache":
            let waiting = value.bool ?? false
            if waiting, bufferingSince == nil { Self.note(.info, "buffering at \(Int(playhead)) s") }
            if !waiting, let since = bufferingSince { Self.note(.info, "buffered after \(AppLog.ms(since: since)) ms") }
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

    /// The playhead moved: the cards that run on playback time. Not Up Next's countdown while
    /// the viewer scrubs, whose keyframe seeks move the playhead under the finger.
    private func played() {
        if scrubbing == nil, var card = upNext, !card.played,
            let left = card.remaining(at: playhead, duration: duration), left <= 0
        {
            card.played = true
            upNext = card
            // Asked for by someone there when they sought into the ending: not one in a row
            // with nobody watching.
            rollPending = !card.sought
            control(.playNext)
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

    /// A seek, the viewer's or the engine's, to `target`: an Up Next countdown it lands in or past
    /// runs again from its full length there, rather than playing the next episode at once.
    private func restartCountdown(at target: Double) {
        guard var card = upNext, !card.played, let start = card.showsAt(duration: duration) else { return }
        card.restartedAt = target >= start ? target : nil
        upNext = card
    }

    /// The viewer sought: Up Next, if it is up, plays next for someone who is there.
    private func sought() {
        guard var card = upNext, !card.played else { return }
        card.sought = true
        upNext = card
    }

    // MARK: Commands

    /// Steer the playback the player shows, and only it (`Command::Control`): a Play Now or an
    /// Undo sent as an episode rolls on cannot reach the next one. Before the engine has named
    /// one, whatever plays.
    private func control(_ command: PlayerCommand) {
        if let playback {
            send(.control(playback: playback, command: command))
        } else {
            send(.player(command))
        }
    }

    // MARK: Reports

    /// Tell the engine, for the playback whose file mpv has.
    private func report(_ report: PlayerReport) {
        guard let target = current ?? opened else {
            Self.note(.debug, "no playback to report \(report) for")
            return
        }
        if report == .closed { closedSaid.insert(target) }
        Self.logReport(report, for: target)
        send(.playerReport(playback: target, report: report))
    }

    private func reportPower() {
        let power = PowerState.now()
        Self.note(.info, "power: \(power.onBattery ? "battery" : "mains")\(power.lowPower ? ", low power mode" : "")")
        report(.power(onBattery: power.onBattery, lowPower: power.lowPower))
    }

    /// The device's power changed: on battery or off, low power mode.
    public func powerChanged() {
        guard backend != nil else { return }
        reportPower()
        if lowPower() { preview.close() }
    }

    // MARK: The surface

    /// The screen the surface is on.
    public func surfaceDisplay(_ display: DisplayInfo) {
        guard backend != nil else { return }
        let size = layer.map { "\(Int($0.drawableSize.width))×\(Int($0.drawableSize.height))" } ?? "none"
        Self.note(.info, "display \(display.width)×\(display.height) at \(display.scale)×, \(Int(display.refreshRate)) Hz, EDR \(display.edrPotential); surface \(size)")
        enqueue(.display(display))
        report(PlayerReport(display))
    }

    public func surfaceResized() {
        enqueue(.resized)
    }

    /// What the window shows now, as mpv renders it, or nil without an mpv.
    func snapshot() async -> CGImage? {
        await backend?.snapshot()
    }

    private func enqueue(_ item: Work) {
        work?.yield(item)
    }

    /// Wait until every mpv this player closed, the preview's too, has shut down: before a
    /// process that played exits, so no mpv thread outlives it.
    public func finished() async {
        let tasks = ending
        ending = []
        for task in tasks { await task.value }
        await preview.finished()
    }

    // MARK: The log

    /// "+412 ms" since the playback was staged, for the log's timings.
    private var elapsed: String {
        since.map { ", +\(AppLog.ms(since: $0)) ms" } ?? ""
    }

    nonisolated static func note(_ level: Level, _ message: String) {
        switch level {
        case .error: AppLog.error("player", message)
        case .warn: AppLog.warn("player", message)
        case .info: AppLog.info("player", message)
        case .debug: AppLog.debug("player", message)
        }
    }

    enum Level { case error, warn, info, debug }

    /// What the log says of a report back to the engine: each one but the playhead's and the
    /// other properties that change every frame, values cut short.
    private static func logReport(_ report: PlayerReport, for playback: PlaybackId) {
        switch report {
        case .property(let name, let json):
            guard !["time-pos", "frame-drop-count", "seeking", "paused-for-cache"].contains(name) else { return }
            let value = json.count > 120 ? json.prefix(120) + "…" : Substring(json)
            note(.info, "#\(playback) report \(name) = \(value)")
        case .display, .power:
            note(.debug, "#\(playback) report \(report)")
        default:
            note(.info, "#\(playback) report \(report)")
        }
    }

    /// The ops worth a line at info: the ones that change what plays.
    static func loud(_ op: PlayerOp) -> Bool {
        switch op {
        case .open, .load, .stop, .quit, .close, .shaders: true
        default: false
        }
    }

    /// An op as the log says it: names, never header values, paths or queries.
    static func describe(_ op: PlayerOp) -> String {
        switch op {
        case .open(let fullscreen, let speed, let volume):
            "open\(fullscreen ? " full screen" : "")\(speed.map { ", speed \($0)" } ?? "")\(volume.map { ", volume \(Int($0))" } ?? "")"
        case .load(let url, let options):
            "load \(AppLog.address(url)) with \(options.map(\.name).sorted().joined(separator: ", "))"
        case .addSubtitle(_, _, let language): "add a subtitle track (\(language ?? "no language"))"
        case .set(let property, _): "set \(property)"
        case .cycle(let property): "cycle \(property)"
        case .toggleFullscreen: "toggle full screen"
        case .add(let property, let delta): "add \(delta) to \(property)"
        case .seek(let seconds, let absolute): absolute ? "seek to \(Int(seconds)) s" : "seek by \(Int(seconds)) s"
        case .shaders(let paths): paths.isEmpty ? "shaders off" : "shaders: \(paths.map { ($0 as NSString).lastPathComponent }.joined(separator: ", "))"
        case .audioFilter(let add, _): add ? "add an audio filter" : "remove an audio filter"
        case .restateChapters: "restate the chapters"
        case .stop: "stop"
        case .quit: "quit"
        case .close: "close"
        }
    }

    private static func describe(_ item: Work) -> String {
        switch item {
        case .call(.load(let url, _)): "load \(AppLog.address(url))"
        case .call(.addSubtitle): "add a subtitle track"
        case .call(.set(let property, _)): "set \(property)"
        case .call(let call): "\(call)".prefix(80).description
        case .display: "the display"
        case .resized: "a resize"
        }
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
    public var startingLine: String? { startingLine(note: nil) }

    /// The same, with the engine's note on the job that starts the episode in place of "Finding
    /// a source" when it has one: what the wait is on.
    public func startingLine(note: String?) -> String? {
        guard let starting else {
            return holdsCurtain ? staged.map { "Starting episode \($0.episode)" } : nil
        }
        return starting.line(at: clock, note: note)
    }

    /// The note on the job starting `episode`, the newest such, from the store's jobs: the engine
    /// calls a play's job "starting ep 7".
    public static func startingNote(in jobs: [Job], episode: String) -> String? {
        jobs.last { $0.activity == "starting ep \(episode)" }?.note
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
        let progress = 1 - left / max(upNext?.countdown ?? PlayerTiming.upNextCountdown, 1)
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

    /// What the seek preview opens: the `Thumbnails` address, or the stream playing.
    public var previewSource: PreviewSource? {
        let stream = current.flatMap { sources[$0] }
        if let thumbnails { return PreviewSource(url: thumbnails, options: stream?.options ?? [:]) }
        return stream
    }

    /// Whether scrubbing shows frames: not in Low Power Mode.
    public var previewsOn: Bool { !lowPower() }

    /// The chapter at `seconds`, by its name.
    public func chapter(at seconds: Double) -> String? {
        chapters.sorted { $0.time < $1.time }.last { $0.time <= seconds }.map(\.title).flatMap { $0.isEmpty ? nil : $0 }
    }

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

    // MARK: When it could not play

    /// The episode starting, as the starting screen names it.
    public var startingEpisode: String? { starting?.episode ?? staged?.episode ?? failure?.episode }

    /// Ask the engine for the episode's releases, to pick one: from "Couldn't play episode 7",
    /// or from a start that has shown nothing for 20 s.
    public func chooseSource() {
        input()
        guard let id = staged?.id ?? failure?.id, let episode = startingEpisode else { return }
        choosingSource = true
        sourceChoices = nil
        send(.loadSources(id: id, episode: episode))
    }

    /// Play `candidate` instead: the engine stages it as a playback of its own.
    public func play(_ candidate: SourceCandidate) {
        input()
        guard let id = staged?.id ?? failure?.id, let episode = startingEpisode else { return }
        Self.note(.info, "playing episode \(episode) from the source picked, \(candidate.providerId)")
        choosingSource = false
        sourceChoices = nil
        failure = nil
        send(.playSource(id: id, episode: episode, providerId: candidate.providerId, sourceId: candidate.id))
    }

    /// Done with the sources: back to what the screen showed.
    public func cancelChoosingSource() {
        choosingSource = false
        sourceChoices = nil
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
        control(.togglePause)
    }

    public func seek(by seconds: Double) {
        input()
        sought()
        control(.seek(seconds))
    }

    public func seek(to seconds: Double) {
        input()
        sought()
        control(.seekTo(seconds))
    }

    /// The finger or pointer on the seek line: keyframe seeks while it moves, an exact one on
    /// release.
    public func scrub(to seconds: Double, pace: Double = 1) {
        if scrubbing == nil { hold("scrub", true) }
        scrubbing = seconds
        scrubPace = pace
        position = seconds
        if !lowPower(), let source = previewSource { preview.show(seconds, of: source) }
        // A keyframe seek each half second of movement: mpv drops the ones it cannot keep up with
        // anyway, and the line follows the finger meanwhile.
        if let sent = scrubSent, abs(sent - seconds) < 0.5 { return }
        scrubSent = seconds
        enqueue(.call(.seek(seconds: seconds, mode: .keyframe)))
    }

    /// The pointer over the seek line at `seconds`, or off it: the time, the chapter and the
    /// frame there show above it, as the terminal's thumbnails do. The preview opens on the first
    /// hover of a file.
    public func hover(at seconds: Double?) {
        hovering = seconds
        guard let seconds else { return }
        if !lowPower(), let source = previewSource { preview.show(seconds, of: source) }
    }

    public func endScrub() {
        guard let seconds = scrubbing else { return }
        scrubbing = nil
        scrubSent = nil
        scrubPace = 1
        hold("scrub", false)
        seek(to: seconds)
    }

    /// Play at `target` times normal speed.
    public func setSpeed(_ target: Double) {
        input()
        guard speed > 0, target > 0 else { return }
        control(.speed(target / speed))
    }

    public func stepVolume(by points: Double) {
        input()
        control(.volume(points))
    }

    public func toggleMute() {
        input()
        muted.toggle()
        enqueue(.call(.set(property: "mute", value: .flag(muted))))
    }

    /// An audio or subtitle track picked, by its `track-list` id, or the subtitles off: the
    /// engine selects it and keeps it for the series (`PlayerCommand::ChooseTrack`).
    public func selectAudio(_ id: Int64) {
        input()
        control(.chooseTrack(kind: .audio, id: id))
    }

    public func selectSubtitle(_ id: Int64?) {
        input()
        control(.chooseTrack(kind: .subtitle, id: id))
    }

    public func takeSkip() {
        input()
        sought()
        skipsTaken += 1
        skip = nil
        control(.takeSkip)
    }

    public func undoSkip() {
        input()
        guard let note = undo else { return }
        undo = nil
        control(.undoSkip(to: note.undoTo))
    }

    public func playNext() {
        input()
        guard var card = upNext, !card.played else { return }
        card.played = true
        upNext = card
        control(.playNext)
    }

    public func cancelNext() {
        input()
        guard var card = upNext else { return }
        card.cancelled = true
        upNext = card
        control(.cancelNext)
    }

    public func choose(_ choice: PlayerChoice) {
        input()
        control(.choose(choice))
    }

    public func setUpscaling(_ id: String) {
        input()
        control(.upscaling(id))
    }

    public func setNightMode(_ on: Bool) {
        input()
        control(.nightMode(on))
    }

    /// The next (`1`) or previous (`-1`) episode.
    public func step(_ by: Int64) {
        input()
        control(.step(by))
    }

    public func switchSource() {
        input()
        stall = nil
        control(.stallSwitch)
    }

    public func keepWaiting() {
        input()
        stall = nil
        stallWaived = true
        control(.stallWait)
    }

    public func startOver() {
        input()
        resume = nil
        control(.seekTo(0))
    }

    /// "Still watching?": Continue watching.
    public func continueWatching() {
        input()
        stillWatching = false
        if paused { control(.togglePause) }
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

    /// Lift the subtitles by `fraction` of the picture's height, above the controls while they
    /// are up, and back down with them: `sub-pos`, which libass applies to the dialogue at the
    /// bottom of ASS subtitles and leaves signs where they are. Eased over the controls' fade, or
    /// at once.
    public func liftSubtitles(_ fraction: Double, animated: Bool) {
        let target = (100 - min(0.5, max(0, fraction)) * 100).rounded(toPlaces: 1)
        guard target != subtitlePosition else { return }
        let from = subtitlePosition
        subtitlePosition = target
        subtitleTween?.cancel()
        guard animated else {
            enqueue(.call(.set(property: "sub-pos", value: .double(target))))
            return
        }
        // The standard spring's 0.3 s, as eight steps on an ease-out.
        subtitleTween = Task { @MainActor [weak self] in
            let steps = 8
            for step in 1...steps {
                let t = Double(step) / Double(steps)
                let eased = 1 - pow(1 - t, 3)
                let value = (from + (target - from) * eased).rounded(toPlaces: 1)
                self?.enqueue(.call(.set(property: "sub-pos", value: .double(value))))
                guard step < steps else { break }
                try? await Task.sleep(for: .milliseconds(300 / steps))
                if Task.isCancelled { return }
            }
        }
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

extension Double {
    func rounded(toPlaces places: Int) -> Double {
        let scale = pow(10, Double(places))
        return (self * scale).rounded() / scale
    }
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
