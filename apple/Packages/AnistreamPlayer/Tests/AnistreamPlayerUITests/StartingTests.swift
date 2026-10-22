// The starting screen's states: waiting, offering another source, and saying it could not play.

import AnistreamKit
import AnistreamPlayer
import Foundation
import Testing

@testable import AnistreamPlayerUI

@MainActor
struct CouldNotPlayTests {
    static let candidate = SourceCandidate(
        id: "release-2", providerId: "lantern-archive", title: "Stardust Wayfarer 07 1080p", quality: 1080,
        seeders: 40, size: "1.2 GB", dualAudio: false, dubbed: false, autoPick: false)

    func staged(_ rig: Rig) {
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1))
    }

    @Test func aPlayNoSourceAnsweredSaysSoAndOffersAnother() {
        let rig = Rig()
        staged(rig)
        rig.model.apply(.playbackEnded(watched: false, ending: .failed, rollingOn: false))
        #expect(rig.model.isPresented, "it stays to say so")
        #expect(rig.model.failure == PlaybackFailure(id: 900_001, episode: "7"))

        rig.model.chooseSource()
        #expect(rig.sent.last == .loadSources(id: 900_001, episode: "7"))
        #expect(rig.model.choosingSource && rig.model.sourceChoices == nil)
        rig.model.apply(.sources(id: 900_001, episode: "7", candidates: [Self.candidate]))
        #expect(rig.model.sourceChoices == [Self.candidate])
        rig.model.play(Self.candidate)
        #expect(rig.sent.last == .playSource(id: 900_001, episode: "7", providerId: "lantern-archive", sourceId: "release-2"))
        #expect(rig.model.failure == nil)
    }

    @Test func closingItSendsNothing() {
        let rig = Rig()
        staged(rig)
        rig.model.apply(.playbackEnded(watched: false, ending: .failed, rollingOn: false))
        let sent = rig.sent.count
        rig.model.viewerClosed()
        #expect(!rig.model.isPresented)
        #expect(rig.sent.count == sent)
    }

    @Test func aPlayerClosedBeforeItsFirstFrameWaitsForTheVerdict() async throws {
        let rig = Rig()
        staged(rig)
        try await rig.opened()
        // The engine gave up on every stream: the player closes, and then the ending says why.
        rig.ops(1, [.close])
        #expect(rig.model.isPresented, "the starting screen stays for the verdict")
        #expect(rig.model.layer == nil)
        rig.model.apply(.playbackEnded(watched: false, ending: .failed, rollingOn: false))
        #expect(rig.model.failure?.episode == "7")
        try await until("mpv shut down") { rig.mpv.ended }
    }

    @Test func aPlayerThatPlayedClosesAtOnce() async throws {
        let rig = Rig()
        staged(rig)
        try await rig.opened()
        rig.mpv.emit(.playbackRestart)
        try await until("the picture") { rig.model.hasPicture }
        rig.ops(1, [.close])
        #expect(!rig.model.isPresented)
    }

    @Test func withNoVerdictTheScreenComesDownByItself() async throws {
        let rig = Rig()
        staged(rig)
        try await rig.opened()
        rig.ops(1, [.close])
        #expect(rig.model.isPresented)
        try await until("down", timeout: PlayerTiming.verdictWait + .seconds(2)) { !rig.model.isPresented }
        #expect(rig.model.failure == nil)
    }
}

struct WaitTests {
    @Test func theWaitShowsFromThreeSecondsAndTheChoiceFromTwenty() {
        let now = ContinuousClock.now
        let screen = StartingScreen(playback: 1, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, since: now)
        #expect(screen.wait(at: now + .seconds(2), note: nil) == nil)
        #expect(screen.wait(at: now + .seconds(4), note: nil) == "Finding a source · 4 s")
        #expect(screen.wait(at: now + .seconds(5), note: "waiting for lantern-archive") == "Waiting for lantern-archive · 5 s")
        #expect(!screen.offersChoice(at: now + .seconds(19)))
        #expect(screen.offersChoice(at: now + .seconds(20)))
    }
}
