// The iPhone's screens: Continue, the calendar a day at a time, the Library and Search.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

// MARK: - Continue

/// Continue on an iPhone: the next episode as the hero, the rest of the queue as a rail of
/// banner cards, then New this week. With nothing watched, the setup card and what airs.
struct PhoneContinue: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let queue = store.continuing.queue(store.titles)
        ScrollView {
            switch store.continuing.list.state {
            case .empty:
                WelcomeContinue(compact: true)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 24)
            case .failed(let reason):
                ListFailed(reason: reason)
            default:
                VStack(alignment: .leading, spacing: 0) {
                    if let hero = queue.inProgress.first ?? queue.newThisWeek.first {
                        PhoneHero(item: hero)
                            .padding(.top, 8)
                    }
                    let rail = Array(queue.inProgress.dropFirst())
                    if !rail.isEmpty {
                        PhoneHeading("In progress").padding(.top, 18)
                        ScrollView(.horizontal) {
                            HStack(alignment: .top, spacing: 12) {
                                ForEach(rail) { PhoneCard(item: $0) }
                            }
                            .padding(.horizontal, 16)
                        }
                        .scrollIndicators(.never)
                        .padding(.horizontal, -16)
                        .padding(.top, 10)
                    }
                    let fresh = queue.inProgress.isEmpty ? Array(queue.newThisWeek.dropFirst()) : queue.newThisWeek
                    if !fresh.isEmpty {
                        PhoneHeading("New this week").padding(.top, 16)
                        ForEach(fresh) { PhoneNewRow(item: $0) }
                            .padding(.top, 2)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 24)
            }
        }
        .scrollContentBackground(.hidden)
        .navigationSubtitle(store.continuing.list.state == .empty ? "Nothing watched yet" : "")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button("Accounts and settings", systemImage: "person.crop.circle") {
                    navigator.open(.settings)
                }
            }
        }
        .task { store.loadContinue() }
        .task(id: queue.inProgress.first?.id) {
            if let id = queue.inProgress.first?.id, store.titles.episodes[id] == nil {
                store.loadEpisodes(id)
            }
        }
    }
}

/// A heading over a group on an iPhone screen: 20 semibold.
struct PhoneHeading: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .font(.system(size: 20, weight: .semibold))
            .fixedLineHeight(24, relativeTo: .title3)
            .foregroundStyle(.silk)
            .accessibilityAddTraits(.isHeader)
    }
}

/// The next episode: its wide art (see `HeroArt`), the title, the episode, how far into it and
/// the amber action. With no wide art the cover stands beside the title instead.
struct PhoneHero: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let item: ContinueItem

    var body: some View {
        let entry = store.titles[item.id] ?? item.entry
        let art = store.heroArt(entry)
        VStack(alignment: .leading, spacing: 0) {
            if art.isCompact {
                HStack(alignment: .bottom, spacing: 14) {
                    CoverArt(entry: entry, width: 88)
                        .matchedTransitionSourceIfAvailable(id: item.id)
                        .onTapGesture { navigator.openTitle(item.id) }
                    titleBlock(entry)
                }
            } else {
                HeroArtView(art: art, entry: entry)
                    .aspectRatio(16 / 9, contentMode: .fit)
                    .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
                    .matchedTransitionSourceIfAvailable(id: item.id)
                    .onTapGesture { navigator.openTitle(item.id) }
                titleBlock(entry)
                    .padding(.top, 8)
            }
            HStack(spacing: 14) {
                VStack(alignment: .leading, spacing: 6) {
                    if let fraction = item.fraction {
                        Meter(fraction: fraction, height: 4)
                    }
                    if let left = item.timeLeft {
                        Text(left).font(.system(size: 13)).foregroundStyle(.haze)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Button {
                    store.send(.play(id: item.id, episode: String(item.episode)))
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "play.fill").font(.system(size: 12))
                        Text(item.action)
                    }
                    .lineLimit(1)
                    .fixedSize()
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 44, fontSize: 17))
                .layoutPriority(1)
            }
            .padding(.top, 8)
        }
        .task(id: item.id) { store.loadStills(item.id) }
    }

    private func titleBlock(_ entry: Entry) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(entry.showTitle)
                .font(.showTitle(size: 22, relativeTo: .title2))
                .fixedLineHeight(28, relativeTo: .title2)
                .foregroundStyle(.silk)
            Text(item.episodeLine(store.titles))
                .font(.system(size: 15))
                .foregroundStyle(.haze)
        }
    }
}

