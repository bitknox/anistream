import Foundation
import Testing
@testable import AnistreamWidgetData

/// A snapshot file from the test bundle.
func fixture(_ name: String) throws -> Data {
    let url = try #require(Bundle.module.url(forResource: name, withExtension: "json", subdirectory: "Snapshots"))
    return try Data(contentsOf: url)
}

/// The boards' clock: Copenhagen, where 20 October 2026 is a Tuesday.
let copenhagen: Calendar = {
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = TimeZone(identifier: "Europe/Copenhagen")!
    calendar.locale = Locale(identifier: "en_GB")
    return calendar
}()

func at(day: Int, _ hour: Int, _ minute: Int = 0) -> Date {
    copenhagen.date(from: DateComponents(year: 2026, month: 10, day: day, hour: hour, minute: minute))!
}

@Suite("Reading the snapshot")
struct SnapshotTests {
    @Test("A version 1 file reads, with the fields version 2 added left unknown")
    func versionOne() throws {
        guard case .snapshot(let snapshot) = WidgetSnapshot.read(try fixture("v1")) else {
            Issue.record("did not read")
            return
        }
        #expect(snapshot.version == 1)
        #expect(snapshot.generatedAt == at(day: 20, 19, 10))
        #expect(snapshot.signedIn == nil)
        #expect(snapshot.nextDays.isEmpty)
        #expect(snapshot.nextAiring == nil)
        let first = try #require(snapshot.continueWatching.first)
        #expect(first.resume == nil)
        // Version 1 says resume by having a fraction.
        #expect(first.resumes)
        #expect(first.progress == 0.42)
        #expect(ContinuePresentation.line(first) == "Resume E7")
        #expect(!snapshot.continueWatching[1].resumes)
        #expect(ContinuePresentation.line(snapshot.continueWatching[1]) == "Play E4")
        #expect(snapshot.airingToday.first?.watched == false)
        #expect(snapshot.airingToday.first?.translation == nil)
    }

    @Test("A version 2 file reads every field")
    func versionTwo() throws {
        guard case .snapshot(let snapshot) = WidgetSnapshot.read(try fixture("v2")) else {
            Issue.record("did not read")
            return
        }
        #expect(snapshot.version == 2)
        #expect(snapshot.signedIn == true)
        #expect(snapshot.tracker == "anilist")
        let first = try #require(snapshot.continueWatching.first)
        #expect(first.episodeTitle == "The Lantern Road")
        #expect(first.minutesLeft == 14)
        #expect(first.art == Art(wide: "wide-1.jpg", poster: "poster-1.jpg", color: "#e4a15d"))
        #expect(ContinuePresentation.line(first) == "E7 · 14 min left")
        #expect(ContinuePresentation.episodeLine(first) == "Episode 7 · The Lantern Road")
        #expect(snapshot.continueWatching[1].nextAiring == at(day: 22, 16, 30))
        #expect(snapshot.continueWatching[1].nextAiringEpisode == 6)
        // The engine's mark: the second is watched dubbed, the first subbed and unmarked.
        #expect(first.mark == nil)
        #expect(snapshot.continueWatching[1].mark == "Dub")
        #expect(ContinuePresentation.line(snapshot.continueWatching[1]) == "Play E4 · Dub")
        #expect(snapshot.airingToday[0].translation == .dub)
        // A translation this build does not know is none, and the rest of the file still reads.
        #expect(snapshot.airingToday[1].translation == nil)
        #expect(snapshot.nextDays.count == 1)
        #expect(snapshot.nextAiring?.airsAt == at(day: 20, 21, 0))
        #expect(snapshot.artFiles == ["wide-1.jpg", "poster-1.jpg", "poster-6.jpg"])
    }

    @Test("A newer version is refused, and so is a file that is no snapshot")
    func refused() throws {
        #expect(WidgetSnapshot.read(try fixture("v3")) == .newer(3))
        #expect(WidgetSnapshot.read(Data("{}".utf8)) == .unreadable)
        #expect(WidgetSnapshot.read(Data("not json".utf8)) == .unreadable)
        #expect(WidgetSnapshot.read(Data(#"{"version": 0, "generated_at": 1}"#.utf8)) == .unreadable)
    }

    @Test("What is written reads back the same, with the engine's field names and whole seconds")
    func roundTrip() throws {
        guard case .snapshot(let snapshot) = WidgetSnapshot.read(try fixture("v2")) else {
            Issue.record("did not read")
            return
        }
        let data = try snapshot.encoded()
        #expect(WidgetSnapshot.read(data) == .snapshot(snapshot))
        let text = try #require(String(data: data, encoding: .utf8))
        #expect(text.contains(#""generated_at":1792516200"#))
        #expect(text.contains(#""continue_watching""#))
        #expect(text.contains(#""minutes_left":14"#))
        #expect(text.contains(#""mark":"Dub""#))
    }

    @Test("The schedule is today's and the next days', in the order they air")
    func schedule() throws {
        guard case .snapshot(let snapshot) = WidgetSnapshot.read(try fixture("v2")) else {
            Issue.record("did not read")
            return
        }
        #expect(snapshot.schedule.map(\.id) == [900006, 900003, 900002])
    }
}

