// Continue on the Mac and iPad: the queue down the left, and the selected title beside it.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// Continue in a wide window: the queue, grouped In progress and New this week, and a preview of
/// the selected title with the one amber action, what it plays.
struct ContinueScreen: View {
    @Environment(AppStore.self) private var store
    @State private var selection: AnilistId?

    var body: some View {
        let queue = store.continuing.queue(store.titles)
        Group {
            switch store.continuing.list.state {
            case .idle, .loading:
                QueueColumn(queue: nil, selection: $selection)
                    .frame(maxWidth: .infinity, alignment: .leading)
            case .empty:
                NothingWatched()
            case .failed(let reason):
                ListFailed(reason: reason)
            case .loaded:
                if queue.isEmpty {
                    NothingToContinue()
                } else {
                    HStack(alignment: .top, spacing: 0) {
                        QueueColumn(queue: queue, selection: $selection)
                        if let item = queue.all.first(where: { $0.id == selection }) ?? queue.all.first {
                            ContinuePreview(item: item)
                                .id(item.id)
                                .transition(.opacity.animation(.fadeIn))
                        }
                    }
                    #if os(iOS)
                    // On the Mac the sidebar sets the queue off the edge; an iPad has none.
                    .padding(.leading, 12)
                    #endif
                }
            }
        }
        .task { store.loadContinue() }
        .onChange(of: queue.all.map(\.id), initial: true) { _, ids in
            if selection == nil || !ids.contains(where: { $0 == selection }) {
                selection = ids.first
            }
        }
        .onChange(of: selection, initial: true) { _, id in
            // The page's fields (where else it streams) and the episode's name.
            guard let id else { return }
            if !store.titles.detailed.contains(id) { store.loadDetail(id) }
            if store.titles.episodes[id] == nil { store.loadEpisodes(id) }
        }
    }
}

/// The queue down the left: 360 points wide, rows 62 high with 36 by 54 covers.
struct QueueColumn: View {
    @Environment(AppStore.self) private var store
    /// `nil` while the list loads: placeholder rows, static, no shimmer.
    let queue: ContinueQueue?
    @Binding var selection: AnilistId?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                if let queue {
                    if !queue.inProgress.isEmpty {
                        GroupHeader(title: "In progress", first: true)
                        ForEach(queue.inProgress) { QueueRow(item: $0, selection: $selection) }
                    }
                    if !queue.newThisWeek.isEmpty {
                        GroupHeader(title: "New this week", first: queue.inProgress.isEmpty)
                        ForEach(queue.newThisWeek) { QueueRow(item: $0, selection: $selection) }
                    }
                } else {
                    GroupHeader(title: "In progress", first: true)
                    ForEach(0..<3, id: \.self) { _ in PlaceholderRow() }
                }
            }
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 16, trailing: 8))
        }
        .scrollIndicators(.never)
        .frame(width: 360)
        .focusable()
        .focusEffectDisabled()
        .onKeyPress(.upArrow) { step(-1) }
        .onKeyPress(.downArrow) { step(1) }
    }

    /// Move the selection `delta` rows, stopping at the ends.
    private func step(_ delta: Int) -> KeyPress.Result {
        guard let ids = queue?.all.map(\.id), !ids.isEmpty else { return .ignored }
        let index = ids.firstIndex { $0 == selection } ?? 0
        selection = ids[min(max(index + delta, 0), ids.count - 1)]
        return .handled
    }
}

/// A group's header in a list: "IN PROGRESS".
struct GroupHeader: View {
    let title: String
    var first = false

    var body: some View {
        Text(title)
            .groupHeaderStyle()
            .accessibilityAddTraits(.isHeader)
            .padding(EdgeInsets(top: first ? 10 : 18, leading: 12, bottom: 6, trailing: 12))
    }
}

/// One title in the queue: its cover, its title, what it plays and, when it was left part-way, how
/// far into the episode.
struct QueueRow: View {
    @Environment(AppStore.self) private var store
    let item: ContinueItem
    @Binding var selection: AnilistId?

    var body: some View {
        let selected = selection == item.id
        Button {
            selection = item.id
        } label: {
            HStack(spacing: 12) {
                ArtworkView(url: item.entry.coverUrl, color: item.entry.plateColor, maxPixelSize: 108)
                    .frame(width: 36, height: 54)
                    .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: 36)))
                VStack(alignment: .leading, spacing: 0) {
                    Text(item.entry.showTitle)
                        .font(.showTitle(size: 15))
                        .fixedLineHeight(20)
                        .foregroundStyle(selected ? .amber : .silk)
                        .lineLimit(1)
                    HStack(spacing: 6) {
                        if item.isNew {
                            Circle().fill(.water).frame(width: 6, height: 6)
                        }
                        Text(item.action)
                            .monospacedDigit()
                    }
                    .font(.system(size: 13))
                    .fixedLineHeight(18)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
                    .padding(.top, 2)
                    if let fraction = item.fraction {
                        Meter(fraction: fraction, height: 3)
                            .padding(.top, 6)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            .frame(height: 62)
            .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 10))
            .contentShape(.rect(cornerRadius: 10))
        }
        .buttonStyle(.plain)
        .animation(.quick, value: selected)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(selected ? .isSelected : [])
        .contextMenu { QueueMenu(item: item) }
    }
}

