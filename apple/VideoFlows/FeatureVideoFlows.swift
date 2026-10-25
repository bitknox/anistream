// The features page's videos: each test is one task done in the Fixtures app at a person's pace,
// while website/scripts/videos/record.sh films the screen. A flow marks where its clip starts and
// ends by printing "ANISTREAM_VIDEO start|end <seconds since 1970>", and the recorder cuts there,
// with a moment of the settled screen before the start. Nothing here asserts more than a flow
// needs to find its way; when a step finds nothing, the screen's elements are printed.
//
// ANISTREAM_VIDEO_MEDIA (TEST_RUNNER_ANISTREAM_VIDEO_MEDIA for xcodebuild) names the episode every
// stream plays: website/scripts/videos/episode.sh's cut of こうしす！ #3.1 (CC BY 4.0).

import XCTest

@MainActor
final class FeatureVideoFlows: XCTestCase {
    override func setUp() {
        continueAfterFailure = false
    }

    override func tearDown() {
        #if os(iOS)
        XCUIDevice.shared.orientation = .portrait
        #endif
    }

    #if os(iOS)
    /// Find something to watch: a search, then a genre from the filters.
    func testSearch() throws {
        let app = launch(["-AnistreamSection", "search"])
        _ = find(app.staticTexts["Trending now"], in: app, timeout: 30)
        settle()
        mark("start")
        let field = app.searchFields.firstMatch
        field.tap()
        field.typeText("star\n")
        _ = find(app.staticTexts["Stardust Wayfarer Season 2"], in: app)
        pause(0.6)
        app.buttons["Filters"].firstMatch.tap()
        find(app.buttons["Adventure"], in: app).tap()
        pause(0.6)
        app.buttons["Done"].firstMatch.tap()
        pause(1.2)
        mark("end")
    }

    /// The week's calendar: the days ahead, then everything airing rather than only your list.
    func testCalendar() throws {
        let app = launch([])
        _ = find(app.staticTexts["Continue"], in: app, timeout: 30)
        settle()
        mark("start")
        app.tabBars.buttons["Calendar"].tap()
        let days = app.buttons.matching(NSPredicate(
            format: "label MATCHES '^(Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday) .*'"))
        _ = find(days.element(boundBy: 6), in: app)
        pause(1.0)
        days.element(boundBy: 6).tap()
        pause(0.9)
        app.buttons["All airing"].tap()
        pause(0.9)
        days.element(boundBy: 2).tap()
        pause(1.2)
        mark("end")
    }

    /// The player, held landscape: an episode resumed from its page, its controls, and a sharper
    /// look picked for the picture.
    func testPlayer() throws {
        let app = launch(["-AnistreamTitle", "900001"])
        let resume = find(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Resume episode 7'")).firstMatch,
                          in: app, timeout: 30)
        XCUIDevice.shared.orientation = .landscapeLeft
        pause(1.0)
        resume.tap()
        // Playing once the player's own loading card has gone, which a busy Mac can keep in the
        // element tree a while after the picture is up: 30 s at most.
        let starting = app.staticTexts["Starting episode 7"]
        _ = find(starting, in: app)
        let gone = XCTNSPredicateExpectation(predicate: NSPredicate(format: "exists == false"), object: starting)
        _ = XCTWaiter().wait(for: [gone], timeout: 30)
        pause(1.5)
        mark("start")
        pause(0.8)
        app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).tap()
        let picture = find(app.buttons["Picture and sound"], in: app, timeout: 5)
        pause(0.6)
        picture.tap()
        pause(0.8)
        let sharp = app.descendants(matching: .any).matching(NSPredicate(format: "label BEGINSWITH 'Sharp'")).firstMatch
        if sharp.waitForExistence(timeout: 3) {
            sharp.tap()
        } else {
            dump(app, "no Sharp look")
        }
        pause(2.0)
        mark("end")
    }

    /// Play on TV: send the episode to a paired TV, and the phone becomes its remote.
    func testPlayOnTV() throws {
        let app = launch(["-AnistreamTitle", "900001", "-AnistreamTV", "paired"])
        let tv = find(app.buttons["tv"], in: app, timeout: 30)
        settle()
        mark("start")
        tv.tap()
        let bedroom = find(app.buttons.matching(NSPredicate(format: "label CONTAINS 'Bedroom'")).firstMatch, in: app)
        pause(0.8)
        bedroom.tap()
        // The remote, with the episode playing on the TV.
        _ = find(app.buttons["Disconnect"], in: app, timeout: 15)
        pause(2.5)
        mark("end")
    }
    #endif

    #if os(macOS)
    /// Find something to watch on the Mac: a search, then a genre from the filters.
    func testSearchOnMac() throws {
        let app = launch(["-AnistreamSection", "search"])
        let window = app.windows.firstMatch
        XCTAssertTrue(window.waitForExistence(timeout: 30))
        settle()
        let screen = NSScreen.main?.frame ?? .zero
        let frame = window.frame
        print("ANISTREAM_VIDEO window \(frame.minX) \(frame.minY) \(frame.width) \(frame.height) screen \(screen.width) \(screen.height)")
        mark("start")
        let field = app.searchFields.firstMatch
        field.click()
        field.typeText("star\r")
        pause(1.0)
        app.buttons["Filters"].firstMatch.click()
        find(app.buttons["Adventure"], in: app).click()
        pause(1.5)
        mark("end")
    }
    #endif

    /// Explore a screen: launch on ANISTREAM_VIDEO_ARGS and print what is on it. For writing a
    /// flow, never recorded.
    func testExplore() throws {
        let arguments = ProcessInfo.processInfo.environment["ANISTREAM_VIDEO_ARGS"]?
            .split(separator: " ").map(String.init) ?? []
        let app = launch(arguments)
        Thread.sleep(forTimeInterval: 6)
        print(app.debugDescription)
    }

    // MARK: - Helpers

    /// The Fixtures app on the Watching scene, with the episode the recorder names as every
    /// episode's stream.
    private func launch(_ arguments: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        var launch = ["-AnistreamScene", "watching"]
        if let media = ProcessInfo.processInfo.environment["ANISTREAM_VIDEO_MEDIA"], !media.isEmpty {
            launch += ["-AnistreamMedia", media]
        }
        app.launchArguments = launch + arguments
        app.launch()
        return app
    }

    /// `element` once it is there, or the screen's elements printed and the flow stopped.
    @discardableResult
    private func find(_ element: XCUIElement, in app: XCUIApplication, timeout: TimeInterval = 10) -> XCUIElement {
        if !element.waitForExistence(timeout: timeout) {
            dump(app, "not found: \(element.description)")
            XCTFail("not found: \(element.description)")
        }
        return element
    }

    private func dump(_ app: XCUIApplication, _ why: String) {
        print("ANISTREAM_VIDEO_DUMP \(why)\n\(app.debugDescription)")
    }

    /// Where the clip starts or ends, for the recorder.
    private func mark(_ edge: String) {
        print("ANISTREAM_VIDEO \(edge) \(String(format: "%.3f", Date().timeIntervalSince1970))")
    }

    /// A person's pause between two steps.
    private func pause(_ seconds: TimeInterval) {
        Thread.sleep(forTimeInterval: seconds)
    }

    /// The art's fade-in after a screen's list arrives.
    private func settle() {
        pause(1.5)
    }
}
