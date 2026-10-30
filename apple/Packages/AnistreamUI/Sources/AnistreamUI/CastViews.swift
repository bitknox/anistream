// Casting to a TV with Chromecast built in: the Play on TV picker's Chromecast group, and the
// casting card in the TV sheet's place. The engine's sender says what the rows and the card read
// (CastModel); these views lay them out as Play on TV's sheet does, each platform its own way: on
// the Mac rows on the ground of a raised sheet with the buttons at its foot, on iPhone and iPad a
// sheet of grouped boxes.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

// MARK: - Words

/// What the casting views say that both platforms share.
@MainActor
enum CastWords {
    /// The line under a row: who fetches the episode.
    static func route(_ row: FfiCastRow) -> String {
        switch row.routeHint {
        case .direct: L.castChromecast
        case .throughThisDevice: L.castRowThrough(device: TVRemoteModel.deviceName)
        }
    }

    /// The note under Show TVs with Chromecast built in, naming the device that sends the episode.
    static var tvsNote: String {
        switch TVRemoteModel.deviceName {
        case "iPhone": L.castTvsNoteIphone
        case "iPad": L.castTvsNoteIpad
        default: L.castTvsNoteMac
        }
    }

    /// The note under Torrents while casting.
    static var torrentsNote: String {
        switch TVRemoteModel.deviceName {
        case "iPhone": L.castTorrentsNoteIphone
        case "iPad": L.castTorrentsNoteIpad
        default: L.castTorrentsNoteMac
        }
    }

    /// The dot beside the card's line: water while it starts and plays, vermilion once it ended.
    static func dot(_ stage: CastStage) -> Color {
        switch stage {
        case .starting, .playing: .water
        case .failed, .stopped: .vermilion
        }
    }

    /// The toolbar's button and the bar: the card's line, with no seconds counted.
    @MainActor static func barLine(_ cast: CastModel) -> String {
        switch cast.stage {
        case .starting: L.tvStartingOn(name: cast.tvName)
        case .playing: L.tvStatusPlaying(name: cast.tvName)
        case .failed: L.castCouldntStart(name: cast.tvName)
        case .stopped: L.castStopped
        }
    }

    /// Where the system lets the app look on the local network.
    static var localNetworkSettings: URL? {
        #if os(macOS)
        URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork") // i18n-skip
        #else
        URL(string: UIApplication.openSettingsURLString)
        #endif
    }
}

// MARK: - The cast icon

/// A screen with waves in its lower corner, as TVs with Chromecast built in are drawn.
struct CastGlyph: Shape {
    func path(in rect: CGRect) -> Path {
        var path = Path()
        path.move(to: CGPoint(x: 1.5, y: 5))
        path.addLine(to: CGPoint(x: 1.5, y: 3.5))
        path.addRelativeArc(center: CGPoint(x: 2.5, y: 3.5), radius: 1, startAngle: .degrees(180), delta: .degrees(90))
        path.addLine(to: CGPoint(x: 13.5, y: 2.5))
        path.addRelativeArc(center: CGPoint(x: 13.5, y: 3.5), radius: 1, startAngle: .degrees(-90), delta: .degrees(90))
        path.addLine(to: CGPoint(x: 14.5, y: 11.5))
        path.addRelativeArc(center: CGPoint(x: 13.5, y: 11.5), radius: 1, startAngle: .degrees(0), delta: .degrees(90))
        path.addLine(to: CGPoint(x: 9.5, y: 12.5))
        path.move(to: CGPoint(x: 1.5, y: 13.5))
        path.addLine(to: CGPoint(x: 1.51, y: 13.5))
        path.move(to: CGPoint(x: 1.5, y: 11))
        path.addRelativeArc(center: CGPoint(x: 1.5, y: 13.5), radius: 2.5, startAngle: .degrees(-90), delta: .degrees(90))
        path.move(to: CGPoint(x: 1.5, y: 8.5))
        path.addRelativeArc(center: CGPoint(x: 1.5, y: 13.5), radius: 5, startAngle: .degrees(-90), delta: .degrees(90))
        return path.applying(
            CGAffineTransform(translationX: rect.minX, y: rect.minY).scaledBy(x: rect.width / 16, y: rect.height / 16))
    }
}

