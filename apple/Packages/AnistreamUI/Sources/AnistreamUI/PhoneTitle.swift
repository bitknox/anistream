// A title's page on iPhone and iPad, as PhoneTitle draws it.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

struct PhoneTitle: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dynamicTypeSize) private var typeSize
    @Namespace private var rotor
    let id: AnilistId

    var body: some View {
        let entry = store.titles[id] ?? Entry.placeholder(id)
        let action = TitleAction(entry, handedOff: navigator.handedOff)
        let art = store.heroArt(entry)
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // The wide art: it stretches when pulled down and scrolls at half speed (T1).
                // With none, the page starts under the navigation bar, the cover beside the title.
                if !art.isCompact {
                    HeroArtView(art: art, entry: entry)
                        .frame(height: 250)
                        // The ground fading up from the art's foot, under the cover and a title
                        // of three lines.
                        .overlay(alignment: .bottom) { BannerScrim(held: 38, height: 82) }
                        .visualEffect { [reduceMotion] content, proxy in
                            let y = proxy.frame(in: .scrollView(axis: .vertical)).minY
                            let h = proxy.size.height
                            return content
                                .scaleEffect(reduceMotion || y <= 0 ? 1 : (h + y) / h, anchor: .bottom)
                                .offset(y: reduceMotion || y >= 0 ? 0 : -y * 0.5)
                        }
                        .clipped()
                }
                HStack(alignment: .bottom, spacing: 16) {
                    ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: 400)
                        .frame(width: 100, height: 150)
                        .clipShape(.rect(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassEdge.opacity(1.25)))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(entry.showTitle)
                            .font(.showTitle(size: 22, relativeTo: .title2))
                            .foregroundStyle(.silk)
                            // As large as `.accessibility3`, in four lines at most.
                            .lineLimit(4)
                            .dynamicTypeSize(...DynamicTypeSize.accessibility3)
                            .titleOverArt()
                            .accessibilityAddTraits(.isHeader)
                        OtherTitlesLine(entry: entry, phone: true)
                        Text(entry.phoneMetadata)
                            .groupHeaderStyle()
                            .padding(.top, 2)
                    }
                    .padding(.bottom, 4)
                }
                .padding(.horizontal, 16)
                // Over the art's foot, except at the accessibility sizes, where the title would
                // grow up into the art and lose its contrast.
                .padding(.top, art.isCompact || typeSize.isAccessibilitySize ? 8 : -60)

                VStack(alignment: .leading, spacing: 16) {
                    // No source has it: nothing to play, and the way to one in Play's place.
                    if action.playable, store.titles.unlisted(entry.id) {
                        AddSourceButton(sentence: true, height: 50, fontSize: 17, fills: true)
                    } else if action.playable {
                        Button {
                            action.play(entry.id, store: store, navigator: navigator)
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "play.fill").font(.ui(13))
                                // Whole on one line, which it fits; wrapped at the accessibility sizes.
                                Text(action.sentence)
                                    .fixedSize(horizontal: !typeSize.isAccessibilitySize, vertical: false)
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))
                    } else {
                        // Still to air: when, in the button's place.
                        AirsStatus(text: action.sentence, height: 50, fontSize: 17, fills: true)
                    }

                    HStack(spacing: 12) {
                        Menu {
                            StatusItems(id: entry.id)
                        } label: {
                            HStack(spacing: 6) {
                                Text(entry.listStatus.map { $0.label.capitalizedSentence } ?? "Add to list")
                                Image(systemName: "chevron.up.chevron.down").font(.ui(11, weight: .semibold))
                            }
                            .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 15))
                        if let trailer = entry.trailerUrl {
                            Button {
                                store.send(.playTrailer(url: trailer, title: entry.title))
                            } label: {
                                Text("Trailer").frame(maxWidth: .infinity)
                            }
                            .buttonStyle(SecondaryCapsuleStyle(height: 44, fontSize: 15))
                        }
                    }

                    VStack(alignment: .leading, spacing: 12) {
                        if !entry.shownSynopsis.isEmpty {
                            // Two lines, and "More" shows the rest in place.
                            InlineMoreText(text: entry.shownSynopsis, lines: 2, font: .ui(15))
                                .id(entry.id)
                                // Read after the episodes: title, metadata, the main button,
                                // progress, episodes, then the synopsis.
                                .accessibilitySortPriority(-1)
                        }
                        // The facts and the top tags, a line each, cut at the end.
                        VStack(alignment: .leading, spacing: 4) {
                            if let facts = entry.factsLine() {
                                Text(facts).lineLimit(1)
                            }
                            if !entry.topTags.isEmpty {
                                Text(entry.topTags.joined(separator: " · ")).lineLimit(1)
                            }
                        }
                        .font(.ui(15))
                        .foregroundStyle(.haze)
                        MoreAboutRow(entry: entry)
                    }

                    let family = TitleFamily(entry)
                    if !family.seasons.isEmpty {
                        // The season on screen in the middle as the page opens, so a long
                        // franchise's later seasons open with their own chip in view.
                        ScrollViewReader { proxy in
                            ScrollView(.horizontal) {
                                HStack(spacing: 8) {
                                    ForEach(Array(family.seasons.enumerated()), id: \.element.id) { index, season in
                                        seasonChip(index: index, season: season, entry: entry).id(season.id)
                                    }
                                }
                                .padding(.horizontal, 16)
                            }
                            .scrollIndicators(.never)
                            .padding(.horizontal, -16)
                            .onAppear { proxy.scrollTo(entry.id, anchor: .center) }
                        }
                    }
                    if entry.detail != nil {
                        let cards = entry.relatedCards(excluding: family.picked)
                        if !cards.isEmpty {
                            VStack(alignment: .leading, spacing: 6) {
                                Text("Related").font(.ui(15)).foregroundStyle(.haze)
                                    .accessibilityAddTraits(.isHeader)
                                ScrollView(.horizontal) {
                                    HStack(spacing: 4) {
                                        ForEach(cards, id: \.id) { linked in
                                            RelatedCard(linked: linked, cover: CGSize(width: 40, height: 60), phone: true)
                                        }
                                    }
                                    .padding(.horizontal, 12)
                                }
                                .scrollIndicators(.never)
                                .padding(.horizontal, -16)
                            }
                        }
                    } else if !family.related.isEmpty {
                        HStack(spacing: 8) {
                            Text("Related").font(.ui(13)).foregroundStyle(.haze)
                            ForEach(family.related, id: \.id) { related in
                                Button {
                                    navigator.openTitle(related.id)
                                } label: {
                                    HStack(spacing: 6) {
                                        Text(related.format?.capitalizedSentence ?? related.title)
                                            .fontWeight(.medium)
                                            .foregroundStyle(.silk)
                                        Text(related.relation.capitalizedSentence).foregroundStyle(.haze)
                                    }
                                    .font(.ui(15))
                                    .padding(.horizontal, 14)
                                    .frame(height: 32)
                                    .overlay(Capsule().strokeBorder(.faintRule))
                                }
                                .buttonStyle(.plain)
                                .pointerHover()
                            }
                        }
                    }

                    PhoneEpisodes(entry: entry, spoilers: store.spoilerProtection(for: id), rotor: rotor)
                }
                .accessibilityElement(children: .contain)
                .padding(.horizontal, 16)
                .padding(.top, 16)
                .padding(.bottom, 32)
            }
        }
        .nudgeableScroll()
        .ignoresSafeArea(edges: art.isCompact ? [] : .top)
        .scrollIndicators(.never)
        .background(.ground)
        .unwatchedRotor(store.titles.episodes[id] ?? [], in: rotor)
        .handsOff(title: entry)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    TitleMoreItems(entry: entry)
                } label: {
                    Label("More", systemImage: "ellipsis")
                }
            }
        }
        .navigationBarTitleDisplayMode(.inline)
        .task(id: id) {
            navigator.focusedTitle = id
            if !store.titles.detailed.contains(id) { store.loadDetail(id) }
            if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
            // What the show sets for itself, its spoiler protection among it: from the database.
            store.loadShowSettings(id)
        }
    }

    private func seasonChip(index: Int, season: TitleFamily.Season, entry: Entry) -> some View {
        Button {
            if !season.isThis { navigator.openTitle(season.id) }
        } label: {
            HStack(spacing: 6) {
                Text("Season \(index + 1)").fontWeight(season.isThis ? .semibold : .medium)
                    .foregroundStyle(season.isThis ? .amber : .silk)
                if season.isThis, let done = entry.progress?.done, let of = entry.progressOf {
                    Text("\(done) / \(of.total)").foregroundStyle(.haze)
                        .contentTransition(.numericText(value: Double(done)))
                        .animation(.standard, value: done)
                }
            }
            .font(.ui(15))
            .padding(.horizontal, 14)
            .frame(height: 32)
            .background(season.isThis ? Palette.selection : .clear, in: .capsule)
            .overlay(Capsule().strokeBorder(season.isThis ? .clear : Palette.faintRule))
        }
        .buttonStyle(.plain)
        .pointerHover()
    }
}

