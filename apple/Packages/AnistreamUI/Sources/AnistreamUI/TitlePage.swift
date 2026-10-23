// A title's page: what it is, its seasons, its episodes, and the one amber action.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// The episode a title's action plays, and how it says so.
struct TitleAction: Equatable {
    let episode: UInt32
    let resume: ResumePoint?
    /// Whether the episode is out, by the title's broadcasts.
    let airing: EpisodeAvailability
    /// The episode another device handed over, which the action resumes where it was there.
    let handedOff: HandedOff?

    init(_ entry: Entry, handedOff: HandedOff? = nil) {
        if let handedOff, handedOff.id == entry.id, let number = UInt32(handedOff.episode) {
            episode = number
            resume = ResumePoint(position: handedOff.position, fraction: nil)
            airing = .aired
            self.handedOff = handedOff
            return
        }
        episode = entry.progress?.next ?? 1
        resume = entry.resume
        airing = resume == nil ? entry.airing(of: episode) : .aired
        self.handedOff = nil
    }

    /// Play it: from the handed-over place, which is then used up, or as the engine would.
    @MainActor
    func play(_ id: AnilistId, store: AppStore, navigator: Navigator) {
        if let handedOff {
            store.play(id, episode: handedOff.episode, at: handedOff.position)
            navigator.handedOff = nil
        } else {
            store.send(.play(id: id, episode: String(episode)))
        }
    }

    /// Whether there is an episode to play: never one still to air, which nothing could find.
    var playable: Bool { airing.hasAired }

    /// "Resume Episode 7 · 10:06", "Start Episode 4": title case, as a Mac button says it. With
    /// the episode still to air, the status in the button's place: "Airs Saturday 18:30".
    var title: String {
        if let resume { return "Resume Episode \(episode) · \(resume.clock)" }
        if !playable { return airsLine }
        return "Start Episode \(episode)"
    }

    /// The same in sentence case, as iPhone says it: "Resume episode 7 · 10:06".
    var sentence: String {
        if let resume { return "Resume episode \(episode) · \(resume.clock)" }
        if !playable { return airsLine }
        return "Start episode \(episode)"
    }

    /// "Episode 10 airs Thursday 16:30", or "Episode 10 is not out yet" when nothing dates it.
    private var airsLine: String {
        "Episode \(episode) " + (airing.airs().map { "airs \($0)" } ?? "is not out yet")
    }
}

/// A title's seasons, and what else it has: films, OVAs and specials.
///
/// The seasons are the franchise's, as the engine walks them along the prequel and sequel chain:
/// the same list, numbered the same, on every one of their pages. Until the engine has walked
/// them there are none, rather than seasons numbered from what this page alone knows, which
/// would renumber as the rest arrived.
struct TitleFamily {
    struct Season: Identifiable, Equatable {
        let id: AnilistId
        let title: String
        let isThis: Bool
    }

    let seasons: [Season]
    /// What the picker holds, or will once the engine has walked the seasons: this title's own
    /// series prequels and sequels meanwhile, kept out of Related so that none moves from there
    /// into the picker.
    let picked: Set<AnilistId>
    let related: [RelatedTitle]

    init(_ entry: Entry) {
        let series: Set<String> = ["TV", "TV SHORT", "ONA"]
        let franchise = entry.detail?.seasons
        seasons = (franchise ?? []).map { Season(id: $0.id, title: $0.title, isThis: $0.id == entry.id) }
        let waiting: [RelatedTitle] = franchise != nil ? [] : entry.related.filter {
            ($0.relation == "prequel" || $0.relation == "sequel") && series.contains($0.format ?? "")
        }
        picked = Set(seasons.map(\.id) + waiting.map(\.id))
        related = entry.related.filter { !series.contains($0.format ?? "") }
    }
}

/// Where a title's next episode airs: "Thursday 16:30".
func nextAiring(_ entry: Entry, now: Date = .now) -> String? {
    guard let seconds = entry.nextAiringIn else { return nil }
    return CalendarWords.weekdayTime(now.addingTimeInterval(TimeInterval(seconds)), now: now)
}