struct CastIcon: View {
    let size: CGFloat
    var color: Color = .haze

    var body: some View {
        CastGlyph()
            .stroke(color, style: StrokeStyle(lineWidth: 1.3 * size / 16, lineCap: .round, lineJoin: .round))
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

// MARK: - The picker's group

/// A row of the Chromecast group: the TV, who fetches the episode, and a pick casts to it. A TV
/// that just left says so for a few seconds, dimmed, and can't be picked.
struct CastPickerRow: View {
    @Environment(AppStore.self) private var store
    let shown: CastRowShown
    let phone: Bool

    var body: some View {
        Button {
            store.tv.choose(cast: shown.row)
        } label: {
            HStack(spacing: phone ? 14 : 12) {
                CastIcon(size: phone ? 22 : 18).opacity(shown.gone ? 0.5 : 1)
                VStack(alignment: .leading, spacing: 2) {
                    Text(shown.row.name)
                        .font(phone ? .ui(17) : .mac(13))
                        .foregroundStyle(shown.gone ? .haze : .silk)
                    if shown.gone {
                        TVProblemLine(text: L.noticeCastNoDevice, touch: phone)
                    } else {
                        Text(CastWords.route(shown.row))
                            .font(phone ? .ui(13) : .mac(12))
                            .foregroundStyle(.haze)
                    }
                }
                Spacer(minLength: 8)
            }
            .padding(.horizontal, phone ? 0 : 14)
            .frame(minHeight: phone ? 52 : 46)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .disabled(shown.gone || store.tv.request == nil)
        .transition(.opacity)
        .accessibilityElement(children: .combine)
    }
}

/// The picker's one box while the system won't let the app look on the local network: both
/// groups need it, so it asks once, with the way to the system's setting.
struct CastLocalNetworkRow: View {
    @Environment(\.openURL) private var openURL

    var body: some View {
        HStack(spacing: 12) {
            Text(L.castLocalNetwork)
                .font(.mac(13))
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button(L.actionOpenSettings) {
                if let url = CastWords.localNetworkSettings { openURL(url) }
            }
            .buttonStyle(PrimaryCapsuleStyle(height: 26))
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
    }
}

#if os(macOS)
/// The Mac picker's Chromecast group, under the anistream TVs: its heading comes and goes with
/// its first and last row.
struct MacCastGroup: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let cast = store.tv.cast
        VStack(alignment: .leading, spacing: 6) {
            if !cast.rows.isEmpty {
                Text(L.castChromecast)
                    .font(.mac(12, weight: .semibold))
                    .foregroundStyle(.haze)
                    .padding(.leading, 4)
                    .transition(.opacity)
                MacSheetRows {
                    ForEach(cast.rows) { shown in
                        CastPickerRow(shown: shown, phone: false)
                    }
                }
                .transition(.opacity)
            }
        }
        .padding(.top, cast.rows.isEmpty ? 0 : 14)
        .animation(.standard, value: cast.rows)
    }
}
#endif

// MARK: - The card's parts

/// The card's top line: the dot, and what the cast is doing. Past four seconds of starting it
/// counts the seconds, and each change crossfades.
struct CastStatusLine: View {
    @Environment(AppStore.self) private var store
    let touch: Bool

    var body: some View {
        let cast = store.tv.cast
        let line = cast.line
        HStack(spacing: 8) {
            Circle()
                .fill(CastWords.dot(cast.stage))
                .frame(width: 7, height: 7)
                .accessibilityHidden(true)
            Text(line)
                .font(touch ? .ui(15, weight: .semibold) : .mac(13, weight: .semibold))
                .foregroundStyle(.silk)
                .monospacedDigit()
                .contentTransition(.opacity)
                .animation(.standard, value: line)
        }
        .accessibilityElement(children: .combine)
    }
}

/// The Up next row on the cast card: the next episode and its count, with Cancel and Play now.
/// At zero the engine plays the episode itself. With auto-next off it waits for Play now.
struct CastUpNextRow: View {
    @Environment(AppStore.self) private var store
    let next: FfiCastUpNext
    let touch: Bool

    var body: some View {
        let cast = store.tv.cast
        let heading = next.secondsLeft.map { L.castUpNextPlaysIn(seconds: Int($0)) } ?? L.playerUpNext
        let episode = next.title.map { L.castEpisodeTitled(episode: next.number, title: $0) }
            ?? L.castEpisode(episode: next.number)
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 3) {
                Text(heading)
                    .font(touch ? .ui(13, weight: .semibold) : .mac(12, weight: .semibold))
                    .foregroundStyle(.haze)
                    .monospacedDigit()
                    .contentTransition(.numericText(countsDown: true))
                    .animation(.linear(duration: 0.25), value: heading)
                Text(episode)
                    .font(touch ? .ui(15) : .mac(13))
                    .foregroundStyle(.silk)
                    .lineLimit(2)
            }
            HStack(spacing: 8) {
                Button(L.actionCancel) { cast.cancelNext() }
                    .buttonStyle(SecondaryCapsuleStyle(height: touch ? 34 : 26, raisedSurface: !touch, fontSize: touch ? 15 : 13))
                Button(L.actionPlayNow) { cast.playNext() }
                    .buttonStyle(PrimaryCapsuleStyle(height: touch ? 34 : 26))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
    }
}

/// The note after an automatic skip, with its Undo.
struct CastUndoNote: View {
    @Environment(AppStore.self) private var store
    let undo: FfiCastUndo
    let touch: Bool

    var body: some View {
        HStack(spacing: 10) {
            Text(undo.kind == .opening ? L.playerSkippedOpening : L.playerSkippedEnding)
                .font(touch ? .ui(15) : .mac(13))
                .foregroundStyle(.haze)
            Button(L.actionUndo) { store.tv.cast.undoSkip() }
                .buttonStyle(SecondaryCapsuleStyle(height: touch ? 34 : 26, raisedSurface: !touch, fontSize: touch ? 15 : 13))
        }
    }
}

/// The TV's volume: on a TV that keeps its own, the line saying so and Mute; on one that takes
/// a level, a slider sending steps as it moves, set again by each of the TV's reports.
struct CastVolumeRow: View {
    @Environment(AppStore.self) private var store
    let touch: Bool
    @State private var dragging: Double?
    @State private var sent: Double?

    var body: some View {
        let cast = store.tv.cast
        Group {
            if let level = cast.now?.volume, !cast.fixedVolume {
                HStack(spacing: 12) {
                    Text(L.tvVolume)
                        .font(touch ? .ui(17) : .mac(13))
                        .foregroundStyle(.silk)
                        .frame(minWidth: touch ? nil : 76, alignment: .leading)
                    Image(systemName: "speaker.fill")
                        .font(touch ? .ui(13) : .mac(11))
                        .foregroundStyle(.haze)
                        .accessibilityHidden(true)
                    Slider(value: Binding(
                        get: { dragging ?? level },
                        set: { value in
                            dragging = value
                            let from = sent ?? level
                            let step = (value - from).rounded()
                            if abs(step) >= 1 {
                                cast.send(.volume(step: step))
                                sent = from + step
                            }
                        }
                    ), in: 0...100) { editing in
                        if !editing {
                            dragging = nil
                            sent = nil
                        }
                    }
                    .tint(.silk)
                    .controlSize(touch ? .regular : .small)
                    .accessibilityLabel(L.tvVolume)
                    Image(systemName: "speaker.wave.3.fill")
                        .font(touch ? .ui(13) : .mac(11))
                        .foregroundStyle(.haze)
                        .accessibilityHidden(true)
                }
                .padding(.horizontal, touch ? 16 : 14)
                .frame(minHeight: touch ? 52 : 36)
            } else {
                HStack(spacing: 12) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(L.tvVolume)
                            .font(touch ? .ui(17) : .mac(13))
                            .foregroundStyle(.silk)
                        Text(cast.muted ? L.playerMuted : L.castVolumeFixed)
                            .font(touch ? .ui(13) : .mac(12))
                            .foregroundStyle(.haze)
                            .contentTransition(.opacity)
                    }
                    Spacer(minLength: 8)
                    Button {
                        cast.send(.mute(muted: !cast.muted))
                    } label: {
                        if cast.muted {
                            Text(L.playerUnmute)
                        } else {
                            Label(L.playerMute, systemImage: "speaker.slash")
                        }
                    }
                    .buttonStyle(SecondaryCapsuleStyle(height: touch ? 34 : 24, fontSize: touch ? 15 : 12))
                }
                .padding(.leading, touch ? 16 : 14)
                .padding(.trailing, touch ? 12 : 10)
                .padding(.vertical, 8)
                .animation(.standard, value: cast.muted)
            }
        }
        .disabled(!cast.live)
    }
}