/// The episodes on an iPhone's page: a row each, the next one marked, stills after it blurred.
struct PhoneEpisodes: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let spoilers: Bool
    /// Where the page's Unwatched episodes rotor finds the rows.
    let rotor: Namespace.ID

    var body: some View {
        let rows = entry.episodeTable(store.titles.episodes[entry.id] ?? [])
        VStack(alignment: .leading, spacing: 0) {
            if store.titles.episodes[entry.id] == nil {
                ForEach(0..<4, id: \.self) { _ in EpisodePlaceholderRow() }
            }
            // The watched run folds away above the last episode watched.
            let fold = WatchedFold(rows)
            if let fold {
                WatchedFoldSection(
                    fold: fold, shown: navigator.watchedShown(entry.id), phone: true,
                    toggle: { navigator.toggleWatched(entry.id) }
                ) {
                    ForEach(rows.prefix(fold.count), id: \.number) {
                        PhoneEpisodeRow(entry: entry, row: $0, spoilers: spoilers, rotor: rotor)
                    }
                }
            }
            ForEach(rows.dropFirst(fold?.count ?? 0), id: \.number) {
                PhoneEpisodeRow(entry: entry, row: $0, spoilers: spoilers, rotor: rotor)
            }
        }
    }
}

/// One episode on an iPhone's page.
struct PhoneEpisodeRow: View {
    @Environment(AppStore.self) private var store
    let entry: Entry
    let row: EpisodeRow
    let spoilers: Bool
    let rotor: Namespace.ID