/// The Mac's title page, over the section it was opened from.
struct TitlePage: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.bannerSpace) private var bannerSpace
    @Namespace private var rotor
    let id: AnilistId

    var body: some View {
        let entry = store.titles[id] ?? Entry.placeholder(id)
        let art = store.heroArt(entry)
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // With no wide art the page starts higher: under the toolbar, not under a banner.
                Color.clear.frame(height: art.isCompact ? 64 : 96)
                TitleHero(entry: entry)
                let family = TitleFamily(entry)
                if !family.seasons.isEmpty || !family.related.isEmpty {
                    TitleFamilyRow(entry: entry, family: family)
                        .padding(.top, 20)
                }
                HStack(alignment: .top, spacing: 32) {
                    EpisodeList(entry: entry, rotor: rotor)
                    TitleDetails(entry: entry)
                        .frame(width: 260)
                }
                .padding(.top, 24)
            }
            .padding(EdgeInsets(top: 0, leading: 24, bottom: 20, trailing: 28))
            .background(alignment: .top) {
                // The wide art, under the clear toolbar, with the ground over its lower part,
                // where the cover and the title sit. None at all where there is none.
                if !art.isCompact {
                    HeroArtView(art: art, entry: entry)
                        .frame(height: 280)
                        .clipped()
                        .overlay(alignment: .bottom) {
                            Palette.ground.frame(height: 96)
                        }
                        .modifier(BannerMatch(id: entry.id, space: bannerSpace))
                        .playerZoomSource(entry.id)
                }
            }
        }
        .nudgeableScroll()
        .scrollIndicators(.never)
        .ignoresSafeArea(edges: .top)
        .background(.ground)
        .unwatchedRotor(store.titles.episodes[id] ?? [], in: rotor)
        .handsOff(title: entry)
        .task(id: id) {
            navigator.focusedTitle = id
            if !store.titles.detailed.contains(id) { store.loadDetail(id) }
            if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
            // What the show sets for itself, its spoiler protection among it: from the database.
            store.loadShowSettings(id)
        }
    }
}

/// The banner's part in the Mac's morph from Continue's preview into the title page (M3).
struct BannerMatch: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let id: AnilistId
    let space: Namespace.ID?

    /// Under Reduce Motion the banner does not travel: the page crossfades in (M3).
    func body(content: Content) -> some View {
        if let space, !reduceMotion {
            content.matchedGeometryEffect(id: "banner-\(id)", in: space)
        } else {
            content
        }
    }
}

struct BannerSpaceKey: EnvironmentKey {
    static let defaultValue: Namespace.ID? = nil
}

extension EnvironmentValues {
    /// The namespace the Mac's banner morph runs in.
    var bannerSpace: Namespace.ID? {
        get { self[BannerSpaceKey.self] }
        set { self[BannerSpaceKey.self] = newValue }
    }
}

