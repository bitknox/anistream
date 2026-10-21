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

    init(_ entry: Entry) {
        episode = entry.progress?.next ?? 1
        resume = entry.resume
        airing = resume == nil ? entry.airing(of: episode) : .aired
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

    /// "Airs Saturday 18:30", or "Episode 4 is not out yet" when the schedule has no date.
    private var airsLine: String {
        airing.airs().map { "Airs \($0)" } ?? "Episode \(episode) is not out yet"
    }
}

/// A title's seasons, from AniList's relations in watch order, and what else it has: films, OVAs
/// and specials.
struct TitleFamily {
    struct Season: Identifiable, Equatable {
        let id: AnilistId
        let title: String
        let isThis: Bool
    }

    let seasons: [Season]
    let related: [RelatedTitle]

    init(_ entry: Entry) {
        let series: Set<String> = ["TV", "TV SHORT", "ONA"]
        let before = entry.related.filter { $0.relation == "prequel" && series.contains($0.format ?? "") }
        let after = entry.related.filter { $0.relation == "sequel" && series.contains($0.format ?? "") }
        let all = before.map { Season(id: $0.id, title: $0.title, isThis: false) }
            + [Season(id: entry.id, title: entry.title, isThis: true)]
            + after.map { Season(id: $0.id, title: $0.title, isThis: false) }
        seasons = all.count > 1 ? all : []
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
        .scrollIndicators(.never)
        .ignoresSafeArea(edges: .top)
        .background(.ground)
        .unwatchedRotor(store.titles.episodes[id] ?? [], in: rotor)
        .task(id: id) {
            navigator.focusedTitle = id
            if !store.titles.detailed.contains(id) { store.loadDetail(id) }
            if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
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
    let entry: Entry

    var body: some View {
        let action = TitleAction(entry)
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
                if let alternate = entry.alternateTitle {
                    Text(alternate)
                        .font(.mac(15))
                        .macLineHeight(20)
                        .foregroundStyle(.haze)
                        .padding(.top, 6)
                }
                Text(entry.titleMetadata)
                    .groupHeaderStyle()
                    .macLineHeight(14)
                    .padding(.top, 4)
                HStack(spacing: 10) {
                    if action.playable {
                        Button {
                            store.send(.play(id: entry.id, episode: String(action.episode)))
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "play.fill").font(.mac(10))
                                Text(action.title)
                            }
                        }
                        .buttonStyle(PrimaryCapsuleStyle())
                        .keyboardShortcut(.defaultAction)
                    } else {
                        AirsStatus(text: action.title)
                    }
                    if let trailer = entry.trailerUrl {
                        Button("Trailer") {
                            store.send(.playTrailer(url: trailer, title: entry.title))
                        }
                        .buttonStyle(SecondaryCapsuleStyle())
                    }
                }
                .padding(.top, 12)
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

    /// The metadata line on an iPhone's page: "TV · 12 EP · 2024 · 82%".
    var phoneMetadata: String {
        var parts: [String] = []
        if let format { parts.append(format) }
        if let episodes, episodes > 0, format != "MOVIE" { parts.append("\(episodes) ep") }
        if let year { parts.append(String(year)) }
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
            trailerUrl: nil, listStatus: nil,
        titleRomaji: nil, titleEnglish: nil, titleNative: nil, season: nil, startDate: nil, endDate: nil,
        source: nil, adult: false, detail: nil
        )
    }
}

/// Seasons, then Related: each a cover with its name and where it stands.
struct TitleFamilyRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let family: TitleFamily

    var body: some View {
        HStack(alignment: .top, spacing: 32) {
            if !family.seasons.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Seasons").groupHeaderStyle().padding(.horizontal, 4)
                        .accessibilityAddTraits(.isHeader)
                    HStack(spacing: 8) {
                        ForEach(Array(family.seasons.enumerated()), id: \.element.id) { index, season in
                            familyCell(
                                id: season.id,
                                name: "Season \(index + 1)",
                                detail: seasonDetail(season),
                                fraction: season.isThis ? entry.watchedFraction : nil,
                                selected: season.isThis
                            )
                        }
                    }
                }
            }
            if !family.related.isEmpty {
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
    }

    private func seasonDetail(_ season: TitleFamily.Season) -> String {
        if season.isThis, let done = entry.progress?.done, let of = entry.progressOf {
            return "\(done) / \(of.total)"
        }
        if let known = store.titles[season.id], let year = known.year { return String(year) }
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
                ArtworkView(url: known?.coverUrl, color: known?.plateColor, maxPixelSize: 108)
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
        .task { if store.titles[id] == nil { store.loadDetail(id) } }
    }
}

