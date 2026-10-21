// The engine's ops, each carried out on a playing file, and the reports that come back.

import AnistreamPlayer
import Foundation
import Testing

extension PlayerTests {
    @Suite struct OpsTests {
        @Test func loadsAFileToItsFirstFrame() async throws {
            let (client, _) = try await startedClient()
            let reports = client.reports()
            try await play(client, "testsrc.mkv")
            #expect(try await client.get("video-params/w").int == 1280)
            #expect(try await client.get("video-params/h").int == 720)
            #expect(await first(reports) { $0 == .event("file-loaded") } != nil)
            await client.shutdown()
        }

        @Test func loadsWithOptionsForThatFileOnly() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "testsrc.mkv", options: ["start": "6", "volume": "40"])
            #expect(try await client.get("time-pos").double ?? 0 >= 6)
            #expect(try await client.get("volume").double == 40)
            try await play(client, "testsrc.mkv")
            #expect(try await client.get("time-pos").double ?? 99 < 3)
            #expect(try await client.get("volume").double == 100)
            await client.shutdown()
        }

        @Test func seeksToTheSecond() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "testsrc.mkv")
            try await client.set("pause", true)
            let events = client.events()
            try await client.apply(.seek(seconds: 7.5, mode: .absolute))
            _ = await first(events) { $0 == .playbackRestart }
            let position = try await client.get("time-pos").double ?? 0
            #expect(abs(position - 7.5) < 0.1)
            try await client.apply(.seek(seconds: -2, mode: .relative))
            let back = try await eventually { () async throws -> Double? in
                let p = try await client.get("time-pos").double ?? 0
                return abs(p - 5.5) < 0.1 ? p : nil
            }
            #expect(back != nil)
            await client.shutdown()
        }

        @Test func setsAnyProperty() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "testsrc.mkv")
            try await client.apply(.set(property: "pause", value: true))
            #expect(try await client.get("pause") == true)
            try await client.apply(.set(property: "speed", value: 1.5))
            #expect(try await client.get("speed").double == 1.5)
            await client.shutdown()
        }

        @Test func addsAnExternalSubtitleAndSelectsIt() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "testsrc.mkv")
            try await client.apply(
                .addSubtitle(url: try Media.path("extra.ass"), title: "Signs", language: "eng", select: true))
            let tracks = try await client.get("track-list").array.filter { $0["type"].string == "sub" }
            let track = try #require(tracks.first)
            #expect(track["title"].string == "Signs")
            #expect(track["lang"].string == "eng")
            #expect(track["external"].bool == true)
            #expect(track["selected"].bool == true)
            #expect(try await client.get("sid").int == track["id"].int)
            await client.shutdown()
        }

        @Test func setsTheShaderChainForTheRenderer() async throws {
            let (client, _) = try await startedClient(width: 1920, height: 1080)
            try await play(client, "testsrc.mkv")
            let chain = ["ArtCNN_C4F16.glsl", "KrigBilateral.glsl"].map {
                Media.shaders.appending(path: $0).path
            }
            try await client.apply(.shaders(chain))
            #expect(try await client.get("glsl-shaders") == .array(chain.map(MpvValue.string)))
            // 720p into 1080p is the 1.5× ArtCNN's //!WHEN asks for: its passes run.
            let passes = try await eventually { () async throws -> [String]? in
                let descs = try await client.get("vo-passes")["fresh"].array.compactMap { $0["desc"].string }
                let artcnn = descs.contains { $0.contains("ArtCNN") }
                let krig = descs.contains { $0.contains("KrigBilateral") }
                return artcnn && krig ? descs : nil
            }
            #expect(passes != nil)
            try await client.apply(.shaders([]))
            #expect(try await client.get("glsl-shaders") == .array([]))
            await client.shutdown()
        }

        @Test func setsAndClearsTheAudioFilter() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "testsrc.mkv")
            try await client.apply(.audioFilter("lavfi=[acompressor=threshold=0.1:ratio=4]"))
            let filters = try await client.get("af").array
            #expect(filters.count == 1)
            #expect(filters.first?["name"].string == "lavfi")
            try await client.apply(.audioFilter(nil))
            #expect(try await client.get("af") == .array([]))
            await client.shutdown()
        }

        @Test func stopsAndSaysSo() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "testsrc.mkv")
            let reports = client.reports()
            try await client.apply(.stop)
            let ended = await first(reports) { if case .ended = $0 { true } else { false } }
            #expect(ended == .ended(MpvEnd(reason: "stop", error: nil)))
            #expect(try await client.get("idle-active") == true)
            await client.shutdown()
        }

        @Test func reportsTheEndOfAFile() async throws {
            let (client, _) = try await startedClient()
            let reports = client.reports()
            try await play(client, "short.mkv")
            let ended = await first(reports, within: .seconds(10)) {
                if case .ended = $0 { true } else { false }
            }
            #expect(ended == .ended(MpvEnd(reason: "eof", error: nil)))
            await client.shutdown()
        }
    }
}
