// The Mac player's controls, clicked as a person clicks them: the pointer moved onto the control
// and the button pressed there, through the window server, where the window's title bar and its
// background can take a click before the control does. Clicks the code sends a model never
// meet either, so the player's earlier checks missed both.
//
// The Fixtures app plays the Playing scene's episode, or the file TEST_RUNNER_ANISTREAM_MEDIA
// names, in the main window; each test checks what the click did to the window, not only to the
// model:
//
//   xcodebuild -project apple/Anistream.xcodeproj -scheme "Anistream Mac Player" \
//     -destination platform=macOS test
//
// macOS asks once for the Mac's password to let XCTest drive apps (automation mode), unless
// `automationmodetool enable-automationmode-without-authentication` was run.

import AppKit
import XCTest

@MainActor
final class MacPlayerClickTests: XCTestCase {
    // MARK: In a window, the controls up

    func testBackClosesThePlayer() throws {
        let app = launch()
        defer { app.terminate() }
        click(app, "Back")
        XCTAssertTrue(browsing(app), "the player stayed up after Back")
        XCTAssertFalse(isFullScreen(app))
    }

    func testFullScreenButtonEntersAndLeavesFullScreen() throws {
        let app = launch()
        defer { app.terminate() }
        click(app, "Enter Full Screen")
        XCTAssertTrue(waitFor { self.isFullScreen(app) }, "the window did not go full screen")
        click(app, "Exit Full Screen")
        XCTAssertTrue(waitFor { !self.isFullScreen(app) }, "the window stayed full screen")
        XCTAssertTrue(button(app, "Enter Full Screen").exists, "the player closed")
    }

    func testPictureInPictureButtonOpensAndClosesTheFloatingWindow() throws {
        let app = launch()
        defer { app.terminate() }
        click(app, "Picture in Picture")
        XCTAssertTrue(waitFor { self.floatingWindow() != nil }, "no Picture in Picture window")
        XCTAssertTrue(gone(app, "Enter Full Screen"), "the player stayed in the main window")
        // Back by the Window menu, clicked as the rest are.
        returnFromPictureInPicture(app)
        XCTAssertTrue(waitFor { self.floatingWindow() == nil }, "the Picture in Picture window stayed up")
        XCTAssertTrue(button(app, "Enter Full Screen").waitForExistence(timeout: 10), "the picture did not come back")
    }

    func testTheBarsControlsTakeClicks() throws {
        let app = launch()
        defer { app.terminate() }
        // Paused first, so the minute-long episode stays where it is while the rest is clicked.
        click(app, "Pause")
        XCTAssertTrue(button(app, "Play").waitForExistence(timeout: 5), "Pause did not pause")

        // The clock beside the buttons, as it shows: the seek line's spoken value does not
        // change in the Mac's accessibility tree while the picture is paused.
        let elapsed = app.staticTexts["Elapsed"]
        XCTAssertTrue(elapsed.waitForExistence(timeout: 5), "no elapsed time")
        let shown = { "\(elapsed.label)\(elapsed.value as? String ?? "")" }
        let before = shown()
        XCTAssertFalse(before.isEmpty, "the elapsed time shows nothing")
        click(app, "Forward 10 seconds")
        XCTAssertTrue(waitFor { shown() != before }, "Forward 10 seconds did not seek")

        click(app, "Mute")
        XCTAssertTrue(button(app, "Unmute").waitForExistence(timeout: 5), "Mute did not mute")
        click(app, "Unmute")
        XCTAssertTrue(button(app, "Mute").waitForExistence(timeout: 5))

        // The speed's menu: the speed set outright.
        click(app, "Playback speed, 1.0×")
        XCTAssertTrue(button(app, "1.5×").waitForExistence(timeout: 5), "no speed menu")
        XCTAssertFalse(app.staticTexts["Upscaling"].exists, "the speed menu holds upscaling")
        click(app, "1.5×")
        XCTAssertTrue(button(app, "Playback speed, 1.5×").waitForExistence(timeout: 5), "1.5× was not picked")
        click(app, "Playback speed, 1.5×")
        XCTAssertTrue(gone(app, "1.5×"), "the speed menu stayed open")

        // Picture and sound: upscaling and night mode.
        click(app, "Picture and sound")
        XCTAssertTrue(app.staticTexts["Upscaling"].waitForExistence(timeout: 5), "no picture and sound menu")
        click(app, "Night mode")
        XCTAssertTrue(waitFor { self.button(app, "Night mode").isSelected }, "night mode did not go on")
        click(app, "Picture and sound")
        XCTAssertTrue(gone(app, "Upscaling", in: app.staticTexts), "the picture and sound menu stayed open")

        // The top bar's menus: subtitles and audio, and episodes.
        click(app, "Subtitles and audio")
        XCTAssertTrue(app.staticTexts["Remembered for this show"].waitForExistence(timeout: 5), "no subtitles menu")
        click(app, "Off")
        XCTAssertTrue(waitFor { self.button(app, "Off").isSelected }, "Off was not picked")
        click(app, "Subtitles and audio")
        XCTAssertTrue(gone(app, "Remembered for this show", in: app.staticTexts))
        click(app, "Episodes")
        let last = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Episode 12'")).firstMatch
        XCTAssertTrue(last.waitForExistence(timeout: 5), "no episodes menu")
        XCTAssertTrue(app.staticTexts["Source"].exists, "no sources under the episode playing")
        click(app, "Episodes")
        XCTAssertTrue(waitFor { !last.exists }, "the episodes menu stayed open")

        click(app, "Play")
        XCTAssertTrue(button(app, "Pause").waitForExistence(timeout: 5), "Play did not play")
    }

