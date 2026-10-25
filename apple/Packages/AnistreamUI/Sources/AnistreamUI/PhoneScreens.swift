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
    @Environment(\.playerUp) private var playerUp
    @Environment(Navigator.self) private var navigator
    /// Whether the queue is what shows, so New this week's rows stand under its hero.
    @State private var rowsShown = false

    var body: some View {
        let queue = store.continuing.queue(store.titles)
        let fresh = Self.fresh(queue)
        // A list, so New this week's rows take their swipes (native-ux), drawn as the scroll
        // view was: the states, the hero and the rail one row, then a row each.
        List {
            ListStates(
                state: store.continuing.list.state,
                placeholder: AnyView(PhoneContinuePlaceholder().padding(.horizontal, 16)),
                activity: "loading continue watching",
                failure: "Couldn't load Continue",
                retry: { store.loadContinue(force: true) },
                offlineInset: 16,
                rowsShown: $rowsShown
            ) { _ in
                if queue.all.isEmpty {
                    NothingToContinue().frame(minHeight: 480)
                } else {
                    queueView(queue, fresh: fresh)
                }
            } empty: {
                WelcomeContinue(compact: true)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 24)
            }
            .bareRow()
            if rowsShown, !fresh.isEmpty {
                ForEach(fresh) { item in
                    PhoneNewRow(item: item)
                        .continueSwipes(item)
                        .bareRow(EdgeInsets(top: 2, leading: 16, bottom: 0, trailing: 16))
                }
                Color.clear.frame(height: 24).bareRow()
            }
        }
        .bareList()
        .navigationSubtitle(store.syncSubtitle(nothingWatched: store.continuing.list.state == .empty))
        .toolbar { AccountButton() }
        .pullToRefresh(store, .continueWatching)
        .task { store.loadContinue() }
        // A watch moves the queue on: asked again once the player has gone, fresh or not.
        .onChange(of: playerUp) { _, up in if !up { store.loadContinue(force: true) } }
        .task(id: queue.inProgress.first?.id) {
            if let id = queue.inProgress.first?.id, store.titles.episodes[id] == nil {
                store.loadEpisodes(id)
            }
        }
    }

    /// New this week's titles, less the one the hero shows.
    private static func fresh(_ queue: ContinueQueue) -> [ContinueItem] {
        queue.inProgress.isEmpty ? Array(queue.newThisWeek.dropFirst()) : queue.newThisWeek
    }

    private func queueView(_ queue: ContinueQueue, fresh: [ContinueItem]) -> some View {
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
            if !fresh.isEmpty {
                // Its rows follow as rows of the list.
                PhoneHeading("New this week").padding(.top, 16)
            }
        }
        .padding(.horizontal, 16)
        .padding(.bottom, fresh.isEmpty ? 24 : 0)
    }
}

/// Continue on an iPhone while it loads, past 600 ms: the hero and a row as they will stand,
/// static and redacted.
struct PhoneContinuePlaceholder: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            RoundedRectangle(cornerRadius: ArtShape.bannerRadius)
                .fill(.raised)
                .aspectRatio(16 / 9, contentMode: .fit)
            VStack(alignment: .leading, spacing: 2) {
                Text("Hoshikuzu no Tabibito").font(.showTitle(size: 22, relativeTo: .title2))
                Text("Episode 7 · The Lantern Road").font(.ui(15))
            }
            .padding(.top, 8)
            Text("In progress").font(.ui(20, weight: .semibold)).padding(.top, 18)
            HStack(spacing: 12) {
                ForEach(0..<2, id: \.self) { _ in
                    VStack(alignment: .leading, spacing: 6) {
                        RoundedRectangle(cornerRadius: ArtShape.bannerRadius).fill(.raised).frame(width: 208, height: 117)
                        Text("Kumo no Ue no Toshokan").font(.showTitle(size: 17, relativeTo: .body))
                        Text("E4 · 12 min left").font(.ui(15))
                    }
                }
            }
            .padding(.top, 10)
        }
        .redacted(reason: .placeholder)
        .foregroundStyle(.haze)
        .padding(.top, 8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .clipped()
        .accessibilityHidden(true)
    }
}


