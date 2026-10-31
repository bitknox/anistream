// Home's sections: a heading with its count and link, then wide cards, posters or the week.
// Desktop shelves page with arrows that show while the pointer is over the shelf.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

struct HomeSectionView: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeWide) private var wide
    let section: HomeSection
    let items: [HomeItem]
    let season: SeasonYear?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            switch section.kind {
            case .continue:
                HomeShelf(spacing: 12) {
                    ForEach(items, id: \.entry.id) { item in
                        HomeContinueCard(item: item).transition(.opacity)
                    }
                }
            case .thisWeek:
                HomeWeek(items: items)
            default:
                HomeShelf(spacing: 12) {
                    ForEach(items, id: \.entry.id) { item in
                        HomePosterCard(item: item, kind: section.kind).transition(.opacity)
                    }
                }
            }
        }
        .animation(.standard, value: items.map(\.entry.id))
    }

    // MARK: Heading

    private var header: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(headingText)
                .font(.ui(20, weight: .semibold))
                .foregroundStyle(.silk)
                .accessibilityAddTraits(.isHeader)
            if section.kind == .continue, section.total > UInt32(items.count), section.total > 12 {
                Text(L.homeContinueCount(count: Int(section.total)))
                    .font(.ui(15))
                    .foregroundStyle(.haze)
            }
            Spacer(minLength: 8)
            if section.kind == .thisWeek || section.kind == .airingToday {
                link(L.homeFullCalendar) { navigator.open(.calendar) }
            } else if let target = section.viewAll {
                link(linkTitle(target)) { follow(target) }
            }
        }
    }

    /// The engine's heading, with the season beside This season.
    private var headingText: String {
        let title = section.title.text
        guard section.kind == .thisSeason, let season else { return title }
        let words = L.seasonWithYear(season: TitleWords.season(season.season), year: String(season.year))
        return "\(title) · \(words)"
    }

    private func linkTitle(_ target: HomeLink) -> String {
        switch target {
        case .watching: L.homeViewAll
        case .seasonalView: L.homeSeasonal
        case .calendarView: L.homeFullCalendar
        case .browse: section.kind == .moreGenre ? L.homeSearch : L.homeViewAll
        }
    }

    private func follow(_ target: HomeLink) {
        switch target {
        case .watching:
            navigator.librarySegment = .watching
            navigator.open(.library)
        case .seasonalView: navigator.open(.seasonal)
        case .calendarView: navigator.open(.calendar)
        case .browse(let query): navigator.search(query, in: store)
        }
    }

    private func link(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title)
                .font(.ui(wide ? 13 : 15, weight: .medium))
                .foregroundStyle(.haze)
                .frame(minHeight: wide ? 24 : 44)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityLabel(Text("\(title), \(headingText)"))
    }
}

// MARK: - Shelf

/// A horizontal run of cards. On a desktop it pages with arrows, which show only while the
/// pointer is over the shelf and only on the sides that have more.
struct HomeShelf<Content: View>: View {
    @Environment(\.homeWide) private var wide
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let spacing: CGFloat
    @ViewBuilder let content: () -> Content

    @State private var position = ScrollPosition()
    @State private var offset: CGFloat = 0
    @State private var extent: CGFloat = 0
    @State private var viewport: CGFloat = 0
    @State private var hovering = false

    private var more: (back: Bool, next: Bool) {
        (offset > 1, offset + viewport < extent - 1)
    }

    var body: some View {
        ScrollView(.horizontal) {
            LazyHStack(alignment: .top, spacing: spacing) {
                content()
            }
            .scrollTargetLayout()
        }
        .scrollIndicators(.never)
        .scrollPosition($position)
        .scrollClipDisabled()
        .onScrollGeometryChange(for: ShelfGeometry.self) { geometry in
            ShelfGeometry(
                offset: geometry.contentOffset.x, extent: geometry.contentSize.width,
                viewport: geometry.containerSize.width)
        } action: { _, now in
            offset = now.offset
            extent = now.extent
            viewport = now.viewport
        }
        .overlay(alignment: .leading) { arrow(forward: false) }
        .overlay(alignment: .trailing) { arrow(forward: true) }
        .onHover { inside in
            guard wide else { return }
            withAnimation(inside ? .fadeIn : .fadeOut) { hovering = inside }
        }
    }

