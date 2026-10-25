// Watch parties outside the player: the form that starts one or answers an invite link, the sheet
// for a party's episode none of the sources has, the anistream://party links, and on the Mac the
// question before quitting leaves one. PartyModel keeps what the engine said; these views lay it
// out, each platform as its other sheets are: on the Mac a raised window sheet with its buttons at
// the foot (as Join a Plugin Store), on iPhone and iPad a grouped list in a navigation stack. The
// words are the Windows app's (#179). The party's panel is the player's (AnistreamPlayerUI).

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The party's sheets, over whatever presents them. On iPhone and iPad the player is a
/// full-screen cover, which presents its own while it is up, the window's keeping out of its way.
public struct PartySheets: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.playerUp) private var playerUp
    let overPlayer: Bool
    #if os(macOS)
    /// The window these sheets are on, for Leave and Close to close.
    @State private var window: NSWindow?
    #endif

    /// The window's, or with `overPlayer` the player's cover's.
    public init(overPlayer: Bool = false) {
        self.overPlayer = overPlayer
    }

    private var here: Bool {
        #if os(iOS)
        overPlayer || (!playerUp && navigator.phoneSettings == nil)
        #else
        !overPlayer
        #endif
    }

    public func body(content: Content) -> some View {
        let party = store.party
        content
            .sheet(item: Binding(
                get: { here ? party.start : nil },
                set: { if $0 == nil, here { party.closeForm() } }
            )) { start in
                Group {
                    #if os(macOS)
                    MacPartyForm(start: start)
                    #else
                    PhonePartyForm(start: start)
                        .presentationDetents([.large])
                        .presentationBackground(Palette.ground)
                    #endif
                }
                .environment(store)
            }
            .sheet(item: Binding(
                get: { here && party.start == nil ? party.noSource : nil },
                set: { if $0 == nil, here { party.closeNoSource() } }
            )) { ask in
                Group {
                    #if os(macOS)
                    MacPartySource(ask: ask)
                    #else
                    // Full height, as the form is: at half, its button sits over Leave party
                    // and the list below.
                    PhonePartySource(ask: ask)
                        .presentationDetents([.large])
                        .presentationBackground(Palette.ground)
                    #endif
                }
                .environment(store)
                .environment(navigator)
            }
            .onOpenURL { url in
                guard !overPlayer, PartyLinks.isInvite(url) else { return }
                party.openInvite(url.absoluteString) { id in navigator.showInvitedTitle(id) }
            }
            #if os(macOS)
            .alert(
                "Leave the party?",
                isPresented: Binding(
                    get: { here && party.askingToLeave != nil },
                    set: { if !$0 { party.askingToLeave = nil } }
                ),
                presenting: party.askingToLeave
            ) { leaving in
                // Stay is the default: Return and Escape both keep the party.
                Button("Stay", role: .cancel) {}
                    .keyboardShortcut(.defaultAction)
                Button(leaving == .quit ? "Leave and Quit" : "Leave and Close") {
                    party.agreeToLeave()
                    switch leaving {
                    case .quit: NSApp.terminate(nil)
                    case .close: window?.performClose(nil)
                    }
                }
            } message: { leaving in
                let room = party.party?.room ?? "the party"
                Text(leaving == .quit
                     ? "Quitting anistream leaves \(room). Everyone else carries on without you."
                     : "Closing the window leaves \(room). Everyone else carries on without you.")
            }
            // The red button and ⌘W ask first while in a party, as quitting does.
            .background {
                if here { PartyCloseGuard(asks: { party.asksBeforeClosing() }) { window = $0 } }
            }
            #endif
    }
}

/// The sheets the player's own buttons open over its cover on iPhone and iPad: Play on TV's and
/// the party's.
public struct PlayerCoverSheets: ViewModifier {
    public init() {}

    public func body(content: Content) -> some View {
        content
            .modifier(TVRemoteSheets(overPlayer: true))
            .modifier(PartySheets(overPlayer: true))
    }
}