/// A row's context menu, as MacHome draws it. What has no screen or player yet is there,
/// disabled, rather than missing.
struct QueueMenu: View {
    @Environment(AppStore.self) private var store
    let item: ContinueItem

    var body: some View {
        Button(item.resume == nil ? "Start Episode \(item.episode)" : "Resume Episode \(item.episode)") {
            store.send(.play(id: item.id, episode: String(item.episode)))
        }
        Button("Mark Episode \(item.episode) as Watched") {
            store.send(.setWatched(id: item.id, episodes: [String(item.episode)], watched: true))
        }
        Divider()
        Button("Go to Title") {}
            .disabled(true)
        Divider()
        Button("Hide from Continue") {
            store.hideFromContinue(item.id)
        }
        Button("Drop") {
            store.send(.setStatus(id: item.id, status: .dropped))
        }
    }
}

/// A row while the list loads.
struct PlaceholderRow: View {
    var body: some View {
        HStack(spacing: 12) {
            RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: 36, height: 54)
            VStack(alignment: .leading, spacing: 2) {
                Text("Hoshikuzu no Tabibito").font(.showTitle(size: 15))
                Text("Resume E7 · 10:06").font(.system(size: 13))
            }
            .redacted(reason: .placeholder)
            .foregroundStyle(.haze)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .frame(height: 62)
        .accessibilityHidden(true)
    }
}

/// The selected title: the banner, the title block, the episode it plays and how far into it, the
/// action, and the synopsis.
struct ContinuePreview: View {
    @Environment(AppStore.self) private var store
    let item: ContinueItem

    var body: some View {
        let entry = store.titles[item.id] ?? item.entry
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                BannerArt(entry: entry)
                    .frame(height: 290)
                    .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))

                VStack(alignment: .leading, spacing: 6) {
                    Text(entry.showTitle)
                        .font(.showTitle(size: 30, relativeTo: .largeTitle))
                        .fixedLineHeight(38, relativeTo: .largeTitle)
                        .foregroundStyle(.silk)
                        .accessibilityAddTraits(.isHeader)
                    if let alternate = entry.alternateTitle {
                        Text(alternate)
                            .font(.system(size: 15))
                            .foregroundStyle(.haze)
                    }
                    Text(entry.metadataLine)
                        .groupHeaderStyle()
                        .padding(.top, 4)
                }

                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Text(item.episodeLine(store.titles))
                            .foregroundStyle(.silk)
                        Spacer()
                        if let left = item.timeLeft {
                            Text(left).foregroundStyle(.haze)
                        }
                    }
                    .font(.system(size: 13))
                    if let fraction = item.fraction {
                        Meter(fraction: fraction, height: 4)
                    }
                }

                HStack(spacing: 10) {
                    Button {
                        store.send(.play(id: item.id, episode: String(item.episode)))
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: "play.fill").font(.system(size: 10))
                            Text(item.buttonTitle)
                        }
                    }
                    .buttonStyle(PrimaryCapsuleStyle())
                    .keyboardShortcut(.defaultAction)
                    Button("Episodes") {}
                        .buttonStyle(SecondaryCapsuleStyle())
                }

                if !entry.synopsis.isEmpty {
                    ClampedText(text: entry.synopsis)
                        .font(.system(size: 13))
                        .fixedLineHeight(20)
                        .foregroundStyle(.haze)
                        .frame(maxWidth: 620, alignment: .leading)
                }
                        .id(entry.id)
                if let alsoOn = entry.alsoOn {
                    Text(alsoOn)
                        .font(.system(size: 12))
                        .foregroundStyle(.haze)
                }
            }
            .padding(EdgeInsets(top: 4, leading: 12, bottom: 20, trailing: 24))
        }
        .scrollIndicators(.never)
        .frame(maxWidth: .infinity, alignment: .topLeading)
    }
}

/// Continue on a database with nothing in it: Nagare, and where to start.
struct NothingWatched: View {
    var body: some View {
        VStack(spacing: 16) {
            Nagare(cell: 5)
            VStack(spacing: 4) {
                Text("Nothing watched yet")
                    .font(.system(size: 15, weight: .semibold))
                    .foregroundStyle(.silk)
                Text("What you start watching shows here.")
                    .font(.system(size: 13))
                    .foregroundStyle(.haze)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// Continue when everything in it has been hidden.
struct NothingToContinue: View {
    var body: some View {
        Text("Nothing to continue")
            .font(.system(size: 13))
            .foregroundStyle(.haze)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// A list that could not be loaded: why, in the engine's words, with a vermilion dot.
struct ListFailed: View {
    let reason: String

    var body: some View {
        HStack(spacing: 6) {
            Circle().fill(.vermilion).frame(width: 6, height: 6).accessibilityHidden(true)
            Text(reason)
                .font(.system(size: 13))
                .foregroundStyle(.silk)
                .textSelection(.enabled)
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