/// The cover, the title block and the actions.
struct TitleHero: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    /// On an iPad the page speaks in sentence case and its actions are touch-sized (PadTitleMeta).
    #if os(iOS)
    static let pad = true
    #else
    static let pad = false
    #endif

    var body: some View {
        let action = TitleAction(entry, handedOff: navigator.handedOff)
        HStack(alignment: .bottom, spacing: 28) {
            ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: 460)
                .frame(width: 150, height: 225)
                .clipShape(.rect(cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassEdge.opacity(1.25), lineWidth: 1))
            VStack(alignment: .leading, spacing: 0) {
                Text(entry.showTitle)
                    .font(.macShowTitle(size: 36, relativeTo: .largeTitle))
                    .macLineHeight(44, relativeTo: .largeTitle)
                    .foregroundStyle(.silk)
                    .lineLimit(2)
                    .accessibilityAddTraits(.isHeader)
                OtherTitlesLine(entry: entry)
                    .padding(.top, 6)
                Text(entry.titleMetadata)
                    .groupHeaderStyle()
                    .macLineHeight(14)
                    .padding(.top, 4)
                HStack(spacing: 10) {
                    // No source has it: nothing to play, and the way to one in Play's place.
                    if action.playable, store.titles.unlisted(entry.id) {
                        AddSourceButton(
                            sentence: Self.pad, height: Self.pad ? 48 : 34, fontSize: Self.pad ? 17 : 13
                        )
                    } else if action.playable {
                        Button {
                            action.play(entry.id, store: store, navigator: navigator)
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "play.fill").font(.mac(10))
                                Text(Self.pad ? action.sentence : action.title)
                            }
                        }
                        .buttonStyle(PrimaryCapsuleStyle(height: Self.pad ? 48 : 34, fontSize: Self.pad ? 17 : 13))
                        .keyboardShortcut(.defaultAction)
                    } else {
                        AirsStatus(text: Self.pad ? action.sentence : action.title)
                    }
                    if let trailer = entry.trailerUrl {
                        Button("Trailer") {
                            store.send(.playTrailer(url: trailer, title: entry.title))
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: Self.pad ? 48 : 28, fontSize: Self.pad ? 17 : 13))
                    }
                    #if os(iOS)
                    // The list's menu, in the header where a narrow window's bar has no room.
                    Menu {
                        StatusItems(id: entry.id)
                    } label: {
                        HStack(spacing: 8) {
                            Text(entry.listStatus.map { $0.label.capitalizedSentence } ?? "Add to list")
                            Image(systemName: "chevron.up.chevron.down").font(.ui(13, weight: .semibold))
                        }
                    }
                    .menuStyle(.button)
                    .buttonStyle(SecondaryCapsuleStyle(height: 48, fontSize: 17))
                    #endif
                }
                .padding(.top, Self.pad ? 16 : 12)
            }
            .padding(.bottom, 4)
        }
    }
}

extension Entry {
    /// The page's metadata line: format, episodes, year, studio and score.
    var titleMetadata: String {
        var line = metadataLine
        if let score { line += (line.isEmpty ? "" : " · ") + "\(score)%" }
        return line
    }

    /// The metadata line on an iPhone's page: "TV · 24 EP · SUMMER 2026 · 79%", "18+" first for an
    /// adult title.
    var phoneMetadata: String {
        var parts: [String] = adult ? ["18+"] : []
        if let format { parts.append(TitleWords.format(format)) }
        if let episodes, episodes > 0, format != "MOVIE" { parts.append("\(episodes) ep") }
        if let seasonWords { parts.append(seasonWords) }
        if let score { parts.append("\(score)%") }
        return parts.joined(separator: " · ")
    }

    /// An entry for a page whose title has not arrived yet.
    static func placeholder(_ id: AnilistId) -> Entry {
        Entry(
            id: id, title: "", secondary: nil, format: nil, episodes: nil, year: nil, score: nil,
            studio: nil, synopsis: "", coverUrl: nil, coverColor: nil, bannerUrl: nil, genres: [],
            availableOn: [], progress: nil, airingIn: nil, airingEpisode: nil, nextEpisode: nil,
            nextAiringIn: nil, lastAired: nil, resume: nil, related: [], recommended: [],
            trailerUrl: nil, listStatus: nil, titleRomaji: nil, titleEnglish: nil, titleNative: nil,
            season: nil, startDate: nil, endDate: nil, source: nil, adult: false, detail: nil, status: nil,
            synopsisSpans: []
        )
    }
}

