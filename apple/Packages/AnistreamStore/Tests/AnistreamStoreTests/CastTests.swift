// Casting as the store holds it: the picker's Chromecast rows, including one that just left, and
// the card of a scripted cast through starting, playing, failing and ending. The engine's own
// cast is covered by its tests; here the session is the Fixtures app's script.

@testable import AnistreamStore
import AnistreamKit
import AnistreamStrings
import Foundation
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct CastTests {
    let request = TVPlayRequest(id: 900_001, title: "Lanterns", episode: "3", from: 312)

    func row(_ id: String, _ name: String) -> FfiCastRow {
        FfiCastRow(key: "cast:\(id)", name: name, model: "", address: "127.0.0.9", routeHint: .throughThisDevice)
    }

    /// A model casting to a scripted session.
    func casting() -> (CastModel, FixtureCastSession) {
        let model = CastModel()
        let session = FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        model.opener = { _, _ in session }
        model.cast("cast:living", name: "Living room", request: request)
        return (model, session)
    }

    func play(_ session: FixtureCastSession) {
        session.change { state in
            state.phase = .connected
            state.loading = false
            state.duration = 1420
            state.position = 492
        }
    }

    @Test func rowsComeByNameAndOneThatLeftSaysSoBeforeItGoes() async throws {
        let model = CastModel()
        model.browsing(true)
        model.refresh([row("b", "Living room"), row("a", "Kitchen display")])
        #expect(model.rows.map(\.row.name) == ["Kitchen display", "Living room"])

        model.refresh([row("b", "Living room")])
        #expect(model.rows.map(\.row.name) == ["Kitchen display", "Living room"])
        #expect(model.rows.first?.gone == true)
        #expect(model.rows.last?.gone == false)

        try await waitFor({ model.rows.count == 1 }, within: .seconds(6))
        #expect(model.rows.map(\.row.name) == ["Living room"])
    }

    @Test func aTvBackInTimeTakesItsRowBack() async throws {
        let model = CastModel()
        model.browsing(true)
        model.refresh([row("a", "Kitchen display")])
        model.refresh([])
        model.refresh([row("a", "Kitchen display")])
        try await Task.sleep(for: .seconds(4.5))
        #expect(model.rows.map(\.gone) == [false])
    }

    @Test func stoppingTheBrowseOrHidingTheGroupClearsTheRows() {
        let model = CastModel()
        model.browsing(true)
        model.refresh([row("a", "Kitchen display")])
        model.browsing(false)
        #expect(model.rows.isEmpty)
        model.browsing(true)
        model.shown = false
        model.refresh([row("a", "Kitchen display")])
        #expect(model.rows.isEmpty)
    }

    @Test func theCardStartsAtOnceThenCountsThenPlays() async throws {
        let (model, session) = casting()
        #expect(model.active)
        #expect(model.stage == .starting)
        #expect(model.now?.title == "Lanterns")
        #expect(model.line == L.tvStartingOn(name: "Living room"))

        session.change { $0.phaseLine = .castPhasePreparing(seconds: 5) }
        try await waitFor { model.card?.phaseLine != nil }
        #expect(model.line == L.castPhasePreparing(seconds: 5))

        play(session)
        try await waitFor { model.stage == .playing }
        #expect(model.live)
        #expect(model.line == L.tvStatusPlaying(name: "Living room"))
        #expect(model.fixedVolume)
        #expect(model.actions.isEmpty)
    }

    @Test func upNextAndUndoComeFromTheEngineAndAnswerBack() async throws {
        let (model, session) = casting()
        play(session)
        session.change {
            $0.upNext = FfiCastUpNext(number: "4", title: "Lanterns", still: nil, auto: true, secondsLeft: 6)
            $0.undo = FfiCastUndo(kind: .opening, to: 12)
        }
        try await waitFor { model.card?.upNext != nil }
        #expect(model.card?.upNext?.secondsLeft == 6)
        model.cancelNext()
        try await waitFor { model.card?.upNext == nil }
        model.undoSkip()
        try await waitFor { model.card?.undo == nil }
        #expect(abs((model.playhead() ?? 0) - 12) < 2)
    }

    @Test func eachFailureOffersItsFix() async throws {
        let cases: [(Msg, [CastAction])] = [
            (.noticeCastRefused(name: "Living room"), [.chooseSource, .playHere]),
            (.noticeCastVideoCodec(name: "Living room", codec: "AV1"), [.chooseSource, .playHere]),
            (.noticeCastNeverFetches(name: "Living room"), [.tryAgain, .playHere]),
            (.noticeCastNotSameNetwork(name: "Living room"), [.playHere, .chooseAnotherTV]),
            (.noticeCastDeepLink, [.chooseSource]),
        ]
        for (notice, actions) in cases {
            let (model, session) = casting()
            session.change {
                $0.notice = notice
                $0.over = true
            }
            try await waitFor { model.stage == .failed }
            #expect(model.actions == actions, "\(notice)")
            #expect(model.reason == notice.text)
            #expect(model.line == L.castCouldntStart(name: "Living room"))
        }
    }

    @Test func aTvThatDropsOffStopsTheCastAndPlayHereCarriesOn() async throws {
        let (model, session) = casting()
        var played: (AnilistId, String, Double?)?
        model.playLocally = { played = ($0, $1, $2) }
        play(session)
        try await waitFor { model.stage == .playing }
        session.change {
            $0.notice = .noticeCastLost(name: "Living room")
            $0.over = true
        }
        try await waitFor { model.stage == .stopped }
        #expect(model.line == L.castStopped)
        #expect(model.actions == [.playHere, .chooseAnotherTV])

        model.playHere()
        #expect(!model.active)
        #expect(played?.0 == 900_001)
        #expect(played?.1 == "3")
        #expect(played?.2 == 492)
    }

    @Test func someoneElseTakingTheTvClosesTheCardWithANotice() async throws {
        let (model, session) = casting()
        var said: [Notice] = []
        var closed = false
        model.say = { said.append($0) }
        model.closed = { closed = true }
        play(session)
        try await waitFor { model.stage == .playing }
        session.change {
            $0.notice = .noticeCastTakenOver(name: "Living room")
            $0.over = true
        }
        try await waitFor { !model.active }
        #expect(closed)
        #expect(said.map(\.msg) == [.noticeCastTakenOver(name: "Living room")])
        #expect(model.takenOver?.from == 492)

        var played: Double?
        model.playLocally = { _, _, position in played = position }
        model.playHereAfterTakeOver()
        #expect(played == 492)
        #expect(model.takenOver == nil)
    }

    @Test func tryAgainCastsTheSameEpisodeToTheSameTv() async throws {
        let (model, session) = casting()
        session.change {
            $0.notice = .noticeCastNeverFetches(name: "Living room")
            $0.over = true
        }
        try await waitFor { model.stage == .failed }
        var opened: [String] = []
        model.opener = { key, _ in
            opened.append(key)
            return FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        }
        model.tryAgain()
        #expect(opened == ["cast:living"])
        #expect(model.stage == .starting)
        #expect(model.request == request)
    }

    @Test func theCardsControlsReachTheTv() async throws {
        let (model, session) = casting()
        play(session)
        try await waitFor { model.stage == .playing }
        model.send(.mute(muted: true))
        try await waitFor { model.muted }
        model.send(.togglePause)
        try await waitFor { model.now?.paused == true }
        #expect(model.playhead() == 492)
    }

    @Test func aPlayStartedWhileCastingStaysOffThePlayerHere() {
        #expect(AppStore.playsHere(.playbackEnded(watched: false, ending: .stopped, rollingOn: false)))
        #expect(!AppStore.playsHere(.toast(Notice(kind: .info, text: "", hint: nil))))
    }
}
