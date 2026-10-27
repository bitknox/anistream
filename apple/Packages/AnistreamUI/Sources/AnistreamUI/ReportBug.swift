// "Report a bug": the entry points share one way to open the pre-filled GitHub issue. It opens in
// the browser, where the reporter signs in to GitHub, writes what happened and submits.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// Opens the bug report for this device, with an error when the report starts from one.
@MainActor
struct ReportBugAction {
    let store: AppStore
    let openURL: OpenURLAction

    func callAsFunction(_ context: BugContext? = nil) {
        openURL(store.bugReportURL(context: context ?? lastProblem))
    }

    /// The newest problem the app has told of this session, for a report started from a menu.
    var lastProblem: BugContext? {
        store.notices.log.first { $0.isAlert }.map { BugContext(kind: "message", message: $0.notice.words) }
    }
}

/// A "Report" capsule beside an error: it opens the bug report with that error in it.
struct ReportButton: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL
    let context: BugContext
    var height: CGFloat = 28
    var fontSize: CGFloat = 13
    /// On a raised sheet, where the capsule needs the lighter fill to show.
    var raisedSurface = false

    var body: some View {
        Button("Report") { ReportBugAction(store: store, openURL: openURL)(context) }
            .buttonStyle(SecondaryCapsuleStyle(height: height, raisedSurface: raisedSurface, fontSize: fontSize))
            .accessibilityLabel("Report this problem")
            .accessibilityHint("Opens a bug report on GitHub in your browser")
    }
}

/// Settings › About on the Mac and iPad: the version, and the bug report.
struct AboutGroup: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL

    var body: some View {
        SettingsGroup(
            title: "About",
            footnote: "Opens a bug report on GitHub. You sign in there. It carries your app and device versions and the last error, with no addresses, file paths or titles."
        ) {
            SettingsRow(title: "Report a bug") {
                Button("Report") { ReportBugAction(store: store, openURL: openURL)() }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
        }
    }
}
