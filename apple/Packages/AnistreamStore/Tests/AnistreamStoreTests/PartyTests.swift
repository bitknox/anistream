// Watch parties as the store holds them: the words the screens say, and a party on the testing
// engine's fake Syncplay server, from the form to leaving.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct PartyTests {
    func member(_ name: String, ready: Bool?, you: Bool = false, file: String? = "Episode 7", matches: Bool? = true) -> PartyMember {
        PartyMember(name: name, ready: ready, file: file, duration: 30, fileMatches: matches, you: you)
    }

    let form = PartyForm(
        recentServers: [], suggestedServer: "syncplay.pl:8995", suggestedRunBy: "the Syncplay project",
        room: "quiet-harbor-k7m2xq9ad4tnwe3p", name: "Ada")

    // MARK: Words

    @Test func theReadyLineSaysWhoTheRoomWaitsFor() {
        let ada = member("Ada", ready: false, you: true)
        #expect(PartyWords.readyLine([ada]) == "Nobody else is here yet. Send the invite link.")
        #expect(PartyWords.readyLine([ada, member("Mika", ready: true)]) == "Play starts once you're ready")
        #expect(PartyWords.readyLine([member("Ada", ready: true, you: true), member("Ren", ready: false)]) == "Play starts once Ren is ready")
        #expect(PartyWords.readyLine([ada, member("Ren", ready: false)]) == "Play starts once you and Ren are ready")
        #expect(PartyWords.readyLine([ada, member("Ren", ready: false), member("Mika", ready: false)]) == "Play starts once 3 more are ready")
        // A client with no ready state does not hold the room up.
        #expect(PartyWords.readyLine([member("Ada", ready: true, you: true), member("Kai", ready: nil)]) == "Everyone is ready")
        #expect(PartyWords.allReady([member("Ada", ready: true, you: true), member("Kai", ready: nil)]))
        #expect(!PartyWords.allReady([member("Ada", ready: true, you: true)]))
    }

    @Test func aMemberSaysTheirStateAndTheirFile() {
        #expect(PartyWords.name(member("Ada", ready: true, you: true)) == "Ada (you)")
        #expect(PartyWords.state(member("Mika", ready: true)) == "Ready")
        #expect(PartyWords.state(member("Ren", ready: false)) == "Not ready")
        #expect(PartyWords.state(member("Kai", ready: nil)) == "No ready state")
        #expect(PartyWords.file(member("Ren", ready: false, matches: false)) == "Playing a different file")
        #expect(PartyWords.file(member("Ren", ready: false, file: nil, matches: nil)) == "Nothing open yet")
        #expect(PartyWords.file(member("Ada", ready: false, you: true, file: nil)) == nil)
        #expect(PartyWords.file(member("Mika", ready: true)) == nil)
    }

    @Test func theConnectionLineSaysHowItStands() {
        var view = PartyView(
            server: "party.example:8999", room: "r", name: "Ada", connection: .connected, encrypted: false,
            ready: false, episode: nil, invite: nil, motd: nil, latencyMs: 91)
        #expect(PartyWords.connection(view) == "Connected, not encrypted · 91 ms")
        view.encrypted = true
        view.latencyMs = nil
        #expect(PartyWords.connection(view) == "Connected, encrypted")
        view.connection = .reconnecting(attempt: 2, of: 5)
        #expect(PartyWords.connection(view) == "Connection lost. Trying again, 2 of 5")
    }

    @Test func theSuggestedServerSaysWhoRunsIt() {
        #expect(PartyWords.serverHint("", form: form) == "No server of your own? syncplay.pl:8995 is a public one, run by the Syncplay project.")
        #expect(PartyWords.serverHint(" syncplay.pl:8995 ", form: form) == "A public server run by the Syncplay project, not by anistream.")
        #expect(PartyWords.inviteServerHint("party.example:8999", form: form) == "A server the person who invited you chose. anistream doesn't run it.")
        #expect(PartyWords.inviteServerHint("syncplay.pl:8995", form: form) == "A public server run by the Syncplay project, not by anistream.")
    }

    @Test func aServerReadsAsAHostAndAPort() {
        for ok in ["", "party.example", "party.example:8999", "[::1]:8999", "127.0.0.1:8999"] {
            #expect(PartyWords.serverProblem(ok) == nil, "\(ok)")
        }
        for bad in ["party.example:", "party.example:0", "party.example:99999", "https://party.example", "a:b:c", ":8999", "[::1", "party example"] {
            #expect(PartyWords.serverProblem(bad) == "Write the server as a host and a port, like party.example:8999.", "\(bad)")
        }
        #expect(!PartyWords.roomOK("  "))
        #expect(PartyWords.roomOK("tuesday"))
    }

    @Test func noSourceSaysWhyAndWhatComesNext() {
        let what = "Episode 7 of Stardust Wayfarer"
        let found = PartyWords.noSource(why: .notFound, reason: "could not match this title: sample-source has no episode 7", episode: "7", what: what)
        #expect(found.title == "None of your sources has this episode")
        #expect(found.body == "Could not match this title: sample-source has no episode 7. Episode 7 of Stardust Wayfarer plays with the room once you choose a source that has it.")
        let none = PartyWords.noSource(why: .noSources, reason: "", episode: "7", what: what)
        #expect(none.title == "You have no sources yet")
        let soon = PartyWords.noSource(why: .notAired, reason: "episode 6 airs tomorrow 18:30", episode: "6", what: what)
        #expect(soon.title == "Episode 6 airs tomorrow at 18:30")
        #expect(soon.body == "Your sources won't have it before then. It plays with the room once one of them does.")
        let later = PartyWords.noSource(why: .notAired, reason: "episode 8 hasn't aired yet; episode 6 airs Saturday 18:30", episode: "8", what: what)
        #expect(later.title == "Episode 8 hasn't aired yet")
        #expect(later.body == "Episode 6 airs Saturday at 18:30. Your sources won't have episode 8 before it's out.")
        #expect(PartyWords.found(1) == "1 source has it")
        #expect(PartyWords.found(3) == "3 sources have it")
    }

    // MARK: A party on the fake server

    func started(root: URL) async throws -> AppStore {
        let store = AppStore()
        let launch = EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host)
        await store.start(launch, host: quietHost())
        try #require(store.phase == .running, "\(store.phase)")
        return store
    }

    @Test func aPartyGoesFromTheFormToTheRoomAndOut() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }
        let party = store.party
        let fake = try #require(store.startFakeParty())
        defer { fake.stop() }

        party.open(.playing)
        try await waitFor { party.form != nil }
        #expect(!party.room.isEmpty)
        #expect(!party.canStart || !party.server.isEmpty)
        party.server = fake.server()
        party.name = "Ada"
        #expect(party.canStart)
        party.join()
        #expect(party.joining)
        try await waitFor { party.party != nil }
        #expect(party.start == nil)
        #expect(!party.joining)
        #expect(store.notices.current?.notice.text == "You're in the party. The people button in the player has the invite link.")
        try await waitFor { party.members?.count == 1 }
        #expect(party.members?.first?.you == true)

        fake.addMember(room: party.party?.room ?? "", name: "Mika")
        fake.memberReady(name: "Mika", ready: true)
        try await waitFor { party.members?.count == 2 && party.members?.last?.ready == true }
        #expect(PartyWords.readyLine(party.members ?? []) == "Play starts once you're ready")
        party.setReady(true)
        try await waitFor { party.party?.ready == true && party.members?.first?.ready == true }
        #expect(PartyWords.readyLine(party.members ?? []) == "Everyone is ready")

        party.leave()
        #expect(party.leaving)
        try await waitFor { party.party == nil }
        #expect(party.members == nil)
        #expect(store.notices.current?.notice.text == "You left the party. What's playing goes on, just for you.")
    }

    @Test func aServerThatRefusesSaysWhyInTheForm() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }
        let party = store.party

        party.open(.episode(id: 900_001, episode: "7"))
        try await waitFor { party.form != nil }
        party.server = "127.0.0.1:9"
        party.join()
        try await waitFor({ party.failure != nil }, within: .seconds(20))
        #expect(party.failure?.contains("127.0.0.1:9") == true)
        #expect(party.start != nil)
        #expect(party.party == nil)
        // What was typed stays for Try again.
        #expect(party.server == "127.0.0.1:9")
        #expect(party.canStart)
        party.closeForm()
        #expect(party.start == nil)
    }

    @Test func anInviteOpensItsTitleAndAsksJoin() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = try await started(root: root)
        defer { store.shutdown() }
        let party = store.party
        var opened: AnilistId?

        let link = partyInviteLink(invite: PartyInvite(server: "party.example:8999", room: "tuesday", id: 900_001, episode: "7"))
        party.openInvite(link) { opened = $0 }
        #expect(opened == 900_001)
        #expect(party.start == .invite(PartyInvite(server: "party.example:8999", room: "tuesday", id: 900_001, episode: "7")))
        #expect(party.place.server == "party.example:8999")
        party.closeForm()

        party.openInvite("anistream://party?room=tuesday") { opened = $0 }
        #expect(party.start == nil)
        #expect(store.notices.current?.notice.text == "That party link is missing its server, room or episode")
    }
}
