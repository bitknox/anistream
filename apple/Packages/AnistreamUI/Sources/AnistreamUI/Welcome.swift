// Continue on a first launch: nothing watched yet, so a compact setup card with Nagare, then
// what airs today and this season, which AniList's public data gives with no account.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// Whether the setup card has been hidden on this device.
let setupHiddenKey = "anistream.setupHidden"

/// Continue with nothing watched yet.
struct WelcomeContinue: View {
    @Environment(AppStore.self) private var store
    @AppStorage(setupHiddenKey) private var setupHidden = false
    var compact = false

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            if !setupHidden {
                SetupCard(compact: compact) { withAnimation(.standard) { setupHidden = true } }
            }
            AiringToday(compact: compact)
            ThisSeason(compact: compact)
        }
        .task {
            if store.calendar.list.state == .idle { store.loadCalendar() }
            if store.seasonal.state == .idle { store.loadSeasonal() }
        }
    }
}

/// The two things to set up: AniList, which is optional, and a plugin store, which is how
/// anistream plays episodes. And the legal line.
struct SetupCard: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    var compact = false
    let hide: () -> Void

    var body: some View {
        Group {
            if compact { phone } else { mac }
        }
        .background(.raised, in: .rect(cornerRadius: compact ? 16 : 12))
    }

    private var mac: some View {
        HStack(spacing: 20) {
            Nagare(cell: 4)
            VStack(alignment: .leading, spacing: 0) {
                Text("Set up anistream")
                    .font(.mac(15, weight: .semibold))
                    .macLineHeight(20)
                    .foregroundStyle(.silk)
                    .accessibilityAddTraits(.isHeader)
                Spacer(minLength: 8)
                HStack(spacing: 40) {
                    HStack(spacing: 16) {
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(alignment: .firstTextBaseline, spacing: 6) {
                                Text("AniList").font(.mac(13, weight: .semibold)).foregroundStyle(.silk)
                                Text("Optional").font(.mac(12)).foregroundStyle(.haze)
                            }
                            Text("Brings in your lists and fills Continue with what you are watching.")
                                .font(.mac(12))
                                .foregroundStyle(.haze)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Button("Connect AniList…") { store.send(.connect(tracker: "anilist")) }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                    .frame(maxWidth: .infinity)
                    HStack(spacing: 16) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Plugin store").font(.mac(13, weight: .semibold)).foregroundStyle(.silk)
                            Text("Episodes play from plugin stores. You join one with an invite link from its owner.")
                                .font(.mac(12))
                                .foregroundStyle(.haze)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Button("Add Plugin Store…") { navigator.open(.pluginStores) }
                            .buttonStyle(PrimaryCapsuleStyle(height: 28))
                    }
                    .frame(maxWidth: .infinity)
                }
                .macLineHeight(16)
                Spacer(minLength: 8)
                HStack(spacing: 16) {
                    Text("Episodes play from the plugins you add. anistream hosts no content.")
                        .font(.mac(11))
                        .macLineHeight(14)
                        .foregroundStyle(.haze)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    Button("Hide", action: hide)
                        .buttonStyle(.plain)
                        .pointerHover()
                        .font(.mac(13, weight: .semibold))
                        .foregroundStyle(.silk)
                        .accessibilityLabel("Hide setup")
                }
            }
            .frame(height: 128)
        }
        .padding(EdgeInsets(top: 14, leading: 16, bottom: 14, trailing: 20))
    }

    private var phone: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 16) {
                Nagare(cell: 4)
                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        Text("Set up anistream")
                            .font(.mac(17, weight: .semibold))
                            .foregroundStyle(.silk)
                        Spacer()
                        Button("Hide", action: hide)
                            .buttonStyle(.plain)
                            .pointerHover()
                            .font(.mac(15, weight: .semibold))
                            .foregroundStyle(.silk)
                    }
                    setupRow("AniList", note: "Optional", detail: "Syncs your lists") {
                        Button("Connect") { store.send(.connect(tracker: "anilist")) }
                            .buttonStyle(SecondaryCapsuleStyle(height: 44, raisedSurface: true, fontSize: 17))
                    }
                    Divider().overlay(.faintRule)
                    setupRow("Plugin store", note: nil, detail: "Needed for playback") {
                        Button("Add") { navigator.open(.pluginStores) }
                            .buttonStyle(PrimaryCapsuleStyle(height: 44, fontSize: 17))
                    }
                }
            }
            Text("Episodes play from the plugins you add. anistream hosts no content.")
                .font(.mac(13))
                .foregroundStyle(.haze)
        }
        .padding(18)
    }

    private func setupRow<Action: View>(
        _ name: String, note: String?, detail: String, @ViewBuilder action: () -> Action
    ) -> some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(name).font(.mac(17)).foregroundStyle(.silk)
                    if let note { Text(note).font(.mac(13)).foregroundStyle(.haze) }
                }
                Text(detail).font(.mac(13)).foregroundStyle(.haze)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            action()
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
                        Text(compact ? "Airing today" : "Airing Today")
                            .font(compact ? .ui(20, weight: .semibold) : .mac(15, weight: .semibold))
                            .foregroundStyle(.silk)
                        Image(systemName: "chevron.right").font(.mac(11, weight: .semibold)).foregroundStyle(.haze)
                    }
                    .frame(height: 28)
                }
                .buttonStyle(.plain)
                .pointerHover()
                .accessibilityLabel("Airing today, show in Calendar")
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
                    Text("Episode \(broadcast.episode) · \(CalendarWords.time(broadcast.airs))").monospacedDigit()
                    Spacer(minLength: 0)
                    let seconds = Int64(broadcast.airs.timeIntervalSince(now))
                    if seconds <= 0 {
                        HStack(spacing: 5) {
                            Circle().fill(.water).frame(width: 6, height: 6)
                            Text("Out \(Wording.ago(-seconds))")
                        }
                    } else {
                        Text("in \(Wording.countdown(seconds))").foregroundStyle(next ? .water : .haze)
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
                            Text(compact ? "This season" : "This Season")
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
