// Covers in a grid: how Seasonal, Search, the Library and This Season browse titles.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// One title in a grid: its cover at 2:3, its title in two lines, a line beneath, and, where it
/// is under way, a meter.
///
/// The title always takes two lines' room, a one-line title too, so every caption is one height
/// and the episode lines and meters run level across a row.
struct PosterCell: View {
    @Environment(Navigator.self) private var navigator
    @Environment(\.posterAsRow) private var asRow
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
            if asRow {
                // Past `.xxxLarge` the grid is a list: a 60 pt cover beside the caption.
                HStack(alignment: .top, spacing: 12) {
                    ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: 180)
                        .frame(width: 60, height: 90)
                        .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: 60)))
                        .matchedTransitionSourceIfAvailable(id: entry.id)
                    VStack(alignment: .leading, spacing: 0) {
                        Text(entry.showTitle)
                            .font(.macShowTitle(size: 14))
                            .foregroundStyle(.silk)
                            .rowTitle()
                        lines
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .contentShape(.rect)
            } else {
                VStack(alignment: .leading, spacing: 0) {
                    ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: Int(width * 3))
                        .aspectRatio(2 / 3, contentMode: .fit)
                        .frame(maxWidth: .infinity)
                        .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: width)))
                        .matchedTransitionSourceIfAvailable(id: entry.id)
                    Text(entry.showTitle)
                        .font(.macShowTitle(size: 14))
                        .macLineHeight(18)
                        .foregroundStyle(.silk)
                        .lineLimit(2, reservesSpace: true)
                        .padding(.top, 8)
                    lines
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(.rect)
                // A grid's captions are no larger than `.xxxLarge`; past it the grid is a list.
                .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
            }
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(entry, value: [detail, fresh].compactMap { $0 }.joined(separator: ", ")) {
            TitleActions(entry: entry)
        }
        .contextMenu { TitleMenu(entry: entry) }
    }

    /// The lines under the title: the detail, the meter, a new episode.
    @ViewBuilder
    private var lines: some View {
        if let detail {
            Text(detail)
                .font(.mac(12))
                .macLineHeight(16)
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
            .font(.mac(12))
            .macLineHeight(16)
            .foregroundStyle(.haze)
            .lineLimit(1)
            .padding(.top, 8)
        }
    }
}

extension EnvironmentValues {
    /// Whether a poster cell is a row of a list rather than a cell of a grid: past `.xxxLarge`.
    @Entry var posterAsRow = false
}

/// A grid of covers, as many columns of at least `minimum` points as fit.
struct PosterGrid<Cell: View>: View {
    let entries: [Entry]
    var minimum: CGFloat = 128
    var spacing: CGFloat = 20
    @ViewBuilder let cell: (Entry) -> Cell
    @Environment(\.dynamicTypeSize) private var typeSize

    var body: some View {
        if typeSize > .xxxLarge {
            // Past `.xxxLarge` the captions would outgrow their columns: one column of rows.
            LazyVStack(alignment: .leading, spacing: 16) {
                ForEach(entries, id: \.id) { entry in
                    cell(entry).environment(\.posterAsRow, true)
                }
            }
        } else {
            LazyVGrid(
                columns: [GridItem(.adaptive(minimum: minimum), spacing: spacing, alignment: .top)],
                alignment: .leading,
                spacing: 24
            ) {
                ForEach(entries, id: \.id) { entry in
                    // A cover and its lines move as one when the columns change, as the Mac's
                    // filters panel opening does: never the title ahead of its cover.
                    cell(entry).geometryGroup()
                }
            }
        }
    }
}

/// Placeholder covers while a grid loads: static plates with redacted lines, held for a moment
/// at least and then replaced in one crossfade (T8).
struct PosterGridPlaceholder: View {
    var count = 14
    var minimum: CGFloat = 128
    var spacing: CGFloat = 20

    var body: some View {
        LazyVGrid(
            columns: [GridItem(.adaptive(minimum: minimum), spacing: spacing, alignment: .top)],
            alignment: .leading,
            spacing: 24
        ) {
            ForEach(0..<count, id: \.self) { _ in
                PosterCellPlaceholder(width: minimum).geometryGroup()
            }
        }
        .accessibilityHidden(true)
    }
}

/// A poster cell to come, at the size the cell will have, so nothing moves when it arrives: the
/// plate, two lines' room for the title and the detail line, as `PosterCell` lays them out.
struct PosterCellPlaceholder: View {
    var width: CGFloat = 128

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Rectangle()
                .fill(.raised)
                .aspectRatio(2 / 3, contentMode: .fit)
                .frame(maxWidth: .infinity)
                .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: width)))
            Text("Hoshikuzu no Tabibito") // i18n-skip
                .font(.macShowTitle(size: 14))
                .macLineHeight(18)
                .lineLimit(2, reservesSpace: true)
                .padding(.top, 8)
            Text("TV · 2026") // i18n-skip
                .font(.mac(12))
                .macLineHeight(16)
                .lineLimit(1)
                .padding(.top, 4)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .redacted(reason: .placeholder)
        .foregroundStyle(.haze)
        .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
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

