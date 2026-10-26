// Play on TV: the sheet that chooses a TV and pairs with it, and the now-playing card that
// drives it. The engine's sender says what the rows, the pairing's failures and the card read
// (TVRemoteModel); these views only lay them out, each platform as its other sheets are: on the
// Mac a raised window sheet with its buttons at the foot (as Join a Plugin Store), on iPhone and
// iPad a grouped list in a navigation stack (as the Settings sheet).

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI
#if canImport(VisionKit) && os(iOS)
import VisionKit
#endif

/// The TV sheet, over whatever presents it. A window presents one sheet at a time: on iPhone and
/// iPad the player is a full-screen cover and Settings a sheet, and each presents its own TV
/// sheet while it is up, the window's keeping out of their way.
public struct TVRemoteSheets: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.playerUp) private var playerUp

    enum Place {
        case window, player, settings
    }

    let place: Place

    /// The window's, or with `overPlayer` the player's cover's.
    public init(overPlayer: Bool = false) {
        place = overPlayer ? .player : .window
    }

    init(place: Place) {
        self.place = place
    }

    private var here: Bool {
        #if os(iOS)
        switch place {
        case .window: !playerUp && navigator.phoneSettings == nil
        case .player, .settings: true
        }
        #else
        place == .window
        #endif
    }

    public func body(content: Content) -> some View {
        let tv = store.tv
        content
            .sheet(isPresented: Binding(
                get: { here && tv.sheet != nil },
                set: { up in if !up, here { tv.sheetClosed() } }
            )) {
                Group {
                    #if os(macOS)
                    MacTVSheet()
                    #else
                    PhoneTVSheet()
                        .presentationBackground(Palette.ground)
                    #endif
                }
                .environment(store)
                .preferredColorScheme(.dark)
            }
            // A TV's QR code, read by the system's camera, opens the app with its link.
            .onOpenURL { url in if place == .window { tv.open(url) } }
    }
}

/// The way back to the card while a TV is driven: its name, in the toolbar.
struct TVNowPlayingButton: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        if let card = store.tv.card, store.tv.connected != nil {
            Button {
                store.tv.openRemote()
            } label: {
                #if os(macOS)
                if card.phase == .connected {
                    Label(store.tv.now == nil ? card.tvName : "Playing on \(card.tvName)", systemImage: "tv")
                        .labelStyle(.titleAndIcon)
                } else {
                    // While the link is down it says so in the same place, with the app's dots.
                    HStack(spacing: 6) {
                        Circle().fill(TVWords.barDot(card)).frame(width: 6, height: 6)
                        Text(TVWords.barLine(card)).foregroundStyle(.silk)
                    }
                }
                #else
                Label(store.tv.now == nil ? card.tvName : "Playing on \(card.tvName)", systemImage: "tv")
                #endif
            }
            .help("Show the remote for \(card.tvName)")
        }
    }
}

#if os(iOS)
/// What the TV plays, over the tab bar while a TV is driven and plays something: the tab bar's
/// own accessory, as a now-playing bar is on iOS 26.1 and later. Before it, the toolbar's button
/// is the way back to the card.
struct TVAccessory: ViewModifier {
    @Environment(AppStore.self) private var store

    func body(content: Content) -> some View {
        if #available(iOS 26.1, *) {
            content.tabViewBottomAccessory(isEnabled: store.tv.connected != nil && store.tv.now != nil) {
                TVNowPlayingBar()
            }
        } else {
            content
        }
    }
}

/// The cover, the title and the TV's name, and play or pause; a tap opens the card. Inline, as
/// the tab bar minimises, the title alone.
struct TVNowPlayingBar: View {
    @Environment(AppStore.self) private var store
    @Environment(\.tabViewBottomAccessoryPlacement) private var placement