    var body: some View {
        let next = entry.progress?.next ?? 1
        let number = UInt32(row.number) ?? 0
        let current = number == next
        if row.isUnaired(), let airs = row.airsDate {
            PhoneUnairedRow(row: row, airs: airs)
                .accessibilityRotorEntry(id: row.number, in: rotor)
        } else if row.metadataOnly {
            // One no source has: nothing to play, so a tap offers what can be done, adding a
            // source among it, rather than playing.
            Menu {
                EpisodeMenu(entry: entry, number: number, row: row)
            } label: {
                label(number: number, next: next, current: current)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: row.spokenValue(entry, current: current, upcoming: false)) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
            .accessibilityRotorEntry(id: row.number, in: rotor)
        } else {
            Button {
                store.send(.play(id: entry.id, episode: row.number))
            } label: {
                label(number: number, next: next, current: current)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .episodeElement(row, value: row.spokenValue(entry, current: current, upcoming: false)) {
                EpisodeMenu(entry: entry, number: number, row: row)
            }
            .accessibilityRotorEntry(id: row.number, in: rotor)
            .contextMenu { EpisodeMenu(entry: entry, number: number, row: row) }
        }
    }

    private func label(number: UInt32, next: UInt32, current: Bool) -> some View {
        HStack(spacing: 12) {
            Text(row.number)
                .font(.ui(15, weight: current ? .semibold : .regular))
                .monospacedDigit()
                .foregroundStyle(current ? .amber : .haze)
                .animation(.quick, value: current)
                .frame(width: 22, alignment: .trailing)
            EpisodeStill(entry: entry, row: row, hidden: spoilers && number > next && !row.completed,
                         unaired: false, size: CGSize(width: 72, height: 40))
                .opacity(row.completed ? 0.7 : 1)
            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(row.title ?? "Episode \(row.number)")
                        .font(.ui(17))
                        .foregroundStyle(row.completed ? .haze : .silk)
                        .rowTitle()
                        .animation(.quick, value: row.completed)
                    if row.kind == .filler { Text("Filler").groupHeaderStyle() }
                }
                if current, let resume = entry.resume, let fraction = resume.fraction, fraction > 0 {
                    HStack(spacing: 8) {
                        Held(value: fraction) { Meter(fraction: $0, height: 3).frame(width: 88) }
                        let left = max(1, Int(((resume.position / fraction - resume.position) / 60).rounded()))
                        Held(value: left) { MinutesLeft(minutes: $0, size: 15) }
                    }
                } else if let seconds = row.durationSecs {
                    Text("\(max(1, Int((Double(seconds) / 60).rounded()))) min")
                        .font(.ui(15))
                        .foregroundStyle(.haze)
                }
            }
            Spacer(minLength: 0)
            WatchedCheck(watched: row.completed, size: 15)
        }
        .frame(minHeight: 56)
        .contentShape(.rect)
    }
}

/// The next episode still to air on an iPhone's page: no button; the flat placeholder, its title
/// in haze and "Airs Thursday 16:30" in water.
struct PhoneUnairedRow: View {
    let row: EpisodeRow
    let airs: Date

    var body: some View {
        let when = AirsWords.airs(airs)
        HStack(spacing: 12) {
            Text(row.number)
                .font(.ui(15))
                .monospacedDigit()
                .foregroundStyle(.haze)
                .frame(width: 22, alignment: .trailing)
            RoundedRectangle(cornerRadius: 6).fill(Palette.artPlaceholder).frame(width: 72, height: 40)
            VStack(alignment: .leading, spacing: 4) {
                Text(row.title ?? "Episode \(row.number)").font(.ui(17)).foregroundStyle(.haze).rowTitle()
                HStack(spacing: 6) {
                    Circle().fill(.water).frame(width: 6, height: 6)
                    Text(when).monospacedDigit()
                }
                .font(.ui(15))
                .foregroundStyle(.water)
            }
            Spacer(minLength: 0)
        }
        .frame(minHeight: 56)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel("Episode \(row.number)" + (row.title.map { ", \($0)" } ?? "") + ". \(when), not out yet")
    }
}
#endif
