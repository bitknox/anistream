// The match sheet: which of a source's titles a show is, when the source could not tell or got it
// wrong. The titles the source offered, a search of the source by hand, and Match automatically
// to forget a choice. A 480-wide sheet on the Mac, a sheet with medium and large detents on
// iPhone, a form sheet on iPad. A sibling of the source picker, whose last row opens it.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// The title whose match sheet is open, and how it was opened.
public struct MatchPickerRequest: Identifiable, Equatable, Sendable {
    public let id: AnilistId
    /// Opened from Fix this match, not by the page's question: the field holds the title and
    /// searches at once.
    public let fromMenu: Bool

    public init(id: AnilistId, fromMenu: Bool) {
        self.id = id
        self.fromMenu = fromMenu
    }
}

extension View {
    /// The match sheet for whichever title the navigator names.
    func matchPickerSheet(_ navigator: Navigator) -> some View {
        modifier(MatchPickerPresenter(navigator: navigator))
    }
}

/// Presents the sheet, telling it the width class of the window it covers: the sheet's own
/// content always reads as compact.
private struct MatchPickerPresenter: ViewModifier {
    let navigator: Navigator
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #endif

    func body(content: Content) -> some View {
        content.sheet(item: Binding<MatchPickerRequest?>(
            get: { navigator.matchPicker },
            set: { navigator.matchPicker = $0 }
        )) { request in
            #if os(iOS)
            MatchPickerSheet(request: request, wide: sizeClass == .regular)
            #else
            MatchPickerSheet(request: request)
            #endif
        }
    }
}

/// What a candidate's row says.
enum MatchWords {
    /// "TV · 2024 · 12 episodes", from what the source lists.
    static func facts(_ candidate: MatchCandidate) -> String {
        [
            candidate.format.map(format),
            candidate.year.map { String($0) },
            candidate.episodes.map { L.episodesCount(count: Int($0)) },
        ]
        .compactMap { $0 }
        .joined(separator: " · ")
    }

    /// How close it is, "71% match", or why it was ruled out.
    static func closeness(_ candidate: MatchCandidate) -> String {
        if let rejected = candidate.rejected { return rejected.text }
        return L.matchSimilarity(similarity: candidate.similarity.formatted(.percent.precision(.fractionLength(0))))
    }

    /// What AniList says of the show: "On AniList: TV · 2024 · 12 episodes".
    static func anilist(_ entry: Entry) -> String {
        let facts = [
            entry.format.map(TitleWords.format),
            entry.year.map { String($0) },
            entry.episodes.flatMap { $0 > 0 ? L.episodesCount(count: Int($0)) : nil },
        ]
        .compactMap { $0 }
        .joined(separator: " · ")
        return facts.isEmpty ? "" : L.matchAnilistFacts(facts: facts)
    }

    static func format(_ format: MediaFormat) -> String {
        switch format {
        case .tv: L.formatTv
        case .tvShort: L.formatTvShort
        case .movie: L.formatMovie
        case .special: L.formatSpecial
        case .ova: L.formatOva
        case .ona: L.formatOna
        case .music: L.formatMusic
        }
    }

    /// The page's line while the match is open: how many titles could be it.
    static func uncertain(_ asked: MatchAsked) -> String {
        asked.possible == 0
            ? L.matchUncertainNone(source: asked.source)
            : L.matchUncertain(count: asked.possible, source: asked.source)
    }
}

/// The sheet's sizes: the Mac's, or the touch ones.
private enum MatchType {
    #if os(macOS)
    static let title = Font.mac(13)
    static let detail = Font.mac(12)
    static let heading = Font.mac(11, weight: .semibold)
    static let row: CGFloat = 48
    static let nowRow: CGFloat = 44
    static let radius: CGFloat = 8
    #else
    static let title = Font.ui(17)
    static let detail = Font.ui(13)
    static let heading = Font.ui(13, weight: .semibold)
    static let row: CGFloat = 60
    static let nowRow: CGFloat = 60
    static let radius: CGFloat = 16
    #endif
}

/// What the list area holds.
private enum MatchShown: Equatable {
    case candidates(providerId: String, source: String, [MatchCandidate])
    case searching(source: String, query: String)
    case empty(source: String, query: String)
    case failed(source: String, query: String, reason: String)
    case nothing
}

