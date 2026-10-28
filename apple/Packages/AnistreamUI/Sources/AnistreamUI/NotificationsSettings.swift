// New-episode notifications in Settings: the Notifications pane on the Mac and iPad, its page on
// iPhone, and the switch in each show's own settings (MacSettingsNotify, PadSettingsNotify,
// PhoneSettingsNotify, NativeNotify).

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// The words the pane and the page share.
@MainActor
enum NoticeWords {
    static var permission: String { L.settingsNotifyPermission }
    static var empty: String { L.settingsWeekEmpty }
    static var onInItsSettings: String { L.settingsNotificationsShowOn }

    /// What the system calls its own settings here.
    static var systemSettings: String {
        #if os(macOS)
        L.settingsSystemAppMac
        #else
        L.settingsSystemAppIos
        #endif
    }

    /// The switch's line: who it is for, and how many shows set their own.
    static func line(_ store: AppStore, counts: Bool = true) -> String {
        if store.notifications.declined || store.notifications.permission == .denied,
           !store.settings.isOn(.episodeNotifications) {
            return L.settingsNotifyNotAllowed(settings: systemSettings)
        }
        let overrides = Int(store.notifications.week?.overrides ?? 0)
        guard counts, overrides > 0 else { return L.settingsNewEpisodesDetail }
        return "\(L.settingsNewEpisodesDetail) · \(overridden(overrides))"
    }

    static func overridden(_ count: Int) -> String {
        L.settingsOverridden(count: count)
    }

    /// The footnote under the week: what notifies later than its broadcast and why, what is
    /// left out and why, and what a row does.
    static func leftOut(_ store: AppStore) -> String {
        leftOut(dubbed: store.notifications.dubbed, off: store.notifications.switchedOff)
    }

    /// The footnote for the shows watched `dubbed`, told when the dub is out rather than at the
    /// time a row says, and those switched `off`.
    static func leftOut(dubbed: [String], off: [String]) -> String {
        var lines: [String] = []
        switch dubbed.count {
        case 0: break
        case 1: lines.append(L.settingsWeekDubOne(show: dubbed[0]))
        default: lines.append(L.settingsWeekDubMany(shows: names(dubbed)))
        }
        switch off.count {
        case 0: break
        case 1: lines.append(L.settingsWeekOffOne(show: off[0]))
        default: lines.append(L.settingsWeekOffMany(shows: names(off)))
        }
        lines.append(L.settingsWeekRowOpens)
        return lines.joined(separator: " ")
    }

    /// "A, B and C".
    private static func names(_ titles: [String]) -> String {
        titles.formatted(.list(type: .and).locale(AirTime.locale))
    }

    /// When an episode airs, as a row says it: "Today 22:00", "Tomorrow 09:30", "Thursday 16:30".
    static func when(_ airsAt: Int64, now: Date = .now, calendar: Calendar = .current) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(airsAt))
        let time = date.formatted(date: .omitted, time: .shortened)
        if calendar.isDate(date, inSameDayAs: now) { return L.settingsWhenToday(time: time) }
        if let tomorrow = calendar.date(byAdding: .day, value: 1, to: now),
           calendar.isDate(date, inSameDayAs: tomorrow) {
            return L.settingsWhenTomorrow(time: time)
        }
        return L.settingsWhenDay(day: date.formatted(.dateTime.weekday(.wide).locale(AirTime.locale)), time: time)
    }

    /// The system's notification settings for the app.
    static var settingsURL: URL? {
        #if os(macOS)
        let id = Bundle.main.bundleIdentifier ?? "tv.anistream.app"
        return URL(string: "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id=\(id)")
        #else
        return URL(string: UIApplication.openNotificationSettingsURLString)
        #endif
    }
}

/// The New episodes switch: on asks for permission the first time; declined, it stays off.
struct EpisodeNoticesToggle<Label: View>: View {
    @Environment(AppStore.self) private var store
    @ViewBuilder let label: Label

    var body: some View {
        Toggle(isOn: Binding(
            get: { store.settings.isOn(.episodeNotifications) },
            set: { on in Task { await store.setEpisodeNotifications(on) } }
        )) {
            label
        }
        .tint(.amber)
        .disabled(store.settings.row(.episodeNotifications) == nil)
    }
}

/// One episode of the week: the cover, the show, "Episode 4 · Today 22:00", and why it notifies
/// when its own settings say so. Dimmed when it follows the switch and the switch is off.
struct ComingUpRow: View {
    @Environment(AppStore.self) private var store
    @Environment(\.displayScale) private var scale
    let row: UpcomingNotice
    /// The iPhone's sizes and its shorter "E4".
    var phone = false

