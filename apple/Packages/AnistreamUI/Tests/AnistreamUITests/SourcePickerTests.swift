// What the source picker's rows say.

@testable import AnistreamUI
import AnistreamKit
import Testing

struct SourcePickerTests {
    private func option(covered: UInt32 = 6, note: SourceNote? = nil) -> SourceOption {
        SourceOption(providerId: "a", name: "A", covered: covered, of: 13, pinned: false, note: note,
                     releases: [], pinnedRelease: nil)
    }

    @Test func aSourceSaysHowMuchItHasOrWhyItHasNothing() {
        #expect(SourcePickerWords.detail(option()) == "6 of 13 episodes")
        #expect(SourcePickerWords.detail(option(covered: 1)) == "1 of 13 episodes")
        #expect(SourcePickerWords.detail(option(note: .off)) == "Switched off")
        #expect(SourcePickerWords.detail(option(note: .gone)) == "Not installed")
    }

    @Test func aFailureIsTheSourcesOwnReasonAsItSaidIt() {
        #expect(SourcePickerWords.detail(option(note: .failed(reason: "timed out after 30 s"))) == "timed out after 30 s")
    }

    @Test func aReleaseSaysItsQualitySizeAndSeeders() {
        let release = SourceCandidate(id: "r", providerId: "a", title: "T", quality: 1080, seeders: 84,
                                      size: "14.2 GiB", dualAudio: false, dubbed: false, autoPick: false)
        #expect(SourcePickerWords.detail(release) == "1080p · 14.2 GiB · 84 seeders")
    }
}