/// The `anistream://party?…` links: an invite to a watch party, which the engine reads.
enum PartyLinks {
    static func isInvite(_ url: URL) -> Bool {
        url.scheme?.lowercased() == "anistream" && url.host?.lowercased() == "party"
    }
}

extension Navigator {
    /// An invite's title, over the section on screen, unless it is the page there.
    func showInvitedTitle(_ id: AnilistId) {
        #if os(iOS)
        phoneSettings = nil
        let onScreen = paths[section]?.last
        #else
        let onScreen = titleOnMac
        #endif
        if onScreen != id { openTitle(id) }
    }
}

/// The form's words and state, the same on each platform.
@MainActor
struct PartyFormState {
    let store: AppStore
    let start: PartyStart

    var party: PartyModel { store.party }

    var title: String {
        start.isInvite ? cased(mac: "Join the Party?", touch: "Join the party?") : cased(mac: "Watch Together", touch: "Watch together")
    }

    var primary: String {
        if party.failure != nil { return cased(mac: "Try Again", touch: "Try again") }
        return start.isInvite ? cased(mac: "Join Party", touch: "Join party") : cased(mac: "Start Party", touch: "Start party")
    }

    var episodeLine: String {
        let episode = start.episode
        return PartyWords.episode(episode?.episode, title: episode.flatMap { store.titles[$0.id]?.title })
    }

    /// The line where the privacy note is: what the form waits on, or why it could not join.
    var status: (text: String, busy: Bool, error: Bool)? {
        if party.joining {
            let (server, room) = party.place
            return ("Joining \(room.trimmingCharacters(in: .whitespaces)) on \(server.trimmingCharacters(in: .whitespaces))…", true, false)
        }
        if let failure = party.failure { return (failure, false, true) }
        if store.offline { return ("A party needs a connection. You're offline.", false, true) }
        if party.form == nil { return ("Getting things ready…", true, false) }
        return nil
    }

    var canStart: Bool { party.canStart && !store.offline }

    /// The line under the server: what is wrong with it as typed, or whose the suggested one is.
    var serverLine: (text: String, problem: Bool)? {
        guard let form = party.form else { return nil }
        if start.isInvite { return (PartyWords.inviteServerHint(party.place.server, form: form), false) }
        if let problem = PartyWords.serverProblem(party.server) { return (problem, true) }
        return (PartyWords.serverHint(party.server, form: form), false)
    }

    /// Whether Use it has something to do: the suggested server is not the one typed.
    var offersSuggested: Bool {
        guard let form = party.form else { return false }
        return !PartyWords.isSuggested(party.server, form: form)
    }

    /// The recent servers that match what is typed.
    var recent: [String] {
        let typed = party.server.trimmingCharacters(in: .whitespaces)
        return (party.form?.recentServers ?? []).filter { typed.isEmpty || $0.localizedCaseInsensitiveContains(typed) }
    }

    static let roomNote = "Anyone who knows the room's name can join it. The invite link carries it, so nobody has to read it out."
    static let privacy = "The server sees your name, the room, and when anyone plays, pauses or seeks. Each of you plays the episode from your own sources. anistream hosts no content."
    /// The longest name the room shows.
    static let nameLimit = 40
}

/// The no-source sheet's words, the same on each platform.
@MainActor
struct PartySourceState {
    let store: AppStore
    let ask: PartyNoSourceAsk

    var party: PartyModel { store.party }

    var what: String {
        PartyWords.episode(ask.episode, title: store.titles[ask.id]?.title)
    }

    var first: (title: String, body: String) {
        PartyWords.noSource(why: ask.why, reason: ask.reason, episode: ask.episode, what: what)
    }

    var title: String {
        party.choice == nil ? first.title : cased(mac: "Choose a Source for Episode \(ask.episode)", touch: "Choose a source for episode \(ask.episode)")
    }

    /// The primary button: the actions that fit the answer. Nothing to choose before the episode
    /// is out, the plugins to add when there are no sources, and the sources' list otherwise.
    enum Primary { case none, providers, choose, play, again }