/// What a failed or stopped card's buttons do.
@MainActor
struct CastActions {
    let store: AppStore
    let navigator: Navigator
    let dismiss: DismissAction

    func title(_ action: CastAction) -> String {
        switch action {
        case .chooseSource: L.playerChooseSource
        case .playHere: L.castPlayHere
        case .tryAgain: L.actionTryAgain
        case .chooseAnotherTV: L.tvChooseAnother
        }
    }

    func run(_ action: CastAction) {
        let cast = store.tv.cast
        switch action {
        case .chooseSource:
            // The episode's source list, the player's: once this sheet has gone, as a window
            // shows one sheet at a time.
            let id = cast.now?.id
            dismiss()
            cast.dismissEnded()
            guard let id else { return }
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(400))
                navigator.sourcePicker = id
            }
        case .playHere:
            playHere()
        case .tryAgain:
            cast.tryAgain()
        case .chooseAnotherTV:
            store.tv.castElsewhere()
        }
    }

    func playHere() {
        dismiss()
        store.tv.cast.playHere()
    }

    /// The sheet goes first, showing the card as it goes down.
    func stop() {
        dismiss()
        store.tv.cast.stop()
    }
}

#if os(macOS)
// MARK: - The Mac's card

/// The TV sheet while a cast is up, 440 wide as the sheet is: the line, the episode, the seek
/// line and transport, the tracks and the TV's volume, and the buttons at the foot. A cast that
/// never started, or stopped, says why under the episode and offers what fixes it.
struct MacCastCard: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let tv = store.tv
        let cast = tv.cast
        let stage = cast.stage
        let running = stage == .starting || stage == .playing
        let actions = CastActions(store: store, navigator: navigator, dismiss: dismiss)
        VStack(alignment: .leading, spacing: 16) {
            CastStatusLine(touch: false)
            if let now = cast.now {
                HStack(spacing: 14) {
                    if let entry = store.titles[now.id] { CoverArt(entry: entry, width: 60) }
                    VStack(alignment: .leading, spacing: 3) {
                        Text(now.title).font(.showTitle(size: 17)).foregroundStyle(.silk).lineLimit(2)
                        Text(now.episodeLine).font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                        if let skip = now.skip, stage == .playing {
                            Button(skip.label.capitalizedSentence) { tv.send(.skip) }
                                .buttonStyle(SecondaryCapsuleStyle(height: 26, raisedSurface: true))
                                .padding(.top, 8)
                                .transition(.opacity)
                        }
                    }
                }
                if let reason = cast.reason {
                    Text(reason)
                        .font(.mac(13))
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                        .transition(.opacity)
                }
                if stage != .failed {
                    TVSeekLine(now: now, touch: false)
                }
                if running {
                    TVTransport(now: now, touch: false)
                    if let next = cast.card?.upNext {
                        CastUpNextRow(next: next, touch: false).transition(.opacity)
                    } else if let undo = cast.card?.undo {
                        CastUndoNote(undo: undo, touch: false).transition(.opacity)
                    }
                    MacSheetRows {
                        SettingsRow(title: L.playerSubtitles) {
                            TVTrackMenu(title: L.playerSubtitles, kind: .subtitle, tracks: now.subtitles, off: now.subtitlesOff, phone: false)
                        }
                        SettingsRow(title: L.playerAudio) {
                            TVTrackMenu(title: L.playerAudio, kind: .audio, tracks: now.audio, off: false, phone: false)
                        }
                        CastVolumeRow(touch: false)
                    }
                    .opacity(tv.live ? 1 : 0.4)
                    .animation(.fadeIn, value: tv.live)
                }
            }
            HStack(spacing: 8) {
                if running {
                    // One row when it fits; otherwise Choose Another TV drops under the rest,
                    // so no button's words are cut short.
                    ViewThatFits(in: .horizontal) {
                        HStack(spacing: 8) { footer(tv, actions, dismiss, together: true) }
                        VStack(alignment: .leading, spacing: 8) { footer(tv, actions, dismiss, together: false) }
                    }
                } else {
                    ForEach(Array(cast.actions.enumerated()), id: \.offset) { index, action in
                        if index == 0 {
                            Button(actions.title(action)) { actions.run(action) }
                                .buttonStyle(PrimaryCapsuleStyle(height: 28))
                        } else {
                            Button(actions.title(action)) { actions.run(action) }
                                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        }
                    }
                    .transition(.opacity)
                    Spacer(minLength: 8)
                    Button(L.actionDone) { dismiss() }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .keyboardShortcut(.cancelAction)
                }
            }
            .padding(.top, 4)
        }
        .animation(.standard, value: stage)
    }

    @ViewBuilder
    private func footer(_ tv: TVRemoteModel, _ actions: CastActions, _ dismiss: DismissAction, together: Bool) -> some View {
        if together {
            chooseAnother(tv)
            Spacer(minLength: 8)
            rest(actions, dismiss)
        } else {
            HStack(spacing: 8) {
                Spacer(minLength: 0)
                rest(actions, dismiss)
            }
            chooseAnother(tv)
        }
    }

    private func chooseAnother(_ tv: TVRemoteModel) -> some View {
        Button(L.tvChooseAnother) { tv.castElsewhere() }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            .fixedSize()
    }

    @ViewBuilder
    private func rest(_ actions: CastActions, _ dismiss: DismissAction) -> some View {
        Button(L.castPlayHere) { actions.playHere() }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            .fixedSize()
        Button(L.castStop) { actions.stop() }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            .fixedSize()
        Button(L.actionDone) { dismiss() }
            .buttonStyle(PrimaryCapsuleStyle(height: 28))
            .keyboardShortcut(.cancelAction)
            .fixedSize()
    }
}
#endif

