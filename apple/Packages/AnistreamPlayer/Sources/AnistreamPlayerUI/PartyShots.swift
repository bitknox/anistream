// A watch party's states for screenshots (apple/scripts/shots.sh, party=<scene>): a party on the
// testing engine's fake Syncplay server, with members the run plays, as the Windows app rehearses
// them (#179). Testing only: the release app has none of this.

#if FIXTURES
import AnistreamKit
import AnistreamStore
import Foundation

/// A party's state a screenshot shows: `-AnistreamParty panel`.
public enum PartyScene: String, CaseIterable, Sendable {
    /// The form from a title's More, for its episode 7, the server empty and the room named.
    case form
    /// Start pressed on an address nothing answers on: "Joining … on …".
    case joining
    /// Start pressed on a port nobody listens on: the form says the server can't be reached.
    case refused
    /// An invite link opened: "Join the party?", with its room and server.
    case invite
    /// The player in a room of three, waiting: Mika ready, Ren not and on another episode, the
    /// party's panel open.
    case panel
    /// Everyone ready, the panel open.
    case ready
    /// Mika jumping ahead: the notice over the video, everyone's player moved with it.
    case sync
    /// Mika saying something: the chat's notice over the video.
    case chat
    /// The server gone mid-episode: the panel trying again.
    case lost
    /// Leave party pressed in the panel: the episode goes on, just for this device.
    case left
    /// In the room with nothing playing: none of the sources has the episode.
    case nosource
    /// The same, after Choose source: what each source lists.
    case choose
    /// An invite to an episode not out yet: joined, with nothing to choose.
    case unaired
    /// Quitting while in a party (Mac): Leave the party?
    case quit
    /// Closing the window while in a party (Mac): the same question, Leave and Close.
    case close

    public static let argument = "-AnistreamParty"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> PartyScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1) else { return nil }
        return PartyScene(rawValue: arguments[index + 1])
    }

    /// The room every scene's party is in.
    static let room = "quiet-harbor-k7m2xq9ad4tnwe3p"

    /// The fake server, held for the run: it stops when let go.
    @MainActor private static var held: FfiFakeParty?

    @MainActor static func server(_ store: AppStore) -> FfiFakeParty? {
        held = store.startFakeParty()
        return held
    }
}

extension PlayerModel {
    /// Bring the party to `scene`, for a shot. False when it did not get there in time.
    public func stage(_ scene: PartyScene, store: AppStore) async -> Bool {
        let party = store.party
        let room = PartyScene.room
        switch scene {
        case .form, .joining, .refused:
            party.open(.episode(id: 900_001, episode: "7"))
            guard await partyWaited(10, { party.filledIn }) else { return false }
            if scene == .form {
                try? await Task.sleep(for: .milliseconds(700))
                return true
            }
            // An address nothing answers on, or a port nobody listens on.
            party.rehearse(server: scene == .joining ? "10.255.255.1:8999" : "127.0.0.1:9")
            party.join()
            let there = scene == .joining ? party.joining : await partyWaited(20) { party.failure != nil }
            try? await Task.sleep(for: .milliseconds(900))
            return there
        case .invite, .unaired:
            guard let fake = PartyScene.server(store) else { return false }
            let (id, episode): (AnilistId, String) = scene == .invite ? (900_001, "7") : (900_002, "6")
            let link = partyInviteLink(invite: PartyInvite(server: fake.server(), room: room, id: id, episode: episode))
            // The title's page is the shot's own (title=<id>).
            party.openInvite(link) { _ in }
            guard await partyWaited(10, { party.filledIn }) else { return false }
            if scene == .invite {
                try? await Task.sleep(for: .milliseconds(700))
                return true
            }
            party.join()
            // The player, up while the sources were asked, closes for the party's sheet.
            let there = await partyWaited(20) { party.noSource != nil && !self.isPresented }
            quiet(store)
            try? await Task.sleep(for: .milliseconds(1500))
            return there
        case .nosource, .choose:
            // In the room, and an episode the sources were asked for and didn't have.
            guard let fake = PartyScene.server(store) else { return false }
            store.send(.partyJoin(server: fake.server(), room: room, name: "Ada"))
            guard await partyWaited(10, { party.inParty }) else { return false }
            quiet(store)
            store.apply(.partyNoSource(
                id: 900_001, episode: "7", why: .notFound,
                reason: .verbatim(text: "could not match this title: sample-source has no episode 7")))
            if scene == .choose {
                try? await Task.sleep(for: .milliseconds(800))
                party.chooseSource()
                // What two sources list, in before the fixture's own answer, which has none.
                store.apply(.sources(id: 900_001, episode: "7", candidates: [
                    SourceCandidate(
                        id: "a", providerId: "sample-source", title: "Stardust Wayfarer 07 1080p", quality: 1080,
                        seeders: 40, size: "1.2 GiB", dualAudio: false, dubbed: false, autoPick: true),
                    SourceCandidate(
                        id: "b", providerId: "night-index", title: "Stardust Wayfarer 07 720p", quality: 720,
                        seeders: 12, size: "640 MiB", dualAudio: false, dubbed: false, autoPick: false),
                ]))
                guard await partyWaited(20, { party.choice != .looking }) else { return false }
            }
            try? await Task.sleep(for: .milliseconds(900))
            return true
        case .quit, .close:
            guard let fake = PartyScene.server(store) else { return false }
            store.send(.partyJoin(server: fake.server(), room: room, name: "Ada"))
            guard await partyWaited(10, { party.inParty }) else { return false }
            quiet(store)
            party.askingToLeave = scene == .quit ? .quit : .close
            try? await Task.sleep(for: .milliseconds(900))
            return true
        case .panel, .ready, .sync, .chat, .lost, .left:
            return await stagePlaying(scene, store: store)
        }
    }

