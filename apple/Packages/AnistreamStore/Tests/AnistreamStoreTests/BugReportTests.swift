// The bug report's address, as the app builds it: the engine's text over this device's facts, and
// every parameter a field of the repository's issue form.

@testable import AnistreamStore
import AnistreamKit
import Foundation
import Testing

private let phone = BugReportDevice(
    appVersion: "2.0.0", platform: "iOS", osVersion: "26.1", device: .phone, build: "release"
)

private func parameters(_ url: URL) -> [String: String] {
    let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
    return Dictionary(uniqueKeysWithValues: items.map { ($0.name, $0.value ?? "") })
}

/// The ids of the inputs in `.github/ISSUE_TEMPLATE/bug_report.yml`.
private func formIds() throws -> Set<String> {
    var url = URL(filePath: #filePath)
    for _ in 0..<6 { url.deleteLastPathComponent() }
    let form = try String(contentsOf: url.appending(path: ".github/ISSUE_TEMPLATE/bug_report.yml"), encoding: .utf8)
    return Set(form.split(separator: "\n").compactMap { line -> String? in
        let line = line.trimmingCharacters(in: .whitespaces)
        return line.hasPrefix("id: ") ? String(line.dropFirst(4)) : nil
    })
}

@Suite struct BugReportTests {
    @Test func theAddressIsAnIssueOnTheTrackerWithTheDiagnosticsFilledIn() {
        let url = BugReports.url(device: phone, providers: [provider("torrents", "torrent")], context: nil)
        #expect(url.host() == "github.com")
        #expect(url.path() == "/bitknox/anistream/issues/new")
        let found = parameters(url)
        #expect(found["template"] == "bug_report.yml")
        #expect(found["title"] == nil)
        let text = found["diagnostics"] ?? ""
        #expect(text.contains("iOS 26.1, phone"))
        #expect(text.contains("Build: release"))
        #expect(text.contains("Last error: none"))
    }

    @Test func everyParameterIsAFieldOfTheIssueForm() throws {
        let ids = try formIds()
        #expect(ids.isSuperset(of: ["what_happened", "steps", "diagnostics"]))
        let context = BugContext(kind: "load", message: "could not load")
        for url in [
            BugReports.url(device: phone, providers: [], context: nil),
            BugReports.url(device: phone, providers: [], context: context),
        ] {
            for name in parameters(url).keys where !["template", "title"].contains(name) {
                #expect(ids.contains(name), "\(name) is not a field of the form")
            }
        }
    }

    @Test func noSourceAddressOrTitleReachesTheAddress() {
        let moon = provider("moonlit")
        let context = BugContext(
            kind: "load",
            message: "moonlit failed for Lantern Harbor at https://cdn.moon.example/v?t=1 from 10.0.0.2",
            title: "Lantern Harbor"
        )
        let url = BugReports.url(device: phone, providers: [moon, provider("lantern")], context: context)
        let text = parameters(url).values.joined(separator: "\n")
        for leak in ["moonlit", "Lantern", "moon.example", "10.0.0.2"] {
            #expect(!text.contains(leak), "\(leak) in \(text)")
        }
        #expect(text.contains("2 plugin sources"))
        #expect(parameters(url)["title"] == "Bug: load")
        #expect(url.absoluteString.utf8.count <= 7_500)
    }

    @Test func aLongErrorIsTrimmedToFit() {
        let context = BugContext(kind: "load", message: String(repeating: "word ", count: 5000))
        #expect(BugReports.url(device: phone, providers: [], context: context).absoluteString.utf8.count <= 7_500)
    }
}