    func testSkipAndUpNextTakeClicks() throws {
        let skip = launch(shot: "skip")
        click(skip, "Skip Opening")
        XCTAssertTrue(gone(skip, "Skip Opening"), "Skip Opening did not skip")
        skip.terminate()

        let next = launch(shot: "up-next")
        defer { next.terminate() }
        click(next, "Cancel")
        XCTAssertTrue(gone(next, "Play Now"), "Cancel left Up Next up")
    }

    // MARK: The controls brought back by the pointer

    func testBackAfterTheControlsFade() throws {
        let app = launch(shot: "fading")
        defer { app.terminate() }
        waitForTheControlsToFade(app)
        wakeTheControls(app)
        click(app, "Back")
        XCTAssertTrue(browsing(app), "the player stayed up after Back")
    }

    func testFullScreenAfterTheControlsFade() throws {
        let app = launch(shot: "fading")
        defer { app.terminate() }
        waitForTheControlsToFade(app)
        wakeTheControls(app)
        click(app, "Enter Full Screen")
        XCTAssertTrue(waitFor { self.isFullScreen(app) }, "the window did not go full screen")
        waitForTheControlsToFade(app)
        wakeTheControls(app)
        click(app, "Exit Full Screen")
        XCTAssertTrue(waitFor { !self.isFullScreen(app) }, "the window stayed full screen")
    }

    func testPictureInPictureAfterTheControlsFade() throws {
        let app = launch(shot: "fading")
        defer { app.terminate() }
        waitForTheControlsToFade(app)
        wakeTheControls(app)
        click(app, "Picture in Picture")
        XCTAssertTrue(waitFor { self.floatingWindow() != nil }, "no Picture in Picture window")
        returnFromPictureInPicture(app)
        XCTAssertTrue(waitFor { self.floatingWindow() == nil }, "the Picture in Picture window stayed up")
    }

    // MARK: In full screen

    func testTheControlsInFullScreen() throws {
        let app = launch(shot: "fading")
        defer { app.terminate() }
        wakeTheControls(app)
        click(app, "Enter Full Screen")
        XCTAssertTrue(waitFor { self.isFullScreen(app) }, "the window did not go full screen")

        // The top bar in full screen, where the menu bar drops in over it.
        waitForTheControlsToFade(app)
        wakeTheControls(app)
        click(app, "Subtitles and audio")
        XCTAssertTrue(app.staticTexts["Remembered for this show"].waitForExistence(timeout: 5), "no subtitles menu")
        click(app, "Subtitles and audio")

        // Picture in Picture from full screen: the window leaves full screen with the player.
        waitForTheControlsToFade(app)
        wakeTheControls(app)
        click(app, "Picture in Picture")
        XCTAssertTrue(waitFor { self.floatingWindow() != nil }, "no Picture in Picture window")
        returnFromPictureInPicture(app)
        // The picture comes back as it left, full screen.
        XCTAssertTrue(waitFor { self.isFullScreen(app) }, "the picture did not come back full screen")
        waitForTheControlsToFade(app)
        wakeTheControls(app)
        click(app, "Back")
        XCTAssertTrue(browsing(app), "the player stayed up after Back in full screen")
        XCTAssertTrue(waitFor { !self.isFullScreen(app) }, "the window stayed full screen after the player")
    }

