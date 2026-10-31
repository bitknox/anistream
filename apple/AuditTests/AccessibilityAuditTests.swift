// The accessibility audit (ux_part3, Accessibility): each main screen of the Fixtures app, on the
// Watching scene, put through XCUITest's own audit, at the default text size and at the largest.
// Every issue found is printed on a line of its own, "AUDIT <screen>: <type> · <what> · <element>",
// for the report, and fails the test unless it is one of the few set aside below, each with why.

import XCTest

final class AccessibilityAuditTests: XCTestCase {
    override func setUp() {
        continueAfterFailure = true
    }

    func testContinue() throws { try audit("continue") }
    func testContinueLargest() throws { try audit("continue", largest: true) }
    func testTitle() throws { try audit("title", ["-AnistreamTitle", "900001"]) }
    func testTitleLargest() throws { try audit("title", ["-AnistreamTitle", "900001"], largest: true) }
    func testCalendar() throws { try audit("calendar", ["-AnistreamSection", "calendar"]) }
    func testCalendarLargest() throws { try audit("calendar", ["-AnistreamSection", "calendar"], largest: true) }
    func testSeasonal() throws { try audit("seasonal", ["-AnistreamSection", "seasonal"]) }
    func testSeasonalLargest() throws { try audit("seasonal", ["-AnistreamSection", "seasonal"], largest: true) }
    func testLibrary() throws { try audit("library", ["-AnistreamSection", "library"]) }
    func testLibraryLargest() throws { try audit("library", ["-AnistreamSection", "library"], largest: true) }
    // Landed since the first audit: Settings, and the discussion sheet over a title (episode 7's
    // thread comes from the testing engine). Home is the Continue screen above.
    func testSettings() throws { try audit("settings", ["-AnistreamSection", "settings"]) }
    func testSettingsLargest() throws { try audit("settings", ["-AnistreamSection", "settings"], largest: true) }
    func testDiscussion() throws { try audit("discussion", ["-AnistreamTitle", "900001", "-AnistreamSheet", "discussion-7"]) }
    func testDiscussionLargest() throws {
        try audit("discussion", ["-AnistreamTitle", "900001", "-AnistreamSheet", "discussion-7"], largest: true)
    }

    /// Launch on `arguments`, let the engine settle, and audit what is on screen.
    private func audit(_ screen: String, _ arguments: [String] = [], largest: Bool = false) throws {
        let app = XCUIApplication()
        app.launchArguments = ["-AnistreamScene", "watching"] + arguments
        if largest {
            app.launchArguments += ["-UIPreferredContentSizeCategoryName", "UICTContentSizeCategoryAccessibilityXXXL"]
        }
        app.launch()
        // The engine starts, the screen asks for its list and the art fades in.
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 20))
        Thread.sleep(forTimeInterval: 5)
        let name = largest ? "\(screen), largest" : screen
        let tabBar = app.tabBars.firstMatch.exists ? app.tabBars.firstMatch.frame : .null
        let window = app.windows.firstMatch.frame
        try app.performAccessibilityAudit { issue in
            let element = issue.element.map { "\($0.elementType.rawValue) “\($0.label)”" } ?? "none"
            let aside = Self.setAside(issue, under: tabBar, in: window)
            print("AUDIT \(name): \(Self.name(issue.auditType)) · \(issue.compactDescription) · \(element)"
                  + (aside.map { " · set aside: \($0)" } ?? ""))
            return aside != nil
        }
        app.terminate()
    }

    /// Why an issue is the brief's own choice and not a fault, or `nil` for one to fix.
    private static func setAside(_ issue: XCUIAccessibilityAuditIssue, under tabBar: CGRect, in window: CGRect) -> String? {
        let label = issue.element?.label ?? ""
        switch issue.auditType {
        case .dynamicType where issue.compactDescription.contains("partially"):
            // ux_part3's table caps these: show titles at .accessibility2 or 3, metadata at
            // .accessibility1, captions, the week strip and times at .xxxLarge.
            return "capped by the brief"
        case .textClipped where label.count > 120:
            // The synopsis is two lines with More, on the iPhone's page as on the board.
            return "the synopsis, clamped with More"
        case .contrast, .textClipped:
            guard let frame = issue.element?.frame else { return nil }
            // Under the tab bar's glass, which the audit measures through: scrolled out of view.
            if frame.intersects(tabBar) { return "under the tab bar" }
            // A rail's card running off the screen's edge, as a horizontal rail's last one does.
            if frame.maxX > window.maxX + 1 { return "off the rail's edge" }
            return nil
        case .elementDetection where issue.compactDescription.contains("inaccessible text"):
            // The hero's meter and minutes left: hidden, since the title's value says them.
            return "said in the title's value"
        default:
            return nil
        }
    }

    private static func name(_ type: XCUIAccessibilityAuditType) -> String {
        switch type {
        case .contrast: "contrast"
        case .elementDetection: "element detection"
        case .hitRegion: "hit region"
        case .sufficientElementDescription: "description"
        case .dynamicType: "dynamic type"
        case .textClipped: "text clipped"
        case .trait: "trait"
        default: "other (\(type.rawValue))"
        }
    }
}
