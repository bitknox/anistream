// Covers in a grid: how Seasonal, Search, the Library and This Season browse titles.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// One title in a grid: its cover at 2:3, its title in two lines, a line beneath, and, where it
/// is under way, a meter.
///
/// The title always takes two lines' room, a one-line title too, so every caption is one height
/// and the episode lines and meters run level across a row.
struct PosterCell: View {
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    /// The line under the title: "Ep 7 of 24", a genre, a status.
    var detail: String?
    /// A new episode's line, with the water dot: "Ep 9 out 2h ago".
    var fresh: String?
    /// How far through, for the meter, when it is under way.
    var fraction: Double?
    var width: CGFloat = 128

    var body: some View {
        Button {
            navigator.openTitle(entry.id)
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: Int(width * 3))
                    .aspectRatio(2 / 3, contentMode: .fit)
                    .frame(maxWidth: .infinity)
                    .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: width)))
                    .matchedTransitionSourceIfAvailable(id: entry.id)
                Text(entry.showTitle)
                    .font(.showTitle(size: 14))
                    .fixedLineHeight(18)
                    .foregroundStyle(.silk)
                    .lineLimit(2, reservesSpace: true)
                    .padding(.top, 8)
                if let detail {
                    Text(detail)
                        .font(.system(size: 12))
                        .fixedLineHeight(16)
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                        .padding(.top, 4)
                }
                if let fraction {
                    Meter(fraction: fraction, height: 3)
                        .padding(.top, 8)
                }
                if let fresh {
                    HStack(spacing: 6) {
                        Circle().fill(.water).frame(width: 6, height: 6)
                        Text(fresh)
                    }
                    .font(.system(size: 12))
                    .fixedLineHeight(16)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .padding(.top, 8)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .contextMenu { TitleMenu(entry: entry) }
    }
}

/// A grid of covers, as many columns of at least `minimum` points as fit.
struct PosterGrid<Cell: View>: View {
    let entries: [Entry]
    var minimum: CGFloat = 128
    var spacing: CGFloat = 20
    @ViewBuilder let cell: (Entry) -> Cell

    var body: some View {
        LazyVGrid(
            columns: [GridItem(.adaptive(minimum: minimum), spacing: spacing, alignment: .top)],
            alignment: .leading,
            spacing: 24
        ) {
            ForEach(entries, id: \.id) { entry in
                cell(entry)
            }
        }
    }
}

/// Placeholder covers while a grid loads: static plates with redacted lines, held for a moment
/// at least and then replaced in one crossfade (T8).
struct PosterGridPlaceholder: View {
    var count = 14
    var minimum: CGFloat = 128

    var body: some View {
        LazyVGrid(
            columns: [GridItem(.adaptive(minimum: minimum), spacing: 20, alignment: .top)],
            alignment: .leading,
            spacing: 24
        ) {
            ForEach(0..<count, id: \.self) { _ in
                VStack(alignment: .leading, spacing: 8) {
                    RoundedRectangle(cornerRadius: 8).fill(.raised).aspectRatio(2 / 3, contentMode: .fit)
                    Text("Hoshikuzu no Tabibito").font(.showTitle(size: 14))
                    Text("Ep 7 of 24").font(.system(size: 12))
                }
                .redacted(reason: .placeholder)
                .foregroundStyle(.haze)
            }
        }
        .accessibilityHidden(true)
    }
}

extension View {
    /// The source of the zoom into a title's page (T1), on iPhone and iPad.
    @ViewBuilder
    func matchedTransitionSourceIfAvailable(id: AnilistId) -> some View {
        #if os(iOS)
        modifier(ZoomSource(id: id))
        #else
        self
        #endif
    }
}

#if os(iOS)
/// The namespace the zoom transition runs in, from the tab that shows the title.
struct ZoomNamespaceKey: EnvironmentKey {
    static let defaultValue: Namespace.ID? = nil
}

extension EnvironmentValues {
    var zoomNamespace: Namespace.ID? {
        get { self[ZoomNamespaceKey.self] }
        set { self[ZoomNamespaceKey.self] = newValue }
    }
}

private struct ZoomSource: ViewModifier {
    @Environment(\.zoomNamespace) private var namespace
    let id: AnilistId

    func body(content: Content) -> some View {
        if let namespace {
            content.matchedTransitionSource(id: id, in: namespace)
        } else {
            content
        }
    }
}
#endif

/// A list's states other than its rows: loading, failed and empty. Placeholders, once up, stay
/// at least 0.3 seconds and then give way to what came in one crossfade (T8); a list already
/// there when the screen opens shows at once.
struct ListStates<Loaded: View, Empty: View>: View {
    let state: ListState
    var placeholder: AnyView = AnyView(PosterGridPlaceholder())
    @ViewBuilder let loaded: ([AnilistId]) -> Loaded
    @ViewBuilder let empty: () -> Empty

    /// Whether the answer is on screen rather than the placeholders.
    @State private var answered = false
    /// When the placeholders came up.
    @State private var since: ContinuousClock.Instant?

    static var minimum: Duration { .milliseconds(300) }

    var body: some View {
        Group {
            if !answered {
                placeholder
            } else {
                switch state {
                case .idle, .loading:
                    placeholder
                case .failed(let reason):
                    ListFailed(reason: reason)
                case .empty:
                    empty()
                case .loaded(let ids):
                    if ids.isEmpty { empty() } else { loaded(ids) }
                }
            }
        }
        .onAppear {
            if state.isAnswer {
                answered = true
            } else {
                since = .now
            }
        }
        .task(id: state) {
            guard !answered, state.isAnswer else { return }
            if let since {
                let left = Self.minimum - (ContinuousClock.now - since)
                if left > .zero { try? await Task.sleep(for: left) }
            }
            withAnimation(.standard) { answered = true }
        }
    }
}

extension ListState {
    /// Whether the list has been answered, whatever the answer.
    var isAnswer: Bool {
        switch self {
        case .idle, .loading: false
        case .loaded, .empty, .failed: true
        }
    }
}