    @ViewBuilder
    private func arrow(forward: Bool) -> some View {
        let available = forward ? more.next : more.back
        if wide {
            Button {
                let step = max(viewport - 80, 120)
                let target = forward ? min(offset + step, extent - viewport) : max(offset - step, 0)
                withAnimation(reduceMotion ? nil : .standard) { position.scrollTo(x: target) }
            } label: {
                Image(systemName: forward ? "chevron.right" : "chevron.left")
                    .font(.ui(14, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(width: 36, height: 36)
                    .background(Palette.raised, in: .circle)
                    .overlay(Circle().strokeBorder(Palette.glassEdge, lineWidth: 1))
                    .contentShape(.circle)
            }
            .buttonStyle(.plain)
            .padding(forward ? .trailing : .leading, 8)
            .opacity(hovering && available ? 1 : 0)
            .allowsHitTesting(hovering && available)
            .accessibilityLabel(forward ? L.homeNext : L.homePrevious)
            .accessibilityHidden(!available)
        }
    }

    private struct ShelfGeometry: Equatable {
        var offset: CGFloat
        var extent: CGFloat
        var viewport: CGFloat
    }
}

// MARK: - Continue card

/// A show under way: its wide art, how far into the episode, the title and what is left.
struct HomeContinueCard: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeWide) private var wide
    let item: HomeItem

    var body: some View {
        let entry = store.titles[item.entry.id] ?? item.entry
        let size = HomeMetrics.card(wide: wide)
        let art = store.heroArt(entry)
        Button {
            navigator.openTitle(entry.id)
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                Group {
                    if art.isCompact {
                        CoverArt(entry: entry, width: size.height * 0.45)
                            .frame(width: size.width, height: size.height, alignment: .leading)
                    } else {
                        HeroArtView(art: art, entry: entry)
                            .frame(width: size.width, height: size.height)
                            .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
                    }
                }
                .matchedTransitionSourceIfAvailable(id: entry.id)
                .playerZoomSource(entry.id)
                Group {
                    if let fraction = item.progress {
                        Meter(fraction: fraction, height: 3, silk: true)
                    } else {
                        Color.clear.frame(height: 3)
                    }
                }
                .padding(.top, 6)
                Text(entry.showTitle)
                    .font(.showTitle(size: wide ? 15 : 17, relativeTo: .body))
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                    .padding(.top, 6)
                HStack(spacing: 6) {
                    if item.new { Circle().fill(.water).frame(width: 6, height: 6).accessibilityHidden(true) }
                    Text(HomeCardLine.text(item, store: store))
                        .lineLimit(1)
                }
                .font(.ui(wide ? 13 : 15))
                .foregroundStyle(.haze)
            }
            .frame(width: size.width, alignment: .leading)
            .contentShape(.rect)
            .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(entry, value: HomeCardLine.spoken(item, store: store)) { HomeCardMenu(item: item, kind: .continue) }
        .contextMenu { HomeCardMenu(item: item, kind: .continue) }
        .onScrollVisibilityChange(threshold: 0.3) { visible in
            if visible { _ = store.loadStills(entry.id) }
        }
    }
}

// MARK: - Poster card

/// A title to start: its cover, the title and the short reason. The hover card and the
/// accessibility label carry the full reason.
struct HomePosterCard: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.homeWide) private var wide
    let item: HomeItem
    let kind: SectionKind

    var body: some View {
        let entry = store.titles[item.entry.id] ?? item.entry
        let size = HomeMetrics.poster(wide: wide)
        Button {
            navigator.openTitle(entry.id)
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                CoverArt(entry: entry, width: size.width)
                    .matchedTransitionSourceIfAvailable(id: entry.id)
                    .playerZoomSource(entry.id)
                    #if os(macOS)
                    .hoverCardSource(entry)
                    #endif
                Text(entry.showTitle)
                    .font(.showTitle(size: wide ? 15 : 17, relativeTo: .body))
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                    .padding(.top, 8)
                HStack(spacing: 6) {
                    if item.new { Circle().fill(.water).frame(width: 6, height: 6).accessibilityHidden(true) }
                    Text(HomeCardLine.text(item, store: store)).lineLimit(1)
                }
                .font(.ui(wide ? 13 : 15))
                .foregroundStyle(.haze)
                .padding(.top, 2)
            }
            .frame(width: size.width, alignment: .leading)
            .contentShape(.rect)
            .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(entry, value: HomeCardLine.spoken(item, store: store)) { HomeCardMenu(item: item, kind: kind) }
        .contextMenu { HomeCardMenu(item: item, kind: kind) }
    }
}

