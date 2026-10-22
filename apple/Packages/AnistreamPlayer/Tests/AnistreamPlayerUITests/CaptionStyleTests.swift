// Plain-text subtitles in the system's caption style: the style as mpv's options, and the player
// taking them as the setting and the style change. Made-up styles throughout; nothing here reads
// or changes the machine's own.

import AnistreamPlayer
import Foundation
import Testing

@testable import AnistreamPlayerUI

struct CaptionStyleTests {
    /// The style the system has until the viewer changes it: its own interface font, white text,
    /// no background, a half-black box behind the lines, no edge.
    static let systemDefault = CaptionStyle(
        font: nil, size: 1,
        text: .white,
        background: .black.with(alpha: 0),
        window: .black.with(alpha: 0.5),
        edge: .none)

    @Test func theSystemsDefaultIsWhiteOnAHalfBlackBox() {
        let options = Self.systemDefault.mpvOptions(defaultFont: "sans-serif")
        #expect(options["sub-font"] == "sans-serif")
        #expect(options["sub-font-size"] == "38.0")
        #expect(options["sub-color"] == "#FFFFFFFF")
        #expect(options["sub-border-style"] == "background-box")
        #expect(options["sub-back-color"] == "#80000000")
        #expect(options["sub-outline-size"] == "0.00")
    }

    @Test func aBackgroundBehindTheLettersIsATightBoxInItsColour() {
        let style = CaptionStyle(
            font: "Menlo", size: 1.5,
            text: CaptionStyle.Colour(red: 1, green: 1, blue: 0, alpha: 1),
            background: CaptionStyle.Colour(red: 0, green: 0, blue: 1, alpha: 0.75),
            window: .black.with(alpha: 0.5),
            edge: .uniform)
        let options = style.mpvOptions(defaultFont: "sans-serif")
        #expect(options["sub-font"] == "Menlo")
        #expect(options["sub-font-size"] == "57.0")
        #expect(options["sub-color"] == "#FFFFFF00")
        // libass draws the tight box with the outline, so the box wins over the edge.
        #expect(options["sub-border-style"] == "opaque-box")
        #expect(options["sub-outline-color"] == "#BF0000FF")
        #expect(options["sub-shadow-offset"] == "0.00")
    }

    @Test func edgesWithoutABoxAreTheOutlineOrTheShadow() {
        var style = Self.systemDefault
        style.window = .black.with(alpha: 0)
        style.edge = .uniform
        var options = style.mpvOptions(defaultFont: "sans-serif")
        #expect(options["sub-border-style"] == "outline-and-shadow")
        #expect(options["sub-outline-size"] == "1.65")
        #expect(options["sub-shadow-offset"] == "0.00")
        style.edge = .dropShadow
        options = style.mpvOptions(defaultFont: "sans-serif")
        #expect(options["sub-outline-size"] == "0.00")
        #expect(options["sub-shadow-offset"] == "2.00")
        #expect(options["sub-back-color"] == "#BF000000")
        style.edge = .depressed
        options = style.mpvOptions(defaultFont: "sans-serif")
        #expect(options["sub-back-color"] == "#99FFFFFF")
    }

    @Test func sizesAndColoursStayInRange() {
        var style = Self.systemDefault
        style.size = 4
        #expect(style.mpvOptions(defaultFont: "x")["sub-font-size"] == "76.0")
        style.size = 0.1
        #expect(style.mpvOptions(defaultFont: "x")["sub-font-size"] == "19.0")
        #expect(CaptionStyle.Colour(red: 2, green: -1, blue: 0.5, alpha: 0.25).mpv == "#40FF0080")
    }

    /// Every option the style sets, mpv's own defaults set back: turning the setting off leaves
    /// nothing of the style behind.
    @Test func turningItOffSetsBackEverythingTheStyleSet() {
        let style = Set(Self.systemDefault.mpvOptions(defaultFont: "x").keys)
        #expect(style == Set(CaptionStyle.mpvDefaults.keys).union(["sub-font"]))
    }
}

@MainActor
struct CaptionStylePlayerTests {
    /// The values the player set on mpv for `property`, in order.
    private func sets(_ rig: Rig, _ property: String) -> [String] {
        rig.mpv.calls.compactMap { call in
            if case .set(property: property, let value) = call { value.string } else { nil }
        }
    }

    @Test func theNextMpvStartsInTheStyleAndTheSettingTurnsItOff() {
        let rig = Rig()
        rig.model.systemCaptionStyle = { CaptionStyleTests.systemDefault }
        #expect(rig.model.captionOptions()["sub-border-style"] == "background-box")
        rig.model.followsSystemCaptions = false
        #expect(rig.model.captionOptions()["sub-border-style"] == "outline-and-shadow")
        #expect(rig.model.captionOptions()["sub-color"] == "#FFFFFFFF")
    }

    @Test func aChangeReachesTheMpvPlayingNow() async throws {
        let rig = Rig()
        var style = CaptionStyleTests.systemDefault
        rig.model.systemCaptionStyle = { style }
        try await rig.opened()
        // The viewer picks yellow in the system's settings, which say so.
        style.text = CaptionStyle.Colour(red: 1, green: 1, blue: 0, alpha: 1)
        rig.model.captionStyleChanged()
        await rig.handedOver()
        #expect(sets(rig, "sub-color") == ["#FFFFFF00"])
        // And turns the setting off: mpv's own look again, every option of it.
        rig.model.followsSystemCaptions = false
        await rig.handedOver()
        #expect(sets(rig, "sub-color") == ["#FFFFFF00", "#FFFFFFFF"])
        #expect(sets(rig, "sub-border-style").last == "outline-and-shadow")
        #expect(sets(rig, "sub-font").last == PlayerModel.defaultSubtitleFont)
    }

    @Test func withNothingPlayingAChangeSendsNothing() async throws {
        let rig = Rig()
        rig.model.systemCaptionStyle = { CaptionStyleTests.systemDefault }
        rig.model.captionStyleChanged()
        rig.model.followsSystemCaptions = false
        #expect(rig.made.isEmpty)
    }
}