/// The account button on Continue and Library: Accounts, Providers, Plugin stores and Settings
/// in a sheet.
struct AccountButton: ToolbarContent {
    @Environment(Navigator.self) private var navigator

    var body: some ToolbarContent {
        ToolbarItem(placement: .topBarTrailing) {
            Button("Accounts and settings", systemImage: "person.crop.circle") {
                navigator.phoneSettings = .root
            }
        }
    }
}

extension AppStore {
    /// Where sync stands, as the iPhone's Continue and Library say it under their titles: the
    /// tracker's state, or with none signed in, that nothing is watched yet when that is so.
    func syncSubtitle(nothingWatched: Bool = false) -> String {
        let summary = accounts.summary
        if summary.state == .signedOut { return nothingWatched ? "Nothing watched yet" : "" }
        if jobs.shown.contains(where: { $0.activity == "syncing" }) { return "Syncing" }
        return summary.text
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
            .font(.ui(20, weight: .semibold))
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
    @Environment(\.dynamicTypeSize) private var typeSize
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
                        .accessibilityHidden(true)
                    titleBlock(entry)
                }
            } else {
                HeroArtView(art: art, entry: entry)
                    .aspectRatio(16 / 9, contentMode: .fit)
                    .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
                    .matchedTransitionSourceIfAvailable(id: item.id)
                    // Where the player zooms out of when this plays (T2).
                    .playerZoomSource(item.id)
                    .onTapGesture { navigator.openTitle(item.id) }
                    .accessibilityHidden(true)
                titleBlock(entry)
                    .padding(.top, 8)
            }
            // At the accessibility sizes the action goes under the meter, the width of the screen.
            let stacked = typeSize.isAccessibilitySize
            let layout = stacked
                ? AnyLayout(VStackLayout(alignment: .leading, spacing: 12))
                : AnyLayout(HStackLayout(spacing: 14))
            layout {
                VStack(alignment: .leading, spacing: 6) {
                    Held(value: item.fraction) { fraction in
                        if let fraction { Meter(fraction: fraction, height: 4, spoken: item.timeLeft) }
                    }
                    Held(value: item.minutesLeft) { minutes in
                        if let minutes { MinutesLeft(minutes: minutes) }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                // The title above says how far, in words.
                .accessibilityHidden(true)
                if item.playable {
                    Button {
                        store.send(.play(id: item.id, episode: String(item.episode)))
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: "play.fill").font(.ui(12))
                            Text(item.action)
                        }
                        .fixedSize(horizontal: !stacked, vertical: false)
                        .frame(maxWidth: stacked ? .infinity : nil)
                    }
                    .buttonStyle(PrimaryCapsuleStyle(height: 44, fontSize: 17))
                    .layoutPriority(1)
                } else {
                    // Caught up: when the next one airs, in the button's place.
                    AirsStatus(text: item.buttonTitle, height: 44, fontSize: 17, fills: stacked)
                        .fixedSize(horizontal: !stacked, vertical: false)
                        .layoutPriority(1)
                }
            }
            .padding(.top, 8)
        }
        .task(id: item.id) { store.loadStills(item.id) }
    }

    private func titleBlock(_ entry: Entry) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(entry.showTitle)
                .font(.showTitle(size: 22, relativeTo: .title2))
                .foregroundStyle(.silk)
                .lineLimit(4)
                .dynamicTypeSize(...DynamicTypeSize.accessibility3)
            Text(item.episodeLine(store.titles))
                .font(.ui(15))
                .foregroundStyle(.haze)
        }
        .titleElement(entry, value: item.spokenProgress) { QueueMenu(item: item) }
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { navigator.openTitle(item.id) }
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
                .playerZoomSource(item.id)
                Held(value: item.fraction) { fraction in
                    if let fraction {
                        Meter(fraction: fraction, height: 3, spoken: item.timeLeft)
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
                HStack(spacing: 6) {
                    if !item.playable { Circle().fill(.water).frame(width: 6, height: 6) }
                    Text(item.timeLeft.map { "E\(item.episode) · \($0)" } ?? item.action)
                }
                .font(.ui(15))
                .foregroundStyle(.haze)
            }
            .frame(width: 208, alignment: .leading)
            // A card's caption is no larger than a grid's.
            .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(entry, value: item.spokenProgress) { QueueMenu(item: item) }
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
                        .rowTitle()
                    HStack(spacing: 6) {
                        Circle().fill(.water).frame(width: 6, height: 6)
                        Text(item.action)
                    }
                    .font(.ui(15))
                    .foregroundStyle(.haze)
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.ui(13, weight: .semibold))
                    .foregroundStyle(.haze)
            }
            .frame(minHeight: 64)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(item.entry, value: item.spokenProgress) { QueueMenu(item: item) }
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
    @Namespace private var rotor

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
                // Seven to a row: no larger than `.xxxLarge`, as controls are.
                .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
                .padding(.vertical, 8)
                ListStates(
                    state: store.calendar.list.state,
                    placeholder: AnyView(PhoneCalendarPlaceholder()),
                    activity: "loading the calendar",
                    failure: "Couldn't load the calendar",
                    retry: { store.loadCalendar() }
                ) { _ in
                    VStack(alignment: .leading, spacing: 20) {
                        ForEach(days.filter { $0 >= chosen }, id: \.self) { day in
                            let today = broadcasts.filter { Calendar.current.isDate($0.airs, inSameDayAs: day) }
                            VStack(alignment: .leading, spacing: 0) {
                                PhoneHeading(dayHeading(day))
                                    .accessibilityRotorEntry(id: day, in: rotor)
                                if today.isEmpty {
                                    Text("Nothing airing")
                                        .font(.ui(15))
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
        .daysRotor(days.filter { $0 >= chosen }, broadcasts: broadcasts, in: rotor)
        .pullToRefresh(store, .calendar)
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
                    .font(.ui(11, weight: selected || today ? .semibold : .medium))
                    .kerning(0.88)
                    .foregroundStyle(selected ? .amber : .haze)
                Text(CalendarWords.dayNumber(day))
                    .font(.ui(17, weight: selected ? .semibold : .regular))
                    .foregroundStyle(selected ? .amber : .silk)
                Circle().fill(fresh ? Palette.water : .clear).frame(width: 5, height: 5)
            }
            .frame(width: 44)
            .frame(minHeight: 64)
            .background(selected ? Palette.selection : .clear, in: .capsule)
            .contentShape(.capsule)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityLabel("\(CalendarWords.weekday(day)) \(CalendarWords.dayMonth(day))")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }
}

/// The day while the calendar loads, past 600 ms: rows as they will stand, static and redacted.
struct PhoneCalendarPlaceholder: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Today").font(.ui(20, weight: .semibold))
            ForEach(0..<3, id: \.self) { _ in
                HStack(spacing: 14) {
                    Text("17:30").font(.ui(15)).frame(minWidth: 52, alignment: .leading)
                    RoundedRectangle(cornerRadius: 6).fill(.raised).frame(width: 50, height: 75)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Natsukaze Relay").font(.showTitle(size: 17, relativeTo: .body))
                        Text("Episode 9 in 1h 30m").font(.ui(15))
                    }
                    Spacer(minLength: 0)
                }
                .padding(.vertical, 8)
            }
        }
        .redacted(reason: .placeholder)
        .foregroundStyle(.haze)
        .padding(.top, 8)
        .accessibilityHidden(true)
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
                Text(broadcast.clock)
                    .font(.ui(15))
                    .monospacedDigit()
                    .foregroundStyle(.haze)
                    // One line, as wide as it needs: no larger than `.xxxLarge`.
                    .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
                    .fixedSize()
                    .frame(minWidth: 52, alignment: .leading)
                ArtworkView(url: broadcast.entry.coverUrl, color: broadcast.entry.plateColor, maxPixelSize: 180)
                    .frame(width: 50, height: 75)
                    .clipShape(.rect(cornerRadius: 6))
                    .opacity(broadcast.watched ? 0.6 : 1)
                    .matchedTransitionSourceIfAvailable(id: broadcast.entry.id)
                VStack(alignment: .leading, spacing: 2) {
                    Text(broadcast.entry.showTitle)
                        .font(.showTitle(size: 17, relativeTo: .body))
                        .foregroundStyle(broadcast.watched ? .haze : .silk)
                        .rowTitle()
                    HStack(spacing: 6) {
                        if broadcast.isNew(now: now) {
                            Circle().fill(.water).frame(width: 6, height: 6)
                        }
                        Text((broadcast.watched ? "Episode \(broadcast.episode)" : broadcast.line(now: now)
                            .replacingOccurrences(of: "Ep ", with: "Episode "))
                            + (broadcast.mark.map { " · \($0)" } ?? ""))
                    }
                    .font(.ui(15))
                    .foregroundStyle(.haze)
                    if !broadcast.watched, let note = broadcast.delayNote {
                        Text("Delayed \(note)")
                            .font(.ui(13))
                            .monospacedDigit()
                            .foregroundStyle(.haze)
                    }
                }
                Spacer(minLength: 0)
                if broadcast.watched {
                    Image(systemName: "checkmark")
                        .font(.ui(15, weight: .semibold))
                        .foregroundStyle(.haze)
                        .accessibilityLabel("Watched")
                }
            }
            .padding(.vertical, 8)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .titleElement(broadcast.entry, value: broadcast.spoken(now: now)) { TitleActions(entry: broadcast.entry) }
        .contextMenu { TitleMenu(entry: broadcast.entry) }
    }
}