/// Seasons, then Related: each a cover with its name and where it stands.
struct TitleFamilyRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let family: TitleFamily

    /// A related card's cover: 36 by 54 on the Mac, 40 by 60 on iPad.
    static var cover: CGSize {
        #if os(macOS)
        CGSize(width: 36, height: 54)
        #else
        CGSize(width: 40, height: 60)
        #endif
    }

    /// How wide Related keeps itself beside the seasons: a card and part of the next, cut by the
    /// edge to say the row scrolls.
    static let relatedRoom: CGFloat = 240

    var body: some View {
        // Side by side, as MacTitle and PadTitle have them, while the seasons fit with room for
        // Related beside them. A long franchise's seasons scroll sideways on a line of their own
        // instead, the season on screen in view, with Related under them.
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 32) {
                if !family.seasons.isEmpty {
                    seasonsGroup {
                        HStack(spacing: 8) {
                            ForEach(Array(family.seasons.enumerated()), id: \.element.id) { index, season in
                                seasonCell(index, season)
                            }
                        }
                    }
                }
                if hasRelated {
                    related.frame(minWidth: Self.relatedRoom, idealWidth: Self.relatedRoom, maxWidth: .infinity, alignment: .leading)
                }
            }
            VStack(alignment: .leading, spacing: 20) {
                if !family.seasons.isEmpty {
                    seasonsGroup { SeasonScroller(family: family) { index, season in seasonCell(index, season) } }
                }
                related
            }
        }
    }

    /// Whether there is anything under Related.
    private var hasRelated: Bool {
        entry.detail != nil ? !entry.relatedCards(excluding: family.picked).isEmpty : !family.related.isEmpty
    }

    private func seasonsGroup(@ViewBuilder _ row: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Seasons").groupHeaderStyle().padding(.horizontal, 4)
                .accessibilityAddTraits(.isHeader)
            row()
        }
    }

    private func seasonCell(_ index: Int, _ season: TitleFamily.Season) -> some View {
        familyCell(
            id: season.id,
            name: "Season \(index + 1)",
            detail: seasonDetail(season),
            fraction: season.isThis ? entry.watchedFraction : nil,
            selected: season.isThis
        )
    }

    @ViewBuilder private var related: some View {
        if entry.detail != nil {
            let cards = entry.relatedCards(excluding: family.picked)
            if !cards.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Related").groupHeaderStyle().padding(.horizontal, 4)
                        .accessibilityAddTraits(.isHeader)
                    // A row that scrolls sideways, its last card cut by the edge to say so.
                    ScrollView(.horizontal) {
                        HStack(spacing: 8) {
                            ForEach(cards, id: \.id) { linked in
                                RelatedCard(linked: linked, cover: Self.cover)
                                    .frame(height: 62)
                            }
                        }
                    }
                    .scrollIndicators(.never)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        } else if !family.related.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text("Related").groupHeaderStyle().padding(.horizontal, 4)
                    .accessibilityAddTraits(.isHeader)
                HStack(spacing: 8) {
                    ForEach(family.related, id: \.id) { related in
                        familyCell(
                            id: related.id,
                            name: related.title,
                            detail: [related.format?.capitalizedSentence, related.relation.capitalizedSentence]
                                .compactMap { $0 }.joined(separator: " · "),
                            fraction: nil,
                            selected: false
                        )
                    }
                }
            }
        }
    }

    private func seasonDetail(_ season: TitleFamily.Season) -> String {
        if season.isThis, let done = entry.progress?.done, let of = entry.progressOf {
            return "\(done) / \(of.total)"
        }
        if let known = store.titles[season.id], let words = known.seasonWords { return words }
        // The page's own card dates a season the app has not loaded: "Spring 2027".
        if let linked = card(season.id), let year = linked.year {
            return linked.season.map { "\(TitleWords.season($0)) \(year)" } ?? String(year)
        }
        return season.isThis ? "This season" : "TV"
    }

    private func familyCell(
        id: AnilistId, name: String, detail: String, fraction: Double?, selected: Bool
    ) -> some View {
        Button {
            if !selected { navigator.openTitle(id) }
        } label: {
            HStack(spacing: 10) {
                let known = store.titles[id]
                let linked = card(id)
                ArtworkView(url: known?.coverUrl ?? linked?.coverUrl, color: known?.plateColor, maxPixelSize: 108)
                    .frame(width: 36, height: 54)
                    .clipShape(.rect(cornerRadius: 6))
                VStack(alignment: .leading, spacing: 0) {
                    Text(name)
                        .font(.mac(13, weight: .semibold))
                        .macLineHeight(18)
                        .foregroundStyle(selected ? .amber : .silk)
                        .lineLimit(1)
                    Text(detail)
                        .font(.mac(12))
                        .macLineHeight(16)
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                        // "6 / 12" to "7 / 12", the changed digit rolling (T9).
                        .contentTransition(.numericText())
                        .animation(.standard, value: detail)
                    if let fraction {
                        Meter(fraction: fraction, height: 3).frame(width: 112).padding(.top, 6)
                    }
                }
                .frame(minWidth: 72, alignment: .leading)
            }
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 4, trailing: 14))
            .frame(height: 62)
            .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 10))
            .contentShape(.rect(cornerRadius: 10))
        }
        .buttonStyle(.plain)
        .pointerHover()
        // The cover, where the page's seasons and relations do not carry it.
        .task { if store.titles[id] == nil, card(id) == nil { store.loadDetail(id) } }
    }

    /// The page's own card for `id`: a season's, which every season has, or a relation's.
    private func card(_ id: AnilistId) -> LinkedTitle? {
        entry.detail?.seasons?.first { $0.id == id } ?? entry.detail?.relations.first { $0.id == id }
    }
}