/// A title under way in the rail: its wide art (see `HeroArt`), how far into the episode, the
/// title and what is left. With no wide art the cover stands in the art's place at its own 2:3,
/// on the ground rather than a plate.
struct PhoneCard: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let item: ContinueItem

    var body: some View {
        let entry = store.titles[item.id] ?? item.entry
        let art = store.heroArt(entry)
        Button {
            navigator.openTitle(item.id)
        } label: {
            VStack(alignment: .leading, spacing: 0) {
                Group {
                    if art.isCompact {
                        CoverArt(entry: entry, width: 78)
                            .frame(width: 208, height: 117, alignment: .leading)
                    } else {
                        HeroArtView(art: art, entry: entry)
                            .frame(width: 208, height: 117)
                            .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
                    }
                }
                .matchedTransitionSourceIfAvailable(id: item.id)
                Group {
                    if let fraction = item.fraction {
                        Meter(fraction: fraction, height: 3)
                    } else {
                        Color.clear.frame(height: 3)
                    }
                }
                .padding(.top, 6)
                Text(entry.showTitle)
                    .font(.showTitle(size: 17, relativeTo: .body))
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                    .padding(.top, 6)
                Text(item.timeLeft.map { "E\(item.episode) · \($0)" } ?? item.action)
                    .font(.system(size: 15))
                    .foregroundStyle(.haze)
            }
            .frame(width: 208, alignment: .leading)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .contextMenu { QueueMenu(item: item) }
        // The cards on screen only: a still stands in for a missing banner.
        .onScrollVisibilityChange(threshold: 0.3) { visible in
            if visible { store.loadStills(item.id) }
        }
    }
}

/// A new episode: the cover, the title and what it plays, with the water dot.
struct PhoneNewRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let item: ContinueItem

    var body: some View {
        Button {
            navigator.openTitle(item.id)
        } label: {
            HStack(spacing: 12) {
                ArtworkView(url: item.entry.coverUrl, color: item.entry.plateColor, maxPixelSize: 180)
                    .frame(width: 40, height: 60)
                    .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: 40)))
                    .matchedTransitionSourceIfAvailable(id: item.id)
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.entry.showTitle)
                        .font(.showTitle(size: 17, relativeTo: .body))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    HStack(spacing: 6) {
                        Circle().fill(.water).frame(width: 6, height: 6)
                        Text(item.action)
                    }
                    .font(.system(size: 15))
                    .foregroundStyle(.haze)
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(.haze)
            }
            .frame(height: 64)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .contextMenu { QueueMenu(item: item) }
    }
}

// MARK: - Calendar

