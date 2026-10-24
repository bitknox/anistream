// Notices: the capsule that says what the work has to say, and Messages, every notice this
// session.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The latest notice, in a glass capsule at the foot of the window, for a few seconds: longer
/// with a button, for a problem and for many words. Its hint is the button on its trailing side;
/// a click or a swipe down dismisses it, and Messages keeps it (its context menu, the menu bar).
struct NoticeCapsule: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.scenePhase) private var scenePhase
    @AccessibilityFocusState private var focused: Bool
    /// Whether the notices show here: over the iPhone's Settings sheet they show on the sheet,
    /// and not twice.
    var here = true
    /// Whether it sits over the iPhone's Settings sheet, where a hint opens that sheet's page and
    /// Messages cannot come up.
    var inSettingsSheet = false
    /// Whether the pointer is over it: its time stands still.
    @State private var hovering = false
    /// How far a swipe has pulled it down.
    @State private var pull: CGFloat = 0

    /// How long a notice stays up: 4 s, or 8 s with a button or for a problem, and 1 s more for
    /// every 10 words past 12.
    static func duration(_ item: NoticeItem) -> Duration {
        let base = item.isAlert || item.notice.hint != nil ? 8 : 4
        let words = item.notice.text.split(whereSeparator: \.isWhitespace).count
        return .seconds(base + max(0, words - 12) / 10)
    }

    var body: some View {
        ZStack {
            if here, let item = store.notices.current {
                HStack(spacing: 0) {
                    Button {
                        withAnimation(.fadeOut) { store.notices.dismiss(item) }
                    } label: {
                        HStack(spacing: 8) {
                            if item.isAlert {
                                Circle().fill(.vermilion).frame(width: 6, height: 6)
                            }
                            Text(item.notice.text.firstCapitalized)
                                .font(.mac(13))
                                .foregroundStyle(.silk)
                                .lineLimit(2)
                        }
                        .padding(.leading, 16)
                        .padding(.trailing, item.notice.hint != nil || item.isAlert ? 8 : 16)
                        .frame(minHeight: 36)
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    if let hint = item.notice.hint {
                        Button(Self.words(hint)) { act(hint, item) }
                            .buttonStyle(SecondaryCapsuleStyle(height: 24, fontSize: 12))
                            .padding(.trailing, 6)
                    } else if item.isAlert {
                        ReportButton(context: BugContext(kind: "message", message: item.notice.text), height: 24, fontSize: 12)
                            .padding(.trailing, 6)
                    }
                }
                .chromeGlass(in: .capsule)
                .frame(maxWidth: 520)
                .offset(y: pull)
                .opacity(1 - min(pull / 80, 0.6))
                .gesture(swipeDown(item))
                .onHover { hovering = $0 }
                .contextMenu {
                    if !inSettingsSheet {
                        Button("Messages", systemImage: "text.bubble") { navigator.messagesShown = true }
                    }
                }
                .transition(.opacity)
                .accessibilityElement(children: .combine)
                .accessibilityLabel(item.notice.text.firstCapitalized)
                .accessibilityAction(named: "Dismiss") { store.notices.dismiss(item) }
                .accessibilityFocused($focused)
                .task(id: item.id) {
                    pull = 0
                    // The time runs only while nobody is on it: not under the pointer, not under
                    // VoiceOver, and not while the app is inactive.
                    var left = Self.duration(item)
                    let step = Duration.milliseconds(100)
                    while left > .zero, !Task.isCancelled {
                        try? await Task.sleep(for: step)
                        if !focused, !hovering, scenePhase == .active { left -= step }
                    }
                    guard !Task.isCancelled else { return }
                    withAnimation(.fadeOut) { store.notices.dismiss(item) }
                }
            } else if here, let release = store.updates.notice {
                // A newer release, told by the update check's rules: in the capsule's place
                // while nothing else is said, until Update or Not Now. Settings keeps offering it.
                UpdateNotice(release: release)
                    .transition(.opacity)
            }
        }
        .animation(.fadeIn, value: store.notices.current?.id)
        .animation(.fadeIn, value: store.updates.notice?.version)
        .modifier(NoticeAnnouncer(enabled: here))
    }

    /// The hint's button, in the platform's casing.
    static func words(_ hint: NoticeHint) -> String {
        switch hint {
        case .stopPlayback: "Stop"
        case .openStores: cased(mac: "Open Stores", touch: "Open stores")
        case .kitsuCredentials: cased(mac: "Sign In", touch: "Sign in")
        }
    }

    private func act(_ hint: NoticeHint, _ item: NoticeItem) {
        store.notices.dismiss(item)
        switch hint {
        case .stopPlayback:
            _ = store.send(.player(.stop))
        case .openStores:
            #if os(iOS)
            if UIDevice.current.userInterfaceIdiom == .phone {
                navigator.phoneSettings = .stores
                return
            }
            #endif
            navigator.open(.pluginStores)
        case .kitsuCredentials:
            navigator.kitsuShown = true
        }
    }

    /// A swipe down: the capsule follows the finger, and past 30 points it goes.
    private func swipeDown(_ item: NoticeItem) -> some Gesture {
        DragGesture(minimumDistance: 8)
            .onChanged { pull = max(0, $0.translation.height) }
            .onEnded { value in
                if value.translation.height > 30 || value.predictedEndTranslation.height > 60 {
                    withAnimation(.fadeOut) { store.notices.dismiss(item) }
                } else {
                    withAnimation(.standard) { pull = 0 }
                }
            }
    }
}

/// Every notice this session, newest first.
struct MessagesView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Group {
                if store.notices.log.isEmpty {
                    Text("No messages yet")
                        .font(.mac(13))
                        .foregroundStyle(.haze)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    List(store.notices.log) { item in
                        HStack(alignment: .firstTextBaseline, spacing: 8) {
                            Circle()
                                .fill(item.isAlert ? Palette.vermilion : .clear)
                                .frame(width: 6, height: 6)
                            Text(item.notice.text.firstCapitalized)
                                .font(.mac(13))
                                .foregroundStyle(.silk)
                                .textSelection(.enabled)
                            Spacer()
                            if item.isAlert {
                                ReportButton(context: BugContext(kind: "message", message: item.notice.text), height: 24, fontSize: 12)
                            }
                            Text(item.at, style: .time)
                                .font(.mac(12))
                                .foregroundStyle(.haze)
                        }
                        .listRowBackground(Color.clear)
                    }
                    .scrollContentBackground(.hidden)
                }
            }
            .background(.ground)
            .navigationTitle("Messages")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                        .keyboardShortcut(.defaultAction)
                }
            }
        }
        .frame(minWidth: 480, minHeight: 360)
        .closesOnEscape { dismiss() }
    }
}

/// What is running and has run long enough to say so: the 600 ms rule. A small spinner and the
/// newest job's words in the toolbar.
struct ActivityIndicator: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        if let job = store.jobs.shown.first {
            HStack(spacing: 6) {
                ProgressView().controlSize(.mini)
                Text(job.note ?? job.activity.firstCapitalized)
                    .font(.mac(12))
                    .foregroundStyle(.haze)
                    .lineLimit(1)
            }
            .transition(.opacity)
            .accessibilityElement(children: .combine)
        }
    }
}