    var primary: Primary {
        switch party.choice {
        case nil:
            switch ask.why {
            case .notAired: return .none
            case .noSources: return .providers
            case .notFound: return .choose
            }
        case .looking?: return .play
        case .listed(let found)?: return found.isEmpty ? .providers : .play
        case .failed?: return .again
        }
    }

    func label(_ primary: Primary) -> String {
        switch primary {
        case .none: ""
        case .providers: cased(mac: "Open Providers", touch: "Open Providers")
        case .choose: cased(mac: "Choose Source", touch: "Choose source")
        case .play: cased(mac: "Play with the Party", touch: "Play with the party")
        case .again: cased(mac: "Try Again", touch: "Try again")
        }
    }

    /// The line over the list: looking, what was found, or why nothing was.
    var line: (text: String, busy: Bool, error: Bool) {
        switch party.choice {
        case .looking?, nil: ("Looking through your sources…", true, false)
        case .listed(let found)?:
            found.isEmpty
                ? ("No source lists episode \(ask.episode). Sources come from plugins, which Providers adds.", false, false)
                : (PartyWords.found(found.count), false, false)
        case .failed(let why)?: (why, false, true)
        }
    }

    var candidates: [SourceCandidate] {
        if case .listed(let found)? = party.choice { return found }
        return []
    }
}

extension SourceCandidate {
    /// The row's identity in the list: its source and its own id.
    var partyKey: String { "\(providerId)/\(id)" }
}

#if os(macOS)
/// Asks before the window closes while in a party: its delegate stands behind one that asks
/// first, and answers everything else as it did. SwiftUI keeps the window's delegate; this one
/// only wraps it.
struct PartyCloseGuard: NSViewRepresentable {
    let asks: @MainActor () -> Bool
    let found: (NSWindow?) -> Void

    func makeNSView(context: Context) -> Holder { Holder(asks: asks, found: found) }
    func updateNSView(_ view: Holder, context: Context) {
        view.asker.asks = asks
        view.install()
    }

    final class Holder: NSView {
        let asker: CloseAsker
        let found: (NSWindow?) -> Void

        init(asks: @escaping @MainActor () -> Bool, found: @escaping (NSWindow?) -> Void) {
            asker = CloseAsker(asks: asks)
            self.found = found
            super.init(frame: .zero)
        }

        required init?(coder: NSCoder) { fatalError("not used") }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            install()
            found(window)
        }

        /// In front of the window's own delegate, once, and again should SwiftUI set another.
        func install() {
            guard let window, window.delegate !== asker else { return }
            asker.original = window.delegate
            window.delegate = asker
        }
    }
}

/// The window delegate that asks: `windowShouldClose` asks while in a party, and every other
/// message goes to the delegate it stands in front of.
final class CloseAsker: NSObject, NSWindowDelegate {
    var asks: @MainActor () -> Bool
    weak var original: NSWindowDelegate?

    init(asks: @escaping @MainActor () -> Bool) {
        self.asks = asks
    }

    @MainActor
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if asks() { return false }
        return original?.windowShouldClose?(sender) ?? true
    }

    override func responds(to selector: Selector!) -> Bool {
        super.responds(to: selector) || (original?.responds(to: selector) ?? false)
    }

    override func forwardingTarget(for selector: Selector!) -> Any? {
        original?.responds(to: selector) == true ? original : super.forwardingTarget(for: selector)
    }
}
#endif

// MARK: - Mac

#if os(macOS)
/// The form on the Mac: a raised sheet, its fields stacked, and its buttons at the foot.
struct MacPartyForm: View {
    @Environment(AppStore.self) private var store
    let start: PartyStart

