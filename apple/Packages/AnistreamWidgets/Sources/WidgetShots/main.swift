// Every widget at every size and in every state, drawn from fixture snapshots, as PNGs to hold
// against the boards (NativeWidgetsPhone, NativeWidgetsMac and NativeWidgetsStates):
//
//   swift run WidgetShots <directory>
//
// The moment is the boards': Tuesday 20 October 2026, 19:30 in Copenhagen. The titles are the
// test kit's fixture catalogue, and the art its stand-in art, made into the widgets' thumbnails
// as the app makes them.

#if os(macOS)
import AnistreamDesign
import AnistreamWidgetData
import AnistreamWidgetUI
import AppKit
import SwiftUI
import WidgetKit

@MainActor
enum Shots {
    static let zone = TimeZone(identifier: "Europe/Copenhagen")!
    static var calendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = zone
        calendar.locale = Locale(identifier: "en_GB")
        return calendar
    }
    static let format = WidgetFormat(calendar: calendar, locale: Locale(identifier: "en_GB"))

    /// 19:30 on Tuesday 20 October 2026.
    static let now = at(day: 20, 19, 30)

    static func at(day: Int, _ hour: Int, _ minute: Int) -> Date {
        calendar.date(from: DateComponents(year: 2026, month: 10, day: day, hour: hour, minute: minute))!
    }

    // MARK: Fixtures

    /// The test kit's stand-in art for each fixture title, and whether AniList has a banner for it.
    static let art: [Int: (name: String, banner: Bool)] = [
        900001: ("stardust", true), 900002: ("library", true), 900003: ("platform", true),
        900004: ("stardust-2", true), 900005: ("gardener", false), 900006: ("relay", false),
    ]

    static func art(_ id: Int, in files: WidgetFiles) -> Art {
        guard let entry = art[id] else { return Art() }
        let source = URL(filePath: #filePath)
            .deletingLastPathComponent()
            .appending(path: "../../../../../crates/anistream-testkit/fixtures/art")
            .standardizedFileURL
        func make(_ kind: ArtKind, from file: String) -> String? {
            let name = "\(kind.rawValue)-\(id).jpg"
            guard let image = ArtThumbnail.image(at: source.appending(path: file), maxPixelSize: 2_000),
                  let data = ArtThumbnail.jpeg(from: image, kind: kind)
            else { return nil }
            try? files.writeArt(data, named: name)
            return name
        }
        return Art(
            wide: entry.banner ? make(.wide, from: "\(entry.name)-wide.png") : nil,
            poster: make(.poster, from: "\(entry.name)-poster.png"))
    }

    static func continuing(_ files: WidgetFiles) -> [SnapshotContinue] {
        [
            SnapshotContinue(
                id: 900001, title: "Hoshikuzu no Tabibito", episode: 7, watched: 6, episodes: 12,
                fraction: 0.42, episodeTitle: "The Lantern Road", resume: true, positionSecs: 600,
                durationSecs: 1_440, minutesLeft: 14, art: art(900001, in: files)),
            SnapshotContinue(
                id: 900005, title: "Hagane no Niwashi", episode: 2, watched: 1, episodes: 10,
                fraction: 0.75, resume: true, positionSecs: 1_080, durationSecs: 1_440, minutesLeft: 6,
                art: art(900005, in: files)),
            SnapshotContinue(
                id: 900002, title: "Kumo no Ue no Toshokan", episode: 4, watched: 3, episodes: 24,
                resume: false, durationSecs: 1_440, minutesLeft: 24, art: art(900002, in: files)),
        ]
    }

    /// The day's schedule: MacCalendar's day, with fixture titles in the places of the boards'.
    static func airing(_ files: WidgetFiles) -> [SnapshotAiring] {
        [
            SnapshotAiring(id: 900004, title: "Hoshikuzu no Tabibito 2nd Season", episode: 3, airsAt: at(day: 20, 13, 0),
                       onList: true, watched: true, art: art(900004, in: files)),
            SnapshotAiring(id: 900006, title: "Natsukaze Relay", episode: 9, airsAt: at(day: 20, 17, 30),
                       onList: true, translation: .sub, art: art(900006, in: files)),
            SnapshotAiring(id: 900005, title: "Hagane no Niwashi", episode: 5, airsAt: at(day: 20, 19, 0),
                       art: art(900005, in: files)),
            SnapshotAiring(id: 900006, title: "Natsukaze Relay", episode: 9, airsAt: at(day: 20, 21, 0),
                       onList: true, translation: .dub, art: art(900006, in: files)),
            SnapshotAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: at(day: 20, 22, 0),
                       onList: true, art: art(900002, in: files)),
            SnapshotAiring(id: 900003, title: "Tsukiakari no Teishajo", episode: 2, airsAt: at(day: 20, 22, 30),
                       art: art(900003, in: files)),
            SnapshotAiring(id: 900001, title: "Hoshikuzu no Tabibito", episode: 8, airsAt: at(day: 20, 23, 45),
                       onList: true, art: art(900001, in: files)),
        ]
    }

    static func snapshot(_ files: WidgetFiles) -> WidgetSnapshot {
        WidgetSnapshot(
            version: 2, generatedAt: at(day: 20, 19, 10), signedIn: true, tracker: "anilist",
            continueWatching: continuing(files), airingToday: airing(files),
            nextDays: [SnapshotAiring(id: 900002, title: "Kumo no Ue no Toshokan", episode: 7,
                                  airsAt: at(day: 27, 22, 0), onList: true)],
            nextAiring: SnapshotNextAiring(id: 900006, title: "Natsukaze Relay", episode: 9, airsAt: at(day: 20, 21, 0)))
    }

    // MARK: Drawing

    struct Device {
        let name: String
        let scale: CGFloat
        let sizes: [(WidgetLayout, CGSize)]
        /// The widget's corner radius.
        let radius: CGFloat

        func size(_ family: WidgetLayout) -> CGSize? {
            sizes.first { $0.0 == family }?.1
        }
    }

    static let iphone = Device(name: "iphone", scale: 3, sizes: [
        (.small, CGSize(width: 158, height: 158)),
        (.medium, CGSize(width: 338, height: 158)),
        (.large, CGSize(width: 338, height: 354)),
        (.rectangular, CGSize(width: 158, height: 72)),
        (.inline, CGSize(width: 300, height: 22)),
    ], radius: 22)

    static let mac = Device(name: "mac", scale: 2, sizes: [
        (.small, CGSize(width: 170, height: 170)),
        (.medium, CGSize(width: 364, height: 170)),
    ], radius: 22)

    static func name(_ layout: WidgetLayout) -> String {
        switch layout {
        case .rectangular: "lock-rectangular"
        case .inline: "lock-inline"
        default: layout.rawValue
        }
    }

    static func render(_ view: some View, family: WidgetLayout, device: Device, files: WidgetFiles, to url: URL) {
        guard let size = device.size(family) else { return }
        let accessory = family == .inline || family == .rectangular
        let widget = view
            .environment(\.widgetFiles, files)
            .environment(\.widgetNow, now)
            .environment(\.widgetDrawsTaps, false)
            .environment(\.timeZone, zone)
            .environment(\.locale, Locale(identifier: "en_GB"))
            .environment(\.colorScheme, .dark)
            .foregroundStyle(accessory ? Color.white : Palette.silk)
            .frame(width: size.width, height: size.height, alignment: accessory ? .leading : .topLeading)
            .background(accessory ? Color.clear : Palette.ground)
            .clipShape(RoundedRectangle(cornerRadius: accessory ? 0 : device.radius, style: .continuous))
        // The boards' dark wallpaper around it.
        let framed = widget
            .padding(16)
            .background(Color(red: 13 / 255, green: 16 / 255, blue: 32 / 255))
        let renderer = ImageRenderer(content: framed)
        renderer.scale = device.scale
        guard let image = renderer.cgImage else {
            print("WidgetShots: could not draw \(url.lastPathComponent)")
            return
        }
        let rep = NSBitmapImageRep(cgImage: image)
        try? rep.representation(using: .png, properties: [:])?.write(to: url)
        print(url.path)
    }

    static func run(out: URL) throws {
        try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
        let files = WidgetFiles(directory: FileManager.default.temporaryDirectory
            .appending(path: "anistream-widget-shots-\(ProcessInfo.processInfo.processIdentifier)"))
        defer { try? FileManager.default.removeItem(at: files.directory) }
        let full = snapshot(files)

        enum Kind { case continuing, airing }

        func shoot(
            _ device: Device, _ prefix: String, _ kind: Kind, _ snapshot: WidgetSnapshot?,
            families: [WidgetLayout]? = nil, publicSchedule: [SnapshotAiring]? = nil,
            filter: AiringFilter = .all, at moment: Date = now
        ) {
            for family in families ?? device.sizes.map(\.0) {
                let url = out.appending(path: "\(device.name)-\(prefix)-\(name(family)).png")
                switch kind {
                case .continuing:
                    render(ContinueWidgetView(snapshot: snapshot, layout: family, now: moment, format: format),
                           family: family, device: device, files: files, to: url)
                case .airing:
                    render(AiringWidgetView(snapshot: snapshot, publicSchedule: publicSchedule, filter: filter,
                                            layout: family, now: moment, format: format),
                           family: family, device: device, files: files, to: url)
                }
            }
        }

        // NativeWidgetsPhone and NativeWidgetsMac.
        for device in [iphone, mac] {
            shoot(device, "continue", .continuing, full)
            shoot(device, "airing", .airing, full)
        }
        shoot(iphone, "airing-my-list", .airing, full, families: [.medium], filter: .myList)

        // NativeWidgetsStates.
        let states: [WidgetLayout] = [.small, .medium]
        var signedOut = full
        signedOut.signedIn = false
        signedOut.tracker = nil
        signedOut.continueWatching = []
        shoot(iphone, "continue-signed-out", .continuing, signedOut, families: states)
        var caughtUp = full
        caughtUp.continueWatching = []
        caughtUp.nextAiring = SnapshotNextAiring(id: 900001, title: "Hoshikuzu no Tabibito", episode: 10, airsAt: at(day: 22, 16, 30))
        shoot(iphone, "continue-caught-up", .continuing, caughtUp, families: states)
        var stale = full
        stale.generatedAt = at(day: 18, 20, 0)
        shoot(iphone, "continue-stale", .continuing, stale, families: states)
        // Wednesday, with nothing that day.
        var quiet = full
        quiet.generatedAt = at(day: 21, 9, 0)
        quiet.airingToday = []
        quiet.nextDays = [SnapshotAiring(id: 900001, title: "Hoshikuzu no Tabibito", episode: 10,
                                     airsAt: at(day: 22, 16, 30), onList: true)]
        quiet.nextAiring = SnapshotNextAiring(id: 900001, title: "Hoshikuzu no Tabibito", episode: 10, airsAt: at(day: 22, 16, 30))
        shoot(iphone, "airing-nothing-today", .airing, quiet, families: states, at: at(day: 21, 10, 0))
        var old = full
        old.generatedAt = at(day: 12, 19, 0)
        shoot(iphone, "airing-stale", .airing, old, families: states)
        shoot(iphone, "airing-signed-out", .airing, signedOut, families: states)
        // No snapshot: AniList's public schedule, which has the times and no marks.
        let publicRows = full.airingToday.map {
            SnapshotAiring(id: $0.id, title: $0.title, episode: $0.episode, airsAt: $0.airsAt)
        }
        shoot(iphone, "airing-no-snapshot", .airing, nil, families: states, publicSchedule: publicRows)

        liveActivity(files: files, out: out)
    }

    // MARK: The downloads' Live Activity (NativeLiveActivity)

    static func liveActivity(files: WidgetFiles, out: URL) {
        // The cover, as the app keeps it for the activity.
        let poster = art(900001, in: files).poster
        if let poster {
            try? FileManager.default.createDirectory(at: files.liveDirectory, withIntermediateDirectories: true)
            try? FileManager.default.copyItem(at: files.art(poster), to: files.liveDirectory.appending(path: poster))
        }
        let one = DownloadsActivityState(
            titleID: 900001, title: "Hoshikuzu no Tabibito", episode: "8", coverFile: poster,
            bytesDone: 412_000_000, bytesTotal: 1_100_000_000, speed: 2_400_000, secondsLeft: 290)
        var several = one
        several.count = 3
        several.bytesDone = 400_000_000
        several.bytesTotal = 3_100_000_000
        several.bytesLeft = 2_700_000_000
        var paused = one
        paused.paused = true
        paused.speed = nil
        paused.secondsLeft = nil
        var finished = one
        finished.finished = true
        finished.downloadID = 7
        finished.bytesDone = finished.bytesTotal

        func draw(_ view: some View, _ name: String, width: CGFloat, height: CGFloat?, radius: CGFloat, background: Color) {
            let content = view
                .environment(\.widgetFiles, files)
                .environment(\.widgetDrawsTaps, false)
                .environment(\.widgetRunsTimers, false)
                .environment(\.locale, Locale(identifier: "en_GB"))
                .environment(\.colorScheme, .dark)
                .frame(width: width, height: height)
                .background(background)
                .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
                .padding(16)
                .background(Color(red: 13 / 255, green: 16 / 255, blue: 32 / 255))
            let renderer = ImageRenderer(content: content)
            renderer.scale = 3
            guard let image = renderer.cgImage else { return }
            let url = out.appending(path: "iphone-live-\(name).png")
            try? NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])?.write(to: url)
            print(url.path)
        }

        let card = Palette.ground.opacity(0.92)
        draw(DownloadsLockScreenView(state: one), "one", width: 358, height: nil, radius: 24, background: card)
        draw(DownloadsLockScreenView(state: several), "several", width: 358, height: nil, radius: 24, background: card)
        draw(DownloadsLockScreenView(state: paused), "paused", width: 358, height: nil, radius: 24, background: card)
        draw(DownloadsLockScreenView(state: finished), "finished", width: 358, height: nil, radius: 24, background: card)
        draw(DownloadsLockScreenView(state: one, stale: true), "stale", width: 358, height: nil, radius: 24, background: card)

        // The island, laid out as the system lays out its regions.
        let compact = HStack {
            DownloadsIslandCover(state: one)
            Spacer()
            DownloadRing(fraction: one.fraction)
        }
        .padding(.horizontal, 8)
        draw(compact, "island-compact", width: 250, height: 37, radius: 18.5, background: .black)
        draw(DownloadRing(fraction: one.fraction), "island-minimal", width: 37, height: 37, radius: 18.5, background: .black)
        let expanded = VStack(spacing: 10) {
            HStack(alignment: .top, spacing: 12) {
                DownloadsIslandCoverLarge(state: one)
                DownloadsIslandTitle(state: one).padding(.top, 4)
                DownloadsIslandTimeLeft(state: one).padding(.top, 6)
            }
            DownloadsIslandBottom(state: one)
        }
        .padding(EdgeInsets(top: 18, leading: 20, bottom: 14, trailing: 16))
        draw(expanded, "island-expanded", width: 371, height: 160, radius: 44, background: .black)

        // MARK: The playback and airing Live Activities

        // Their dates are the drawing's moment, which is when the countdowns are read.
        let moment = Date()
        let airingPoster = art(900002, in: files).poster
        if let airingPoster {
            try? FileManager.default.copyItem(at: files.art(airingPoster), to: files.liveDirectory.appending(path: airingPoster))
        }
        let playing = PlaybackActivityState(
            titleID: 900001, title: "Hoshikuzu no Tabibito", episode: "7", coverFile: poster,
            position: 600, duration: 1_440, at: moment)
        var pausedPlay = playing
        pausedPlay.paused = true
        draw(PlaybackLockScreenView(state: playing), "playback", width: 358, height: nil, radius: 24, background: card)
        draw(PlaybackLockScreenView(state: pausedPlay), "playback-paused", width: 358, height: nil, radius: 24, background: card)
        draw(PlaybackLockScreenView(state: playing, stale: true), "playback-stale", width: 358, height: nil, radius: 24, background: card)
        let playCompact = HStack {
            LiveActivityCover(file: playing.coverFile)
            Spacer()
            PlaybackIslandTimeLeft(state: playing, compact: true)
        }
        .padding(.horizontal, 8)
        draw(playCompact, "playback-island-compact", width: 250, height: 37, radius: 18.5, background: .black)
        let playExpanded = VStack(spacing: 10) {
            HStack(alignment: .top, spacing: 12) {
                LiveActivityCoverLarge(file: playing.coverFile)
                PlaybackIslandTitle(state: playing).padding(.top, 4)
                PlaybackIslandTimeLeft(state: playing).padding(.top, 6)
            }
            PlaybackIslandBottom(state: playing)
        }
        .padding(EdgeInsets(top: 18, leading: 20, bottom: 14, trailing: 16))
        draw(playExpanded, "playback-island-expanded", width: 371, height: 160, radius: 44, background: .black)

        let airing = AiringActivityState(
            titleID: 900002, title: "Kumo no Ue no Toshokan", episode: 6, airsAt: moment.addingTimeInterval(12 * 60 + 4),
            since: moment.addingTimeInterval(-20 * 60), coverFile: airingPoster)
        draw(AiringLockScreenView(state: airing, out: false), "airing", width: 358, height: nil, radius: 24, background: card)
        draw(AiringLockScreenView(state: airing, out: true), "airing-out", width: 358, height: nil, radius: 24, background: card)
        let airCompact = HStack {
            LiveActivityCover(file: airing.coverFile)
            Spacer()
            AiringIslandCountdown(state: airing, out: false, compact: true)
        }
        .padding(.horizontal, 8)
        draw(airCompact, "airing-island-compact", width: 250, height: 37, radius: 18.5, background: .black)
        let airExpanded = HStack(alignment: .top, spacing: 12) {
            LiveActivityCoverLarge(file: airing.coverFile)
            AiringIslandTitle(state: airing, out: false).padding(.top, 4)
            AiringIslandCountdown(state: airing, out: false).padding(.top, 6)
        }
        .padding(EdgeInsets(top: 18, leading: 20, bottom: 18, trailing: 16))
        draw(airExpanded, "airing-island-expanded", width: 371, height: 96, radius: 44, background: .black)
    }
}

