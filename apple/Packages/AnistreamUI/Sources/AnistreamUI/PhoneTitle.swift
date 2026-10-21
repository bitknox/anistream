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
    @AppStorage("anistream.spoilerProtection") private var spoilers = true
    let id: AnilistId

    var body: some View {
        let entry = store.titles[id] ?? Entry.placeholder(id)
        let action = TitleAction(entry)
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                // The banner: it stretches when pulled down and scrolls at half speed (T1).
                BannerArt(entry: entry)
                    .frame(height: 250)
                    .visualEffect { [reduceMotion] content, proxy in
                        let y = proxy.frame(in: .scrollView(axis: .vertical)).minY
                        let h = proxy.size.height
                        return content
                            .scaleEffect(reduceMotion || y <= 0 ? 1 : (h + y) / h, anchor: .bottom)
                            .offset(y: reduceMotion || y >= 0 ? 0 : -y * 0.5)
                    }
                    .clipped()
                HStack(alignment: .bottom, spacing: 16) {
                    ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: 400)
                        .frame(width: 100, height: 150)
                        .clipShape(.rect(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Palette.glassEdge.opacity(1.25)))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(entry.showTitle)
                            .font(.showTitle(size: 22, relativeTo: .title2))
                            .fixedLineHeight(28, relativeTo: .title2)
                            .foregroundStyle(.silk)
                            .accessibilityAddTraits(.isHeader)
                        if let alternate = entry.alternateTitle {
                            Text(alternate).font(.system(size: 15)).foregroundStyle(.haze)
                        }
                        Text(entry.phoneMetadata)
                            .groupHeaderStyle()
                            .padding(.top, 2)
                    }
                    .padding(.bottom, 4)
                }
                .padding(.horizontal, 16)
                .padding(.top, -60)

                VStack(alignment: .leading, spacing: 16) {
                    Button {
                        store.send(.play(id: entry.id, episode: String(action.episode)))
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: "play.fill").font(.system(size: 13))
                            Text(action.sentence)
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 50, fontSize: 17))

                    HStack(spacing: 12) {
                        Menu {
                            StatusItems(id: entry.id)
                        } label: {
                            HStack(spacing: 6) {
                                Text(entry.listStatus.map { $0.label.capitalizedSentence } ?? "Add to list")
                                Image(systemName: "chevron.up.chevron.down").font(.system(size: 11, weight: .semibold))
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

                    if !entry.synopsis.isEmpty {
                        ClampedText(text: entry.synopsis, lines: 2)
                            .font(.system(size: 15))
                            .fixedLineHeight(21)
                            .foregroundStyle(.haze)
                            .id(entry.id)
                    }

                    let family = TitleFamily(entry)
                    if !family.seasons.isEmpty {
                        ScrollView(.horizontal) {
                            HStack(spacing: 8) {
                                ForEach(Array(family.seasons.enumerated()), id: \.element.id) { index, season in
                                    seasonChip(index: index, season: season, entry: entry)
                                }
                            }
                            .padding(.horizontal, 16)
                        }
                        .scrollIndicators(.never)
                        .padding(.horizontal, -16)
                    }
                    if !family.related.isEmpty {
                        HStack(spacing: 8) {
                            Text("Related").font(.system(size: 13)).foregroundStyle(.haze)
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
                                    .font(.system(size: 15))
                                    .padding(.horizontal, 14)
                                    .frame(height: 32)
                                    .overlay(Capsule().strokeBorder(.faintRule))
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }

                    PhoneEpisodes(entry: entry, spoilers: spoilers)
                }
                .padding(.horizontal, 16)
                .padding(.top, 16)
                .padding(.bottom, 32)
            }
        }
        .ignoresSafeArea(edges: .top)
        .scrollIndicators(.never)
        .background(.ground)
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
                }
            }
            .font(.system(size: 15))
            .padding(.horizontal, 14)
            .frame(height: 32)
            .background(season.isThis ? Palette.selection : .clear, in: .capsule)
            .overlay(Capsule().strokeBorder(season.isThis ? .clear : Palette.faintRule))
        }
        .buttonStyle(.plain)
    }
}

extension Entry {
    /// The metadata line on an iPhone's page: "TV · 12 EP · 2024 · 82%".
    var phoneMetadata: String {
        var parts: [String] = []
        if let format { parts.append(format) }
        if let episodes, episodes > 0, format != "MOVIE" { parts.append("\(episodes) ep") }
        if let year { parts.append(String(year)) }
        if let score { parts.append("\(score)%") }
        return parts.joined(separator: " · ")
    }
}

/// The episodes on an iPhone's page: a row each, the next one marked, stills after it blurred.
struct PhoneEpisodes: View {
    @Environment(AppStore.self) private var store
    let entry: Entry
    let spoilers: Bool

    var body: some View {
        let rows = store.titles.episodes[entry.id] ?? []
        let next = entry.progress?.next ?? 1
        VStack(alignment: .leading, spacing: 0) {
            if store.titles.episodes[entry.id] == nil {
                ForEach(0..<4, id: \.self) { _ in EpisodePlaceholderRow() }
            }
            ForEach(rows, id: \.number) { row in
                let number = UInt32(row.number) ?? 0
                let current = number == next
                Button {
                    store.send(.play(id: entry.id, episode: row.number))
                } label: {
                    HStack(spacing: 12) {
                        Text(row.number)
                            .font(.system(size: 15, weight: current ? .semibold : .regular))
                            .monospacedDigit()
                            .foregroundStyle(current ? .amber : .haze)
                            .frame(width: 22, alignment: .trailing)
                        EpisodeStill(entry: entry, row: row, hidden: spoilers && number > next && !row.completed,
                                     unaired: false, size: CGSize(width: 72, height: 40))
                            .opacity(row.completed ? 0.7 : 1)
                        VStack(alignment: .leading, spacing: 4) {
                            HStack(alignment: .firstTextBaseline, spacing: 8) {
                                Text(row.title ?? "Episode \(row.number)")
                                    .font(.system(size: 17))
                                    .foregroundStyle(row.completed ? .haze : .silk)
                                    .lineLimit(1)
                                if row.kind == .filler { Text("Filler").groupHeaderStyle() }
                            }
                            if current, let resume = entry.resume, let fraction = resume.fraction, fraction > 0 {
                                HStack(spacing: 8) {
                                    Meter(fraction: fraction, height: 3).frame(width: 88)
                                    let left = max(1, Int(((resume.position / fraction - resume.position) / 60).rounded()))
                                    Text("\(left) min left").font(.system(size: 15)).foregroundStyle(.haze)
                                }
                            } else if let seconds = row.durationSecs {
                                Text("\(max(1, Int((Double(seconds) / 60).rounded()))) min")
                                    .font(.system(size: 15))
                                    .foregroundStyle(.haze)
                            }
                        }
                        Spacer(minLength: 0)
                        if row.completed {
                            Image(systemName: "checkmark")
                                .font(.system(size: 15, weight: .semibold))
                                .foregroundStyle(.haze)
                                .accessibilityLabel("Watched")
                        }
                    }
                    .frame(minHeight: 56)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
                .accessibilityElement(children: .combine)
                .contextMenu { EpisodeMenu(entry: entry, number: number, row: row) }
            }
        }
    }
}
#endif