    var body: some View {
        let state = PartyFormState(store: store, start: start)
        let party = store.party
        @Bindable var model = party
        let locked = party.form == nil || party.joining
        VStack(alignment: .leading, spacing: 0) {
            Text(state.title)
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(state.episodeLine)
                .font(.mac(13))
                .foregroundStyle(.haze)
                .padding(.top, 4)
            if case .invite(let invite) = start {
                Text("Room \(invite.room) on \(invite.server)")
                    .font(.mac(13))
                    .foregroundStyle(.silk)
                    .textSelection(.enabled)
                    .padding(.top, 18)
                hint(state.serverLine?.text ?? " ", problem: false)
                    .padding(.top, 6)
            } else {
                label("Server").padding(.top, 18)
                field("Server", text: $model.server, prompt: "host:port")
                    .textInputSuggestions {
                        ForEach(state.recent, id: \.self) { server in
                            Text(server).textInputCompletion(server)
                        }
                    }
                    .padding(.top, 6)
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    hint(state.serverLine?.text ?? " ", problem: state.serverLine?.problem ?? false)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button(cased(mac: "Use It", touch: "Use it")) {
                        if let form = party.form { party.server = form.suggestedServer }
                    }
                    .controlSize(.small)
                    .accessibilityLabel("Use \(party.form?.suggestedServer ?? "the public server")")
                    // Faded rather than removed, so the line keeps its height.
                    .opacity(state.offersSuggested ? 1 : 0)
                    .disabled(!state.offersSuggested || locked)
                    .animation(.quick, value: state.offersSuggested)
                }
                .padding(.top, 6)
                label("Room").padding(.top, 14)
                field("Room", text: $model.room, prompt: "")
                    .padding(.top, 6)
                hint(PartyFormState.roomNote, problem: false)
                    .padding(.top, 6)
            }
            label("Your name in the room").padding(.top, 14)
            field("Your name in the room", text: $model.name, prompt: "")
                .padding(.top, 6)
                .onChange(of: party.name) { _, name in
                    if name.count > PartyFormState.nameLimit { party.name = String(name.prefix(PartyFormState.nameLimit)) }
                }
            FormFoot(status: state.status)
                .padding(.top, 16)
            HStack(spacing: 10) {
                Spacer()
                Button("Cancel") { party.closeForm() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                Button(state.primary) { party.join() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
                    .disabled(!state.canStart)
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(520)
        .background(.raised)
        .environment(\.partyFieldsLocked, locked)
    }

    private func label(_ text: String) -> some View {
        Text(text)
            .font(.mac(12))
            .foregroundStyle(.haze)
    }

    private func hint(_ text: String, problem: Bool) -> some View {
        Text(text)
            .font(.mac(12))
            .macLineHeight(17)
            .foregroundStyle(problem ? .vermilion : .haze)
            .fixedSize(horizontal: false, vertical: true)
            .contentTransition(.opacity)
            .animation(.quick, value: text)
    }

    private func field(_ title: String, text: Binding<String>, prompt: String) -> some View {
        PartyField(title: title, text: text, prompt: prompt)
    }
}

/// A field of the Mac's form, as Join a Plugin Store draws them, locked while the form loads or
/// joins.
private struct PartyField: View {
    let title: String
    @Binding var text: String
    let prompt: String
    @Environment(\.partyFieldsLocked) private var locked

    var body: some View {
        TextField(title, text: $text, prompt: Text(prompt).foregroundStyle(.haze))
            .textFieldStyle(.plain)
            .font(.mac(13))
            .autocorrectionDisabled()
            .padding(.horizontal, 12)
            .frame(height: 32)
            .background(.ground, in: .rect(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassRule, lineWidth: 1))
            .labelsHidden()
            .disabled(locked)
            .opacity(locked ? 0.6 : 1)
            .animation(.quick, value: locked)
    }
}

extension EnvironmentValues {
    @Entry var partyFieldsLocked = false
}

/// The privacy note, and in its place the joining or error line: one place for both, so the
/// sheet keeps its height and has no empty band waiting for an error.
private struct FormFoot: View {
    let status: (text: String, busy: Bool, error: Bool)?

    var body: some View {
        ZStack(alignment: .topLeading) {
            Text(PartyFormState.privacy)
                .font(.mac(12))
                .macLineHeight(17)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .opacity(status == nil ? 1 : 0)
                .accessibilityHidden(status != nil)
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                if status?.busy == true {
                    ProgressView().controlSize(.small)
                } else if status?.error == true {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.vermilion)
                        .accessibilityHidden(true)
                }
                Text(status?.text ?? "")
                    .foregroundStyle(status?.error == true ? .silk : .haze)
                    .fixedSize(horizontal: false, vertical: true)
                    .lineLimit(3)
            }
            .font(.mac(12))
            .opacity(status == nil ? 0 : 1)
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.updatesFrequently)
        }
        .animation(.quick, value: status?.text)
    }
}