    var body: some View {
        if let card = store.tv.card, let now = store.tv.now {
            HStack(spacing: 12) {
                if placement != .inline, let entry = store.titles[now.id] { CoverArt(entry: entry, width: 24) }
                VStack(alignment: .leading, spacing: 0) {
                    Text(now.title).font(.showTitle(size: 15)).foregroundStyle(.silk).lineLimit(1)
                    if placement != .inline {
                        Text(card.phase == .connected ? "Playing on \(card.tvName)" : TVWords.status(card))
                            .font(.ui(13)).foregroundStyle(.haze).lineLimit(1)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Button {
                    store.tv.send(.togglePause)
                } label: {
                    Image(systemName: now.paused ? "play.fill" : "pause.fill")
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(width: 44, height: 44)
                        .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .disabled(now.loading || !store.tv.live)
                .opacity(now.loading || !store.tv.live ? 0.4 : 1)
                .accessibilityLabel(now.paused ? "Play" : "Pause")
            }
            .padding(.leading, 16)
            .padding(.trailing, 4)
            .contentShape(.rect)
            .onTapGesture { store.tv.openRemote() }
            .accessibilityElement(children: .contain)
            .accessibilityAction(named: "Show the remote") { store.tv.openRemote() }
        }
    }
}
#endif

// MARK: - Words

/// What the sheets say that both platforms share.
enum TVWords {
    static func pickerTitle(_ request: TVPlayRequest?) -> String {
        request == nil ? "Pair a TV" : "Play on TV"
    }

    static func pickerLine(_ request: TVPlayRequest?) -> String {
        guard let request else {
            return "Open anistream on the TV. It needs to be on the same Wi-Fi as this \(Device.name)."
        }
        return request.title.isEmpty ? "Episode \(request.episode)" : "\(request.title), episode \(request.episode)"
    }

    static func pairingLine(_ name: String) -> String {
        "Type the six digits \(name) shows. After this, this \(Device.name) connects by itself."
    }

    static let empty = "Looking for TVs on this network"
    static let noneFound = "No TVs found"
    /// What to check when no TV answers. The picker's own line says it already when it is
    /// pairing, with nothing to play.
    static func noneLine(_ request: TVPlayRequest?) -> String {
        request == nil
            ? "Check that anistream is open on the TV."
            : "Check that anistream is open on the TV, on the same Wi-Fi as this \(Device.name)."
    }
    static let address = "The TV shows its address on its Pair a phone screen."
    static let nothing = "Choose Play on TV on an episode, or search on the TV from here."

    /// A track as the card names it: its language in this device's language, and its title.
    static func track(_ track: FfiTvTrack) -> String {
        let language = track.language.flatMap { Locale.current.localizedString(forLanguageCode: $0) }
        switch (language, track.title) {
        case (let language?, let title?) where !title.isEmpty: return "\(language) · \(title)"
        case (let language?, _): return language
        case (nil, let title?) where !title.isEmpty: return title
        default: return "Track \(track.id)"
        }
    }

    static func chosen(_ tracks: [FfiTvTrack], off: Bool) -> String {
        off ? "Off" : tracks.first(where: \.selected).map(track) ?? "None"
    }

    static func volume(_ now: FfiNowCard) -> String? {
        now.volume.map { "\(Int($0.rounded())) of 100" }
    }

    /// The dot beside the link's line: water while connected, vermilion when the TV let this
    /// device go, haze while it connects.
    static func dot(_ phase: FfiLinkPhase?) -> Color {
        switch phase {
        case .connected: .water
        case .unpaired, .keyChanged: .vermilion
        default: .haze
        }
    }

    static func status(_ card: FfiRemoteCard?) -> String {
        card?.phaseLine ?? "Connected"
    }

    /// What the toolbar's button says while the link is down: "Reconnecting to Living room",
    /// "Living room not found" once the engine has given up on it, "Can't reach Living room"
    /// when it never answered, "Pair Living room again".
    static func barLine(_ card: FfiRemoteCard) -> String {
        let name = card.tvName
        switch card.phase {
        case .connected: return name
        case .connecting: return card.reached ? "Reconnecting to \(name)" : "Connecting to \(name)…"
        case .retrying where !card.reached: return "Can't reach \(name)"
        case .retrying: return card.over ? "\(name) not found" : "Reconnecting to \(name)"
        case .unpaired, .keyChanged: return "Pair \(name) again"
        }
    }

    /// The toolbar's dot beside it: vermilion once the link will not come back by itself, or
    /// never answered.
    static func barDot(_ card: FfiRemoteCard) -> Color {
        card.over || lost(card) || (card.phase == .retrying && !card.reached) ? .vermilion : dot(card.phase)
    }

    static func lost(_ card: FfiRemoteCard?) -> Bool {
        card?.phase == .unpaired || card?.phase == .keyChanged
    }
}

/// The digits typed, once there are six.
private func sixDigits(_ code: String) -> String? {
    let digits = code.filter(\.isNumber)
    return digits.count == 6 ? digits : nil
}

/// Something that went wrong, as the sheets say it: the error's dot, then the words in the
/// sheet's own colour, never coloured text. The dot sits on the first line, however many the
/// words wrap to.
struct TVProblemLine: View {
    let text: String
    let touch: Bool

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Circle()
                .fill(.vermilion)
                .frame(width: 6, height: 6)
                .alignmentGuide(.firstTextBaseline) { $0[.bottom] }
                .accessibilityHidden(true)
            Text(text)
                .font(touch ? .ui(13) : .mac(12))
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
        }
        .accessibilityElement(children: .combine)
    }
}

/// What the TV sheet shows. Closing the sheet clears the model's sheet and pairing at once,
/// while the sheet is still going down: it keeps showing what it last showed, at the height it
/// had, rather than the picker that nothing up would make of it.
struct TVSheetShown: Equatable {
    var sheet: TVRemoteModel.Sheet?
    var pairing: TVPairingStep?

    init() {}

    @MainActor init(_ tv: TVRemoteModel) {
        sheet = tv.sheet
        pairing = tv.pairing
    }

