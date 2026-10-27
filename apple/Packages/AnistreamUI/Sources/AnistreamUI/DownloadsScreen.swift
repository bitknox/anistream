// Downloads on the Mac and a wide iPad: the queue by title, each episode with where it stands
// and what can be done with it, and the watched episodes still seeding.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

struct DownloadsScreen: View {
    @Environment(AppStore.self) private var store
    @Namespace private var rotor

    var body: some View {
        let groups = store.downloads.groups
        Group {
            if store.downloads.loaded, groups.isEmpty, store.downloads.seeding.isEmpty {
                NoDownloads()
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        #if os(iOS)
                        if store.continued.ask != nil,
                           store.downloads.rows.contains(where: { $0.state == .queued || $0.state == .active }) {
                            Text(L.downloadsBackgroundNote)
                                .font(.mac(12))
                                .foregroundStyle(.haze)
                                .padding(EdgeInsets(top: 8, leading: 4, bottom: 8, trailing: 0))
                        }
                        #endif
                        ForEach(Array(groups.enumerated()), id: \.element.id) { index, group in
                            if index > 0 { Rectangle().fill(.faintRule).frame(height: 1) }
                            DownloadGroupView(group: group, rotor: rotor)
                        }
                        if !store.downloads.seeding.isEmpty {
                            Rectangle().fill(.faintRule).frame(height: 1).padding(.top, 8)
                            SeedingList()
                        }
                    }
                    .padding(EdgeInsets(top: 4, leading: 12, bottom: 24, trailing: 16))
                }
                .scrollIndicators(.never)
                .inProgressRotor(groups, titles: store.titles, in: rotor)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .pullToRefresh(store, .downloads)
        .task { store.loadDownloads() }
    }
}

/// Downloads with nothing in it (the spec's empty states).
struct NoDownloads: View {
    var body: some View {
        EmptyState(title: L.downloadsEmptyTitle, line: L.downloadsEmptyDetail)
    }
}

/// Downloads' title in the Mac's toolbar: the name, and beneath it the storage meter, what
/// downloads take against what is free, beside what it says in words.
struct DownloadsTitle: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let used = Double(store.downloads.bytesOnDisk)
        let free = FreeSpace.bytes.map { Double($0) }
        VStack(alignment: .leading, spacing: 2) {
            Text(L.downloadsTitle)
                .font(.mac(15, weight: .semibold))
                .foregroundStyle(.silk)
            HStack(spacing: 8) {
                if let free, used + free > 0 {
                    Capsule().fill(.rule)
                        .overlay(alignment: .leading) {
                            GeometryReader { proxy in
                                Capsule().fill(.haze).frame(width: proxy.size.width * used / (used + free))
                            }
                        }
                        .frame(width: 80, height: 4)
                }
                Text(downloadsSubtitle(store))
                    .font(.mac(11))
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    .lineLimit(1)
            }
        }
        .fixedSize()
        .padding(.leading, 4)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L.downloadsTitle)
        .accessibilityValue(downloadsSubtitle(store).replacingOccurrences(of: " · ", with: ", "))
    }
}

/// What the toolbar says under Downloads: what is free, and what downloads take.
@MainActor
func downloadsSubtitle(_ store: AppStore) -> String {
    let used = Wording.bytes(store.downloads.bytesOnDisk)
    guard let free = FreeSpace.bytes else { return L.downloadsSpaceUsed(size: used) }
    return L.downloadsSubtitleFree(free: Wording.bytes(free), size: used)
}

/// One title's downloads: its cover and name, how many and how much, and each episode.
struct DownloadGroupView: View {
    @Environment(AppStore.self) private var store
    let group: DownloadGroup
    /// Where the In progress rotor finds the episodes.
    let rotor: Namespace.ID