/// The no-source sheet on the Mac.
struct MacPartySource: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let ask: PartyNoSourceAsk
    @State private var picked: String?

    var body: some View {
        let state = PartySourceState(store: store, ask: ask)
        let party = store.party
        VStack(alignment: .leading, spacing: 0) {
            Text(state.title)
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
                .contentTransition(.opacity)
            if party.choice == nil {
                Text(PartyWords.place(party.party))
                    .font(.mac(13))
                    .foregroundStyle(.silk)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 12)
                Text(state.first.body)
                    .font(.mac(13))
                    .macLineHeight(18)
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 8)
            } else {
                PartySourceLine(line: state.line)
                    .padding(.top, 12)
                List(state.candidates, id: \.partyKey, selection: $picked) { candidate in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(candidate.title).font(.mac(13)).foregroundStyle(.silk).lineLimit(1)
                        Text(PartyWords.detail(candidate)).font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                    }
                    .padding(.vertical, 3)
                }
                .listStyle(.bordered)
                .alternatingRowBackgrounds(.disabled)
                .scrollContentBackground(.hidden)
                .background(.ground, in: .rect(cornerRadius: 8))
                // As tall as its rows, up to five, and none while there are none: the sheet
                // grows once the sources answer, as the Windows dialog's list does.
                .frame(height: state.candidates.isEmpty ? 0 : min(CGFloat(state.candidates.count) * 42 + 8, 218))
                .opacity(state.candidates.isEmpty ? 0 : 1)
                .animation(.fadeIn, value: state.candidates.isEmpty)
                .padding(.top, state.candidates.isEmpty ? 0 : 10)
                .accessibilityLabel("Sources for this episode")
            }
            HStack(spacing: 10) {
                Button(cased(mac: "Leave Party", touch: "Leave party")) {
                    party.leave()
                }
                .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                Spacer()
                Button(cased(mac: "Not Now", touch: "Not now")) { party.closeNoSource() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
                if state.primary != .none {
                    Button(state.label(state.primary)) { primary(state) }
                        .buttonStyle(PrimaryCapsuleStyle(height: 28))
                        .keyboardShortcut(.defaultAction)
                        .disabled(state.primary == .play && picked == nil)
                }
            }
            .padding(.top, 20)
        }
        .padding(30)
        .sheetWidth(520)
        .background(.raised)
        .animation(.quick, value: party.choice == nil)
    }

    private func primary(_ state: PartySourceState) {
        switch state.primary {
        case .none: break
        case .providers:
            store.party.closeNoSource()
            navigator.openProviders()
        case .choose, .again:
            picked = nil
            store.party.chooseSource()
        case .play:
            if let candidate = state.candidates.first(where: { $0.partyKey == picked }) { store.party.play(candidate) }
        }
    }
}
#endif