    /// The model's, while a sheet is up; what was last shown, while it goes.
    @MainActor init(_ tv: TVRemoteModel, last: TVSheetShown) {
        self = tv.sheet == nil ? last : TVSheetShown(tv)
    }
}

// MARK: - Parts both platforms share

/// The TV's playhead as the player's seek line draws it: a capsule track with the played part in
/// amber, dragged to seek, and the time either side.
struct TVSeekLine: View {
    @Environment(AppStore.self) private var store
    let now: FfiNowCard
    let touch: Bool
    @State private var dragging: Double?
    /// Where a drag let go, held until the TV's next card says where it went, so the line does
    /// not jump back to the old playhead meanwhile.
    @State private var held: Double?
    @State private var heldRev: UInt64?

    /// The TV has reported this episode playing: a length, and not starting.
    private var reported: Bool {
        !now.loading && now.duration != nil
    }

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            let duration = max(now.duration ?? 0, 1)
            let position = min(dragging ?? held ?? store.tv.playhead(at: context.date) ?? now.position, duration)
            VStack(spacing: 6) {
                GeometryReader { proxy in
                    let width = proxy.size.width
                    Capsule()
                        .fill(.rule)
                        .overlay(alignment: .leading) {
                            Rectangle().fill(.amber).frame(width: width * position / duration)
                        }
                        .clipShape(.capsule)
                        .frame(height: dragging != nil ? 6 : 4)
                        .frame(maxHeight: .infinity)
                        .contentShape(.rect)
                        .gesture(
                            DragGesture(minimumDistance: 0)
                                .onChanged { drag in
                                    dragging = min(max(0, drag.location.x / max(width, 1)), 1) * duration
                                }
                                .onEnded { _ in
                                    if let to = dragging {
                                        held = to
                                        heldRev = store.tv.card?.rev
                                        store.tv.send(.seekTo(position: to))
                                    }
                                    dragging = nil
                                }
                        )
                }
                .frame(height: touch ? 24 : 16)
                .disabled(now.duration == nil || now.loading || !store.tv.live)
                HStack {
                    Text(ResumePoint(position: position, fraction: nil).clock)
                    Spacer()
                    Text(now.duration.map { "−" + ResumePoint(position: max(0, $0 - position), fraction: nil).clock } ?? "")
                }
                .font(touch ? .ui(13) : .mac(11))
                .monospacedDigit()
                .foregroundStyle(.haze)
                // Until the TV has said where it is, there is nothing to read: the place stays,
                // so nothing moves when it comes.
                .opacity(reported ? 1 : 0)
                .accessibilityHidden(!reported)
            }
            .accessibilityElement()
            .accessibilityLabel("Position")
            .accessibilityValue(ResumePoint(position: position, fraction: nil).clock)
            .accessibilityAdjustableAction { direction in
                store.tv.send(.seek(by: direction == .increment ? 10 : -10))
            }
        }
        .onChange(of: store.tv.card?.rev) { _, rev in
            if held != nil, rev != heldRev { held = nil }
        }
        .task(id: held) {
            // A seek the TV never answered lets go after a while.
            guard held != nil else { return }
            try? await Task.sleep(for: .seconds(3))
            if !Task.isCancelled { held = nil }
        }
    }
}

/// Back and forward 10 seconds, play or pause, and the next episode: silk glyphs, as the
/// player's bar draws them.
struct TVTransport: View {
    @Environment(AppStore.self) private var store
    let now: FfiNowCard
    let touch: Bool

    var body: some View {
        HStack(spacing: touch ? 24 : 16) {
            glyph("gobackward.10", "Back 10 seconds", size: touch ? 24 : 18) { store.tv.send(.seek(by: -10)) }
            glyph(now.paused ? "play.fill" : "pause.fill", now.paused ? "Play" : "Pause", size: touch ? 34 : 24) {
                store.tv.send(.togglePause)
            }
            .keyboardShortcut(.space, modifiers: [])
            glyph("goforward.10", "Forward 10 seconds", size: touch ? 24 : 18) { store.tv.send(.seek(by: 10)) }
            glyph("forward.end.fill", "Next episode", size: touch ? 20 : 15) { store.tv.send(.next) }
        }
        .frame(maxWidth: .infinity)
        // Starting on the TV, or the link lost: there, but nothing to press.
        .disabled(now.loading || !store.tv.live)
        .opacity(now.loading || !store.tv.live ? 0.4 : 1)
    }

    private func glyph(_ symbol: String, _ label: String, size: CGFloat, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size, weight: .medium))
                .foregroundStyle(.silk)
                .frame(width: touch ? 56 : 44, height: touch ? 56 : 44)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerLift()
        .accessibilityLabel(label)
    }
}

/// The subtitle or audio choice, as a settings menu: the choices inline, the one on ticked.
struct TVTrackMenu: View {
    @Environment(AppStore.self) private var store
    let title: String
    let kind: FfiTvTrackKind
    let tracks: [FfiTvTrack]
    let off: Bool
    let phone: Bool

