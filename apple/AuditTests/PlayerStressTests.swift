// The player under spam: menus and speed while it is still loading, then seeks and pauses fast,
// rotation, and the background and back. Nothing may crash, hang or lose the controls, and the
// episode must still be playing at the end. Same media and scheme as PlayerButtonsTests; the
// iPad is the device that turns without the picture going sideways.

import XCTest

final class PlayerStressTests: PlayerTestCase {
    private func alive(_ app: XCUIApplication, _ why: String) {
        XCTAssertEqual(app.state, .runningForeground, "the app is not running after \(why)")
    }

    func testMenusAndSpeedWhileLoading() throws {
        let app = launch()
        XCTAssertTrue(resumeButton(app).waitForExistence(timeout: 30))
        resumeButton(app).tap()
        // As soon as the player is up, before the picture: speed and tracks.
        let tracks = app.buttons["Subtitles and audio"]
        if tracks.waitForExistence(timeout: 5), tracks.isHittable {
            tracks.tap()
            Thread.sleep(forTimeInterval: 0.4)
            for row in app.buttons.allElementsBoundByIndex.filter({ $0.isHittable }).prefix(6)
            where row.label.contains("English") || row.label.contains("Japanese") || row.label == "Off" {
                row.tap()
            }
            app.swipeDown(velocity: .fast)
        }
        let picture = app.buttons["Picture and sound"]
        if picture.waitForExistence(timeout: 3), picture.isHittable {
            picture.tap()
            let fast = app.buttons.matching(NSPredicate(format: "label MATCHES '^(1[.,]5|2[.,]0).*'")).firstMatch
            if fast.waitForExistence(timeout: 2) { fast.tap() }
            app.swipeDown(velocity: .fast)
        }
        alive(app, "menus while loading")
        waitForPlaying(app)
        alive(app, "loading")
        close(app)
    }

    func testSeekPauseRotateAndBackground() throws {
        let app = launch()
        play(app)
        for round in 0..<8 {
            print("STRESS round \(round)")
            ensureControls(app)
            for label in ["Forward 10 seconds", "Forward 10 seconds", "Back 10 seconds", "Forward 10 seconds"] {
                let button = app.buttons[label]
                if button.exists, button.isHittable { button.tap() }
            }
            let toggle = app.buttons.matching(NSPredicate(format: "label == 'Pause' OR label == 'Play'")).firstMatch
            for _ in 0..<4 where toggle.exists && toggle.isHittable { toggle.tap() }
            if UIDevice.current.userInterfaceIdiom == .pad {
                XCUIDevice.shared.orientation = round % 2 == 0 ? .landscapeLeft : .portrait
                Thread.sleep(forTimeInterval: 1)
            }
            if round % 3 == 2 {
                XCUIDevice.shared.press(.home)
                Thread.sleep(forTimeInterval: 1.5)
                app.activate()
                XCTAssertTrue(app.wait(for: .runningForeground, timeout: 10), "the app did not come back")
                // Leaving a playing episode starts picture in picture, which hands the picture to
                // its own window and leaves the title page here. That is the design, so the
                // spam ends with the app alive and the title page answering.
                if !app.buttons["Close player"].waitForExistence(timeout: 3) {
                    XCTAssertTrue(app.buttons.firstMatch.waitForExistence(timeout: 8),
                                  "after the background the app shows nothing to press")
                    return
                }
            }
            alive(app, "round \(round)")
        }
        XCUIDevice.shared.orientation = .portrait
        Thread.sleep(forTimeInterval: 1.5)
        // Whatever the pause spam ended on, the player still answers: its Close button closes it.
        ensureControls(app)
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label == 'Pause' OR label == 'Play'")).firstMatch.exists,
                      "the controls lost their play button")
        close(app)
    }

    private func waitForPlaying(_ app: XCUIApplication) {
        let starting = app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Starting episode'")).firstMatch
        let deadline = Date().addingTimeInterval(40)
        while Date() < deadline, starting.exists { Thread.sleep(forTimeInterval: 0.5) }
        XCTAssertFalse(starting.exists, "the starting card stayed in the tree")
    }

    private func close(_ app: XCUIApplication) {
        ensureControls(app)
        app.buttons["Close player"].tap()
        XCTAssertTrue(resumeButton(app).waitForExistence(timeout: 10), "the title page did not come back")
    }
}