/// The way back to the card while casting, in the toolbar: the cast icon and the card's line,
/// or once it ended the vermilion dot.
struct CastToolbarButton: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let cast = store.tv.cast
        let ended = cast.stage == .failed || cast.stage == .stopped
        Button {
            store.tv.openRemote()
        } label: {
            HStack(spacing: 6) {
                if ended {
                    Circle().fill(.vermilion).frame(width: 6, height: 6)
                } else {
                    CastIcon(size: 15, color: .water)
                }
                Text(CastWords.barLine(cast)).foregroundStyle(.silk)
                    .contentTransition(.opacity)
            }
            .animation(.standard, value: cast.stage)
        }
        .help(L.tvShowRemote(name: cast.tvName))
        .accessibilityLabel(CastWords.barLine(cast))
    }
}

#if os(iOS)
// MARK: - iPhone and iPad's card

/// The TV sheet while a cast is up: the line and Done, the note to keep the app open, the
/// episode, the seek line and transport, the tracks and the TV's volume, and what else to do.
/// On iPad a form sheet, with those buttons in a row. The screen stays awake while it is up.
struct PhoneCastCard: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        let tv = store.tv
        let cast = tv.cast
        let stage = cast.stage
        let running = stage == .starting || stage == .playing
        let actions = CastActions(store: store, navigator: navigator, dismiss: dismiss)
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                HStack(spacing: 12) {
                    CastStatusLine(touch: true)
                    Spacer(minLength: 8)
                    Button(L.actionDone) { dismiss() }
                        .font(.ui(17, weight: .semibold))
                        .foregroundStyle(.silk)
                }
                .padding(.top, 4)
                if running {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Image(systemName: "info.circle")
                            .font(.ui(15))
                            .foregroundStyle(.water)
                            .accessibilityHidden(true)
                        Text(L.castKeepOpen(device: TVRemoteModel.deviceName, name: cast.tvName))
                            .font(.ui(13))
                            .foregroundStyle(.haze)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    .transition(.opacity)
                }
                if let now = cast.now {
                    HStack(spacing: 14) {
                        if let entry = store.titles[now.id] { CoverArt(entry: entry, width: 72) }
                        VStack(alignment: .leading, spacing: 3) {
                            Text(now.title).font(.showTitle(size: 20)).foregroundStyle(.silk).lineLimit(2)
                            Text(now.episodeLine).font(.ui(14)).foregroundStyle(.haze).lineLimit(2)
                        }
                    }
                    .padding(.top, 6)
                    if let reason = cast.reason {
                        Text(reason)
                            .font(.ui(15))
                            .foregroundStyle(.silk)
                            .fixedSize(horizontal: false, vertical: true)
                            .transition(.opacity)
                    }
                    if stage != .failed {
                        TVSeekLine(now: now, touch: true)
                    }
                    if running {
                        TVTransport(now: now, touch: true)
                        if let next = cast.card?.upNext {
                            CastUpNextRow(next: next, touch: true).transition(.opacity)
                        } else if let undo = cast.card?.undo {
                            CastUndoNote(undo: undo, touch: true).transition(.opacity)
                        }
                        if let skip = now.skip, stage == .playing {
                            Button(skip.label.capitalizedSentence) { tv.send(.skip) }
                                .buttonStyle(SecondaryCapsuleStyle(height: 34, fontSize: 15))
                                .frame(maxWidth: .infinity)
                                .transition(.opacity)
                        }
                        CastBox {
                            trackRow(L.playerSubtitles, kind: .subtitle, tracks: now.subtitles, off: now.subtitlesOff)
                            CastBoxRule()
                            trackRow(L.playerAudio, kind: .audio, tracks: now.audio, off: false)
                            CastBoxRule()
                            CastVolumeRow(touch: true)
                        }
                        .opacity(tv.live ? 1 : 0.4)
                        .animation(.fadeIn, value: tv.live)
                    }
                }
                if running {
                    if sizeClass == .regular {
                        HStack(spacing: 10) {
                            padButton(L.castPlayHere) { actions.playHere() }
                            padButton(L.tvChooseAnother) { tv.castElsewhere() }
                            padButton(L.castStop) { actions.stop() }
                        }
                    } else {
                        CastBox {
                            actionRow(L.castPlayHere) { actions.playHere() }
                            CastBoxRule()
                            actionRow(L.tvChooseAnother) { tv.castElsewhere() }
                            CastBoxRule()
                            actionRow(L.castStop) { actions.stop() }
                        }
                    }
                } else {
                    HStack(spacing: 10) {
                        ForEach(Array(cast.actions.enumerated()), id: \.offset) { index, action in
                            if index == 0 {
                                Button(actions.title(action)) { actions.run(action) }
                                    .buttonStyle(PrimaryCapsuleStyle(height: 44, fontSize: 17))
                            } else {
                                Button(actions.title(action)) { actions.run(action) }
                                    .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 17))
                            }
                        }
                    }
                    .padding(.top, 6)
                    .transition(.opacity)
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 16)
            .animation(.standard, value: stage)
        }
        .scrollBounceBehavior(.basedOnSize)
        .presentationSizing(.form)
        // A relayed stream stops when the app leaves the screen, so the screen stays on.
        .onAppear { UIApplication.shared.isIdleTimerDisabled = true }
        .onDisappear { UIApplication.shared.isIdleTimerDisabled = false }
    }

    private func trackRow(_ title: String, kind: FfiTvTrackKind, tracks: [FfiTvTrack], off: Bool) -> some View {
        HStack {
            Text(title).font(.ui(17)).foregroundStyle(.silk)
            Spacer(minLength: 8)
            TVTrackMenu(title: title, kind: kind, tracks: tracks, off: off, phone: true)
        }
        .padding(.horizontal, 16)
        .frame(minHeight: 48)
    }

    private func actionRow(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.ui(17))
                .foregroundStyle(.silk)
                .frame(maxWidth: .infinity, minHeight: 48, alignment: .leading)
                .padding(.horizontal, 16)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
    }

    private func padButton(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.ui(17))
                .foregroundStyle(.silk)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .frame(maxWidth: .infinity, minHeight: 44)
                .background(Palette.raised, in: .rect(cornerRadius: 12))
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
    }
}

