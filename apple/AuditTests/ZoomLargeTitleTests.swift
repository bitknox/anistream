// A screen's large title is back the moment a title's page zooms home into the cover that opened
// it (T1): the iPhone's Continue, Calendar, Library and Search keep their large title through the pop
// rather than coming back inline and growing later. Run with the Audit scheme, as the
// accessibility audit is.

import XCTest

@MainActor
final class ZoomLargeTitleTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    func testContinueKeepsItsLargeTitle() throws {
        let app = launch([])
        let hero = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Stardust Wayfarer,'")).firstMatch
        XCTAssertTrue(hero.waitForExistence(timeout: 30), "no hero on Continue")
        Thread.sleep(forTimeInterval: 1.5)
        let before = largeTitle(in: app, "Continue")
        // The hero's art, above its title block, which is the art's one way in by touch.
        hero.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0)).withOffset(CGVector(dx: 185, dy: -104)).tap()
        try popHome(in: app, to: hero)
        assertLarge(in: app, "Continue", as: before)
    }

    func testCalendarKeepsItsLargeTitle() throws {
        let app = launch(["-AnistreamSection", "calendar"])
        let row = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'The Library Above the Clouds,'")).firstMatch
        XCTAssertTrue(row.waitForExistence(timeout: 30), "no row on Calendar")
        Thread.sleep(forTimeInterval: 1.5)
        let before = largeTitle(in: app, "Calendar")
        row.tap()
        try popHome(in: app, to: row)
        assertLarge(in: app, "Calendar", as: before)
    }

    func testSearchKeepsItsLargeTitle() throws {
        let app = launch(["-AnistreamSection", "search"])
        let cover = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Stardust Wayfarer'")).firstMatch
        XCTAssertTrue(cover.waitForExistence(timeout: 30), "no titles on Search")
        Thread.sleep(forTimeInterval: 1.5)
        let before = largeTitle(in: app, "Search")
        cover.tap()
        try popHome(in: app, to: cover)
        assertLarge(in: app, "Search", as: before)
    }

    func testLibraryKeepsItsLargeTitle() throws {
        let app = launch(["-AnistreamSection", "library"])
        let cover = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'The Library Above the Clouds,'")).firstMatch
        XCTAssertTrue(cover.waitForExistence(timeout: 30), "no titles in the Library")
        Thread.sleep(forTimeInterval: 1.5)
        let before = largeTitle(in: app, "Library")
        cover.tap()
        try popHome(in: app, to: cover)
        assertLarge(in: app, "Library", as: before)
    }

    private func launch(_ arguments: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchArguments = ["-AnistreamScene", "watching"] + arguments
        app.launch()
        return app
    }

    /// The title's page up, then back: the zoom home into the cover.
    private func popHome(in app: XCUIApplication, to source: XCUIElement) throws {
        let more = app.navigationBars.buttons["More"]
        XCTAssertTrue(more.waitForExistence(timeout: 10), "no title page")
        Thread.sleep(forTimeInterval: 0.8)
        app.navigationBars.buttons.element(boundBy: 0).tap()
        XCTAssertTrue(source.waitForExistence(timeout: 10))
        // The zoom home is 0.5 s or so; the title is read as soon as it ends, before anything
        // else could grow it back.
        let gone = NSPredicate(format: "exists == false")
        expectation(for: gone, evaluatedWith: more)
        waitForExpectations(timeout: 5)
    }

    /// The large title's text, where it stands: nil when the bar shows the title inline.
    private func largeTitle(in app: XCUIApplication, _ title: String) -> CGRect? {
        let text = app.navigationBars.staticTexts[title]
        guard text.exists else { return nil }
        let frame = text.frame
        // The inline title is 17 points, its frame about 22 high; the large one 34, about 41.
        return frame.height > 30 ? frame : nil
    }

    private func assertLarge(in app: XCUIApplication, _ title: String, as before: CGRect?,
                             file: StaticString = #filePath, line: UInt = #line) {
        guard let before else {
            XCTFail("\(title) had no large title to begin with", file: file, line: line)
            return
        }
        // Read three times over the next half second: large every time, where it was.
        for _ in 0..<3 {
            let now = largeTitle(in: app, title)
            XCTAssertNotNil(now, "\(title)'s title came back inline after the zoom home", file: file, line: line)
            if let now { XCTAssertEqual(now.minY, before.minY, accuracy: 1, "\(title)'s large title moved", file: file, line: line) }
            Thread.sleep(forTimeInterval: 0.2)
        }
    }
}