/// A long franchise's seasons in a row that scrolls sideways, its last cell cut by the edge to say
/// so: lazy, so only the seasons in view are made and only their covers asked for, and the season
/// on screen in the middle as the page opens and as it changes.
struct SeasonScroller<Cell: View>: View {
    let family: TitleFamily
    @ViewBuilder let cell: (Int, TitleFamily.Season) -> Cell

    var body: some View {
        let chosen = family.seasons.first(where: \.isThis)?.id
        ScrollViewReader { proxy in
            ScrollView(.horizontal) {
                LazyHStack(spacing: 8) {
                    ForEach(Array(family.seasons.enumerated()), id: \.element.id) { index, season in
                        cell(index, season).id(season.id)
                    }
                }
            }
            .scrollIndicators(.never)
            .onAppear { if let chosen { proxy.scrollTo(chosen, anchor: .center) } }
            .onChange(of: chosen) { _, now in
                if let now { withAnimation(.standard) { proxy.scrollTo(now, anchor: .center) } }
            }
        }
    }
}

/// The episodes: a row each, the next one marked, stills of what comes after it blurred.
struct EpisodeList: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var unwatchedOnly = false
    let entry: Entry
    /// Where the page's Unwatched episodes rotor finds the rows.
    let rotor: Namespace.ID

    var body: some View {
        let rows = episodeRows
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                Text("Episodes")
                    .font(.mac(15, weight: .semibold))
                    .foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                Spacer()
                SegmentedCapsule(selection: $unwatchedOnly, options: [(false, "All"), (true, "Unwatched")])
            }
            .frame(height: 28)
            .padding(.bottom, 6)
            if store.titles.episodes[entry.id] == nil {
                ForEach(0..<4, id: \.self) { _ in EpisodePlaceholderRow() }
            } else if rows.isEmpty {
                QuietLine(unwatchedOnly ? "Every episode is watched" : "No episodes yet")
            }
            // The watched run folds away above the last episode watched; Unwatched has none.
            let fold = unwatchedOnly ? nil : WatchedFold(rows)
            if let fold {
                WatchedFoldSection(
                    fold: fold, shown: navigator.watchedShown(entry.id), phone: false,
                    toggle: { navigator.toggleWatched(entry.id) }
                ) {
                    ForEach(rows.prefix(fold.count), id: \.number) { row($0) }
                }
            }
            ForEach(rows.dropFirst(fold?.count ?? 0), id: \.number) { row($0) }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func row(_ row: EpisodeRow) -> some View {
        EpisodeRowView(entry: entry, row: row, spoilers: store.spoilerProtection(for: entry.id))
            .accessibilityRotorEntry(id: row.number, in: rotor)
    }

    /// The table, dated by the schedule, with the next episode still to air at its end.
    private var episodeRows: [EpisodeRow] {
        guard let loaded = store.titles.episodes[entry.id] else { return [] }
        let rows = entry.episodeTable(loaded)
        return unwatchedOnly ? rows.filter { !$0.completed } : rows
    }
}

