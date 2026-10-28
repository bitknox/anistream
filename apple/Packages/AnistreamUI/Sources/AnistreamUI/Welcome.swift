// Continue with nothing watched yet: what airs today and this season, which AniList's public data
// gives with no account. Setting up is first launch's, before this.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// Continue with nothing watched yet.
struct WelcomeContinue: View {
    @Environment(AppStore.self) private var store
    var compact = false

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            AiringToday(compact: compact)
            ThisSeason(compact: compact)
        }
        .task {
            if store.calendar.list.state == .idle { store.loadCalendar() }
            if store.seasonal.state == .idle { store.loadSeasonal() }
        }
    }
}

/// What airs today, every airing title's: four cards on the Mac, a rail on iPhone.
struct AiringToday: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false

    var body: some View {
        let now = Date.now
        let today = store.broadcasts(mine: false).filter { Calendar.current.isDateInToday($0.airs) }
        let nextUp = today.first { $0.airs > now }?.id
        if !today.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Button {
                    navigator.calendarMine = false
                    navigator.open(.calendar)
                } label: {
                    HStack(spacing: 8) {
                        Text(L.widgetAiringToday)
                            .font(compact ? .ui(20, weight: .semibold) : .mac(15, weight: .semibold))
                            .foregroundStyle(.silk)
                        Image(systemName: "chevron.right").font(.mac(11, weight: .semibold)).foregroundStyle(.haze)
                    }
                    .frame(height: 28)
                }
                .buttonStyle(.plain)
                .pointerHover()
                .accessibilityLabel(L.welcomeAiringTodayLabel)
                if compact {
                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 12) {
                            ForEach(today) { card($0, now: now, next: $0.id == nextUp).frame(width: 240) }
                        }
                        .padding(.horizontal, 16)
                    }
                    .scrollIndicators(.never)
                    .padding(.horizontal, -16)
                } else {
                    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 16, alignment: .top), count: 4),
                              alignment: .leading, spacing: 16) {
                        ForEach(today.prefix(8)) { card($0, now: now, next: $0.id == nextUp) }
                    }
                }
            }
        }
    }

    private func card(_ broadcast: Broadcast, now: Date, next: Bool) -> some View {
        Button {
            navigator.openTitle(broadcast.entry.id)
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                BannerArt(entry: broadcast.entry)
                    .aspectRatio(16 / 9, contentMode: .fit)
                    .clipShape(.rect(cornerRadius: 8))
                Text(broadcast.entry.showTitle)
                    .font(.macShowTitle(size: 14))
                    .macLineHeight(18)
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                    .padding(.top, 8)
                HStack(spacing: 8) {
                    Text(L.welcomeBroadcast(episode: L.episodeNumberText(number: String(broadcast.episode)) + broadcast.markSuffix, clock: broadcast.clock)).monospacedDigit()
                    Spacer(minLength: 0)
                    let seconds = Int64(broadcast.airs.timeIntervalSince(now))
                    if seconds <= 0 {
                        HStack(spacing: 5) {
                            Circle().fill(.water).frame(width: 6, height: 6)
                            Text(L.calendarOut(ago: Wording.ago(-seconds)))
                        }
                    } else {
                        Text(L.timeIn(span: Wording.countdown(seconds))).foregroundStyle(next ? .water : .haze)
                    }
                }
                .font(.mac(12))
                .macLineHeight(16)
                .foregroundStyle(.haze)
                .lineLimit(1)
                .padding(.top, 4)
            }
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(broadcast.entry, value: broadcast.spoken(now: now)) { TitleActions(entry: broadcast.entry) }
        .contextMenu { TitleMenu(entry: broadcast.entry) }
    }
}

/// This season's titles: a grid on the Mac, a row on iPhone.
struct ThisSeason: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false

    var body: some View {
        let season = store.seasonal.state.ids.compactMap { store.titles[$0] }
        if !season.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Button {
                        navigator.open(.seasonal)
                    } label: {
                        HStack(spacing: 8) {
                            Text(L.continueThisSeason)
                                .font(compact ? .ui(20, weight: .semibold) : .mac(15, weight: .semibold))
                                .foregroundStyle(.silk)
                            Image(systemName: "chevron.right").font(.mac(11, weight: .semibold)).foregroundStyle(.haze)
                        }
                        .frame(height: 28)
                    }
                    .buttonStyle(.plain)
                    .pointerHover()
                    if !compact, let name = store.seasonName {
                        Text(name).font(.mac(12)).foregroundStyle(.haze)
                    }
                }
                if compact {
                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 12) {
                            ForEach(season, id: \.id) { PosterCell(entry: $0, width: 136).frame(width: 136) }
                        }
                        .padding(.horizontal, 16)
                    }
                    .scrollIndicators(.never)
                    .padding(.horizontal, -16)
                } else {
                    PosterGrid(entries: season) { entry in
                        PosterCell(entry: entry, detail: entry.genres.first)
                    }
                }
            }
        }
    }
}
