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
    /// Continue to a title page, on to its next season and back: the hero's art zooms into the
    /// page (T1), Season 2 is pushed over it, and two steps back the page zooms home into the
    /// art, where the loop starts again.
    func testTitle() throws {
        let app = launch([])
        let hero = find(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Stardust Wayfarer,'")).firstMatch,
                        in: app, timeout: 30)
        settle()
        mark("start")
        pause(0.2)
        // The hero's art, above its title block, which is the art's one way in by touch.
        hero.coordinate(withNormalizedOffset: CGVector(dx: 0, dy: 0)).withOffset(CGVector(dx: 185, dy: -104)).tap()
        let season = find(app.buttons["Season 2"], in: app)
        _ = find(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Resume episode 7'")).firstMatch, in: app)
        pause(0.4)
        season.tap()
        _ = find(app.staticTexts["Stardust Wayfarer Season 2"], in: app)
        pause(0.6)
        back(in: app)
        _ = find(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Resume episode 7'")).firstMatch, in: app)
        pause(0.4)
        back(in: app)
        _ = find(hero, in: app)
        pause(0.8)
        mark("end")
    }

    /// The week's calendar, its estimated times and Dub marks, then a show's page from it and
    /// back: the day picked scrolls the week, the row's cover zooms into the page (T1) and home
    /// again, and today scrolls the week back to where the loop starts.
    func testCalendar() throws {
        let app = launch(["-AnistreamSection", "calendar", "-AnistreamShowcase", "calendar"])
        let wednesday = find(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Wednesday '")).firstMatch,
                             in: app, timeout: 30)
        let monday = find(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Monday '")).firstMatch, in: app)
        settle()
        mark("start")
        pause(0.2)
        wednesday.tap()
        pause(0.6)
        // Wednesday's row, now in view under its heading.
        let rows = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'The Library Above the Clouds,'"))
        let row = rows.allElementsBoundByIndex.last { $0.isHittable && $0.frame.minY > 200 } ?? rows.firstMatch
        row.tap()
        _ = find(app.buttons["More about this show"].firstMatch, in: app)
        pause(0.6)
        back(in: app)
        _ = find(monday, in: app)
        pause(0.6)
        monday.tap()
        pause(0.9)
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
        pause(0.4)
        bedroom.tap()
        // The remote, with the episode playing on the TV.
        _ = find(app.buttons["Disconnect"], in: app, timeout: 15)
        pause(0.6)
        // The remote's own Pause, in its row of transport buttons: the page's card under the
        // sheet has one too, and a tap there lands on the sheet's Choose another TV.
        let transport = find(app.buttons["Back 10 seconds"].firstMatch, in: app).frame.midY
        let pauses = app.buttons.matching(NSPredicate(format: "label == 'Pause'")).allElementsBoundByIndex
        if let remote = pauses.first(where: { abs($0.frame.midY - transport) < 4 }) {
            remote.tap()
        } else {
            dump(app, "no Pause on the remote")
            XCTFail("no Pause on the remote")
        }
        pause(0.9)
        mark("end")
    }

    /// The navigation bar's back button: one step back.
    private func back(in app: XCUIApplication) {
        find(app.navigationBars.buttons.firstMatch, in: app).tap()
    }
    #endif

    #if os(macOS)
    /// Find something to watch on the Mac: Trending Now in full, the filters panel sliding in as
    /// the covers move to fewer columns, Fantasy taking half of them away, then a result's page
    /// and its other season. The test kit's catalogue fills the grid: fourteen titles, seven of
    /// them Fantasy.
    func testSearchOnMac() throws {
        let app = launch(["-AnistreamCatalogue", "-AnistreamSection", "search"])
        frame(of: app)
        let seeAll = ready(app.buttons["See All"].firstMatch, in: app)
        rest(at: CGPoint(x: 784, y: 616), in: app)
        mark("start")
        pause(0.2)
        press(seeAll, in: app)
        // The rails gone and the grid crossfaded in.
        gone(app.buttons["See All"].firstMatch, in: app)
        let filters = ready(app.buttons["Filters"].firstMatch, in: app)
        pause(0.4)
        press(filters, in: app)
        // The panel slid in and the covers took their new columns, standing still again.
        let panel = app.scrollViews.containing(NSPredicate(format: "label == 'Action'")).firstMatch
        let fantasy = ready(panel.buttons["Fantasy"].firstMatch, in: app)
        pause(0.2)
        press(fantasy, in: app)
        // The grid without the titles that are not Fantasy, crossfaded in.
        gone(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'The Paper Lantern Festival'")).firstMatch, in: app)
        let result = ready(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Stardust Wayfarer Season 2'")).firstMatch, in: app)
        pause(0.4)
        press(result, in: app)
        let season = ready(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Season 1,'")).firstMatch, in: app)
        pause(0.3)
        press(season, in: app)
        pause(0.9)
        mark("end")
    }

    /// Continue on the Mac: another title picked from the queue, its preview crossfading in (M2),
    /// then its banner growing into the title page (M3), and the page moving to the next season.
    func testContinueOnMac() throws {
        let app = launch(["-AnistreamSelect", "900002"])
        frame(of: app)
        let row = ready(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Stardust Wayfarer,'")).firstMatch, in: app)
        rest(at: CGPoint(x: 560, y: 520), in: app)
        mark("start")
        pause(0.3)
        press(row, in: app)
        pause(0.3)
        press(ready(app.buttons["Episodes"].firstMatch, in: app), in: app)
        let season = ready(app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Season 2,'")).firstMatch, in: app)
        pause(0.5)
        press(season, in: app)
        pause(1.0)
        mark("end")
    }

    /// `element` once it stands still inside the window and takes a click: a panel sliding in or
    /// a page fading in has finished moving. Never found or never still, the screen's elements
    /// are printed and the flow stopped.
    @discardableResult
    private func ready(_ element: XCUIElement, in app: XCUIApplication, timeout: TimeInterval = 10) -> XCUIElement {
        find(element, in: app, timeout: timeout)
        let window = app.windows.firstMatch.frame
        let deadline = Date().addingTimeInterval(timeout)
        var last = CGRect.null
        while Date() < deadline {
            let frame = element.frame
            if frame == last, window.contains(frame) { return element }
            last = frame
            Thread.sleep(forTimeInterval: 0.1)
        }
        dump(app, "never still in the window: \(element.description) at \(element.frame)")
        XCTFail("never still in the window: \(element.description)")
        return element
    }

    /// Once `element` has left the screen: what a crossfade replaced has finished fading out.
    private func gone(_ element: XCUIElement, in app: XCUIApplication, timeout: TimeInterval = 10) {
        // Polled, not an expectation: those look once a second, a second of still screen.
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if !element.exists { return }
            Thread.sleep(forTimeInterval: 0.05)
        }
        dump(app, "never gone: \(element.description)")
        XCTFail("never gone: \(element.description)")
    }

    /// A click at the element's centre. On the Mac, XCUITest reports buttons inside a SwiftUI
    /// scroll view as not hittable and tries to scroll them into view, which fails, so the click
    /// goes to the point instead. The recording leaves the real pointer out: the click is printed,
    /// "ANISTREAM_VIDEO click <x> <y> <time>" in the window's points, and the encode draws a
    /// pointer of its own moving there.
    private func press(_ element: XCUIElement, in app: XCUIApplication) {
        let window = app.windows.firstMatch
        let origin = window.frame.origin
        let centre = CGPoint(x: element.frame.midX - origin.x, y: element.frame.midY - origin.y)
        cursor("click", centre)
        window.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: centre.x, dy: centre.y)).click()
    }

    /// The real pointer moved to a point with nothing under it to light up, where the drawn one
    /// starts the clip.
    private func rest(at point: CGPoint, in app: XCUIApplication) {
        let window = app.windows.firstMatch
        window.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: point.x, dy: point.y)).hover()
        cursor("rest", point)
        settle()
    }

    /// A key about to be typed: the drawn pointer hides while the keyboard is in use, as the
    /// system's does.
    private func typed() {
        print("ANISTREAM_VIDEO type 0 0 \(String(format: "%.3f", Date().timeIntervalSince1970))")
    }

    private func cursor(_ kind: String, _ point: CGPoint) {
        print("ANISTREAM_VIDEO \(kind) \(String(format: "%.1f %.1f %.3f", point.x, point.y, Date().timeIntervalSince1970))")
    }

    /// The window once it is up and settled, its frame and the screen's printed for the crop. A
    /// launch can come up with no window at all, only the menu bar: Window › Main Window (⌘0)
    /// brings it, from the keyboard, so the pointer stays where it is.
    private func frame(of app: XCUIApplication) {
        let window = app.windows.firstMatch
        if !window.waitForExistence(timeout: 10) {
            print("ANISTREAM_VIDEO no window after launch; asking for the main window")
            app.activate()
            app.typeKey("0", modifierFlags: .command)
        }
        XCTAssertTrue(window.waitForExistence(timeout: 20), "the app opened no window")
        settle()
        let screen = NSScreen.main?.frame ?? .zero
        let frame = window.frame
        print("ANISTREAM_VIDEO window \(frame.minX) \(frame.minY) \(frame.width) \(frame.height) screen \(screen.width) \(screen.height)")
    }
    #endif

    /// Explore a screen: launch on ANISTREAM_VIDEO_ARGS and print what is on it. For writing a
    /// flow, never recorded.
    func testExplore() throws {
        let arguments = ProcessInfo.processInfo.environment["ANISTREAM_VIDEO_ARGS"]?
            .split(separator: " ").map(String.init) ?? []
        let app = launch(arguments)
        #if os(macOS)
        frame(of: app)
        #endif
        Thread.sleep(forTimeInterval: 6)
        print(app.debugDescription)
    }

    // MARK: - Helpers

    /// The Fixtures app on the Watching scene, with the episode the recorder names as every
    /// episode's stream.
    private func launch(_ arguments: [String]) -> XCUIApplication {
        let app = XCUIApplication()
        var launch = ["-AnistreamScene", "watching"]
        #if os(macOS)
        // A window of its own each time, never the state the last run left, as the Mac player's
        // UI tests launch it.
        launch += ["-ApplePersistenceIgnoreState", "YES"]
        // The window at the app's default size, 1280 by 800, in the middle of the screen below the
        // menu bar: AppKit's saved frame, given for this launch only.
        if let screen = NSScreen.main?.visibleFrame {
            let x = Int(screen.minX + (screen.width - 1280) / 2), y = Int(screen.minY + (screen.height - 800) / 2)
            // Then the screen's visible frame, as AppKit writes it: "127 239 1048 652 0 0 1512 949".
            let visible = "\(Int(screen.minX)) \(Int(screen.minY)) \(Int(screen.width)) \(Int(screen.height))"
            launch += ["-NSWindow Frame main", "\(x) \(y) 1280 800 \(visible)"]
        }
        #endif
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
