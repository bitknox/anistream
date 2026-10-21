// The layer as a view would size it, the screen as a view would report it, and mpv's picture.

import AnistreamPlayer
import CoreGraphics
import Foundation
import Testing

extension PlayerTests {
    @Suite struct SurfaceTests {
        /// mpv's own idea of the surface's size.
        private func osd(_ client: MpvClient) async throws -> [Int64] {
            let osd = try await client.get("osd-dimensions")
            return [osd["w"].int ?? 0, osd["h"].int ?? 0]
        }

        @Test func followsTheLayerAsItResizes() async throws {
            let (client, layer) = try await startedClient()
            try await play(client, "testsrc.mkv")
            let first = try await eventually { () async throws -> [Int64]? in
                let now = try await osd(client)
                return now == [1280, 720] ? now : nil
            }
            #expect(first == [1280, 720])
            // A wider window, a square one, full screen on a 14" MacBook Pro: MPVKit's own moltenvk
            // context stayed at the first size (apple/patches/mpv/0004).
            for size in [[1600, 900], [900, 900], [3024, 1898]] {
                layer.resize(to: CGSize(width: size[0] / 2, height: size[1] / 2), scale: 2)
                await client.surfaceResized()
                let followed = try await eventually { () async throws -> [Int64]? in
                    let now = try await osd(client)
                    return now == size.map(Int64.init) ? now : nil
                }
                #expect(followed == size.map(Int64.init), "at \(size)")
            }
            await client.shutdown()
        }

        @Test func followsTheLayerWhilePaused() async throws {
            let (client, layer) = try await startedClient()
            try await play(client, "testsrc.mkv")
            try await client.set("pause", true)
            try await Task.sleep(for: .milliseconds(300))
            layer.resize(to: CGSize(width: 400, height: 400), scale: 2)
            // Paused, mpv's render thread sleeps; the surface's wake is what makes it look.
            await client.surfaceResized()
            let followed = try await eventually(within: .seconds(2)) { () async throws -> [Int64]? in
                let now = try await osd(client)
                return now == [800, 800] ? now : nil
            }
            #expect(followed == [800, 800])
            await client.shutdown()
        }

        @Test func picturesWhatItDrewAtTheLayersSize() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "testsrc.mkv")
            let image = try await client.screenshot()
            #expect(image.width == 1280)
            #expect(image.height == 720)
            #expect(variety(image) > 0.01, "a test pattern has more than one colour")
            let video = try await client.screenshot(.video)
            #expect(video.width == 1280 && video.height == 720)
            await client.shutdown()
        }

        @Test func passesTheScreenToMpvAndTheEngine() async throws {
            let (client, _) = try await startedClient()
            let reports = client.reports()
            let display = DisplayInfo(
                width: 3024, height: 1964, scale: 2, refreshRate: 120, edrHeadroom: 1.2, edrPotential: 16,
                name: "Built-in Retina Display")
            try await client.updateDisplay(display)
            #expect(try await client.get("display-fps-override").double == 120)
            #expect(await first(reports) { $0 == .display(display) } != nil)
            await client.shutdown()
        }
    }
}