    var body: some View {
        let width: CGFloat = phone ? 40 : 36
        HStack(spacing: 12) {
            ArtworkView(url: row.coverUrl, maxPixelSize: Int(width * 1.5 * scale))
                .frame(width: width, height: (width * 3 / 2).rounded())
                .clipShape(.rect(cornerRadius: 6))
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.title)
                    .font(.macShowTitle(size: phone ? 17 : 15))
                    .foregroundStyle(.silk)
                    .lineLimit(phone ? 2 : 1)
                Text("\(phone ? L.episodeNumberShort(number: Int(row.episode)) : L.episodeNumber(number: Int(row.episode))) · \(NoticeWords.when(row.airsAt))")
                    .font(phone ? .ui(15) : .mac(12))
                    .foregroundStyle(.haze)
                if phone, row.reason == .switchedOn {
                    Text(NoticeWords.onInItsSettings).font(.ui(13)).foregroundStyle(.haze)
                }
            }
            Spacer(minLength: 8)
            if !phone, row.reason == .switchedOn {
                Text(NoticeWords.onInItsSettings).font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
            }
            // On iPhone the list's own disclosure chevron says the row opens a page.
            if !phone {
                Image(systemName: "chevron.right")
                    .font(.mac(11, weight: .semibold))
                    .foregroundStyle(.haze)
            }
        }
        .opacity(store.notifications.notifies(row) ? 1 : 0.4)
        .contentShape(.rect)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(L.settingsWeekRowLabel(title: row.title, episode: Int(row.episode), when: NoticeWords.when(row.airsAt)))
        .accessibilityHint(L.settingsWeekRowHint)
    }
}

// MARK: - Mac and iPad

/// The Notifications pane: the New episodes switch, and what will notify in the next 7 days.
struct NotificationsPane: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        SettingsGroup(title: L.settingsNewEpisodes, footnote: NoticeWords.permission) {
            SettingsRow(title: L.settingsNewEpisodes, note: NoticeWords.line(store)) {
                EpisodeNoticesToggle { Text(L.settingsNewEpisodes) }
                    .toggleStyle(.switch)
                    .labelsHidden()
            }
        }
        SettingsGroup(title: L.settingsComingUp, footnote: NoticeWords.leftOut(store)) {
            let rows = store.notifications.comingUp
            if rows.isEmpty {
                SettingsRow(title: store.notifications.week == nil ? L.settingsWeekLoading : NoticeWords.empty)
            }
            ForEach(rows, id: \.rowID) { row in
                Button {
                    store.loadDetail(row.id)
                    navigator.showSettings = row.id
                } label: {
                    ComingUpRow(row: row)
                        .padding(.horizontal, 10)
                        .frame(minHeight: 62)
                        .padding(.horizontal, 4)
                }
                .buttonStyle(.plain)
            }
        }
        .task {
            await store.notifications.refreshPermission()
            store.refreshNotices()
        }
    }
}

/// The column beside the pane: a notification as it will look, and how the switch and each
/// show's own setting combine.
struct NotificationsAside: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL

    var body: some View {
        let on = store.settings.isOn(.episodeNotifications)
        Text(L.settingsNewEpisodes).groupHeaderStyle().macLineHeight(14)
            .accessibilityAddTraits(.isHeader)
        NoticePreview(row: store.notifications.comingUp.first)
            .opacity(on ? 1 : 0.4)
        Text(on ? L.settingsNotifyExplainOn(tracker: AccountsModel.name("anilist")) : L.settingsNotifyExplainOff)
            .font(.mac(13))
            .macLineHeight(18)
            .foregroundStyle(.silk)
            .fixedSize(horizontal: false, vertical: true)
        #if os(macOS)
        Button(L.settingsSystemNotifications) { NoticeWords.settingsURL.map { openURL($0) } }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
        Text(L.settingsSystemNotificationsApple)
            .font(.mac(12))
            .foregroundStyle(.haze)
            .fixedSize(horizontal: false, vertical: true)
        #else
        Button(L.settingsSystemNotifications) { NoticeWords.settingsURL.map { openURL($0) } }
            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
        #endif
    }
}

/// A notification as the system shows one: the app's name, the show, the words, the cover on
/// the right. Drawn, never posted.
struct NoticePreview: View {
    @Environment(\.displayScale) private var scale
    let row: UpcomingNotice?

    var body: some View {
        let words = row.map { NoticePlan.words(episode: $0.episode, nextToWatch: $0.nextToWatch).body }
        HStack(alignment: .top, spacing: 10) {
            AppIconImage()
                .frame(width: 32, height: 32)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text("anistream").font(.mac(11, weight: .medium)) // i18n-skip: the app's name.foregroundStyle(.haze)
                Text(row?.title ?? L.settingsPreviewShow)
                    .font(.mac(13, weight: .semibold))
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                Text(words ?? L.settingsPreviewWords).font(.mac(13)).foregroundStyle(.silk)
            }
            Spacer(minLength: 0)
            ArtworkView(url: row?.bannerUrl ?? row?.coverUrl, maxPixelSize: Int(40 * scale))
                .frame(width: 40, height: 40)
                .clipShape(.rect(cornerRadius: 8))
        }
        .padding(12)
        .chromeGlass(in: .rect(cornerRadius: 18))
        .accessibilityElement(children: .combine)
        .accessibilityLabel(L.settingsPreviewLabel(show: row?.title ?? L.settingsPreviewShow, words: words ?? L.settingsPreviewWords))
    }
}

