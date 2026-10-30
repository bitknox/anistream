// The iPhone and iPad player's controls act on the first tap, and the double-tap seek still works.
// Plays long.mkv from apple/scripts/player-media.sh on a Simulator. Run with the
// Audit scheme: -only-testing:AnistreamAuditTests/PlayerButtonsTests. PLAYER_MEDIA (as
// TEST_RUNNER_PLAYER_MEDIA) names another file; 

import XCTest

@MainActor
class PlayerTestCase: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    override func tearDown() {
        XCUIDevice.shared.orientation = .portrait
    }

    var media: String {
        if let named = ProcessInfo.processInfo.environment["PLAYER_MEDIA"], !named.isEmpty { return named }
        let apple = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
        return apple.appendingPathComponent(".mpv/media/long.mkv").path
    }

    func launch(_ arguments: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-AnistreamScene", "watching", "-AnistreamTitle", "900001", "-AnistreamMedia", media] + arguments
        app.launch()
        return app
    }

    func resumeButton(_ app: XCUIApplication) -> XCUIElement {
        let all = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Resume episode' OR label BEGINSWITH 'Play episode'"))
        return all.allElementsBoundByIndex.first { $0.isHittable } ?? all.firstMatch
    }

    /// Open the episode, held sideways as a person holds the phone to watch, and wait for the
    /// picture: the starting card leaves the tree when it fades.
    func play(_ app: XCUIApplication, landscape: Bool = true) {
        XCTAssertTrue(resumeButton(app).waitForExistence(timeout: 30), "no episode to play")
        if landscape && UIDevice.current.userInterfaceIdiom == .phone {
            XCUIDevice.shared.orientation = .landscapeLeft
            Thread.sleep(forTimeInterval: 1)
        }
        resumeButton(app).tap()
        let starting = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Starting episode'")).firstMatch
        // Playing: the controls are in the tree and the starting card is not.
        let deadline = Date().addingTimeInterval(40)
        var playing = false
        while Date() < deadline {
            if app.buttons["Close player"].exists, !starting.exists { playing = true; break }
            Thread.sleep(forTimeInterval: 0.5)
        }
        if !playing {
            print("PLAYER-DUMP\n\(app.debugDescription)")
            shot("player-not-playing")
        }
        XCTAssertTrue(playing, "the player did not come up, or its starting card stayed in the tree")
        Thread.sleep(forTimeInterval: 2.5)
    }

    /// A screenshot into PLAYER_SHOTS (as TEST_RUNNER_PLAYER_SHOTS), when it is set.
    func shot(_ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["PLAYER_SHOTS"], !dir.isEmpty else { return }
        let url = URL(fileURLWithPath: dir).appendingPathComponent("\(name).png")
        try? XCUIScreen.main.screenshot().pngRepresentation.write(to: url)
    }

    /// Show the controls whatever they were doing: tap, and tap once more if that hid them.
    func ensureControls(_ app: XCUIApplication) {
        for _ in 0..<2 {
            app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
            if app.buttons["Close player"].waitForExistence(timeout: 2) { Thread.sleep(forTimeInterval: 0.6); return }
        }
        print("PLAYER-DUMP controls\n\(app.debugDescription)")
        shot("no-controls")
        XCTFail("no controls after two taps")
    }

    /// Show the controls: a tap on the picture, and wait for the Close button.
    func showControls(_ app: XCUIApplication) {
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        XCTAssertTrue(app.buttons["Close player"].waitForExistence(timeout: 5), "no controls after a tap")
        Thread.sleep(forTimeInterval: 0.6)
    }
}

final class PlayerButtonsTests: PlayerTestCase {
    func testEveryControlActsOnTheFirstTap() throws {
        let app = launch()
        play(app)
        showControls(app)

        // Pause turns into Play, and the controls stay up.
        let pause = app.buttons["Pause"]
        XCTAssertTrue(pause.waitForExistence(timeout: 5), "no Pause button")
        pause.tap()
        XCTAssertTrue(app.buttons["Play"].waitForExistence(timeout: 3), "Pause did not pause on the first tap")
        app.buttons["Play"].tap()
        XCTAssertTrue(app.buttons["Pause"].waitForExistence(timeout: 3), "Play did not resume on the first tap")

        // Forward 10 seconds moves the position on, and the controls stay up.
        let before = position(app)
        app.buttons["Forward 10 seconds"].tap()
        Thread.sleep(forTimeInterval: 1)
        XCTAssertTrue(app.buttons["Close player"].exists, "Forward 10 seconds hid the controls")
        XCTAssertGreaterThan(position(app), before + 5, "Forward 10 seconds did not seek")

        // Picture and sound opens its sheet.
        app.buttons["Picture and sound"].tap()
        let speed = app.buttons.matching(NSPredicate(format: "label MATCHES '^1[.,]5.*'")).firstMatch
        if !speed.waitForExistence(timeout: 3) {
            print("PLAYER-DUMP picture\n\(app.debugDescription)")
            shot("picture-sheet")
        }
        XCTAssertTrue(speed.exists, "Picture and sound did not open on the first tap")
        app.swipeDown(velocity: .fast)
        XCTAssertTrue(app.buttons["Close player"].waitForExistence(timeout: 3))

        // Close player closes it.
        app.buttons["Close player"].tap()
        XCTAssertTrue(resumeButton(app).waitForExistence(timeout: 10), "Close player did not close on the first tap")
    }

    func testDoubleTapOnTheSidesStillSeeks() throws {
        let app = launch()
        play(app)
        let before = position(app)
        shot("double-tap-before")
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.85, dy: 0.5)).doubleTap()
        // The seek lands within a moment; the "+10 s" flash is too brief to wait for.
        let deadline = Date().addingTimeInterval(6)
        var after = position(app)
        while after <= before + 5, Date() < deadline {
            Thread.sleep(forTimeInterval: 0.5)
            after = position(app)
        }
        shot("double-tap-after")
        XCTAssertGreaterThan(after, before + 5, "the double tap did not seek")
    }

    func testTheStartingCardLeavesTheTreeOncePlaying() throws {
        let app = launch()
        play(app)
        XCTAssertFalse(app.staticTexts["Starting episode 7"].exists)
    }

    /// The seek bar's position in seconds, from "10 minutes, 11 seconds of 25 minutes".
    func position(_ app: XCUIApplication) -> Double {
        let slider = app.descendants(matching: .any).matching(NSPredicate(format: "label == 'Position'")).firstMatch
        guard slider.waitForExistence(timeout: 3), let text = slider.value as? String else {
            print("PLAYER-DUMP position\n\(app.debugDescription)")
            return -1
        }
        let played = text.components(separatedBy: " of ").first ?? text
        var seconds = 0.0
        for part in played.components(separatedBy: ",") {
            let words = part.split(separator: " ")
            guard words.count >= 2, let n = Double(words[0]) else { continue }
            if words[1].hasPrefix("hour") { seconds += n * 3600 } else if words[1].hasPrefix("minute") { seconds += n * 60 } else { seconds += n }
        }
        return seconds
    }
}