// MARK: - Library

/// The Library on iPhone and iPad: Downloads, the lists as chips with their counts, the covers;
/// or, signed out, where the lists come from.
struct PhoneLibrary: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass

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
                                        .font(.ui(17))
                                        .foregroundStyle(selected ? .amber : .silk)
                                        .padding(.horizontal, 16)
                                        .frame(height: 40)
                                        .background(selected ? Palette.selection : .clear, in: .capsule)
                                        .overlay(Capsule().strokeBorder(selected ? .clear : Palette.faintRule))
                                    }
                                    .buttonStyle(.plain)
                                    .pointerHover()
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
        .pullToRefresh(store, .library(navigator.librarySegment))
        .navigationSubtitle(sizeClass == .compact ? store.syncSubtitle() : "")
        .toolbar { if sizeClass == .compact { AccountButton() } }
        .task {
            for segment in LibraryModel.order where store.library.segment(segment).state == .idle {
                store.loadLibrary(segment)
            }
        }
    }
}

/// The Downloads row at the top of the Library, with how many there are: Downloads in the
/// sidebar of a wide iPad, pushed over the Library on iPhone and in a narrow window.
struct DownloadsRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        let count = store.downloads.rows.count
        Button {
            navigator.showDownloads(compact: sizeClass == .compact)
        } label: {
            HStack(spacing: 12) {
                Image(systemName: "arrow.down.to.line").font(.ui(17)).accessibilityHidden(true)
                Text("Downloads").font(.ui(17))
                Spacer()
                if count > 0 {
                    Text("\(count)").font(.ui(17)).monospacedDigit().foregroundStyle(.haze)
                }
                Image(systemName: "chevron.right").font(.ui(13, weight: .semibold)).foregroundStyle(.haze)
            }
            .foregroundStyle(.silk)
            .padding(.horizontal, 16)
            .frame(minHeight: 52)
            .background(.raised, in: .rect(cornerRadius: 20))
            .contentShape(.rect(cornerRadius: 20))
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityValue(count > 0 ? Wording.plural(count, "episode") : "")
        .task { store.loadDownloads() }
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
