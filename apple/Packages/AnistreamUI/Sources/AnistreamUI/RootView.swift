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

    /// A start that failed on `config.toml` offers Reset settings on iPhone and iPad, where
    /// nobody can edit the file. The Mac's is a file in the terminal's folder.
    private var resetsSettings: Bool {
        #if os(macOS)
        false
        #else
        store.settingsUnusable
        #endif
    }

    public var body: some View {
        Group {
            switch store.phase {
            case .failed(let reason):
                StartFailed(
                    title: L.shellStartFailed, message: L.shellStartFailedBody, reason: reason,
                    reset: resetsSettings ? { Task { await store.resetSettings() } } : nil
                ) {
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
/// said, for a bug report. Settings it could not start on can be reset, after a confirmation.
struct StartFailed: View {
    let title: String
    let message: String
    let reason: String
    var reset: (() -> Void)? = nil
    let retry: () -> Void
    @State private var confirmingReset = false

    var body: some View {
        // It scrolls only when larger text makes it taller than the screen.
        ViewThatFits(in: .vertical) {
            content
            ScrollView { content }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(.ground)
        .confirmationDialog(L.settingsResetTitle, isPresented: $confirmingReset, titleVisibility: .visible) {
            Button(L.settingsReset, role: .destructive) { reset?() }
        } message: {
            Text(L.settingsResetDetail)
        }
    }

    private var content: some View {
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
            // Side by side, or stacked when the text is too large for one row.
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 12) { actions }
                VStack(spacing: 12) { actions }
            }
            .padding(.top, 8)
        }
        .padding(24)
        .frame(maxWidth: .infinity)
    }

    @ViewBuilder private var actions: some View {
        Button(L.actionTryAgain, action: retry)
            .buttonStyle(PrimaryCapsuleStyle(height: 28))
        if reset != nil {
            Button(L.settingsReset) { confirmingReset = true }
                .buttonStyle(SecondaryCapsuleStyle())
        }
        ReportButton(context: BugContext(kind: reason.isEmpty ? "stopped" : "start", message: reason))
    }
}
