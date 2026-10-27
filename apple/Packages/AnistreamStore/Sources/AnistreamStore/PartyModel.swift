// Watch parties: the party this device is in, the form that starts one or answers an invite, and
// the sheet for a party's episode none of the sources has. The engine speaks Syncplay and drives
// the player; this keeps what it said for the screens, as the Windows app does (#179).

import AnistreamKit
import Foundation
import Observation

/// What a party is started for.
public enum PartyStart: Equatable, Sendable, Identifiable {
    /// From a title's More: this episode plays with the room.
    case episode(id: AnilistId, episode: String)
    /// From the player's people button: what plays now plays with the room.
    case playing
    /// From an invite link: its room, on its server, with its episode.
    case invite(PartyInvite)

    public var id: String {
        switch self {
        case .episode(let id, let episode): "episode \(id) \(episode)"
        case .playing: "playing"
        case .invite(let invite): "invite \(invite.server) \(invite.room) \(invite.id) \(invite.episode)"
        }
    }

    /// The title and episode it plays, when it names one.
    public var episode: (id: AnilistId, episode: String)? {
        switch self {
        case .episode(let id, let episode): (id, episode)
        case .invite(let invite): (invite.id, invite.episode)
        case .playing: nil
        }
    }

    public var isInvite: Bool {
        if case .invite = self { true } else { false }
    }
}

/// A party's episode none of this device's sources has: the viewer is in the room with nothing
/// playing.
public struct PartyNoSourceAsk: Equatable, Sendable, Identifiable {
    public var id: AnilistId
    public var episode: String
    public var why: NoSourceWhy
    public var reason: String
}

/// What leaving a party asks about: quitting the app, or closing the window that holds the
/// player.
public enum PartyLeaving: Sendable {
    case quit
    case close
}

/// The no-source sheet's second step: what each source lists for the episode.
public enum PartySourceChoice: Equatable, Sendable {
    case looking
    case listed([SourceCandidate])
    /// The engine said why it found nothing, as an alert rather than an empty list.
    case failed(String)
}

@Observable @MainActor
public final class PartyModel {
    // MARK: The party

    /// The party this device is in, or nil when it is in none.
    public private(set) var party: PartyView?
    /// The party's members, this device's first, once the room has listed them.
    public private(set) var members: [PartyMember]?
    /// Leave party was pressed, and the engine has not said it left yet.
    public private(set) var leaving = false

    // MARK: The form

    /// The form that starts a party or answers an invite, while it is up.
    public private(set) var start: PartyStart?
    /// What the form opens with, once the engine has said.
    public private(set) var form: PartyForm?
    /// The form's fields, as typed.
    public var server = ""
    public var room = ""
    public var name = ""
    /// Start was pressed, and the room has not answered yet.
    public private(set) var joining = false
    /// Why the last start did not get into the room.
    public private(set) var failure: String?
    @ObservationIgnored private var filled = false

    // MARK: No source

    /// The no-source sheet, while it is up.
    public private(set) var noSource: PartyNoSourceAsk?
    /// Its second step, once Choose source was pressed.
    public private(set) var choice: PartySourceChoice?

    // MARK: Elsewhere

    /// Quitting, or closing the window that holds the player, was asked while in a party: the
    /// question before it leaves (Mac), as the Windows app asks on closing.
    public var askingToLeave: PartyLeaving?
    /// Leave and Quit, or Leave and Close, was pressed: what was asked goes ahead.
    public private(set) var leaveAgreed = false
    /// Counts the asks to open the party panel on the player's people button.
    public private(set) var panelRequests = 0

    @ObservationIgnored var send: @MainActor (Command) -> Void = { _ in }
    /// Says something to the viewer: `notice` as a notice while the player is down, `player`
    /// over the video while it is up, where nil says nothing (the engine's own notice does).
    @ObservationIgnored var say: @MainActor (_ notice: String, _ player: String?, _ kind: NoticeKind) -> Void = { _, _, _ in }
    /// Whether the player is up, where the engine's own notices say what a party does.
    @ObservationIgnored public var playerUp: @MainActor () -> Bool = { false }

    public init() {}

    /// Whether this device is in a party.
    public var inParty: Bool { party != nil }

    // MARK: Asking

    /// Open the form for `start`, asking the engine what it opens with.
    public func open(_ start: PartyStart) {
        self.start = start
        form = nil
        failure = nil
        joining = false
        filled = false
        server = ""
        room = ""
        name = ""
        send(.loadPartyForm)
    }

    /// An `anistream://party` link: its title's page opens (`openTitle`), and the form asks
    /// Join. A link missing a part, or for the room this device is in already, says so instead.
    public func openInvite(_ link: String, openTitle: (AnilistId) -> Void) {
        guard let invite = parsePartyInvite(link: link) else {
            let text = "That party link is missing its server, room or episode"
            say(text, text, .alert)
            return
        }
        if let party, party.server == invite.server, party.room == invite.room {
            say("You're already in this party", "You're already in this party", .info)
            return
        }
        if !playerUp() { openTitle(invite.id) }
        open(.invite(invite))
    }

    /// Whether Start can be pressed.
    public var canStart: Bool {
        guard form != nil, !joining else { return false }
        let (server, room) = place
        let trimmed = server.trimmingCharacters(in: .whitespaces)
        return !trimmed.isEmpty && PartyWords.serverProblem(server) == nil && PartyWords.roomOK(room)
            && !name.trimmingCharacters(in: .whitespaces).isEmpty
    }