/// The app's own icon, as the system draws it beside a notification.
struct AppIconImage: View {
    var body: some View {
        #if os(macOS)
        Image(nsImage: NSApplication.shared.applicationIconImage)
            .resizable()
            .interpolation(.high)
        #else
        if let icon = Self.icon {
            Image(uiImage: icon)
                .resizable()
                .clipShape(.rect(cornerRadius: 7, style: .continuous))
        } else {
            RoundedRectangle(cornerRadius: 7, style: .continuous).fill(.raised)
        }
        #endif
    }

    #if os(iOS)
    /// The primary icon the bundle names, at its largest.
    static let icon: UIImage? = {
        let icons = Bundle.main.object(forInfoDictionaryKey: "CFBundleIcons") as? [String: Any]
        let primary = icons?["CFBundlePrimaryIcon"] as? [String: Any]
        let files = primary?["CFBundleIconFiles"] as? [String]
        return files?.last.flatMap(UIImage.init(named:)) ?? UIImage(named: "AppIcon")
    }()
    #endif
}

// MARK: - A show's own switch

/// A show's New episodes choice: Use default, On or Off, saying what following Settings means
/// for it. A show not being watched that follows Settings is off, and says so.
struct ShowNoticesMenu: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId
    /// The Mac's capsule, in title case; the iPhone's value and chevrons otherwise.
    var mac = false

    var body: some View {
        let choice = store.notifications.choice(id)
        let label = choice.label(
            settingOn: store.settings.isOn(.episodeNotifications),
            watching: store.notifications.isWatching(id))
        Menu {
            Picker(L.settingsNewEpisodes, selection: Binding(
                get: { choice },
                set: { new in Task { await store.setShowNotices(id, to: new) } }
            )) {
                ForEach(ShowNoticeChoice.allCases, id: \.self) { option in
                    Text(option.name).tag(option)
                }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            if mac {
                MenuCapsule(text: label, fill: true)
            } else {
                #if os(iOS)
                PhoneMenuValue(text: label)
                #else
                MenuCapsule(text: label)
                #endif
            }
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .accessibilityLabel(L.settingsShowNoticesLabel(value: label))
        .task {
            store.loadShowNotices(id)
            if store.notifications.week == nil { store.refreshNotices() }
        }
    }
}

// MARK: - iPhone

#if os(iOS)
/// The Notifications page of the iPhone's Settings sheet.
struct PhoneNotificationsPage: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL

    var body: some View {
        let overrides = Int(store.notifications.week?.overrides ?? 0)
        PhoneList {
            Section {
                EpisodeNoticesToggle {
                    PhoneRowTitle(title: L.settingsNewEpisodes, note: NoticeWords.line(store, counts: false))
                }
                .listRowBackground(Palette.raised)
            } header: {
                PhoneSectionHeader(L.settingsNewEpisodes)
            } footer: {
                Text(overrides > 0 ? "\(NoticeWords.overridden(overrides)). \(NoticeWords.permission)" : NoticeWords.permission)
            }
            Section {
                let rows = store.notifications.comingUp
                if rows.isEmpty {
                    Text(store.notifications.week == nil ? L.settingsWeekLoading : NoticeWords.empty)
                        .font(.ui(17)).foregroundStyle(.haze)
                        .listRowBackground(Palette.raised)
                }
                ForEach(rows, id: \.rowID) { row in
                    NavigationLink(value: PhoneSettingsPage.showNotices(row.id)) {
                        ComingUpRow(row: row, phone: true)
                    }
                    .listRowBackground(Palette.raised)
                }
            } header: {
                PhoneSectionHeader(L.settingsComingUp)
            } footer: {
                Text(NoticeWords.leftOut(store))
            }
            Section {
                Button {
                    NoticeWords.settingsURL.map { openURL($0) }
                } label: {
                    HStack {
                        Text(L.settingsSystemNotifications).font(.ui(17)).foregroundStyle(.silk)
                        Spacer()
                        Image(systemName: "arrow.up.forward").font(.ui(14, weight: .semibold)).foregroundStyle(.haze)
                    }
                }
                .listRowBackground(Palette.raised)
            }
        }
        .navigationTitle(L.settingsGroupNotifications)
        .navigationBarTitleDisplayMode(.inline)
        .task {
            await store.notifications.refreshPermission()
            store.refreshNotices()
        }
    }
}

/// A show's own New episodes switch, opened from a row of the week.
struct PhoneShowNoticesPage: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId

    var body: some View {
        PhoneList {
            Section {
                HStack {
                    PhoneRowTitle(title: L.settingsNewEpisodes)
                    Spacer()
                    ShowNoticesMenu(id: id)
                }
                .listRowBackground(Palette.raised)
            } header: {
                PhoneSectionHeader(L.settingsGroupNotifications)
            } footer: {
                Text(L.settingsNotifyShow)
            }
        }
        .navigationTitle(store.titles[id]?.showTitle
            ?? store.notifications.week?.airings.first { $0.id == id }?.title ?? L.settingsThisShow)
        .navigationBarTitleDisplayMode(.inline)
    }
}
#endif

extension UpcomingNotice {
    /// A row of the week: a show's episode.
    var rowID: String { "\(id).\(episode)" }
}
