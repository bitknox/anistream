// The app's window: the Mac's split view, or the tab bar on iPad and iPhone.

import AnistreamDesign
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// What the app's window shows, over the store in the environment.
///
/// The app hands it the store and a `Navigator` in the environment.
public struct RootView: View {
    @Environment(AppStore.self) private var store

    public init() {}

    public var body: some View {
        Group {
            switch store.phase {
            case .failed(let reason):
                StartFailed(title: L.shellStartFailed, message: L.shellStartFailedBody, reason: reason) {
                    Task { await store.restart() }
                }
            case .stopped:
                StartFailed(title: L.shellEngineStopped, message: L.shellEngineStoppedBody, reason: "") {
                    Task { await store.restart() }
                }
            default:
                #if os(macOS)
                MacShell()
                #else
                TabShell()
                #endif
            }
        }
        // The widgets' taps: a title, an episode, the Calendar, Accounts.
        .opensWidgetLinks()
        // The Mac's controls in amber, the accent. On iPhone and iPad toolbar text actions are
        // silk, as the brief has it: amber is only ever the one primary action, which sets its
        // own, and a switch that is on.
        #if os(macOS)
        .tint(.amber)
        #else
        .tint(.silk)
        #endif
        .preferredColorScheme(.dark)
        .modifier(SpotlightResults())
        .modifier(TrailerPresentation())
        // A pinned source the engine could not use, said once in the notice capsule.
        .onChange(of: store.sourceChoices.pinNotice) { _, notice in
            if let notice { store.say(L.sourcePinUnavailable(name: notice.name)) }
        }
    }
}

/// The engine did not start, or stopped: what happened in plain words, Try again, and what it
/// said, for a bug report.
struct StartFailed: View {
    let title: String
    let message: String
    let reason: String
    let retry: () -> Void

    var body: some View {
        VStack(spacing: 8) {
            Text(title)
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            Text(message)
                .font(.mac(13))
                .foregroundStyle(.silk)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 480)
            if !reason.isEmpty {
                Text(reason)
                    .font(.mac(12))
                    .foregroundStyle(.haze)
                    .multilineTextAlignment(.center)
                    .textSelection(.enabled)
                    .frame(maxWidth: 480)
            }
            HStack(spacing: 12) {
                Button(L.actionTryAgain, action: retry)
                    .buttonStyle(.borderedProminent)
                ReportButton(context: BugContext(kind: reason.isEmpty ? "stopped" : "start", message: reason))
            }
            .padding(.top, 8)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.ground)
    }
}