    var body: some View {
        Menu {
            Picker(title, selection: Binding<Int64?>(
                get: { off ? nil : tracks.first(where: \.selected)?.id },
                set: { store.tv.send(.track(kind: kind, id: $0)) }
            )) {
                if kind == .subtitle { Text("Off").tag(Int64?.none) }
                ForEach(tracks, id: \.id) { track in
                    Text(TVWords.track(track)).tag(Optional(track.id))
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            MenuValue(text: TVWords.chosen(tracks, off: off), phone: phone)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .disabled(tracks.isEmpty || !store.tv.live)
        .accessibilityLabel(title)
    }
}

/// Six digits in boxes, typed into one field under them, as the code reads on the TV.
struct TVCodeBoxes: View {
    @Binding var code: String
    var focused: FocusState<Bool>.Binding
    let touch: Bool
    /// The code ran out: nothing can be typed until a new one is asked for.
    var locked = false
    let submit: () -> Void

    var body: some View {
        let digits = Array(code.filter(\.isNumber).prefix(6))
        ZStack(alignment: .leading) {
            TextField("", text: $code)
                .focused(focused)
                #if os(iOS)
                .keyboardType(.numberPad)
                .textContentType(.oneTimeCode)
                #endif
                .opacity(0.02)
                .onChange(of: code) { _, typed in
                    let kept = String(typed.filter(\.isNumber).prefix(6))
                    if kept != typed { code = kept }
                    if kept.count == 6 { submit() }
                }
                .onSubmit(submit)
                .disabled(locked)
                .accessibilityLabel("Pairing code")
            HStack(spacing: touch ? 8 : 6) {
                ForEach(0..<6, id: \.self) { index in
                    Text(index < digits.count ? String(digits[index]) : "")
                        .font(touch ? .ui(28, weight: .semibold) : .mac(22, weight: .semibold))
                        .monospacedDigit()
                        .foregroundStyle(.silk)
                        .frame(width: touch ? 44 : 40, height: touch ? 56 : 46)
                        .background(touch ? Palette.raised : Palette.ground, in: .rect(cornerRadius: 8))
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .strokeBorder(index == digits.count && focused.wrappedValue ? Palette.silk : Palette.glassRule, lineWidth: 1)
                        )
                    if index == 2 { Spacer().frame(width: touch ? 8 : 10) }
                }
            }
            .allowsHitTesting(false)
            .accessibilityHidden(true)
            .opacity(locked ? 0.4 : 1)
        }
        .contentShape(.rect)
        .onTapGesture { if !locked { focused.wrappedValue = true } }
    }
}

/// A row of the picker: the TV, what the engine says under it, and what choosing it does.
struct TVRow: View {
    @Environment(AppStore.self) private var store
    let row: FfiTvRow
    let phone: Bool

    var body: some View {
        Button {
            store.tv.choose(row)
        } label: {
            HStack(spacing: phone ? 14 : 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(row.name).font(phone ? .ui(17) : .mac(13)).foregroundStyle(.silk)
                    Text(row.detail).font(phone ? .ui(13) : .mac(12)).foregroundStyle(.haze)
                }
                Spacer(minLength: 8)
                switch row.kind {
                case .new:
                    Text("Pair")
                        .font(phone ? .ui(17, weight: .semibold) : .mac(13, weight: .semibold))
                        .foregroundStyle(.silk)
                case .paired, .pairedAway:
                    if store.tv.connected == row.fingerprint {
                        Image(systemName: "checkmark")
                            .font(phone ? .ui(15, weight: .semibold) : .mac(12, weight: .semibold))
                            .foregroundStyle(.silk)
                            .accessibilityLabel("Connected")
                    } else if !phone {
                        Image(systemName: "chevron.right").font(.mac(11, weight: .semibold)).foregroundStyle(.haze)
                    }
                case .needsUpdate:
                    EmptyView()
                }
            }
            .padding(.horizontal, phone ? 0 : 14)
            .frame(minHeight: phone ? 52 : 46)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .disabled(row.kind == .needsUpdate)
        .opacity(row.kind == .needsUpdate ? 0.55 : 1)
        .accessibilityElement(children: .combine)
        .accessibilityHint(row.kind == .new ? "Pairs with this TV" : "")
    }
}

/// The picker's row while no TV is listed: nothing for the first 600 ms, as most TVs answer by
/// then; then the search, said; and after 10 s with none, what to check. The browse goes on
/// throughout, and a TV found replaces the row. The row keeps one height in each, so nothing
/// below it moves.
struct TVSearchingRow: View {
    let phone: Bool
    let request: TVPlayRequest?

    enum Stage { case quiet, looking, none }
    @State private var stage = Stage.quiet

    static let quietFor: Duration = .milliseconds(600)
    static let giveUpAfter: Duration = .seconds(10)

    var body: some View {
        HStack(spacing: phone ? 12 : 10) {
            switch stage {
            case .quiet:
                Color.clear.frame(height: 1)
            case .looking:
                ProgressView().controlSize(phone ? .regular : .small)
                Text(TVWords.empty).font(phone ? .ui(17) : .mac(13)).foregroundStyle(.haze)
            case .none:
                VStack(alignment: .leading, spacing: 2) {
                    Text(TVWords.noneFound).font(phone ? .ui(17) : .mac(13)).foregroundStyle(.silk)
                    Text(TVWords.noneLine(request)).font(phone ? .ui(13) : .mac(12)).foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .frame(maxWidth: .infinity, minHeight: phone ? 52 : 46, alignment: .leading)
        .animation(.standard, value: stage)
        .accessibilityElement(children: .combine)
        .task {
            try? await Task.sleep(for: Self.quietFor)
            guard !Task.isCancelled else { return }
            stage = .looking
            try? await Task.sleep(for: Self.giveUpAfter - Self.quietFor)
            guard !Task.isCancelled else { return }
            stage = .none
        }
    }
}

#if os(macOS)
// MARK: - The Mac's sheet

/// A raised window sheet, 440 wide: its title and line, what it shows, and its buttons at the
/// foot, as the app's other sheets.
struct MacTVSheet: View {
    @Environment(AppStore.self) private var store
    @State private var last = TVSheetShown()

    var body: some View {
        let tv = store.tv
        let shown = TVSheetShown(tv, last: last)
        VStack(alignment: .leading, spacing: 0) {
            if let pairing = shown.pairing {
                MacTVPairing(step: pairing)
            } else if shown.sheet == .remote {
                MacTVRemote()
            } else {
                MacTVPicker()
            }
        }
        .padding(24)
        .sheetWidth(440)
        .background(.raised)
        .onChange(of: TVSheetShown(tv), initial: true) { _, now in if now.sheet != nil { last = now } }
    }
}

/// The sheet's title and the line under it.
private struct MacSheetHeading: View {
    let title: String
    let line: String

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(line)
                .font(.mac(13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// Rows on the ground inside the raised sheet, with hairlines between them.
private struct MacSheetRows<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 0) {
            Group(subviews: content) { rows in
                ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                    if index > 0 {
                        Rectangle().fill(.faintRule).frame(height: 1).padding(.leading, 14)
                    }
                    row
                }
            }
        }
        .padding(.vertical, 4)
        .background(.ground, in: .rect(cornerRadius: 10))
    }
}

struct MacTVPicker: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var address = ""
    @State private var typing = false

    var body: some View {
        let tv = store.tv
        MacSheetHeading(title: TVWords.pickerTitle(tv.request), line: TVWords.pickerLine(tv.request))
        MacSheetRows {
            if tv.rows.isEmpty {
                TVSearchingRow(phone: false, request: tv.request).padding(.horizontal, 14)
            }
            ForEach(tv.rows, id: \.key) { row in
                TVRow(row: row, phone: false)
            }
        }
        .padding(.top, 16)
        if typing {
            HStack(spacing: 10) {
                TextField("Address", text: $address, prompt: Text("192.168.1.20:47811"))
                    .textFieldStyle(SheetFieldStyle())
                    .labelsHidden()
                    .onSubmit(connect)
                Button("Connect", action: connect)
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .disabled(address.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            .padding(.top, 14)
            Text(TVWords.address)
                .font(.mac(12))
                .foregroundStyle(.haze)
                .padding(.top, 6)
        }
        HStack(spacing: 10) {
            if !typing {
                Button("Type an Address…") { typing = true }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
            Spacer()
            Button("Done") { dismiss() }
                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                .keyboardShortcut(.cancelAction)
        }
        .padding(.top, 20)
        .onAppear { tv.startBrowsing() }
        .onDisappear { tv.stopBrowsing() }
    }

    private func connect() {
        let typed = address.trimmingCharacters(in: .whitespaces)
        if !typed.isEmpty { store.tv.begin(typed) }
    }
}

struct MacTVPairing: View {
    @Environment(AppStore.self) private var store
    let step: TVPairingStep
    @State private var code = ""
    @FocusState private var focused: Bool

    var body: some View {
        let tv = store.tv
        let typing = step.needsCode && !step.tvName.isEmpty
        if typing {
            MacSheetHeading(title: "Pair with \(step.tvName)", line: TVWords.pairingLine(step.tvName))
            TVCodeBoxes(code: $code, focused: $focused, touch: false, locked: step.expired, submit: submit)
                .padding(.top, 18)
        } else {
            MacSheetHeading(
                title: step.tvName.isEmpty ? "Connecting to the TV" : "Pairing with \(step.tvName)",
                line: "This takes a moment.")
        }
        if let problem = step.problem {
            TVProblemLine(text: problem, touch: false).padding(.top, 12)
        }
        HStack(spacing: 10) {
            Button("Back") { tv.cancelPairing() }
                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                .keyboardShortcut(.cancelAction)
            Spacer()
            if step.working { ProgressView().controlSize(.small) }
            if typing, step.expired {
                // The one way on once the code ran out.
                Button("Get a New Code", action: newCode)
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled(step.working)
            } else if typing {
                Button("Get a New Code", action: newCode)
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .disabled(step.working)
                Button("Pair", action: submit)
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled(sixDigits(code) == nil || step.working)
            }
        }
        .padding(.top, 20)
        .onAppear { focused = !step.expired }
        .onChange(of: step.problem) { _, problem in
            if problem != nil { code = ""; focused = !step.expired }
        }
        .onChange(of: step.expired) { _, expired in
            if !expired { focused = true }
        }
    }

    private func newCode() {
        code = ""
        store.tv.requestCodeAgain()
    }

    private func submit() {
        if let digits = sixDigits(code) { store.tv.submit(code: digits) }
    }
}

struct MacTVRemote: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var search = ""

    var body: some View {
        let tv = store.tv
        let card = tv.card
        let now = tv.now
        VStack(alignment: .leading, spacing: 4) {
            Text(card?.tvName ?? "TV")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            HStack(spacing: 6) {
                Circle().fill(TVWords.dot(card?.phase)).frame(width: 6, height: 6)
                Text(TVWords.status(card)).font(.mac(12)).foregroundStyle(.haze)
            }
            .accessibilityElement(children: .combine)
        }
        if let now {
            HStack(spacing: 12) {
                if let entry = store.titles[now.id] { CoverArt(entry: entry, width: 36) }
                VStack(alignment: .leading, spacing: 2) {
                    Text(now.title).font(.showTitle(size: 15)).foregroundStyle(.silk).lineLimit(1)
                    Text(now.loading ? "Starting on the TV" : now.episodeLine)
                        .font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                }
                Spacer(minLength: 8)
                if let skip = now.skip {
                    Button(skip.label.capitalized) { tv.send(.skip) }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                        .disabled(!tv.live)
                        .opacity(tv.live ? 1 : 0.4)
                }
            }
            .padding(.top, 18)
            TVSeekLine(now: now, touch: false).padding(.top, 14)
            TVTransport(now: now, touch: false).padding(.top, 2)
            MacSheetRows {
                SettingsRow(title: "Subtitles") {
                    TVTrackMenu(title: "Subtitles", kind: .subtitle, tracks: now.subtitles, off: now.subtitlesOff, phone: false)
                }
                .opacity(tv.live ? 1 : 0.4)
                SettingsRow(title: "Audio") {
                    TVTrackMenu(title: "Audio", kind: .audio, tracks: now.audio, off: false, phone: false)
                }
                .opacity(tv.live ? 1 : 0.4)
                SettingsRow(title: "TV Volume", note: TVWords.volume(now)) {
                    HStack(spacing: 6) {
                        volume("speaker.wave.1", "Volume Down", -5)
                        volume("speaker.wave.3", "Volume Up", 5)
                    }
                    .disabled(!tv.live)
                    .opacity(tv.live ? 1 : 0.4)
                }
            }
            .padding(.top, 12)
        } else if card?.phase == .connected {
            Text(TVWords.nothing)
                .font(.mac(13))
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 14)
        }
        TextField("Search on the TV", text: $search, prompt: Text("Search on the TV"))
            .textFieldStyle(SheetFieldStyle())
            .labelsHidden()
            .disabled(!tv.live)
            .onChange(of: search) { _, text in tv.send(.text(text: text, submit: false)) }
            .onSubmit { tv.send(.text(text: search, submit: true)) }
            .padding(.top, 14)
        if let problem = tv.problem {
            TVProblemLine(text: problem, touch: false).padding(.top, 10)
        }
        HStack(spacing: 10) {
            if TVWords.lost(card) {
                Button("Pair Again") { pairAgain() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            } else {
                Button("Choose Another TV…") { tv.openPicker() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
            Button("Disconnect") {
                tv.disconnect()
                dismiss()
            }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            Spacer()
            Button("Done") { dismiss() }
                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                .keyboardShortcut(.cancelAction)
        }
        .padding(.top, 20)
    }

    private func volume(_ symbol: String, _ label: String, _ step: Double) -> some View {
        Button {
            store.tv.send(.volume(step: step))
        } label: {
            Image(systemName: symbol).frame(width: 16)
        }
        .buttonStyle(SecondaryCapsuleStyle())
        .accessibilityLabel(label)
        .help(label)
    }

    private func pairAgain() {
        if let fingerprint = store.tv.connected { store.tv.forget(fingerprint) }
        store.tv.openPicker()
    }
}
#endif

#if os(iOS)
// MARK: - iPhone and iPad's sheet

/// A grouped list in a navigation stack, as the Settings sheet: the picker, pairing, or the card.
struct PhoneTVSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var last = TVSheetShown()

    var body: some View {
        let tv = store.tv
        let shown = TVSheetShown(tv, last: last)
        let remote = shown.sheet == .remote
        NavigationStack {
            Group {
                if let pairing = shown.pairing {
                    PhoneTVPairing(step: pairing)
                } else if remote {
                    PhoneTVRemote()
                } else {
                    PhoneTVPicker()
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }.fontWeight(.semibold)
                }
            }
        }
        .tint(.silk)
        .presentationDetents(remote ? [.large] : [.medium, .large])
        .onChange(of: TVSheetShown(tv), initial: true) { _, now in if now.sheet != nil { last = now } }
    }
}

/// A button row of a list, as Settings writes them: silk, semibold.
private struct PhoneActionRow: View {
    let title: String
    var systemImage: String?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 14) {
                if let systemImage {
                    Image(systemName: systemImage).font(.ui(17)).foregroundStyle(.haze).frame(width: 28)
                }
                Text(title).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
            }
        }
        .listRowBackground(Palette.raised)
    }
}

struct PhoneTVPicker: View {
    @Environment(AppStore.self) private var store
    @State private var address = ""
    @State private var scanning = false

    var body: some View {
        let tv = store.tv
        PhoneList {
            Section {
                if tv.rows.isEmpty {
                    TVSearchingRow(phone: true, request: tv.request).listRowBackground(Palette.raised)
                }
                ForEach(tv.rows, id: \.key) { row in
                    TVRow(row: row, phone: true).listRowBackground(Palette.raised)
                }
            } header: {
                PhoneSectionHeader("On this network")
            } footer: {
                Text(TVWords.pickerLine(tv.request))
            }
            Section {
                if TVCodeScanner.isSupported {
                    PhoneActionRow(title: "Scan the code on the TV", systemImage: "qrcode.viewfinder") { scanning = true }
                }
                TextField("Address", text: $address, prompt: Text("Type an address"))
                    .font(.ui(17))
                    .keyboardType(.numbersAndPunctuation)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .submitLabel(.go)
                    .onSubmit {
                        let typed = address.trimmingCharacters(in: .whitespaces)
                        if !typed.isEmpty { tv.begin(typed) }
                    }
                    .listRowBackground(Palette.raised)
            } header: {
                PhoneSectionHeader("Not listed")
            } footer: {
                Text(TVWords.address)
            }
        }
        .navigationTitle(TVWords.pickerTitle(tv.request))
        .onAppear { tv.startBrowsing() }
        .onDisappear { tv.stopBrowsing() }
        .sheet(isPresented: $scanning) {
            TVCodeScanner { link in
                scanning = false
                tv.begin(link)
            }
            .ignoresSafeArea()
        }
    }
}

struct PhoneTVPairing: View {
    @Environment(AppStore.self) private var store
    let step: TVPairingStep
    @State private var code = ""
    @FocusState private var focused: Bool

    var body: some View {
        let tv = store.tv
        let typing = step.needsCode && !step.tvName.isEmpty
        PhoneList {
            if typing {
                Section {
                    TVCodeBoxes(code: $code, focused: $focused, touch: true, locked: step.expired, submit: submit)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets(top: 4, leading: 4, bottom: 4, trailing: 4))
                } header: {
                    PhoneSectionHeader("Code on \(step.tvName)")
                } footer: {
                    if let problem = step.problem {
                        TVProblemLine(text: problem, touch: true)
                    } else {
                        Text(TVWords.pairingLine(step.tvName))
                    }
                }
                // Once the code ran out, the button at the foot asks for a new one instead.
                if !step.expired {
                    Section {
                        PhoneActionRow(title: "Get a new code", action: newCode)
                            .disabled(step.working)
                    }
                }
            } else {
                Section {
                    HStack(spacing: 12) {
                        ProgressView()
                        Text(step.tvName.isEmpty ? "Connecting to the TV" : "Pairing with \(step.tvName)")
                            .font(.ui(17)).foregroundStyle(.haze)
                    }
                    .listRowBackground(Palette.raised)
                } footer: {
                    if let problem = step.problem { TVProblemLine(text: problem, touch: true) }
                }
            }
        }
        .navigationTitle("Pair with a TV")
        .navigationBarBackButtonHidden()
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button("Back") { tv.cancelPairing() }
            }
        }
        .safeAreaInset(edge: .bottom) {
            if typing {
                Button {
                    if step.expired { newCode() } else { submit() }
                } label: {
                    // The label stays centred; the spinner sits beside it without moving it.
                    Text(step.expired ? "Get a new code" : "Pair")
                        .overlay(alignment: .leading) {
                            if step.working { ProgressView().tint(.ground).offset(x: -28) }
                        }
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                .disabled((!step.expired && sixDigits(code) == nil) || step.working)
                .padding(.horizontal, 16)
                .padding(.bottom, 8)
            }
        }
        .onAppear { focused = !step.expired }
        .onChange(of: step.problem) { _, problem in
            if problem != nil { code = ""; focused = !step.expired }
        }
        .onChange(of: step.expired) { _, expired in
            if !expired { focused = true }
        }
    }