    /// The server and room Start joins: the invite's, or as typed.
    public var place: (server: String, room: String) {
        if case .invite(let invite) = start { return (invite.server, invite.room) }
        return (server, room)
    }

    /// Start party, Join party or Try again: the room first, and the episode once in it, so a
    /// server that can't be reached leaves nothing playing behind the form.
    public func join() {
        guard canStart else { return }
        let (server, room) = place
        joining = true
        failure = nil
        send(.partyJoin(
            server: server.trimmingCharacters(in: .whitespaces),
            room: room.trimmingCharacters(in: .whitespaces),
            name: name.trimmingCharacters(in: .whitespaces)))
    }

    /// The form closed: Cancel while the room is still answering leaves, so no party starts
    /// behind the viewer's back.
    public func closeForm() {
        if joining { send(.partyLeave) }
        joining = false
        start = nil
    }

    /// This device's ready switch.
    public func setReady(_ ready: Bool) {
        send(.partyReady(ready: ready))
    }

    /// Leave party: what plays goes on, just for this device.
    public func leave() {
        guard party != nil else { return }
        leaving = true
        send(.partyLeave)
    }

    /// Leave and Quit, or Leave and Close: the party is left, and the quit or the close asks no
    /// more.
    public func agreeToLeave() {
        leaveAgreed = true
        askingToLeave = nil
        leave()
    }

    /// Whether quitting now leaves a party, and so asks first: asked once per quit.
    public func asksBeforeQuitting() -> Bool {
        asks(.quit)
    }

    /// Whether closing the window that holds the player now leaves a party, and so asks first:
    /// the red button and ⌘W, as Windows asks on closing. Nothing is asked outside a party.
    public func asksBeforeClosing() -> Bool {
        asks(.close)
    }

    private func asks(_ leaving: PartyLeaving) -> Bool {
        guard inParty, !leaveAgreed else { return false }
        askingToLeave = leaving
        return true
    }

    /// Open the party panel on the player's people button.
    public func showPanel() {
        panelRequests += 1
    }

    // MARK: No source

    /// Choose source, or Try again: what each source lists for the episode, loading in place.
    public func chooseSource() {
        guard let noSource else { return }
        choice = .looking
        send(.loadSources(id: noSource.id, episode: noSource.episode))
    }

    /// Play with the party: the source picked plays with the room this device is in.
    public func play(_ candidate: SourceCandidate) {
        guard let noSource else { return }
        send(.playSource(id: noSource.id, episode: noSource.episode, providerId: candidate.providerId, sourceId: candidate.id))
        closeNoSource()
    }

    /// Not now, or done: the sheet goes, and the room stays.
    public func closeNoSource() {
        noSource = nil
        choice = nil
    }

    // MARK: The engine

    func apply(_ event: Event) {
        switch event {
        case .partyForm(let form):
            self.form = form
            if !filled {
                filled = true
                server = form.recentServers.first ?? ""
                room = form.room
                name = form.name
            }
        case .partyJoined(let view):
            let fresh = party == nil
            // A party joined after one was left asks again before it is left by a close.
            if fresh { leaveAgreed = false }
            // A room joined afresh lists its members afresh.
            if party?.server != view.server || party?.room != view.room { members = nil }
            party = view
            leaving = false
            if joining {
                joining = false
                // In the room: the episode plays with it, or says it has no source here.
                if let (id, episode) = start?.episode {
                    send(.playParty(id: id, episode: episode, server: nil, room: nil, name: nil))
                }
                start = nil
            }
            if fresh {
                say("You're in the party. The people button in the player has the invite link.",
                    "You're in the party. The people button has the invite link.", .info)
            }
        case .partyState(let view):
            party = view
        case .partyMembers(let members):
            self.members = members
        case .partyLeft(let message):
            let reason = message.map { msgEnglish(msg: $0) }  // i18n P2: replace with the catalogue
            let was = party != nil
            party = nil
            members = nil
            leaving = false
            noSource = nil
            choice = nil
            askingToLeave = nil
            if joining {
                // The form says why itself, and Try again keeps what was typed.
                joining = false
                failure = reason ?? "The party ended before it began."
            } else if was {
                // Over the video the engine's own notice says it.
                if let reason {
                    say(reason, nil, .alert)
                } else {
                    say("You left the party. What's playing goes on, just for you.", nil, .info)
                }
            }
        case .partyNoSource(let id, let episode, let why, let reason):
            noSource = PartyNoSourceAsk(id: id, episode: episode, why: why, reason: msgEnglish(msg: reason))  // i18n P2: replace with the catalogue
            choice = nil
        case .sources(let id, let episode, let candidates):
            if choice == .looking, let noSource, noSource.id == id, noSource.episode == episode {
                choice = .listed(candidates)
            }
        case .toast(let notice):
            if choice == .looking, notice.kind == .alert {
                choice = .failed(PartyWords.sentence(notice.text))
            }
        default:
            break
        }
    }

    #if FIXTURES
    /// A screenshot run's typing in the form: the server.
    public func rehearse(server: String) {
        self.server = server
    }

    /// The form as the engine has filled it, for a screenshot run to wait on.
    public var filledIn: Bool { filled }
    #endif
}