@Suite("The widgets' files")
struct WidgetFilesTests {
    func temporary() -> WidgetFiles {
        WidgetFiles(directory: FileManager.default.temporaryDirectory
            .appending(path: "widget-files-\(UUID().uuidString)", directoryHint: .isDirectory))
    }

    @Test("A snapshot written is read back, and the art it no longer names is deleted")
    func writeAndRead() throws {
        let files = temporary()
        defer { try? FileManager.default.removeItem(at: files.directory) }
        #expect(files.read() == nil)
        try files.writeArt(Data([1]), named: "wide-1.jpg")
        try files.writeArt(Data([2]), named: "old.jpg")
        let snapshot = WidgetSnapshot(
            generatedAt: at(day: 20, 19, 10), signedIn: true,
            continueWatching: [SnapshotContinue(id: 900001, title: "Hoshikuzu no Tabibito", episode: 7,
                                            art: Art(wide: "wide-1.jpg"))])
        try files.write(snapshot)
        #expect(files.snapshot() == snapshot)
        #expect(files.hasArt("wide-1.jpg"))
        #expect(!files.hasArt("old.jpg"))
    }

    @Test("The App Group comes from the bundle's Info.plist, and an unexpanded or teamless one is none")
    func group() {
        #expect(WidgetFiles.group(in: .main) == nil)
        #expect(WidgetFiles.shared() == nil)
    }

    @Test("Sideloaded, the group is the one the store registered under its team")
    func sideloadedGroup() {
        let group = "group.tv.anistream"
        #expect(WidgetFiles.sideloadedGroup(for: group, among: ["group.tv.anistream.ABCDE12345"])
            == "group.tv.anistream.ABCDE12345")
        #expect(WidgetFiles.sideloadedGroup(
            for: group, among: ["group.com.rileytestut.AltStore.ABCDE12345", "group.tv.anistream.ABCDE12345"])
            == "group.tv.anistream.ABCDE12345")
        // The group itself, another app's, and one only beginning like it are none.
        #expect(WidgetFiles.sideloadedGroup(for: group, among: ["group.tv.anistream"]) == nil)
        #expect(WidgetFiles.sideloadedGroup(for: group, among: ["group.tv.anistreamer.ABCDE12345"]) == nil)
        #expect(WidgetFiles.sideloadedGroup(for: group, among: ["group.tv.anistream.app.ABCDE12345"]) == nil)
        #expect(WidgetFiles.sideloadedGroup(for: group, among: []) == nil)
    }

    @Test("A provisioning profile's App Groups are read from the plist inside its signature")
    func provisionedGroups() throws {
        let plist = try PropertyListSerialization.data(
            fromPropertyList: [
                "Name": "AltStore tv.anistream.app.ABCDE12345",
                "Entitlements": [
                    "application-identifier": "ABCDE12345.tv.anistream.app.ABCDE12345",
                    "com.apple.security.application-groups": ["group.tv.anistream.ABCDE12345"],
                ],
            ],
            format: .xml, options: 0)
        // The CMS signature's bytes around it, as a profile has them.
        let profile = Data([0x30, 0x82, 0x3c, 0x00, 0x06, 0x09]) + plist + Data([0xa0, 0x82, 0x0b, 0x00])
        #expect(WidgetFiles.provisionedGroups(profile) == ["group.tv.anistream.ABCDE12345"])
        #expect(WidgetFiles.provisionedGroups(Data()) == [])
        #expect(WidgetFiles.provisionedGroups(Data("<?xml no plist".utf8)) == [])
    }

    @Test("Art is named by its address, so the same address is the same file")
    func artNames() {
        let one = WidgetFiles.artName(for: "https://img.example/banner/900001.jpg", kind: .wide)
        #expect(one == WidgetFiles.artName(for: "https://img.example/banner/900001.jpg", kind: .wide))
        #expect(one != WidgetFiles.artName(for: "https://img.example/banner/900002.jpg", kind: .wide))
        #expect(one.hasPrefix("wide-") && one.hasSuffix(".jpg"))
        #expect(WidgetFiles.artName(for: "https://img.example/banner/900001.jpg", kind: .poster).hasPrefix("poster-"))
    }
}

@Suite("Links into the app")
struct LinkTests {
    @Test("Each link reads back from its URL")
    func roundTrip() {
        for link in [WidgetLink.title(900001), .play(900001, episode: 7), .calendar, .accounts] {
            #expect(WidgetLink(url: link.url) == link)
        }
        #expect(WidgetLink.play(900001, episode: 7).url.absoluteString == "anistream://play/900001/7")
        #expect(WidgetLink.title(900001).url.absoluteString == "anistream://title/900001")
        #expect(WidgetLink.accounts.url.absoluteString == "anistream://settings/accounts")
    }

    @Test("Other anistream links are not the widgets'")
    func others() {
        for text in [
            "anistream://auth/anilist#access_token=x", "anistream://join?link=https://a.example/join/x",
            "anistream://plugins.example/join/code#key", "anistream://title/abc", "anistream://play/900001",
            "anistream://play/900001/0", "https://anistream.tv/title/900001", "anistream://settings/playback",
        ] {
            #expect(WidgetLink(url: URL(string: text)!) == nil, "\(text)")
        }
    }
}