    private func newCode() {
        code = ""
        store.tv.requestCodeAgain()
    }

    private func submit() {
        if let digits = sixDigits(code) { store.tv.submit(code: digits) }
    }
}

struct PhoneTVRemote: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var search = ""

    var body: some View {
        let tv = store.tv
        let card = tv.card
        PhoneList {
            Section {
                if let now = tv.now {
                    VStack(alignment: .leading, spacing: 14) {
                        HStack(spacing: 14) {
                            if let entry = store.titles[now.id] { CoverArt(entry: entry, width: 40) }
                            VStack(alignment: .leading, spacing: 2) {
                                Text(now.title).font(.showTitle(size: 17)).foregroundStyle(.silk).lineLimit(2)
                                Text(now.loading ? "Starting on the TV" : now.episodeLine)
                                    .font(.ui(15)).foregroundStyle(.haze).lineLimit(2)
                            }
                        }
                        TVSeekLine(now: now, touch: true)
                        TVTransport(now: now, touch: true)
                    }
                    .padding(.vertical, 10)
                    .listRowBackground(Palette.raised)
                    if let skip = now.skip {
                        PhoneActionRow(title: skip.label, systemImage: "forward") { tv.send(.skip) }
                            .disabled(!tv.live)
                            .opacity(tv.live ? 1 : 0.4)
                    }
                } else {
                    // What to do from here is said only while the TV can hear it; otherwise the
                    // header's line says what the link is doing.
                    PhoneRowTitle(
                        title: tv.live ? "Nothing playing" : (card?.tvName ?? "TV"),
                        note: tv.live ? TVWords.nothing : nil)
                        .padding(.vertical, 6)
                        .listRowBackground(Palette.raised)
                }
            } header: {
                HStack(spacing: 6) {
                    Circle().fill(TVWords.dot(card?.phase)).frame(width: 6, height: 6)
                    PhoneSectionHeader(TVWords.status(card))
                }
            }
            if let now = tv.now {
                Section {
                    menuRow("Subtitles", kind: .subtitle, tracks: now.subtitles, off: now.subtitlesOff)
                    menuRow("Audio", kind: .audio, tracks: now.audio, off: false)
                    HStack(spacing: 8) {
                        PhoneRowTitle(title: "TV volume", note: TVWords.volume(now))
                        Spacer()
                        volume("speaker.wave.1", "Volume down", -5)
                        volume("speaker.wave.3", "Volume up", 5)
                    }
                    .disabled(!tv.live)
                    .opacity(tv.live ? 1 : 0.4)
                    .listRowBackground(Palette.raised)
                }
            }
            Section {
                TextField("Search on the TV", text: $search, prompt: Text("Type a search"))
                    .font(.ui(17))
                    .disabled(!tv.live)
                    .autocorrectionDisabled()
                    .submitLabel(.search)
                    .onChange(of: search) { _, text in tv.send(.text(text: text, submit: false)) }
                    .onSubmit { tv.send(.text(text: search, submit: true)) }
                    .listRowBackground(Palette.raised)
            } header: {
                PhoneSectionHeader("Search on the TV")
            } footer: {
                if let problem = tv.problem { TVProblemLine(text: problem, touch: true) }
            }
            Section {
                if TVWords.lost(card) {
                    PhoneActionRow(title: "Pair again") {
                        if let fingerprint = tv.connected { tv.forget(fingerprint) }
                        tv.openPicker()
                    }
                } else {
                    PhoneActionRow(title: "Choose another TV") { tv.openPicker() }
                }
                PhoneActionRow(title: "Disconnect") {
                    tv.disconnect()
                    dismiss()
                }
            }
        }
        .navigationTitle(card?.tvName ?? "TV")
    }

    private func menuRow(_ title: String, kind: FfiTvTrackKind, tracks: [FfiTvTrack], off: Bool) -> some View {
        HStack {
            PhoneRowTitle(title: title)
            Spacer()
            TVTrackMenu(title: title, kind: kind, tracks: tracks, off: off, phone: true)
        }
        .opacity(store.tv.live ? 1 : 0.4)
        .listRowBackground(Palette.raised)
    }

    private func volume(_ symbol: String, _ label: String, _ step: Double) -> some View {
        Button {
            store.tv.send(.volume(step: step))
        } label: {
            Image(systemName: symbol)
                .font(.ui(17))
                .foregroundStyle(.silk)
                .frame(width: 44, height: 44)
                .contentShape(.rect)
        }
        .buttonStyle(.borderless)
        .accessibilityLabel(label)
    }
}