/// One episode in the list.
struct EpisodeRowView: View {
    @Environment(AppStore.self) private var store
    let entry: Entry
    let row: EpisodeRow
    let spoilers: Bool

    var body: some View {
        if row.isUnaired(), let date = row.airsDate {
            UnairedEpisodeRow(row: row, airs: date)
        } else {
            airedRow
                #if os(macOS)
                .help(row.airsDate.map(AirsWords.aired) ?? "")
                #endif
        }
    }

    @ViewBuilder
    private var airedRow: some View {
        let number = UInt32(row.number) ?? 0
        let current = number == (entry.progress?.next ?? 1)
        // One no source has: nothing to play, so a click offers what can be done, adding a
        // source among it, rather than playing.
        if row.metadataOnly {
            Menu {
                EpisodeMenu(entry: entry, number: number, row: row)
            } label: {
                rowLabel(number: number, current: current)
            }
            .menuStyle(.button)
            .menuIndicator(.hidden)
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: row.spokenValue(entry, current: current, upcoming: false)) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
        } else {
            Button {
                store.send(.play(id: entry.id, episode: row.number))
            } label: {
                rowLabel(number: number, current: current)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: row.spokenValue(entry, current: current, upcoming: false)) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
            .contextMenu { EpisodeMenu(entry: entry, number: number, row: row) }
        }
    }

    private func rowLabel(number: UInt32, current: Bool) -> some View {
        let next = entry.progress?.next ?? 1
        return HStack(spacing: 14) {
            Text(row.number)
                .font(.mac(13, weight: current ? .semibold : .regular))
                .monospacedDigit()
                .foregroundStyle(current ? .amber : .haze)
                .animation(.quick, value: current)
                .frame(width: 22, alignment: .trailing)
            EpisodeStill(
                entry: entry, row: row,
                hidden: spoilers && number > next && !row.completed,
                unaired: false
            )
            .opacity(row.completed ? 0.7 : 1)
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Text(row.title ?? "Episode \(row.number)")
                        .foregroundStyle(row.completed ? .haze : .silk)
                        .lineLimit(1)
                        .animation(.quick, value: row.completed)
                    if row.kind == .filler {
                        Text("Filler").groupHeaderStyle()
                    }
                }
                if current, let fraction = entry.resume?.fraction {
                    Held(value: fraction) { Meter(fraction: $0, height: 3).frame(width: 200) }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing(current: current)
            WatchedCheck(watched: row.completed, size: 11)
                .frame(width: 14)
        }
        .font(.mac(13))
        .padding(.horizontal, 8)
        .frame(height: 44)
        .background(current ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
        .contentShape(.rect(cornerRadius: 8))
    }

    @ViewBuilder
    private func trailing(current: Bool) -> some View {
        if current, let resume = entry.resume, let fraction = resume.fraction, fraction > 0 {
            let left = max(1, Int(((resume.position / fraction - resume.position) / 60).rounded()))
            Held(value: left) { MinutesLeft(minutes: $0) }
        } else if let seconds = row.durationSecs {
            Text("\(max(1, Int((Double(seconds) / 60).rounded()))) min").foregroundStyle(.haze)
        }
    }
}

/// The next episode still to air: no button, since nothing can play it; the flat placeholder for
/// its still, its title in haze, and when it airs in water ("Airs Thursday 16:30").
struct UnairedEpisodeRow: View {
    let row: EpisodeRow
    let airs: Date