/// The expanded island's leading cover for the playback and airing activities.
struct LiveActivityCoverLarge: View {
    let file: String?
    @Environment(\.widgetFiles) private var files

    var body: some View {
        if let file, let files, let image = ArtThumbnail.image(at: files.liveDirectory.appending(path: file)) {
            Image(decorative: image, scale: 1)
                .resizable()
                .aspectRatio(contentMode: .fill)
                .frame(width: 36, height: 54)
                .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        }
    }
}

/// The expanded island's leading cover, as the extension draws it.
struct DownloadsIslandCoverLarge: View {
    let state: DownloadsActivityState
    @Environment(\.widgetFiles) private var files

    var body: some View {
        if let file = state.coverFile, let files,
           let image = ArtThumbnail.image(at: files.liveDirectory.appending(path: file)) {
            Image(decorative: image, scale: 1)
                .resizable()
                .aspectRatio(contentMode: .fill)
                .frame(width: 36, height: 54)
                .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        }
    }
}

let arguments = CommandLine.arguments.dropFirst()
let out = URL(filePath: arguments.first ?? FileManager.default.currentDirectoryPath, directoryHint: .isDirectory)
try MainActor.assumeIsolated { try Shots.run(out: out) }
#else
print("WidgetShots draws on the Mac.")
#endif