/// The line over the sources: a spinner while looking, a mark once answered, the error's in its
/// place if that failed; the line never moves.
private struct PartySourceLine: View {
    let line: (text: String, busy: Bool, error: Bool)
    #if os(macOS)
    private let firstLine: CGFloat = 16
    #else
    @ScaledMetric(relativeTo: .body) private var firstLine: CGFloat = 20
    #endif

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            ZStack {
                ProgressView().controlSize(.small).opacity(line.busy ? 1 : 0)
                Image(systemName: line.error ? "exclamationmark.triangle.fill" : "info.circle")
                    .foregroundStyle(line.error ? .vermilion : .haze)
                    .opacity(line.busy ? 0 : 1)
                    .accessibilityHidden(true)
            }
            // As tall as the first line, so the mark sits beside it however many lines follow.
            .frame(width: 16, height: firstLine)
            Text(line.text)
                .foregroundStyle(line.error ? .silk : .haze)
                .fixedSize(horizontal: false, vertical: true)
                .contentTransition(.opacity)
        }
        .font(.plat(mac: 13, touch: 15))
        .animation(.quick, value: line.text)
        .accessibilityElement(children: .combine)
    }
}

// MARK: - iPhone and iPad

#if os(iOS)
/// The form on iPhone and iPad: a grouped list in a navigation stack, its action at the bottom.
struct PhonePartyForm: View {
    @Environment(AppStore.self) private var store
    let start: PartyStart

    var body: some View {
        let state = PartyFormState(store: store, start: start)
        let party = store.party
        @Bindable var model = party
        let locked = party.form == nil || party.joining
        NavigationStack {
            PhoneList {
                Section {
                    Text(state.episodeLine)
                        .font(.ui(17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets(top: 0, leading: 4, bottom: 0, trailing: 4))
                }
                if case .invite(let invite) = start {
                    Section {
                        LabeledContent("Room", value: invite.room)
                        LabeledContent("Server", value: invite.server)
                    } footer: {
                        Text(state.serverLine?.text ?? " ")
                    }
                    .listRowBackground(Palette.raised)
                } else {
                    Section {
                        HStack {
                            TextField("Server", text: $model.server, prompt: Text("host:port"))
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                                .keyboardType(.URL)
                            if !state.recent.isEmpty, !locked {
                                Menu {
                                    ForEach(state.recent, id: \.self) { server in
                                        Button(server) { party.server = server }
                                    }
                                } label: {
                                    Image(systemName: "clock.arrow.circlepath")
                                        .foregroundStyle(.haze)
                                }
                                .accessibilityLabel("Recent servers")
                            }
                        }
                        .disabled(locked)
                    } header: {
                        Text("Server")
                    } footer: {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(state.serverLine?.text ?? " ")
                                .foregroundStyle(state.serverLine?.problem == true ? .vermilion : .haze)
                                .contentTransition(.opacity)
                            Button("Use \(party.form?.suggestedServer ?? "it")") {
                                if let form = party.form { party.server = form.suggestedServer }
                            }
                            .font(.ui(13, weight: .semibold))
                            .foregroundStyle(.silk)
                            // Faded rather than removed, so the list keeps its height.
                            .opacity(state.offersSuggested ? 1 : 0)
                            .disabled(!state.offersSuggested || locked)
                        }
                        .animation(.quick, value: state.serverLine?.text)
                    }
                    .listRowBackground(Palette.raised)
                    Section {
                        TextField("Room", text: $model.room)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .disabled(locked)
                    } header: {
                        Text("Room")
                    } footer: {
                        Text(PartyFormState.roomNote)
                    }
                    .listRowBackground(Palette.raised)
                }
                Section {
                    TextField("Your name in the room", text: $model.name)
                        .autocorrectionDisabled()
                        .disabled(locked)
                        .onChange(of: party.name) { _, name in
                            if name.count > PartyFormState.nameLimit { party.name = String(name.prefix(PartyFormState.nameLimit)) }
                        }
                } header: {
                    Text("Your name in the room")
                } footer: {
                    PhoneFormFoot(status: state.status)
                }
                .listRowBackground(Palette.raised)
            }
            .navigationTitle(state.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("Cancel") { party.closeForm() }.tint(.silk) }
            }
            .safeAreaInset(edge: .bottom) {
                Button {
                    party.join()
                } label: {
                    Text(state.primary).frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                .disabled(!state.canStart)
                .padding(.horizontal, 16)
                .padding(.bottom, 8)
            }
        }
        .tint(.silk)
        .preferredColorScheme(.dark)
    }
}

