// Notices: the capsule that says what the work has to say, and Messages, every notice this
// session.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The latest notice, in a glass capsule at the foot of the window, for a few seconds: longer for
/// a problem. Clicking it opens Messages.
struct NoticeCapsule: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @AccessibilityFocusState private var focused: Bool
    /// Whether the notices show here: over the iPhone's Settings sheet they show on the sheet,
    /// and not twice.
    var here = true
    /// Whether a tap opens Messages, which a sheet over the window cannot.
    var opensMessages = true

    /// How long a notice stays up.
    static func duration(_ item: NoticeItem) -> Duration {
        item.isAlert ? .seconds(8) : .seconds(4)
    }

    var body: some View {
        ZStack {
            if here, let item = store.notices.current {
                HStack(spacing: 0) {
                    Button {
                        if opensMessages { navigator.messagesShown = true }
                    } label: {
                        HStack(spacing: 8) {
                            if item.isAlert {
                                Circle().fill(.vermilion).frame(width: 6, height: 6)
                            }
                            Text(item.notice.text.capitalizedSentence)
                                .font(.mac(13))
                                .foregroundStyle(.silk)
                                .lineLimit(2)
                        }
                        .padding(.leading, 16)
                        .padding(.trailing, item.isAlert ? 8 : 16)
                        .frame(minHeight: 36)
                    }
                    .buttonStyle(.plain)
                    if item.isAlert {
                        ReportButton(context: BugContext(kind: "message", message: item.notice.text), height: 24, fontSize: 12)
                            .padding(.trailing, 6)
                    }
                }
                .chromeGlass(in: .capsule)
                .frame(maxWidth: 520)
                .transition(.opacity)
                .accessibilityLabel(item.notice.text.capitalizedSentence)
                .accessibilityHint(opensMessages ? "Opens Messages" : "")
                .accessibilityFocused($focused)
                .task(id: item.id) {
                    try? await Task.sleep(for: Self.duration(item))
                    // It stays while VoiceOver is on it.
                    while focused, !Task.isCancelled {
                        try? await Task.sleep(for: .seconds(1))
                    }
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
}

/// Every notice this session, newest first.
struct MessagesView: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        #if os(macOS)
        // One surface, as the Mac's other sheets are: its title, the notices, and Done, with no
        // toolbar bands above and below and no list chrome between.
        VStack(alignment: .leading, spacing: 0) {
            Text("Messages")
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
                .padding(.horizontal, 24)
                .padding(.top, 22)
                .padding(.bottom, 10)
            if store.notices.log.isEmpty {
                empty
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(store.notices.log) { item in
                            MessageRow(item: item)
                                .padding(.vertical, 10)
                                .overlay(alignment: .top) {
                                    if item.id != store.notices.log.first?.id {
                                        Rectangle().fill(.faintRule).frame(height: 1)
                                    }
                                }
                        }
                    }
                    .padding(.horizontal, 24)
                }
            }
            HStack {
                Spacer()
                Button("Done") { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    .keyboardShortcut(.defaultAction)
            }
            .padding(.horizontal, 24)
            .padding(.top, 12)
            .padding(.bottom, 20)
        }
        .frame(width: 520, height: 440)
        .background(.raised)
        .closesOnEscape { dismiss() }
        #else
        NavigationStack {
            Group {
                if store.notices.log.isEmpty {
                    empty
                } else {
                    PhoneList {
                        ForEach(store.notices.log) { item in
                            MessageRow(item: item)
                                .listRowBackground(Palette.raised)
                        }
                    }
                }
            }
            .background(.ground)
            .navigationTitle("Messages")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                        .fontWeight(.semibold)
                        .tint(.silk)
                        .keyboardShortcut(.defaultAction)
                }
            }
        }
        #endif
    }

    private var empty: some View {
        Text("No messages yet")
            .font(Self.font(13))
            .foregroundStyle(.haze)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    /// The Mac's size, or the larger one a phone reads at.
    static func font(_ mac: CGFloat) -> Font {
        #if os(macOS)
        .mac(mac)
        #else
        .ui(mac + 4)
        #endif
    }
}

/// One notice in Messages: a dot for a problem, the words, Report for a problem, and the time.
private struct MessageRow: View {
    let item: NoticeItem

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Circle()
                .fill(item.isAlert ? Palette.vermilion : .clear)
                .frame(width: 6, height: 6)
                .accessibilityHidden(true)
            Text(item.notice.text.firstCapitalized)
                .font(MessagesView.font(13))
                .foregroundStyle(.silk)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
            if item.isAlert {
                #if os(macOS)
                ReportButton(context: BugContext(kind: "message", message: item.notice.text), height: 24, fontSize: 12, raisedSurface: true)
                #else
                ReportButton(context: BugContext(kind: "message", message: item.notice.text), height: 28, fontSize: 15)
                #endif
            }
            Text(item.at, style: .time)
                .font(MessagesView.font(12))
                .foregroundStyle(.haze)
                .monospacedDigit()
        }
        .accessibilityElement(children: .combine)
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
                Text(job.note ?? job.activity.capitalizedSentence)
                    .font(.mac(12))
                    .foregroundStyle(.haze)
                    .lineLimit(1)
            }
            .transition(.opacity)
            .accessibilityElement(children: .combine)
        }
    }
}
