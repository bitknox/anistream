import CoreGraphics
import Foundation
import ImageIO
import Testing
@testable import AnistreamWidgetData

@Suite("AniList's public schedule")
struct PublicScheduleTests {
    @Test("An answer reads as broadcasts: no adult titles, English titles where AniList has them")
    func decode() throws {
        let (rows, more) = try PublicSchedule.decode(try fixture("anilist-airing"))
        #expect(more)
        #expect(rows.map(\.id) == [900006, 900002, 900003])
        #expect(rows.map(\.title) == ["Summer Wind Relay", "Kumo no Ue no Toshokan", "Tsukiakari no Teishajo"])
        #expect(rows[0].airsAt == at(day: 20, 21))
        #expect(rows.allSatisfy { !$0.onList })
        #expect(rows[1].art.color == "#5d9be4")
    }

    @Test("An error answer is no schedule")
    func error() {
        #expect(throws: PublicSchedule.ScheduleError.noData) {
            try PublicSchedule.decode(Data(#"{"errors": [{"message": "Too Many Requests."}], "data": null}"#.utf8))
        }
    }

    @Test("The request asks for the day, AniList's bounds being exclusive")
    func request() throws {
        let request = PublicSchedule.request(from: at(day: 20, 0), to: at(day: 21, 0), page: 2)
        #expect(request.httpMethod == "POST")
        let body = try #require(request.httpBody.flatMap { try JSONSerialization.jsonObject(with: $0) as? [String: Any] })
        let variables = try #require(body["variables"] as? [String: Int])
        #expect(variables["from"] == Int(at(day: 20, 0).timeIntervalSince1970) - 1)
        #expect(variables["to"] == Int(at(day: 21, 0).timeIntervalSince1970))
        #expect(variables["page"] == 2)
    }

    @Test("It is asked once per lifetime, and the copy kept stands in when AniList cannot be reached")
    func cached() async throws {
        let files = WidgetFiles(directory: FileManager.default.temporaryDirectory
            .appending(path: "widget-public-\(UUID().uuidString)", directoryHint: .isDirectory))
        defer { try? FileManager.default.removeItem(at: files.directory) }
        let row = SnapshotAiring(id: 900006, title: "Natsukaze Relay", episode: 9, airsAt: at(day: 20, 21))
        let asked = Counter()
        let fetch: (Date) async throws -> [SnapshotAiring] = { _ in
            await asked.add()
            return [row]
        }
        #expect(await files.publicSchedule(at: at(day: 20, 19), calendar: copenhagen, fetch: fetch) == [row])
        #expect(await files.publicSchedule(at: at(day: 20, 20), calendar: copenhagen, fetch: fetch) == [row])
        #expect(await asked.count == 1)
        // Past its lifetime it is asked again; failing, the kept copy of the same day stands.
        let failing: (Date) async throws -> [SnapshotAiring] = { _ in throw PublicSchedule.ScheduleError.status(429) }
        #expect(await files.publicSchedule(at: at(day: 20, 23), calendar: copenhagen, fetch: failing) == [row])
        // The next day it does not.
        #expect(await files.publicSchedule(at: at(day: 21, 9), calendar: copenhagen, fetch: failing) == nil)
    }
}

actor Counter {
    private(set) var count = 0
    func add() { count += 1 }
}

@Suite("Art thumbnails")
struct ArtThumbnailTests {
    func image(width: Int, height: Int) throws -> CGImage {
        let context = try #require(CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
        context.setFillColor(red: 0.9, green: 0.6, blue: 0.3, alpha: 1)
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        return try #require(context.makeImage())
    }

    func size(_ data: Data) throws -> (Int, Int) {
        let source = try #require(CGImageSourceCreateWithData(data as CFData, nil))
        let image = try #require(CGImageSourceCreateImageAtIndex(source, 0, nil))
        return (image.width, image.height)
    }

    @Test("A banner is scaled to just cover 680 × 240, uncropped")
    func banner() throws {
        let data = try #require(ArtThumbnail.jpeg(from: try image(width: 1900, height: 400), kind: .wide))
        #expect(try size(data) == (1140, 240))
        let still = try #require(ArtThumbnail.jpeg(from: try image(width: 1280, height: 720), kind: .wide))
        #expect(try size(still) == (680, 383))
    }

    @Test("A cover is scaled to cover 96 × 144, and nothing is scaled up")
    func poster() throws {
        let data = try #require(ArtThumbnail.jpeg(from: try image(width: 460, height: 650), kind: .poster))
        #expect(try size(data) == (102, 144))
        let small = try #require(ArtThumbnail.jpeg(from: try image(width: 50, height: 75), kind: .poster))
        #expect(try size(small) == (50, 75))
    }
}