/// The words under a card, and what VoiceOver says for it.
enum HomeCardLine {
    @MainActor
    static func text(_ item: HomeItem, store: AppStore) -> String {
        if store.discovery.playable(item.entry.id, sent: item.playable) == .notFound { return L.homeNotFound }
        if let line = item.line { return line.text }
        if let reason = item.reason { return reasonText(reason) }
        if let airs = item.airsAt { return AirTime.clock(Date(timeIntervalSince1970: TimeInterval(airs))) }
        return HomeWords.starts(item.starts) ?? ""
    }

    static func reasonText(_ reason: UpNextReason) -> String {
        switch reason {
        case .newSeason: L.homeReasonNewSeason
        case .planning: L.homeReasonPlanning
        case .recommended: L.homeReasonRecommended
        case .airingToday: L.homeAiringToday
        }
    }

    /// The value after the title: the full reason when there is one, else the line.
    @MainActor
    static func spoken(_ item: HomeItem, store: AppStore) -> String {
        var parts: [String] = []
        if let because = item.because { parts.append(L.homeSpotlightBecause(title: because)) }
        parts.append(text(item, store: store))
        return parts.filter { !$0.isEmpty }.joined(separator: ", ")
    }
}

// MARK: - Menu

/// The menu on a card, on a long press or a right click.
struct HomeCardMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let item: HomeItem
    let kind: SectionKind

    var body: some View {
        let entry = item.entry
        let facts = HomeFacts(playable: store.discovery.playable(entry.id, sent: item.playable), action: item.action)
        if let episode = item.episode, let main = HomeMain(action: facts.action, episode: episode) {
            Button(main.title) {
                switch main.action {
                case .addToPlanning: store.send(.setStatus(id: entry.id, status: .planning))
                default: store.send(.play(id: entry.id, episode: String(episode)))
                }
            }
        } else if facts.action == .addToPlanning {
            Button(L.libraryAddPlanning) { store.send(.setStatus(id: entry.id, status: .planning)) }
        }
        if kind == .continue, let episode = item.episode, facts.action == .resume || facts.action == .play {
            Button(L.homeMarkWatched(episode: Int(episode))) {
                store.send(.setWatched(id: entry.id, episodes: [String(episode)], watched: true))
                navigator.watchedByHand += 1
            }
        }
        Divider()
        Button(L.titleOpen) { navigator.openTitle(entry.id, fromPreview: true) }
        Divider()
        if kind == .continue {
            Button(L.homeRemoveContinue) { store.hideFromContinue(entry.id) }
        } else if kind != .thisWeek {
            Button(L.upNextNotInterested) { store.notInterested(entry, title: entry.showTitle, unaired: item.starts != nil) }
        }
    }
}

// MARK: - This week

/// The next seven days from today: seven columns on a desktop, the next three days as a list on a
/// phone.
struct HomeWeek: View {
    @Environment(\.homeWide) private var wide
    let items: [HomeItem]

    var body: some View {
        let days = Dictionary(grouping: items, by: { Int($0.day ?? 0) })
        if wide {
            HStack(alignment: .top, spacing: 12) {
                ForEach(0..<7, id: \.self) { day in
                    HomeDayColumn(day: day, items: days[day] ?? [])
                }
            }
        } else {
            VStack(alignment: .leading, spacing: 18) {
                ForEach(0..<3, id: \.self) { day in
                    HomeDayList(day: day, items: days[day] ?? [])
                }
            }
        }
    }

    static func date(_ day: Int) -> Date {
        Calendar.current.date(byAdding: .day, value: day, to: Calendar.current.startOfDay(for: .now)) ?? .now
    }