    var body: some View {
        let entry = store.titles[group.id]
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 14) {
                ArtworkView(url: entry?.coverUrl, color: entry?.plateColor, maxPixelSize: 170)
                    .frame(width: 36, height: 54)
                    .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: 36)))
                VStack(alignment: .leading, spacing: 2) {
                    Text(entry?.showTitle ?? group.title)
                        .font(.macShowTitle(size: 15))
                        .foregroundStyle(.silk)
                    Text(summary)
                        .font(.mac(13))
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                }
                Spacer()
                // Keep next and Quality are the engine's for every show alike, so they are set
                // once, in Settings › Downloads, and not per show here.
            }
            .padding(.vertical, 12)
            ForEach(group.rows, id: \.id) { row in
                DownloadLine(row: row)
                    .accessibilityRotorEntry(id: row.id, in: rotor)
            }
        }
        .padding(.bottom, 8)
        .task(id: group.id) {
            if store.titles[group.id] == nil { store.loadDetail(group.id) }
            // The episodes' own titles, for "Episode 8 · A Town Without Night".
            if store.titles.episodes[group.id] == nil { store.loadEpisodes(group.id) }
        }
    }

    private var summary: String {
        let count = L.episodesCount(count: group.rows.count)
        let bytes = group.bytesOnDisk
        return bytes > 0 ? L.downloadsGroupSummary(count: count, size: Wording.bytes(bytes)) : count
    }
}

/// One episode in the queue: what it is, where it stands, its size and its actions.
struct DownloadLine: View {
    @Environment(AppStore.self) private var store
    let row: DownloadRow
    @State private var confirmDelete = false

    var body: some View {
        HStack(spacing: 16) {
            HStack(spacing: 8) {
                Text(name)
                    .font(.mac(13))
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                // Queued by keep next rather than asked for. The name gives way first.
                if row.ahead {
                    Text(L.downloadsAhead)
                        .font(.mac(11))
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                        .fixedSize()
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            standing
                .frame(width: 320, alignment: .leading)
            Text(row.total > 0 ? Wording.bytes(row.total) : "")
                .font(.mac(13))
                .monospacedDigit()
                .foregroundStyle(.haze)
                .frame(width: 80, alignment: .trailing)
            actions
                .frame(width: 120, alignment: .trailing)
        }
        .padding(.leading, 50)
        .frame(minHeight: 44)
        .accessibilityElement(children: .contain)
        .confirmationDialog(L.downloadsDeleteTitle(name: name), isPresented: $confirmDelete) {
            Button(L.actionDelete, role: .destructive) { store.deleteDownload(row.id) }
        } message: {
            Text(L.downloadsDeleteDetail)
        }
    }

    /// "Episode 8 · A Town Without Night", with the title when the episode list is known.
    private var name: String {
        let title = store.titles.episodes[row.anilistId]?.first { $0.number == row.episode }?.title
        let number = Int(episode: row.episode)
        return title.map { L.episodeWithTitle(number: number, title: $0) } ?? L.downloadsEpisodeName(episode: number)
    }

    @ViewBuilder
    private var standing: some View {
        switch row.state {
        case .done:
            HStack(spacing: 6) {
                Image(systemName: "checkmark").font(.mac(11, weight: .semibold)).foregroundStyle(.water)
                // One keep next queued goes a day after it is watched.
                Text(row.ahead ? L.downloadsCaptionAhead : L.downloadsCaptionDone)
                    .foregroundStyle(.haze)
                    .lineLimit(1)
            }
            .font(.mac(13))
        case .active where row.total == 0:
            Text(L.downloadsCaptionMetadata).font(.mac(13)).foregroundStyle(.haze)
        case .active:
            progress(activeLine, color: .water)
        case .paused:
            progress(L.downloadsCaptionPaused(percent: row.fraction.formatted(.percent.precision(.fractionLength(0)))), color: .haze)
        case .queued:
            Text(L.downloadsCaptionQueuedSlot).font(.mac(13)).foregroundStyle(.haze)
        case .failed:
            // What stopped it is in Messages and the bug report (copy rule 5).
            FailedLine(L.downloadsCaptionFailed)
                .lineLimit(2)
        }
    }

    private var activeLine: String {
        var parts = [L.downloadsOfTotal(done: Wording.bytes(row.downloaded), total: Wording.bytes(row.total))]
        if let rate = store.downloads.rates[row.id], rate > 0 {
            parts.append(L.downloadsRate(rate: Wording.bytes(UInt64(rate))))
        }
        if let left = store.downloads.secondsLeft(row) {
            parts.append(L.downloadsTimeLeft(left: Wording.countdown(left)))
        }
        return parts.joined(separator: " · ")
    }

    /// A line and a bar: the bar in water while it runs, haze while it waits.
    private func progress(_ line: String, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(line).font(.mac(13)).monospacedDigit().foregroundStyle(.haze)
            Capsule()
                .fill(.rule)
                .overlay(alignment: .leading) {
                    GeometryReader { proxy in
                        Capsule().fill(color).frame(width: proxy.size.width * min(max(row.fraction, 0), 1))
                    }
                }
                .frame(height: 3)
                .animation(.standard, value: row.fraction)
        }
        .accessibilityElement(children: .combine)
        .accessibilityValue(Text(row.fraction, format: .percent.precision(.fractionLength(0))))
    }

    @ViewBuilder
    private var actions: some View {
        HStack(spacing: 4) {
            switch row.state {
            case .done:
                icon("play", L.actionPlay) { store.playDownload(row) }
                #if os(macOS)
                icon("folder", L.downloadsShowInFolder) {
                    if let path = row.path {
                        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
                    }
                }
                #endif
                icon("trash", L.actionDelete) { confirmDelete = true }
            case .active:
                icon("pause", L.downloadsPauseOne) { store.toggleDownload(row.id) }
                icon("xmark", L.downloadsRemoveFromQueue) { store.cancelDownload(row.id) }
            case .paused:
                icon("play", L.downloadsResumeOne) { store.toggleDownload(row.id) }
                icon("xmark", L.downloadsRemoveFromQueue) { store.cancelDownload(row.id) }
            case .queued:
                Button(L.downloadsStartNow) { store.startDownloadNow(row.id) }
                    .buttonStyle(SecondaryCapsuleStyle())
                    .fixedSize()
                    .help(L.downloadsStartNowHelp)
                icon("xmark", L.downloadsRemoveFromQueue) { store.cancelDownload(row.id) }
            case .failed:
                Button(L.actionTryAgain) { store.retryDownload(row) }
                    .buttonStyle(SecondaryCapsuleStyle())
                icon("xmark", L.downloadsRemoveFromQueue) { store.cancelDownload(row.id) }
            }
        }
        .font(.mac(13, weight: .medium))
        .foregroundStyle(.silk)
    }

    private func icon(_ symbol: String, _ label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .frame(width: 28, height: 28)
                .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .help(label)
        .accessibilityLabel(label)
    }
}

/// Watched episodes still seeding: how far toward the ratio, how long is left, what went up.
struct SeedingList: View {
    @Environment(AppStore.self) private var store

