// The client's life: what it starts mpv with, its calls, its streams, and its end.

import AnistreamPlayer
import Foundation
import Testing

extension PlayerTests {
    @Suite struct ClientTests {
        @Test func startsMpvOnTheLayerWithTheRenderPath() async throws {
            let (client, layer) = try await startedClient()
            func names(_ property: String) async throws -> [String] {
                try await client.get(property).array.compactMap { $0["name"].string ?? $0.string }
            }
            #expect(try await names("vo") == ["gpu-next"])
            #expect(try await names("gpu-api") == ["vulkan"])
            #expect(try await names("gpu-context") == ["moltenvk"])
            #expect(try await names("hwdec") == ["videotoolbox"])
            #expect(try await client.get("target-colorspace-hint") == true)
            #expect(try await client.get("wid").int == layer.wid)
            await client.shutdown()
        }

        @Test func refusesEveryCallOnceShutDown() async throws {
            let (client, layer) = try await startedClient()
            await client.shutdown()
            await #expect(throws: MpvError.terminated) { try await client.get("pause") }
            await #expect(throws: MpvError.terminated) { try await client.apply(.stop) }
            await #expect(throws: MpvError.terminated) { try await client.start(on: layer) }
            await client.shutdown()  // a second time does nothing
        }

        @Test func refusesCallsBeforeItStarts() async throws {
            let client = MpvClient()
            await #expect(throws: MpvError.notStarted) { try await client.get("pause") }
            await client.shutdown()
        }

        @Test func startsOnlyOnce() async throws {
            let (client, layer) = try await startedClient()
            await #expect(throws: MpvError.alreadyStarted) { try await client.start(on: layer) }
            await client.shutdown()
        }

        @Test func endsItsStreamsWhenItShutsDown() async throws {
            let (client, _) = try await startedClient()
            let events = client.events()
            let reports = client.reports()
            await client.shutdown()
            var sawShutdown = false
            for await event in events where event == .shutdown { sawShutdown = true }
            #expect(sawShutdown)
            for await _ in reports {}  // ends, or this test hangs
            var afterwards = 0
            for await _ in client.events() { afterwards += 1 }
            #expect(afterwards == 0)
        }

        @Test func saysWhatMpvRefused() async throws {
            let (client, _) = try await startedClient()
            await #expect(throws: MpvError.mpv(code: -8, call: "get no-such-property")) {
                try await client.get("no-such-property")
            }
            await client.shutdown()
        }

        @Test func roundTripsNestedValuesThroughNodes() async throws {
            let (client, _) = try await startedClient()
            let value: MpvValue = [
                "title": "Episode 3", "chapters": [1, 2.5, "three"], "skip": true, "nothing": .none,
            ]
            try await client.set("user-data/anistream/test", value)
            #expect(try await client.get("user-data/anistream/test") == value)
            await client.shutdown()
        }

        @Test func reportsObservedPropertiesAsJSON() async throws {
            let (client, _) = try await startedClient()
            let pause = client.reports()
            let state = client.reports()
            try await client.observe(["pause", "user-data/anistream/state"])
            try await client.set("pause", true)
            try await client.set("user-data/anistream/state", ["episode": 3, "title": "Ep 3"])
            #expect(await first(pause) { $0 == .property(name: "pause", json: "true") } != nil)
            let expected = MpvReport.property(
                name: "user-data/anistream/state", json: #"{"episode":3,"title":"Ep 3"}"#)
            #expect(await first(state) { $0 == expected } != nil)
            await client.shutdown()
        }

        @Test func carriesMpvsLogLines() async throws {
            let (client, _) = try await startedClient { $0.logLevel = "error" }
            let recorder = LogRecorder(client)
            let events = client.events()
            try await client.load("/no/such/file.mkv")
            let ended = await first(events) { if case .endFile = $0 { true } else { false } }
            #expect(ended == .endFile(MpvEnd(reason: "error", error: "loading failed")))
            await client.shutdown()
            let lines = await recorder.lines()
            #expect(lines.contains { $0.level == "error" && $0.text.contains("/no/such/file.mkv") })
        }
    }
}