    static func label(_ day: Int) -> String {
        day == 0 ? L.homeToday : CalendarWords.weekday(date(day))
    }
}

private struct HomeDayColumn: View {
    @Environment(Navigator.self) private var navigator
    let day: Int
    let items: [HomeItem]
    private let shown = 3

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(HomeWeek.label(day))
                .font(.ui(13, weight: .semibold))
                .foregroundStyle(day == 0 ? .silk : .haze)
                .accessibilityAddTraits(.isHeader)
            if items.isEmpty {
                Text(L.homeNothingAiring).font(.ui(13)).foregroundStyle(.haze)
            } else {
                ForEach(items.prefix(shown), id: \.entry.id) { item in
                    HomeWeekCell(item: item)
                }
                if items.count > shown {
                    Button(L.homeDayMore(count: items.count - shown)) { navigator.open(.calendar) }
                        .buttonStyle(.plain)
                        .font(.ui(13))
                        .foregroundStyle(.haze)
                        .pointerHover()
                }
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .background(day == 0 ? Palette.raised : .clear, in: .rect(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(day == 0 ? .clear : Palette.faintRule))
    }
}

private struct HomeDayList: View {
    let day: Int
    let items: [HomeItem]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(HomeWeek.label(day))
                .font(.ui(15, weight: .semibold))
                .foregroundStyle(day == 0 ? .silk : .haze)
                .accessibilityAddTraits(.isHeader)
            if items.isEmpty {
                Text(L.homeNothingAiring).font(.ui(15)).foregroundStyle(.haze)
            } else {
                ForEach(items, id: \.entry.id) { item in HomeWeekCell(item: item) }
            }
        }
    }
}

/// One broadcast of the week: its cover, the title and when it airs.
private struct HomeWeekCell: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let item: HomeItem

    var body: some View {
        let entry = store.titles[item.entry.id] ?? item.entry
        Button {
            navigator.openTitle(entry.id)
        } label: {
            HStack(spacing: 10) {
                ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: 120)
                    .frame(width: 32, height: 48)
                    .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: 32)))
                VStack(alignment: .leading, spacing: 1) {
                    Text(entry.showTitle)
                        .font(.showTitle(size: 13, relativeTo: .footnote))
                        .foregroundStyle(.silk)
                        .lineLimit(2)
                    Text(HomeCardLine.text(item, store: store))
                        .font(.ui(12))
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(entry, value: HomeCardLine.spoken(item, store: store)) { HomeCardMenu(item: item, kind: .thisWeek) }
        .contextMenu { HomeCardMenu(item: item, kind: .thisWeek) }
    }
}

// MARK: - Hover card extras

/// What the hover card adds for a title on Home: the full reason, a miss from the sources check,
/// and Not interested.
struct HomeHover {
    let entry: Entry
    let action: HomeAction
    let episode: UInt32?
    let line: String?
    let notFound: Bool
    let unaired: Bool
    let suggestion: Bool

    @MainActor
    init?(_ store: AppStore, _ id: AnilistId) {
        guard let view = store.discovery.view else { return nil }
        var found: (entry: Entry, action: HomeAction, episode: UInt32?, because: String?, reason: UpNextReason?,
                    playable: Playable, starts: FuzzyDate?, kind: SectionKind?)?
        if let spot = view.spotlight, spot.entry.id == id {
            found = (spot.entry, spot.action, spot.episode, spot.because, spot.reason, spot.playable, spot.starts, nil)
        }
        for section in view.sections where found == nil {
            if let item = section.items.first(where: { $0.entry.id == id }) {
                found = (item.entry, item.action, item.episode, item.because, item.reason, item.playable, item.starts, section.kind)
            }
        }
        guard let found else { return nil }
        let playable = store.discovery.playable(id, sent: found.playable)
        entry = found.entry
        notFound = playable == .notFound
        action = notFound && (found.action == .play || found.action == .resume) ? .addToPlanning : found.action
        episode = found.episode
        unaired = found.starts != nil
        suggestion = found.kind != .continue && found.kind != .thisWeek
        if notFound {
            line = L.homeNotFound
        } else if let because = found.because {
            line = L.homeSpotlightBecause(title: because)
        } else {
            line = found.reason.map(HomeCardLine.reasonText)
        }
    }
}