/// A box of rows on the sheet, with hairlines between.
private struct CastBox<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) { content }
            .background(Palette.raised, in: .rect(cornerRadius: 12))
    }
}

private struct CastBoxRule: View {
    var body: some View {
        Rectangle().fill(.rule).opacity(0.6).frame(height: 1).padding(.leading, 16)
    }
}

/// The bar over the tab bar while casting: the cover, the title and the card's line, and play
/// or pause; a tap opens the card.
struct CastNowPlayingBar: View {
    @Environment(AppStore.self) private var store
    let inline: Bool

    var body: some View {
        let cast = store.tv.cast
        let ended = cast.stage == .failed || cast.stage == .stopped
        if let now = cast.now {
            HStack(spacing: 12) {
                if !inline, let entry = store.titles[now.id] { CoverArt(entry: entry, width: 24) }
                VStack(alignment: .leading, spacing: 0) {
                    Text(now.title).font(.showTitle(size: 15)).foregroundStyle(.silk).lineLimit(1)
                    if !inline {
                        HStack(spacing: 6) {
                            if ended { Circle().fill(.vermilion).frame(width: 6, height: 6) }
                            Text(CastWords.barLine(cast)).font(.ui(13)).foregroundStyle(.haze).lineLimit(1)
                                .contentTransition(.opacity)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if !ended {
                    Button {
                        cast.send(.togglePause)
                    } label: {
                        Image(systemName: now.paused ? "play.fill" : "pause.fill")
                            .font(.system(size: 17, weight: .semibold))
                            .foregroundStyle(.silk)
                            .frame(width: 44, height: 44)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .disabled(!cast.live)
                    .opacity(cast.live ? 1 : 0.4)
                    .accessibilityLabel(now.paused ? L.actionPlay : L.actionPause)
                }
            }
            .padding(.leading, 16)
            .padding(.trailing, 4)
            .frame(minHeight: 44)
            .contentShape(.rect)
            .onTapGesture { store.tv.openRemote() }
            .animation(.standard, value: cast.stage)
            .accessibilityElement(children: .contain)
            .accessibilityAction(named: L.tvRemoteAction) { store.tv.openRemote() }
        }
    }
}
#endif