/// The calendar a day at a time: the week as a strip of days, then the chosen day and the rest of
/// the week as lists.
struct PhoneCalendar: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var chosen = Calendar.current.startOfDay(for: .now)

    var body: some View {
        @Bindable var navigator = navigator
        let days = weekDays(offset: navigator.calendarWeek)
        let broadcasts = store.broadcasts(mine: navigator.calendarMine)
        let now = Date.now
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 0) {
                    ForEach(days, id: \.self) { day in
                        dayButton(day, now: now, broadcasts: broadcasts)
                            .frame(maxWidth: .infinity)
                    }
                }
                .padding(.vertical, 8)
                ListStates(state: store.calendar.list.state, placeholder: AnyView(Color.clear.frame(height: 200))) { _ in
                    VStack(alignment: .leading, spacing: 20) {
                        ForEach(days.filter { $0 >= chosen }, id: \.self) { day in
                            let today = broadcasts.filter { Calendar.current.isDate($0.airs, inSameDayAs: day) }
                            VStack(alignment: .leading, spacing: 0) {
                                PhoneHeading(dayHeading(day))
                                if today.isEmpty {
                                    Text("Nothing airing")
                                        .font(.system(size: 15))
                                        .foregroundStyle(.haze)
                                        .padding(.top, 4)
                                }
                                ForEach(today) { broadcast in
                                    PhoneBroadcastRow(broadcast: broadcast, now: now)
                                    if broadcast.id != today.last?.id {
                                        Rectangle().fill(.faintRule).frame(height: 1).padding(.leading, 66)
                                    }
                                }
                            }
                        }
                    }
                    .padding(.top, 8)
                } empty: {
                    QuietLine("Nothing airing")
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 24)
        }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Picker("Show", selection: $navigator.calendarMine) {
                    Text("My list").tag(true)
                    Text("All airing").tag(false)
                }
                .pickerStyle(.segmented)
                .fixedSize()
            }
        }
        .task { if store.calendar.list.state == .idle { store.loadCalendar() } }
    }

    private func dayHeading(_ day: Date) -> String {
        if Calendar.current.isDateInToday(day) { return "Today" }
        if Calendar.current.isDateInTomorrow(day) { return "Tomorrow" }
        return CalendarWords.weekday(day)
    }

    private func dayButton(_ day: Date, now: Date, broadcasts: [Broadcast]) -> some View {
        let selected = Calendar.current.isDate(day, inSameDayAs: chosen)
        let today = Calendar.current.isDateInToday(day)
        let fresh = broadcasts.contains { Calendar.current.isDate($0.airs, inSameDayAs: day) && !$0.watched }
        return Button {
            withAnimation(.quick) { chosen = Calendar.current.startOfDay(for: day) }
        } label: {
            VStack(spacing: 4) {
                Text(CalendarWords.shortWeekday(day))
                    .font(.system(size: 11, weight: selected || today ? .semibold : .medium))
                    .kerning(0.88)
                    .foregroundStyle(selected ? .amber : .haze)
                Text(CalendarWords.dayNumber(day))
                    .font(.system(size: 17, weight: selected ? .semibold : .regular))
                    .foregroundStyle(selected ? .amber : .silk)
                Circle().fill(fresh ? Palette.water : .clear).frame(width: 5, height: 5)
            }
            .frame(width: 44, height: 64)
            .background(selected ? Palette.selection : .clear, in: .capsule)
            .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("\(CalendarWords.weekday(day)) \(CalendarWords.dayMonth(day))")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// A broadcast in the iPhone's calendar: its time, its cover, the title and what it is.
struct PhoneBroadcastRow: View {
    @Environment(Navigator.self) private var navigator
    let broadcast: Broadcast
    let now: Date

    var body: some View {
        Button {
            navigator.openTitle(broadcast.entry.id)
        } label: {
            HStack(spacing: 14) {
                Text(CalendarWords.time(broadcast.airs))
                    .font(.system(size: 15))
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    .frame(width: 52, alignment: .leading)
                ArtworkView(url: broadcast.entry.coverUrl, color: broadcast.entry.plateColor, maxPixelSize: 180)
                    .frame(width: 50, height: 75)
                    .clipShape(.rect(cornerRadius: 6))
                    .opacity(broadcast.watched ? 0.6 : 1)
                    .matchedTransitionSourceIfAvailable(id: broadcast.entry.id)
                VStack(alignment: .leading, spacing: 2) {
                    Text(broadcast.entry.showTitle)
                        .font(.showTitle(size: 17, relativeTo: .body))
                        .foregroundStyle(broadcast.watched ? .haze : .silk)
                        .lineLimit(2)
                    HStack(spacing: 6) {
                        if broadcast.isNew(now: now) {
                            Circle().fill(.water).frame(width: 6, height: 6)
                        }
                        Text(broadcast.watched ? "Episode \(broadcast.episode)" : broadcast.line(now: now)
                            .replacingOccurrences(of: "Ep ", with: "Episode "))
                    }
                    .font(.system(size: 15))
                    .foregroundStyle(.haze)
                }
                Spacer(minLength: 0)
                if broadcast.watched {
                    Image(systemName: "checkmark")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(.haze)
                        .accessibilityLabel("Watched")
                }
            }
            .padding(.vertical, 8)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .contextMenu { TitleMenu(entry: broadcast.entry) }
    }
}

// MARK: - Library

/// The Library on iPhone and iPad: Downloads, the lists as chips with their counts, the covers;
/// or, signed out, where the lists come from.
struct PhoneLibrary: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        @Bindable var navigator = navigator
        Group {
            if store.accounts.summary.state == .signedOut {
                ScrollView {
                    VStack(spacing: 0) {
                        DownloadsRow().padding(.top, 8)
                        SignedOutLibrary()
                            .frame(minHeight: 560)
                    }
                    .padding(.horizontal, 16)
                }
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        DownloadsRow().padding(.top, 8)
                        ScrollView(.horizontal) {
                            HStack(spacing: 8) {
                                ForEach(LibraryModel.order, id: \.self) { segment in
                                    let count = store.library.segment(segment).state.ids.count
                                    let selected = navigator.librarySegment == segment
                                    Button {
                                        withAnimation(.quick) { navigator.librarySegment = segment }
                                    } label: {
                                        HStack(spacing: 6) {
                                            Text(segment.label.capitalizedSentence)
                                            Text("\(count)").foregroundStyle(selected ? .amber : .haze)
                                        }
                                        .font(.system(size: 17))
                                        .foregroundStyle(selected ? .amber : .silk)
                                        .padding(.horizontal, 16)
                                        .frame(height: 40)
                                        .background(selected ? Palette.selection : .clear, in: .capsule)
                                        .overlay(Capsule().strokeBorder(selected ? .clear : Palette.faintRule))
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                            .padding(.horizontal, 16)
                        }
                        .scrollIndicators(.never)
                        .padding(.horizontal, -16)
                        LibraryScreen(embedded: true)
                    }
                    .padding(.horizontal, 16)
                    .padding(.bottom, 24)
                }
            }
        }
        .task {
            for segment in LibraryModel.order where store.library.segment(segment).state == .idle {
                store.loadLibrary(segment)
            }
        }
    }
}

/// The Downloads row at the top of the Library.
struct DownloadsRow: View {
    @Environment(Navigator.self) private var navigator

    var body: some View {
        Button {
            navigator.open(.downloads)
        } label: {
            HStack(spacing: 14) {
                Image(systemName: "arrow.down.to.line").font(.system(size: 17))
                Text("Downloads").font(.system(size: 17))
                Spacer()
                Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold)).foregroundStyle(.haze)
            }
            .foregroundStyle(.silk)
            .padding(.horizontal, 18)
            .frame(height: 56)
            .background(.raised, in: .rect(cornerRadius: 16))
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Search

/// Search on iPhone and iPad: recent searches, this season and the genres before anything is
/// typed, the titles found after, with the search field the tab bar gives it.
struct PhoneSearch: View {
    var body: some View {
        SearchScreen(compact: true)
    }
}
#endif