struct MatchPickerSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let request: MatchPickerRequest
    #if os(iOS)
    /// Over a regular-width window, the iPad's: a form sheet in the middle. Over a compact one,
    /// the iPhone's or a narrow split view: the medium and large detents.
    var wide = false
    private var detents: Set<PresentationDetent> { wide ? [.large] : [.medium, .large] }
    #endif

    @State private var query = ""
    /// What the sheet filled the field with: typing anything else holds the sheet open.
    @State private var filled = ""
    /// The row Return picks on the Mac: the best one at first.
    @State private var selected: ProviderKey?
    /// The search has run 600 ms: the searching row shows.
    @State private var searchSlow = false
    /// The pick has run 600 ms: its row says Matching.
    @State private var pickSlow = false
    @FocusState private var fieldFocused: Bool
    #if os(iOS)
    @State private var detent: PresentationDetent = .medium
    #endif

    private var id: AnilistId { request.id }

    private var source: (providerId: String?, name: String) {
        if let known = store.match.source(for: id) { return (known.providerId, known.name) }
        let first = store.providers.rows.first { $0.usable }
        return (first?.id, first?.name ?? "")
    }

    private var shown: MatchShown {
        let name = source.name
        switch store.match.searches[id] {
        case .searching(let query): return .searching(source: name, query: query)
        case .found(_, let providerId, let source, let candidates):
            return .candidates(providerId: providerId, source: source, candidates)
        case .empty(let query, let source): return .empty(source: source, query: query)
        case .failed(let query, let reason): return .failed(source: name, query: query, reason: reason)
        case nil:
            if let asked = store.match.asked[id] {
                return .candidates(providerId: asked.providerId, source: asked.source, asked.candidates)
            }
            return .nothing
        }
    }

    /// The match there is now, shown above the list when the sheet was not opened by a question.
    private var now: CurrentMatch? {
        store.match.asks(id) ? nil : store.match.current[id]
    }

    var body: some View {
        content
            .task { await opened() }
            .onChange(of: shown) { _, now in
                if selected == nil, case .candidates(_, _, let candidates) = now { selected = best(candidates) }
            }
            .onDisappear { store.match.closed(id) }
            .holdsInput(when: query != filled)
            .onChange(of: store.match.settled) { _, settled in
                if settled?.id == id { dismiss() }
            }
            .task(id: store.match.searches[id]) {
                searchSlow = false
                guard case .searching = store.match.searches[id] else { return }
                try? await Task.sleep(for: .milliseconds(600))
                searchSlow = true
            }
            .task(id: store.match.picking[id]) {
                pickSlow = false
                guard store.match.picking[id] != nil else { return }
                try? await Task.sleep(for: .milliseconds(600))
                pickSlow = true
            }
    }

    private func best(_ candidates: [MatchCandidate]) -> ProviderKey? {
        (candidates.first { $0.rejected == nil } ?? candidates.first)?.key
    }

    private func opened() async {
        if case .candidates(_, _, let candidates) = shown { selected = best(candidates) }
        // A screenshot's state, staged on the testing engine.
        let name = navigator.launchSheet
        if AppStore.matchSheetOpens(name) != nil {
            navigator.launchSheet = nil
            if let shot = await store.stageMatchShot(id, name: name) {
                if shot.fromMenu { show(query: shot.query) }
                return
            }
        }
        guard request.fromMenu, query.isEmpty, let entry = store.titles[id] else { return }
        // From the menu: the AniList title, searched at once.
        show(query: entry.showTitle)
        search()
    }

    private func show(query: String) {
        self.query = query
        filled = query
        #if os(iOS)
        detent = .large
        #endif
    }

    private func search() {
        store.searchMatch(id, query: query, providerId: source.providerId)
    }

    /// Return in the field: a new query searches, the same one picks the selected row.
    private func submitted() {
        let typed = query.trimmingCharacters(in: .whitespacesAndNewlines)
        if !typed.isEmpty, typed != store.match.searches[id]?.query {
            search()
        } else if case .candidates(let providerId, let source, let candidates) = shown,
                  let candidate = candidates.first(where: { $0.key == selected }) {
            pick(candidate, providerId: providerId, source: source)
        }
    }

    private func pick(_ candidate: MatchCandidate, providerId: String, source: String) {
        guard store.match.picking[id] == nil else { return }
        selected = candidate.key
        store.pickMatch(id, providerId: providerId, source: source, candidate: candidate)
    }

    private func move(_ step: Int) {
        guard case .candidates(_, _, let candidates) = shown, !candidates.isEmpty else { return }
        let at = candidates.firstIndex { $0.key == selected } ?? (step > 0 ? -1 : candidates.count)
        selected = candidates[min(max(at + step, 0), candidates.count - 1)].key
    }

    // MARK: Chrome

    #if os(macOS)
    private var content: some View {
        let entry = store.titles[id]
        return VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                if let entry { CoverArt(entry: entry, width: 36) }
                VStack(alignment: .leading, spacing: 1) {
                    Text(L.matchChoose).font(.mac(15, weight: .semibold)).foregroundStyle(.silk)
                    Text(entry?.showTitle ?? "").font(.mac(13)).foregroundStyle(Palette.silk.opacity(0.85)).lineLimit(1)
                    if let entry, !MatchWords.anilist(entry).isEmpty {
                        Text(MatchWords.anilist(entry)).font(.mac(12)).foregroundStyle(.haze)
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 18)
            .padding(.top, 16)
            .padding(.bottom, 12)
            field
                .padding(.horizontal, 18)
            ScrollView {
                list.padding(12)
            }
            .frame(height: 290)
            .scrollBounceBehavior(.basedOnSize)
            Rectangle().fill(.faintRule).frame(height: 1)
            HStack(spacing: 8) {
                if store.match.current[id]?.chosenByYou == true {
                    Button(L.matchAutomatic) {
                        store.matchAutomatically(id)
                        dismiss()
                    }
                    .buttonStyle(SecondaryCapsuleStyle(height: 28))
                }
                Spacer()
                Button(L.actionCancel) { dismiss() }
                    .buttonStyle(SecondaryCapsuleStyle(height: 28, raisedSurface: true))
                    .keyboardShortcut(.cancelAction)
            }
            .padding(.horizontal, 18)
            .padding(.top, 12)
            .padding(.bottom, 14)
        }
        .frame(width: 480)
        .background(.raised)
        .closesOnEscape { dismiss() }
        .onKeyPress(.upArrow) { move(-1); return .handled }
        .onKeyPress(.downArrow) { move(1); return .handled }
    }
    #else
    private var content: some View {
        let entry = store.titles[id]
        return NavigationStack {
            VStack(alignment: .leading, spacing: 0) {
                field
                    .padding(.horizontal, 16)
                    .padding(.top, 6)
                if let entry, !MatchWords.anilist(entry).isEmpty {
                    Text(MatchWords.anilist(entry))
                        .font(.ui(13))
                        .foregroundStyle(.haze)
                        .padding(.horizontal, 20)
                        .padding(.top, 6)
                }
                ScrollView {
                    list
                        .padding(.horizontal, 16)
                        .padding(.top, 14)
                        .padding(.bottom, 24)
                }
                .scrollDismissesKeyboard(.interactively)
            }
            .background(.raised)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L.actionCancel) { dismiss() }.tint(.silk)
                }
                ToolbarItem(placement: .principal) {
                    VStack(spacing: 0) {
                        Text(L.matchChoose).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                        Text(entry?.showTitle ?? "").font(.ui(12)).foregroundStyle(.haze).lineLimit(1)
                    }
                    .accessibilityElement(children: .combine)
                    .accessibilityAddTraits(.isHeader)
                }
            }
        }
        .presentationDetents(detents, selection: Binding(
            get: { detents.contains(detent) ? detent : .large },
            set: { detent = $0 }
        ))
        .presentationSizing(.form)
        .onChange(of: fieldFocused) { _, focused in
            if focused { detent = .large }
        }
    }
    #endif

    // MARK: The field

    private var field: some View {
        HStack(spacing: 7) {
            Image(systemName: "magnifyingglass")
                .font(MatchType.detail)
                .foregroundStyle(.haze)
                .accessibilityHidden(true)
            TextField(L.matchSearchField(source: source.name), text: $query)
                .textFieldStyle(.plain)
                .font(MatchType.title)
                .foregroundStyle(.silk)
                .focused($fieldFocused)
                .submitLabel(.search)
                .onSubmit(submitted)
                .autocorrectionDisabled()
                #if os(iOS)
                .textInputAutocapitalization(.never)
                #endif
            if !query.isEmpty {
                Button {
                    query = ""
                    fieldFocused = true
                } label: {
                    Image(systemName: "xmark.circle.fill").foregroundStyle(.haze)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L.actionClear)
            }
        }
        #if os(macOS)
        .padding(.horizontal, 9)
        .frame(height: 28)
        #else
        .padding(.horizontal, 12)
        .frame(height: 40)
        #endif
        .background(Palette.ground.opacity(0.55), in: .rect(cornerRadius: MatchType.radius - 2))
        .overlay {
            RoundedRectangle(cornerRadius: MatchType.radius - 2)
                .strokeBorder(fieldFocused ? Color.water.opacity(0.6) : Color.rule, lineWidth: fieldFocused ? 2 : 1)
        }
    }

    // MARK: The list

    private var list: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let now {
                heading(L.matchNow)
                HStack(spacing: 12) {
                    Text(now.title ?? now.source).font(MatchType.title).foregroundStyle(.silk).lineLimit(1)
                    Spacer(minLength: 8)
                    Text(now.chosenByYou ? L.matchChosenByYou : L.matchChosenAutomatically)
                        .font(MatchType.detail)
                        .foregroundStyle(.haze)
                }
                .padding(.horizontal, 12)
                .frame(minHeight: MatchType.nowRow)
                .background(Palette.ground.opacity(0.55), in: .rect(cornerRadius: MatchType.radius))
                .accessibilityElement(children: .combine)
                .padding(.bottom, 10)
            }
            Group {
                switch shown {
                case .candidates(let providerId, let source, let candidates):
                    heading(L.matchSectionSource(source: source))
                    VStack(spacing: 0) {
                        ForEach(candidates, id: \.key) { candidate in
                            row(candidate, providerId: providerId, source: source)
                            if candidate.key != candidates.last?.key {
                                Rectangle().fill(.faintRule).frame(height: 1)
                            }
                        }
                    }
                    .background(Palette.ground.opacity(0.55), in: .rect(cornerRadius: MatchType.radius))
                    .clipShape(.rect(cornerRadius: MatchType.radius))
                    if let failed = store.match.pickFailed[id] {
                        said(dot: true, L.matchFixFailed, detail: failed)
                    }
                case .searching(let source, let query):
                    heading(L.matchSectionSource(source: source))
                    VStack(spacing: 10) {
                        ProgressView().controlSize(.small)
                        Text(L.matchSearching(source: source, query: query))
                            .font(MatchType.detail)
                            .foregroundStyle(.haze)
                            .multilineTextAlignment(.center)
                    }
                    .frame(maxWidth: .infinity, minHeight: 200)
                    .opacity(searchSlow ? 1 : 0)
                case .empty(let source, let query):
                    said(dot: false, L.matchSearchEmpty(source: source, query: query), detail: L.matchSearchEmptyHint)
                case .failed(let source, _, let reason):
                    said(dot: true, L.matchSearchFailed(source: source), detail: reason, retry: true)
                case .nothing:
                    EmptyView()
                }
            }
            .transition(.opacity)
            #if os(iOS)
            if store.match.current[id]?.chosenByYou == true {
                Button {
                    store.matchAutomatically(id)
                    dismiss()
                } label: {
                    Text(L.matchAutomatic)
                        .font(MatchType.title)
                        .foregroundStyle(.silk)
                        .frame(maxWidth: .infinity, minHeight: 50, alignment: .leading)
                        .padding(.horizontal, 16)
                        .background(Palette.ground.opacity(0.55), in: .rect(cornerRadius: MatchType.radius))
                }
                .buttonStyle(.plain)
                .padding(.top, 18)
                Text(L.matchAutomaticDetail)
                    .font(MatchType.detail)
                    .foregroundStyle(.haze)
                    .padding(.horizontal, 16)
                    .padding(.top, 2)
            }
            #endif
        }
        .animation(reduceMotion ? nil : .standard, value: shown)
    }

    private func heading(_ text: String) -> some View {
        Text(text)
            .font(MatchType.heading)
            .foregroundStyle(.haze)
            .padding(.horizontal, 6)
            .padding(.bottom, 2)
            .accessibilityAddTraits(.isHeader)
    }

    /// A state said in the list's place: no results, or a failure with its reason.
    private func said(dot: Bool, _ text: String, detail: String, retry: Bool = false) -> some View {
        VStack(spacing: 6) {
            HStack(spacing: 8) {
                if dot {
                    Circle().fill(.vermilion).frame(width: 8, height: 8).accessibilityHidden(true)
                }
                Text(text).font(MatchType.title.weight(.semibold)).foregroundStyle(.silk)
            }
            Text(detail).font(MatchType.detail).foregroundStyle(.haze)
            if retry {
                Button(L.actionTryAgain, action: search)
                    .buttonStyle(SecondaryCapsuleStyle(height: MatchType.row > 50 ? 40 : 26, raisedSurface: true))
                    .padding(.top, 6)
            }
        }
        .multilineTextAlignment(.center)
        .padding(.horizontal, 24)
        .frame(maxWidth: .infinity, minHeight: 220)
    }

    private func row(_ candidate: MatchCandidate, providerId: String, source: String) -> some View {
        let picking = store.match.picking[id]
        let isPicked = picking == candidate.key
        let ruledOut = candidate.rejected != nil
        let dimmed: Double = if picking != nil, pickSlow, !isPicked {
            ruledOut ? 0.3 : 0.5
        } else {
            ruledOut ? 0.55 : 1
        }
        let closeness = MatchWords.closeness(candidate)
        let facts = MatchWords.facts(candidate)
        return Button {
            pick(candidate, providerId: providerId, source: source)
        } label: {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 1) {
                    Text(candidate.title).font(MatchType.title).foregroundStyle(.silk).lineLimit(1)
                    if !facts.isEmpty {
                        Text(facts).font(MatchType.detail).foregroundStyle(.haze)
                    }
                }
                Spacer(minLength: 8)
                if isPicked, pickSlow {
                    ProgressView().controlSize(.mini)
                }
                Text(isPicked && pickSlow ? L.matchPicking : closeness)
                    .font(MatchType.detail.monospacedDigit())
                    .foregroundStyle(isPicked && pickSlow ? Color.silk.opacity(0.85) : Color.haze)
                    .lineLimit(1)
            }
            .padding(.horizontal, 12)
            .frame(minHeight: MatchType.row)
            .background(rowFill(candidate, picked: isPicked))
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .opacity(dimmed)
        .disabled(picking != nil && !isPicked)
        .pointerHover()
        .accessibilityLabel(L.matchRowLabel(title: candidate.title, facts: facts, closeness: closeness))
        .accessibilityValue(isPicked ? L.matchPicking : "")
        .accessibilityAddTraits(selected == candidate.key ? .isSelected : [])
    }

    private func rowFill(_ candidate: MatchCandidate, picked: Bool) -> Color {
        #if os(macOS)
        selected == candidate.key ? Palette.raisedSelected : .clear
        #else
        picked ? Palette.rowPressed : .clear
        #endif
    }
}

