// Subtitles lift above the player's controls with `sub-pos`, which libass applies to the ASS
// dialogue at the bottom (its line position) and not to signs placed where they belong.

import AnistreamPlayer
import CoreGraphics
import Foundation
import Testing

extension PlayerTests {
    @Suite struct SubtitleLiftTests {
        /// The lowest row of the bottom half with a run of the subtitle's pure white fill.
        static func subtitleBottom(_ image: CGImage) -> Int? {
            guard let data = image.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return nil }
            for y in stride(from: image.height - 1, through: image.height / 2, by: -1) {
                var white = 0
                for x in 0..<image.width {
                    let p = bytes + y * image.bytesPerRow + x * 4
                    if p[0] > 245 && p[1] > 245 && p[2] > 245 { white += 1 }
                }
                if white >= 6 { return y }
            }
            return nil
        }

        @Test func subPosLiftsASSDialogue() async throws {
            let (client, _) = try await startedClient()
            try await play(client, "episode.mkv", options: ["start": "31", "pause": "yes"])
            try await Task.sleep(for: .milliseconds(500))
            let resting = try #require(Self.subtitleBottom(try await client.screenshot()))
            try await client.set("sub-pos", .double(85))
            try await Task.sleep(for: .milliseconds(300))
            let lifted = try #require(Self.subtitleBottom(try await client.screenshot()))
            await client.shutdown()
            #expect(resting - lifted > 80, "lifted by \(resting - lifted) px of 720")
        }
    }
}