    /// A room of three around the episode playing: this device first, then Mika and Ren, who
    /// open the same file (Ren the next episode on the panel's shot).
    private func stagePlaying(_ scene: PartyScene, store: AppStore) async -> Bool {
        let party = store.party
        let room = PartyScene.room
        guard let fake = PartyScene.server(store),
              let item = store.continuing.queue(store.titles).all.first
        else { return false }
        store.send(.partyJoin(server: fake.server(), room: room, name: "Ada"))
        guard await partyWaited(10, { party.inParty }) else { return false }
        store.send(.play(id: item.id, episode: String(item.episode)))
        guard await partyWaited(30, { self.isPresented && self.hasPicture && self.duration != nil }) else { return false }
        // Until this device's member has the episode open, for the others to open the same.
        _ = await partyWaited(10) { party.members?.first(where: \.you)?.duration != nil }
        let you = party.members?.first(where: \.you)
        let file = you?.file ?? "Stardust Wayfarer · Episode \(item.episode)"
        let length = you?.duration ?? 60
        // Held at the start, as a room waiting to be ready is, and clear of Up Next.
        if !paused { togglePause() }
        pinnedControls = true
        fake.addMember(room: room, name: "Mika")
        fake.memberOpens(name: "Mika", file: file, duration: length)
        fake.addMember(room: room, name: "Ren")
        fake.memberOpens(name: "Ren", file: scene == .panel ? "Stardust Wayfarer · Episode \(item.episode + 1)" : file, duration: length)
        fake.memberReady(name: "Mika", ready: true)
        fake.memberReady(name: "Ren", ready: false)
        guard await partyWaited(10, { party.members?.count == 3 }) else { return false }
        try? await Task.sleep(for: .milliseconds(800))
        var there = true
        switch scene {
        case .panel:
            party.showPanel()
        case .ready:
            fake.memberReady(name: "Ren", ready: true)
            try? await Task.sleep(for: .milliseconds(400))
            party.setReady(true)
            there = await partyWaited(10) { party.members.map(PartyWords.allReady) == true }
            try? await Task.sleep(for: .milliseconds(600))
            party.showPanel()
        case .sync:
            fake.memberSeeks(name: "Mika", to: 20)
            there = await partyWaited(10) { self.alert != nil && self.position >= 19 }
        case .chat:
            fake.memberSays(name: "Mika", message: "Starting in a minute, grabbing tea")
            there = await partyWaited(10) { self.alert != nil }
        case .lost:
            fake.goDown(down: true)
            there = await partyWaited(10) {
                if case .reconnecting = party.party?.connection { true } else { false }
            }
            try? await Task.sleep(for: .milliseconds(1500))
            party.showPanel()
        case .left:
            party.showPanel()
            try? await Task.sleep(for: .milliseconds(1500))
            party.leave()
            there = await partyWaited(10) { !party.inParty }
        default:
            break
        }
        // The popover's or the sheet's own animation in, and the notices' fades.
        try? await Task.sleep(for: .milliseconds(900))
        return there
    }

    /// The join's "You're in the party" toast gone: the shot is of the sheet, and the Mac
    /// harness's own drawing, with the screen locked, cannot draw the window under a toast.
    private func quiet(_ store: AppStore) {
        if let notice = store.notices.current { store.notices.dismiss(notice) }
    }

    private func partyWaited(_ seconds: Int, _ condition: @MainActor () -> Bool) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(seconds)
        while !condition() {
            guard ContinuousClock.now < deadline else { return false }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return true
    }
}
#endif