/// The title page's one amber action while the source could not choose: Choose the match.
struct ChooseMatchButton: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let id: AnilistId
    var height: CGFloat = 34
    var fontSize: CGFloat = 13
    /// Whether it spans its row, as the iPhone's main button does.
    var fills = false

    var body: some View {
        Button {
            navigator.matchPicker = MatchPickerRequest(id: id, fromMenu: false)
        } label: {
            Text(L.matchChoose).frame(maxWidth: fills ? .infinity : nil)
        }
        .buttonStyle(PrimaryCapsuleStyle(height: height, fontSize: fontSize))
        .keyboardShortcut(.defaultAction)
        .task(id: id) {
            // A screenshot of the page after a pick, or after Match automatically.
            let name = navigator.launchSheet
            guard name == "match-picked" || name == "match-cleared" else { return }
            navigator.launchSheet = nil
            _ = await store.stageMatchShot(id, name: name)
        }
    }
}

/// The line under the action while the match is open: how many titles could be it.
struct MatchUncertainLine: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId
    var font: Font = .mac(13)
    var top: CGFloat = 0

    var body: some View {
        if let asked = store.match.asked[id] {
            Text(MatchWords.uncertain(asked))
                .font(font)
                .foregroundStyle(.haze)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, top)
                .transition(.opacity)
        }
    }
}

