// The states every screen shares beside its content: the empty state with Nagare, a load that
// failed, the device offline, and the words for each platform.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The same words in each platform's casing: title case on the Mac, as its controls are, and
/// sentence case on iPhone and iPad.
func cased(mac: String, touch: String) -> String {
    #if os(macOS)
    mac
    #else
    touch
    #endif
}

extension Font {
    /// The Mac's fixed size there, and the touch size, which follows Dynamic Type, on iPhone and iPad.
    static func plat(mac: CGFloat, touch: CGFloat, weight: Font.Weight = .regular) -> Font {
        #if os(macOS)
        .mac(mac, weight: weight)
        #else
        .ui(touch, weight: weight)
        #endif
    }
}

/// A screen not used yet: Nagare above a title, one line and at most one button (the spec's
/// empty states). Small, frequent empties use a quiet line or `ContentUnavailableView` instead.
struct EmptyState<Action: View>: View {
    let title: String
    let line: String
    @ViewBuilder var action: () -> Action

    var body: some View {
        VStack(spacing: 0) {
            Nagare(cell: 5)
                .accessibilityHidden(true)
            Text(title)
                .font(.plat(mac: 15, touch: 17, weight: .semibold))
                .foregroundStyle(.silk)
                .multilineTextAlignment(.center)
                .padding(.top, 20)
                .accessibilityAddTraits(.isHeader)
            Text(line)
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 340)
                .padding(.top, 4)
            action()
                .padding(.top, 16)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

extension EmptyState where Action == EmptyView {
    init(title: String, line: String) {
        self.init(title: title, line: line) { EmptyView() }
    }
}

/// A screen that could not load: the triangle in vermilion, what failed, the first clause of why
/// in plain words, and Try again. The whole reason went to Messages as it came.
struct ScreenFailed: View {
    let title: String
    let reason: String
    var retry: (() -> Void)?

    var body: some View {
        ContentUnavailableView {
            Label {
                Text(title).foregroundStyle(.silk)
            } icon: {
                Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.vermilion)
            }
        } description: {
            if let why = Self.plain(reason) {
                Text(why).foregroundStyle(.haze)
            }
        } actions: {
            if let retry {
                Button(cased(mac: "Try Again", touch: "Try again"), action: retry)
                    .buttonStyle(SecondaryCapsuleStyle(height: Self.buttonHeight, fontSize: Self.buttonFont))
            }
        }
    }

    static var buttonHeight: CGFloat {
        #if os(macOS)
        28
        #else
        44
        #endif
    }

    static var buttonFont: CGFloat {
        #if os(macOS)
        13
        #else
        17
        #endif
    }

    /// The reason's first clause, its first letter capitalised, when it reads as words: nothing
    /// when it is a chain of a library's errors, an address or a code.
    static func plain(_ reason: String) -> String? {
        let first = reason.split(separator: ":", maxSplits: 1).first.map(String.init) ?? reason
        let clause = first.trimmingCharacters(in: .whitespacesAndNewlines)
        let technical = ["http", "://", "(", "{", "_", "error", "os error", "errno"]
        guard !clause.isEmpty, clause.count <= 80,
              !technical.contains(where: { clause.lowercased().contains($0) })
        else { return nil }
        let sentence = clause.firstCapitalized
        return sentence.hasSuffix(".") ? sentence : sentence + "."
    }
}

/// Offline, on a screen with what it had: a haze line with the glyph, under the screen's title.
/// Nothing is disabled; the line fades when the connection comes back.
struct OfflineLine: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        Group {
            if store.offline {
                HStack(spacing: 6) {
                    Image(systemName: "wifi.slash")
                        .accessibilityHidden(true)
                    Text("Offline. Downloaded episodes still play.")
                }
                .font(.plat(mac: 13, touch: 15))
                .foregroundStyle(.haze)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.bottom, 8)
                .transition(.opacity)
            }
        }
        .animation(.standard, value: store.offline)
    }
}

/// Offline, on a screen with nothing to show: "You're offline", with the way to what plays
/// without a connection.
struct OfflineEmpty: View {
    @Environment(Navigator.self) private var navigator
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    var body: some View {
        ContentUnavailableView {
            Label {
                Text("You're offline").foregroundStyle(.silk)
            } icon: {
                Image(systemName: "wifi.slash").foregroundStyle(.haze)
            }
        } description: {
            Text("Downloaded episodes still play.").foregroundStyle(.haze)
        } actions: {
            Button(cased(mac: "Show Downloads", touch: "Show downloads")) {
                navigator.showDownloads(compact: compact)
            }
            .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
        }
    }

    private var compact: Bool {
        #if os(iOS)
        sizeClass == .compact
        #else
        false
        #endif
    }
}

/// Past 4 s of loading, under the placeholders: what the job is doing, in its words, and what it
/// waits on when it says.
struct ActivityLine: View {
    @Environment(AppStore.self) private var store
    /// The engine's words for the job, as `Command::activity` has them: "loading this season".
    let activity: String

    var body: some View {
        let job = store.jobs.running.last { $0.activity == activity }
        Text(job?.note ?? activity.firstCapitalized)
            .font(.plat(mac: 13, touch: 15))
            .foregroundStyle(.haze)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.top, 12)
            .transition(.opacity)
    }
}

extension Navigator {
    /// Downloads, from wherever it is asked: its section on the Mac and a wide iPad, Library's
    /// first row pushed on iPhone and a narrow iPad window, which have no Downloads tab.
    func showDownloads(compact: Bool) {
        if compact {
            section = .library
            downloadsPushed = true
        } else {
            open(.downloads)
        }
    }
}

/// A failure said in a line: the triangle in vermilion and fixed words in haze. The engine's own
/// text stays in Messages and the bug report (copy rule 5).
struct FailedLine: View {
    let text: String
    var size: CGFloat = 13

    init(_ text: String, size: CGFloat = 13) {
        self.text = text
        self.size = size
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Image(systemName: "exclamationmark.triangle.fill")
                .foregroundStyle(.vermilion)
                .accessibilityHidden(true)
            Text(text).foregroundStyle(.haze)
        }
        .font(.plat(mac: size, touch: size + 2))
    }
}

/// The fixed words for a link that leads to no store, from the join board.
let notAStoreWords = "This link does not lead to a plugin store. Ask the store's owner for a new invite."

/// A small spinner beside a button while what it started runs, as the sign-in sheets have it;
/// nothing otherwise.
struct Working: View {
    let on: Bool

    init(_ on: Bool) { self.on = on }

    var body: some View {
        if on {
            ProgressView()
                .controlSize(.small)
                .accessibilityLabel("Working")
        }
    }
}