    /// "Shared 0.8 of 1.0 · 2h left · 1.2 GB up".
    private func seedingLine(_ row: SeedRow) -> String {
        let ratio = FloatingPointFormatStyle<Double>.number.precision(.fractionLength(1))
        return L.downloadsSeedingUploaded(
            shared: row.ratio.formatted(ratio), goal: row.goal.formatted(ratio),
            left: L.downloadsTimeLeft(left: Wording.countdown(Int64(row.leftSecs))),
            size: Wording.bytes(row.uploadedBytes))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(L.downloadsSeedingAfterWatching)
                .groupHeaderStyle()
                .accessibilityAddTraits(.isHeader)
                .macLineHeight(14)
                .padding(EdgeInsets(top: 16, leading: 0, bottom: 8, trailing: 0))
            ForEach(Array(store.downloads.seeding.enumerated()), id: \.offset) { _, row in
                HStack(spacing: 16) {
                    Text(row.name)
                        .font(.mac(13))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    HStack(spacing: 10) {
                        Capsule()
                            .fill(.rule)
                            .overlay(alignment: .leading) {
                                GeometryReader { proxy in
                                    Capsule().fill(.silk)
                                        .frame(width: proxy.size.width * min(row.goal > 0 ? row.ratio / row.goal : 0, 1))
                                }
                            }
                            .frame(width: 60, height: 3)
                        Text(seedingLine(row))
                            .font(.mac(13))
                            .monospacedDigit()
                            .foregroundStyle(.haze)
                            .lineLimit(1)
                    }
                    .frame(width: 320, alignment: .leading)
                    Text(row.size.map(Wording.bytes) ?? "")
                        .font(.mac(13))
                        .monospacedDigit()
                        .foregroundStyle(.haze)
                        .frame(width: 80, alignment: .trailing)
                }
                .padding(.leading, 50)
                .frame(minHeight: 32)
            }
        }
    }
}