    /// In and out by a double click on the video and by the player's button, mixed: each way
    /// out gives the window back at the size it went in with, the title strip along its top and
    /// the control row along its bottom. A full screen change started from the double click
    /// once left the content view the size of neither, the video in a box at the window's
    /// corner and the title strip in mid-window.
    func testFullScreenBackAndForthKeepsThePlayerInTheWindow() throws {
        let app = launch()
        defer { app.terminate() }
        let before = try XCTUnwrap(largestWindow(app)?.1)
        for (into, outOf) in [("double click", "button"), ("button", "double click"), ("double click", "double click")] {
            enterOrLeave(app, by: into)
            XCTAssertTrue(waitFor { self.isFullScreen(app) }, "the window did not go full screen by \(into)")
            Thread.sleep(forTimeInterval: 1)
            assertFillsTheWindow(app, "in full screen by \(into)")
            enterOrLeave(app, by: outOf)
            XCTAssertTrue(waitFor { !self.isFullScreen(app) }, "the window stayed full screen after \(outOf)")
            Thread.sleep(forTimeInterval: 1)
            let after = try XCTUnwrap(largestWindow(app)?.1)
            XCTAssertEqual(after.size, before.size, "the window came back another size after \(outOf)")
            assertFillsTheWindow(app, "out of full screen by \(outOf)")
        }
    }

    // MARK: Helpers