// MARK: - Scanning a TV's code

/// The camera, reading the QR code on a TV's Pair a phone screen: hands over the link it carries.
/// VisionKit's scanner, on a device with a camera that has it; the system's Camera app reads the
/// same code and opens the app with the link where it does not.
struct TVCodeScanner: UIViewControllerRepresentable {
    let found: (String) -> Void

    static var isSupported: Bool {
        DataScannerViewController.isSupported && DataScannerViewController.isAvailable
    }

    func makeUIViewController(context: Context) -> DataScannerViewController {
        let scanner = DataScannerViewController(
            recognizedDataTypes: [.barcode(symbologies: [.qr])],
            qualityLevel: .fast,
            isHighlightingEnabled: true
        )
        scanner.delegate = context.coordinator
        try? scanner.startScanning()
        return scanner
    }

    func updateUIViewController(_ scanner: DataScannerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator(found: found) }

    final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        let found: (String) -> Void
        private var done = false

        init(found: @escaping (String) -> Void) {
            self.found = found
        }

        func dataScanner(_ scanner: DataScannerViewController, didAdd items: [RecognizedItem], allItems: [RecognizedItem]) {
            for item in items {
                guard !done, case .barcode(let code) = item,
                      let link = code.payloadStringValue, link.hasPrefix("anistream-remote://") else { continue }
                done = true
                scanner.stopScanning()
                found(link)
            }
        }
    }
}
#endif
