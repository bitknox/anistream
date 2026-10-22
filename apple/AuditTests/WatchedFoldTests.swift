// The watched episodes a title's list folds away open and close in place: the rows below the
// fold row stay where they were on screen, on an iPhone's page and on the iPad's, which is the
// Mac's. Run with the Audit scheme, as the accessibility audit is.

import XCTest

@MainActor
final class WatchedFoldTests: XCTestCase {
    func testTheRowsBelowTheFoldStayPut() throws {
        let app = XCUIApplication()
        app.launchArguments = ["-AnistreamScene", "watching", "-AnistreamTitle", "900001"]
        app.launch()
        let fold = app.buttons["Episodes 1–5 watched"]
        XCTAssertTrue(fold.waitForExistence(timeout: 30), "no fold row over episode 6")
        let sixth = app.buttons.containing(NSPredicate(format: "label CONTAINS 'Cartographer'")).firstMatch
        XCTAssertTrue(sixth.waitForExistence(timeout: 10))
        // The fold mid-screen, with page above it to scroll into.
        var swipes = 0
        while fold.frame.midY > app.frame.midY + 80, swipes < 6 {
            app.swipeUp(velocity: .slow)
            swipes += 1
        }
        let before = sixth.frame.minY

        fold.tap()
        let hide = app.buttons["Hide watched episodes"]
        XCTAssertTrue(hide.waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons.containing(NSPredicate(format: "label CONTAINS 'Lantern Road'")).firstMatch.exists)
        XCTAssertEqual(sixth.frame.minY, before, accuracy: 1, "opening the fold moved the rows below it")

        hide.tap()
        XCTAssertTrue(app.buttons["Episodes 1–5 watched"].waitForExistence(timeout: 5))
        XCTAssertEqual(sixth.frame.minY, before, accuracy: 1, "closing the fold moved the rows below it")
    }
}