extension Navigator {
    /// Fix this match, from a menu or the palette: the question, when the page asks one, else
    /// the field filled with the title and searched.
    func fixMatch(_ id: AnilistId, store: AppStore) {
        matchPicker = MatchPickerRequest(id: id, fromMenu: !store.match.asks(id))
    }
}

/// The source picker's last row: Wrong title? Fix this match. It closes the picker and opens the
/// match sheet in its place.
struct MatchFixRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let id: AnilistId
    let done: () -> Void

    var body: some View {
        Button {
            done()
            Task { @MainActor in
                // The picker goes down first, so the sheet that takes its place can come up.
                try? await Task.sleep(for: .milliseconds(350))
                navigator.fixMatch(id, store: store)
            }
        } label: {
            HStack(spacing: 10) {
                Text(L.matchSourcePickerRow)
                    .font(PickerType.title)
                    .foregroundStyle(.silk)
                Spacer(minLength: 8)
                Image(systemName: "chevron.right")
                    .font(PickerType.detail.weight(.semibold))
                    .foregroundStyle(.haze)
                    .accessibilityHidden(true)
            }
            .padding(.horizontal, 12)
            .frame(minHeight: PickerType.row - 4)
            .background(Palette.rowPressed, in: .rect(cornerRadius: 8))
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .padding(.top, 8)
    }
}