    var body: some View {
        let title = row.title ?? "Episode \(row.number)"
        let when = AirsWords.airs(airs)
        HStack(spacing: 14) {
            Text(row.number)
                .monospacedDigit()
                .foregroundStyle(.haze)
                .frame(width: 22, alignment: .trailing)
            RoundedRectangle(cornerRadius: 6).fill(Palette.artPlaceholder).frame(width: 64, height: 36)
            Text(title).foregroundStyle(.haze).lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 6) {
                Circle().fill(.water).frame(width: 6, height: 6)
                Text(when).monospacedDigit()
            }
            .foregroundStyle(.water)
            Color.clear.frame(width: 14)
        }
        .font(.mac(13))
        .padding(.horizontal, 8)
        .frame(height: 44)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Episode \(row.number)" + (row.title.map { ", \($0)" } ?? "") + ". \(when), not out yet")
    }
}

/// An episode's still: its art, blurred with an eye when spoiler protection hides it, and a plain
/// plate before it airs.
struct EpisodeStill: View {
    let entry: Entry
    let row: EpisodeRow
    let hidden: Bool
    let unaired: Bool
    var size = CGSize(width: 64, height: 36)

    var body: some View {
        Group {
            if unaired {
                Rectangle().fill(CoverPlate.muted(entry.plateColor))
            } else if hidden {
                ZStack {
                    ArtworkView(url: row.thumbnail ?? entry.bannerUrl, color: entry.plateColor, maxPixelSize: 128)
                        .blur(radius: 4)
                        .scaleEffect(1.15)
                    Image(systemName: "eye.slash")
                        .font(.mac(12, weight: .medium))
                        .foregroundStyle(.silk)
                }
                .accessibilityLabel("Still hidden: spoiler protection")
            } else {
                ArtworkView(url: row.thumbnail ?? entry.bannerUrl, color: entry.plateColor, maxPixelSize: 128)
            }
        }
        .frame(width: size.width, height: size.height)
        .clipShape(.rect(cornerRadius: 6))
    }
}

/// An episode row while the table loads.
struct EpisodePlaceholderRow: View {
    var body: some View {
        HStack(spacing: 14) {
            Text("7").frame(width: 22, alignment: .trailing)
            RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: 64, height: 36)
            Text("The Lantern Road")
            Spacer()
            Text("24 min")
        }
        .font(.mac(13))
        .foregroundStyle(.haze)
        .redacted(reason: .placeholder)
        .padding(.horizontal, 8)
        .frame(height: 44)
        .accessibilityHidden(true)
    }
}

/// The synopsis, the facts and the sheet's opener beside the episodes: the synopsis whole on the
/// Mac, three lines on iPad with "More" to show the rest in place.
struct TitleDetails: View {
    let entry: Entry

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            let synopsis = entry.shownSynopsis
            if !synopsis.isEmpty {
                #if os(macOS)
                Text(synopsis)
                    .font(.mac(13))
                    .macLineHeight(20)
                    .foregroundStyle(.haze)
                    .fixedSize(horizontal: false, vertical: true)
                #else
                InlineMoreText(text: synopsis, lines: 3, font: .mac(13))
                    .id(entry.id)
                #endif
            }
            MetaFactsGrid(entry: entry)
            MoreAboutButton(entry: entry)
                .padding(.top, 4)
        }
        .padding(.top, 4)
    }
}

/// The title page's toolbar on the Mac: back, its list, and More.
struct TitleToolbar: ToolbarContent {
    let store: AppStore
    let navigator: Navigator
    let id: AnilistId

    var body: some ToolbarContent {
        ToolbarItem(placement: .navigation) {
            Button("Back", systemImage: "chevron.left") {
                withAnimation(.standard) { navigator.back() }
            }
            .keyboardShortcut("[", modifiers: .command)
        }
        ToolbarItem(placement: .primaryAction) {
            Menu {
                StatusItems(id: id)
            } label: {
                Text(store.titles[id]?.listStatus.map { $0.label.capitalizedSentence } ?? "Add to List")
            }
            .fixedSize()
        }
        ToolbarItem(placement: .primaryAction) {
            Menu {
                if let entry = store.titles[id] {
                    TitleMoreItems(entry: entry)
                }
            } label: {
                Label("More", systemImage: "ellipsis")
            }
        }
    }
}