/// A list's states other than its rows: loading, failed, offline and empty (the spec's States).
///
/// Waiting is silent for 600 ms: what was on screen stays, or on a first visit the ground stays
/// empty. Past that, placeholders stand in the real layout; once up they stay at least 0.3 s and
/// give way to the answer in one crossfade (T8), and past 4 s the job's activity line shows
/// under them. A list already there when the screen opens shows at once. Offline, a list keeps
/// what it has under the offline line, and one that failed with nothing says it is offline.
struct ListStates<Loaded: View, Empty: View>: View {
    @Environment(AppStore.self) private var store
    let state: ListState
    var placeholder: AnyView = AnyView(PosterGridPlaceholder())
    /// The engine's words for the load, which the activity line names past 4 s.
    var activity: String?
    /// What failed, said over the reason when the list could not load.
    var failure = L.browseListFailed
    /// Ask again, as Try again does.
    var retry: (() -> Void)?
    /// How far the offline line sits in from the sides, where the list's own rows set their
    /// margins themselves.
    var offlineInset: CGFloat = 0
    /// Set while the loaded list is what shows, for a List that draws rows of its own under it:
    /// they come and go with the answer, in its crossfade.
    var rowsShown: Binding<Bool>?
    @ViewBuilder let loaded: ([AnilistId]) -> Loaded
    @ViewBuilder let empty: () -> Empty

    private enum Stage: Equatable {
        /// Asked, under 600 ms: nothing new shows.
        case waiting
        /// Past 600 ms with no answer: the placeholders.
        case placeholders
        /// The answer, `shown`.
        case answer
    }

    @State private var stage = Stage.waiting
    /// The answer on screen, kept while the list is asked again.
    @State private var shown = ListState.idle
    /// When the placeholders came up.
    @State private var since: ContinuousClock.Instant?
    /// Whether the load has run past 4 s.
    @State private var slow = false

    static var delay: Duration { JobsModel.delay }
    static var minimum: Duration { .milliseconds(300) }
    static var activityAfter: Duration { .seconds(4) }

    var body: some View {
        Group {
            switch stage {
            case .waiting:
                if shown.isAnswer { answer(shown) } else { placeholder.hidden() }
            case .placeholders:
                VStack(alignment: .leading, spacing: 0) {
                    placeholder
                    if slow, let activity { ActivityLine(activity: activity) }
                }
            case .answer:
                answer(shown)
            }
        }
        .onAppear {
            if state.isAnswer {
                shown = state
                stage = .answer
            }
        }
        .task(id: state) { await follow(state) }
        .onChange(of: rowsOnScreen, initial: true) { old, now in
            guard let rowsShown, rowsShown.wrappedValue != now else { return }
            withAnimation(old == now ? nil : .standard) { rowsShown.wrappedValue = now }
        }
    }

    /// Whether the answer on screen is a loaded list with something in it.
    private var rowsOnScreen: Bool {
        guard stage != .placeholders, case .loaded(let ids) = shown else { return false }
        return !ids.isEmpty
    }

    @ViewBuilder
    private func answer(_ state: ListState) -> some View {
        switch state {
        case .failed(let reason):
            Group {
                if store.offline {
                    OfflineEmpty()
                } else {
                    ScreenFailed(title: failure, reason: reason, retry: retry)
                }
            }
            // Across the screen and a little down it, inside a scroll view as well, where it
            // would otherwise take its own width at the top left.
            .frame(maxWidth: .infinity, minHeight: 360)
        case .empty:
            empty()
        case .loaded(let ids):
            if ids.isEmpty {
                empty()
            } else {
                VStack(alignment: .leading, spacing: 0) {
                    OfflineLine().padding(.horizontal, offlineInset)
                    loaded(ids)
                }
            }
        case .idle, .loading:
            placeholder
        }
    }

    /// Follow the list to `state`: an answer replaces what shows, after the placeholders' 0.3 s;
    /// an ask waits 600 ms before the placeholders, and 4 s before the activity line.
    private func follow(_ state: ListState) async {
        if state.isAnswer {
            if stage == .placeholders, let since {
                let left = Self.minimum - (ContinuousClock.now - since)
                if left > .zero { try? await Task.sleep(for: left) }
                guard !Task.isCancelled else { return }
            }
            let fresh = stage != .answer || shown != state
            withAnimation(fresh ? .standard : nil) {
                shown = state
                stage = .answer
                slow = false
            }
            return
        }
        if stage == .answer { stage = .waiting }
        guard stage == .waiting else { return }
        try? await Task.sleep(for: Self.delay)
        guard !Task.isCancelled else { return }
        since = .now
        withAnimation(.standard) { stage = .placeholders }
        try? await Task.sleep(for: Self.activityAfter - Self.delay)
        guard !Task.isCancelled else { return }
        withAnimation(.fadeIn) { slow = true }
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