/// The privacy note, and in its place the joining or error line, as on the Mac.
private struct PhoneFormFoot: View {
    let status: (text: String, busy: Bool, error: Bool)?

    var body: some View {
        ZStack(alignment: .topLeading) {
            Text(PartyFormState.privacy)
                .opacity(status == nil ? 1 : 0)
                .accessibilityHidden(status != nil)
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                if status?.busy == true {
                    ProgressView().controlSize(.small)
                } else if status?.error == true {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.vermilion)
                        .accessibilityHidden(true)
                }
                Text(status?.text ?? "")
                    .foregroundStyle(status?.error == true ? .silk : .haze)
            }
            .opacity(status == nil ? 0 : 1)
            .accessibilityElement(children: .combine)
        }
        .padding(.top, 8)
        .animation(.quick, value: status?.text)
    }
}

/// The no-source sheet on iPhone and iPad.
struct PhonePartySource: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let ask: PartyNoSourceAsk
    @State private var picked: String?

    var body: some View {
        let state = PartySourceState(store: store, ask: ask)
        let party = store.party
        NavigationStack {
            PhoneList {
                // What happened, in full: a title bar would cut "None of your sources has this
                // episode" short beside Not now.
                Section {
                    Text(state.title)
                        .font(.ui(17, weight: .semibold))
                        .foregroundStyle(.silk)
                        .contentTransition(.opacity)
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets(top: 0, leading: 4, bottom: 0, trailing: 4))
                }
                if party.choice == nil {
                    Section {
                        Text(PartyWords.place(party.party))
                            .foregroundStyle(.silk)
                    } footer: {
                        Text(state.first.body)
                    }
                    .listRowBackground(Palette.raised)
                } else {
                    Section {
                        ForEach(state.candidates, id: \.partyKey) { candidate in
                            Button {
                                picked = candidate.partyKey
                            } label: {
                                HStack {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(candidate.title).foregroundStyle(.silk).lineLimit(1)
                                        Text(PartyWords.detail(candidate)).font(.ui(13)).foregroundStyle(.haze).lineLimit(1)
                                    }
                                    Spacer(minLength: 8)
                                    Image(systemName: "checkmark")
                                        .font(.ui(15, weight: .semibold))
                                        .foregroundStyle(.silk)
                                        .opacity(picked == candidate.partyKey ? 1 : 0)
                                }
                                .contentShape(.rect)
                            }
                            .buttonStyle(.plain)
                            .accessibilityAddTraits(picked == candidate.partyKey ? .isSelected : [])
                        }
                    } header: {
                        PartySourceLine(line: state.line)
                            .textCase(nil)
                            .padding(.bottom, 4)
                    }
                    .listRowBackground(Palette.raised)
                }
                Section {
                    Button(cased(mac: "Leave Party", touch: "Leave party"), role: .destructive) { party.leave() }
                }
                .listRowBackground(Palette.raised)
            }
            .navigationTitle("Watch party")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) { Button("Not now") { party.closeNoSource() }.tint(.silk) }
            }
            .safeAreaInset(edge: .bottom) {
                if state.primary != .none {
                    Button {
                        primary(state)
                    } label: {
                        Text(state.label(state.primary)).frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                    .disabled(state.primary == .play && picked == nil)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 8)
                }
            }
        }
        .tint(.silk)
        .preferredColorScheme(.dark)
    }

    private func primary(_ state: PartySourceState) {
        switch state.primary {
        case .none: break
        case .providers:
            store.party.closeNoSource()
            navigator.openProviders()
        case .choose, .again:
            picked = nil
            store.party.chooseSource()
        case .play:
            if let candidate = state.candidates.first(where: { $0.partyKey == picked }) { store.party.play(candidate) }
        }
    }
}
#endif