/// The episodes: a row each, the next one marked, stills of what comes after it blurred.
struct EpisodeList: View {
    @Environment(AppStore.self) private var store
    @AppStorage("anistream.spoilerProtection") private var spoilers = true
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
            ForEach(rows, id: \.number) { row in
                EpisodeRowView(entry: entry, row: row, spoilers: spoilers)
                    .accessibilityRotorEntry(id: row.number, in: rotor)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// The table, with the next broadcast after it when the source does not have it yet.
    private var episodeRows: [EpisodeRow] {
        var rows = store.titles.episodes[entry.id] ?? []
        if let next = entry.nextEpisode, !rows.contains(where: { $0.number == String(next) }),
           store.titles.episodes[entry.id] != nil {
            rows.append(EpisodeRow(
                number: String(next), title: nil, thumbnail: nil, description: nil,
                durationSecs: nil, watched: 0, completed: false, kind: nil, skippable: false, airsAt: nil
            ))
        }
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
        let number = UInt32(row.number) ?? 0
        let next = entry.progress?.next ?? 1
        let current = number == next
        let upcoming = entry.nextEpisode == number && !hasAired(number)
        Button {
            store.send(.play(id: entry.id, episode: row.number))
        } label: {
            HStack(spacing: 14) {
                Text(row.number)
                    .font(.mac(13, weight: current ? .semibold : .regular))
                    .monospacedDigit()
                    .foregroundStyle(current ? .amber : .haze)
                    .animation(.quick, value: current)
                    .frame(width: 22, alignment: .trailing)
                EpisodeStill(
                    entry: entry, row: row,
                    hidden: spoilers && number > next && !row.completed,
                    unaired: upcoming
                )
                .opacity(row.completed ? 0.7 : 1)
                VStack(alignment: .leading, spacing: 6) {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Text(row.title ?? "Episode \(row.number)")
                            .foregroundStyle(row.completed || upcoming ? .haze : .silk)
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
                trailing(current: current, upcoming: upcoming)
                WatchedCheck(watched: row.completed, size: 11)
                    .frame(width: 14)
            }
            .font(.mac(13))
            .padding(.horizontal, 8)
            .frame(height: 44)
            .background(current ? Palette.selection : .clear, in: .rect(cornerRadius: 8))
            .contentShape(.rect(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .pointerHover()
        .episodeElement(row, value: row.spokenValue(entry, current: current, upcoming: upcoming)) {
            EpisodeMenu(entry: entry, number: number, row: row)
        }
        .contextMenu { EpisodeMenu(entry: entry, number: number, row: row) }
    }

    private func hasAired(_ number: UInt32) -> Bool {
        (entry.latestAired?.episode ?? 0) >= number || store.titles.episodes[entry.id]?
            .contains(where: { $0.number == String(number) }) == true
    }

    @ViewBuilder
    private func trailing(current: Bool, upcoming: Bool) -> some View {
        if current, let resume = entry.resume, let fraction = resume.fraction, fraction > 0 {
            let left = max(1, Int(((resume.position / fraction - resume.position) / 60).rounded()))
            Held(value: left) { MinutesLeft(minutes: $0) }
        } else if upcoming, let when = nextAiring(entry) {
            HStack(spacing: 6) {
                Circle().fill(.water).frame(width: 6, height: 6)
                Text(when)
            }
            .foregroundStyle(.water)
        } else if let seconds = row.durationSecs {
            Text("\(max(1, Int((Double(seconds) / 60).rounded()))) min").foregroundStyle(.haze)
        }
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

/// The synopsis and the facts beside the episodes.
struct TitleDetails: View {
    let entry: Entry

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            if !entry.synopsis.isEmpty {
                ClampedText(text: entry.synopsis, lines: 8)
                    .font(.mac(13))
                    .macLineHeight(20)
                    .foregroundStyle(.haze)
                    .id(entry.id)
            }
            Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 10) {
                fact("Studio", entry.studio)
                fact("Season", entry.year.map(String.init))
                fact("Next episode", nextAiring(entry))
                fact("Also on", entry.availableOn.isEmpty ? nil : ListFormatter.localizedString(byJoining: entry.availableOn))
            }
            .font(.mac(12))
        }
        .padding(.top, 4)
    }

    @ViewBuilder
    private func fact(_ name: String, _ value: String?) -> some View {
        if let value {
            GridRow {
                Text(name).foregroundStyle(.haze).frame(width: 124, alignment: .leading)
                Text(value).foregroundStyle(.silk)
            }
        }
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