    /// Full screen toggled by a double click on the middle of the video, or by the player's own
    /// button.
    private func enterOrLeave(_ app: XCUIApplication, by way: String) {
        wakeTheControls(app)
        if way == "double click" {
            mainWindow(app).coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).doubleClick()
        } else {
            click(app, isFullScreen(app) ? "Exit Full Screen" : "Enter Full Screen")
        }
    }

    /// Back at the window's top and Full Screen at its bottom: the player laid out over all of
    /// the window, not in a box of another size.
    private func assertFillsTheWindow(_ app: XCUIApplication, _ when: String) {
        wakeTheControls(app)
        guard let window = largestWindow(app)?.1 else { return XCTFail("no window \(when)") }
        let back = button(app, "Back")
        let full = app.buttons.matching(NSPredicate(format: "label ENDSWITH 'Full Screen'")).firstMatch
        XCTAssertTrue(back.waitForExistence(timeout: 5) && full.waitForExistence(timeout: 5), "no controls \(when)")
        XCTAssertLessThan(back.frame.minY - window.minY, 60, "the title strip is not along the top \(when)")
        XCTAssertLessThan(window.maxY - full.frame.maxY, 52, "the control row is not along the bottom \(when)")
        XCTAssertLessThan(window.maxX - full.frame.maxX, 52, "the control row does not reach the right \(when)")
    }

    /// The Fixtures app on the Playing scene, the player up on the episode: its controls pinned up
    /// for `playing`, left to fade for `fading`.
    private func launch(shot: String = "playing") -> XCUIApplication {
        continueAfterFailure = false
        let app = XCUIApplication()
        app.launchArguments = [
            "-AnistreamScene", "playing", "-AnistreamPlayerShot", shot,
            // A window of its own size each time, not the last run's, and never full screen.
            "-ApplePersistenceIgnoreState", "YES",
        ]
        if let media = ProcessInfo.processInfo.environment["ANISTREAM_MEDIA"], !media.isEmpty {
            app.launchArguments += ["-AnistreamMedia", media]
        }
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 30))
        // The player's controls, there once the shot has played the episode to its place: the
        // pointer brings them up, should they have faded by then.
        let first = button(app, shot == "skip" ? "Skip Opening" : shot == "up-next" ? "Play Now" : "Enter Full Screen")
        let up = waitFor(timeout: 90) {
            if first.exists { return true }
            if shot == "fading", self.mainWindow(app).exists { self.wakeTheControls(app) }
            return first.exists
        }
        XCTAssertTrue(up, "the player did not come up")
        // The shot's own pauses: its fades, and the first frames.
        Thread.sleep(forTimeInterval: 1.5)
        return app
    }

    private func button(_ app: XCUIApplication, _ label: String) -> XCUIElement {
        app.buttons[label].firstMatch
    }

    /// Move the pointer onto the control and click it there, as a person does.
    private func click(_ app: XCUIApplication, _ label: String) {
        let control = button(app, label)
        XCTAssertTrue(control.waitForExistence(timeout: 10), "no \(label)")
        control.hover()
        control.click()
    }

    private func gone(_ app: XCUIApplication, _ label: String, in query: XCUIElementQuery? = nil) -> Bool {
        let element = (query ?? app.buttons)[label].firstMatch
        return waitFor { !element.exists }
    }

    /// The main window back to browsing: its toolbar's search field, which the player hides.
    /// Not the player's buttons gone, which they are too while its controls are faded.
    private func browsing(_ app: XCUIApplication) -> Bool {
        app.searchFields.firstMatch.waitForExistence(timeout: 10)
    }

    /// Two seconds without input, and the controls fade (M5).
    private func waitForTheControlsToFade(_ app: XCUIApplication) {
        let back = button(app, "Back")
        let faded = waitFor(timeout: 8) { !back.exists || !back.isHittable }
        XCTAssertTrue(faded, "the controls did not fade")
        Thread.sleep(forTimeInterval: 0.5)
    }

    /// The pointer moved over the middle of the video, which brings the controls back.
    private func wakeTheControls(_ app: XCUIApplication) {
        let window = mainWindow(app)
        window.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.45)).hover()
        window.coordinate(withNormalizedOffset: CGVector(dx: 0.52, dy: 0.5)).hover()
        // fade-in, 0.2 s.
        Thread.sleep(forTimeInterval: 0.4)
    }

    /// The main window: the largest of the app's, not the borderless one Picture in Picture's
    /// layer sits in.
    private func mainWindow(_ app: XCUIApplication) -> XCUIElement {
        guard let (index, _) = largestWindow(app) else { return app.windows.firstMatch }
        return app.windows.element(boundBy: index)
    }

    /// The largest window's place among the app's and its frame, read from one snapshot: a
    /// window that closes while they are read, as one does going in and out of full screen and
    /// Picture in Picture, cannot fail the read.
    private func largestWindow(_ app: XCUIApplication) -> (Int, CGRect)? {
        let windows = (try? app.snapshot())?.children.filter { $0.elementType == .window } ?? []
        let area = { (frame: CGRect) in frame.width * frame.height }
        return windows.enumerated().max { area($0.element.frame) < area($1.element.frame) }
            .map { ($0.offset, $0.element.frame) }
    }

    /// Whether the app's main window fills its screen, as a full-screen window does: below the
    /// camera housing, where a screen has one.
    private func isFullScreen(_ app: XCUIApplication) -> Bool {
        guard let (_, window) = largestWindow(app) else { return false }
        return NSScreen.screens.contains { screen in
            abs(window.width - screen.frame.width) < 1
                && window.height >= screen.frame.height - screen.safeAreaInsets.top - 1
        }
    }

    /// The system's Picture in Picture window, wherever it is: drawn by its own process, named
    /// so.
    private func floatingWindow() -> CGRect? {
        let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]] ?? []
        for window in windows {
            guard (window[kCGWindowOwnerName as String] as? String) == "Picture in Picture",
                let bounds = window[kCGWindowBounds as String] as? [String: CGFloat],
                let width = bounds["Width"], let height = bounds["Height"], width > 100, height > 60
            else { continue }
            return CGRect(x: bounds["X"] ?? 0, y: bounds["Y"] ?? 0, width: width, height: height)
        }
        return nil
    }

    /// Window › Picture in Picture, clicked in the menu bar.
    private func returnFromPictureInPicture(_ app: XCUIApplication) {
        let window = app.menuBars.menuBarItems["Window"]
        window.click()
        let item = window.menuItems["Picture in Picture"]
        XCTAssertTrue(item.waitForExistence(timeout: 5))
        item.click()
    }

    private func waitFor(timeout: TimeInterval = 10, _ condition: () -> Bool) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() { return true }
            Thread.sleep(forTimeInterval: 0.2)
        }
        return condition()
    }
}
