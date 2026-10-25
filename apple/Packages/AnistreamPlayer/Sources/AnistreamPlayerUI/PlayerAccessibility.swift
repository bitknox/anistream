// The player with VoiceOver (ux_part3, Accessibility): the controls stay up while it runs, the
// magic tap plays or pauses (iOS has one; on the Mac Space does), the escape gesture closes, and
// Up Next and a skip offer are said once each, never every second.

import Accessibility
import AnistreamDesign
import SwiftUI

extension View {
    /// The player's VoiceOver behaviour, over either player view.
    func playerAccessibility(_ model: PlayerModel) -> some View {
        modifier(PlayerAccessibility(model: model))
    }
}

struct PlayerAccessibility: ViewModifier {
    @Environment(\.accessibilityVoiceOverEnabled) private var voiceOver
    let model: PlayerModel

    func body(content: Content) -> some View {
        content
            // Controls, cards and labels no larger than `.xxxLarge`, so they never cover the
            // picture (ux_part3, Dynamic Type).
            .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
            .onChange(of: voiceOver, initial: true) { _, on in model.hold("voiceover", on) }
            #if os(iOS)
            .accessibilityAction(.magicTap) { model.togglePause() }
            #endif
            .accessibilityAction(.escape) { model.viewerClosed() }
            .onChange(of: model.upNextShown) { _, shown in
                guard shown, let card = model.upNext else { return }
                let episode = "Up next: episode \(card.next.number)"
                    + (model.episode.map { ", \($0.translation.label.lowercased())" } ?? "")
                announce(card.next.title.map { "\(episode), \($0)" } ?? episode)
            }
            .onChange(of: model.countdown?.seconds) { _, seconds in
                if seconds == 8 { announce("Plays in 8 seconds") }
            }
            .onChange(of: model.skip?.kind) { _, kind in
                if kind != nil, let skip = model.skip { announce(skip.label) }
            }
    }

    private func announce(_ words: String) {
        AccessibilityNotification.Announcement(words).post()
    }
}

extension View {
    /// What Voice Control takes for an icon-only control: its label, and the shorter word one
    /// says for it ("Subtitles", "Back").
    func voiceControlLabels(_ label: String) -> some View {
        let short: [String: String] = [
            "Subtitles and audio": "Subtitles",
            "Back 10 seconds": "Back",
            "Forward 10 seconds": "Forward",
        ]
        return accessibilityInputLabels(([label] + [short[label]].compactMap { $0 }).map { Text($0) })
    }
}

/// A scrim under the controls: these stops top to bottom, or the ground at 88% under Reduce
/// Transparency, the companion's panel opacity (ux_part3).
struct Scrim: View {
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    let stops: [Gradient.Stop]

    var body: some View {
        if reduceTransparency {
            Palette.ground.opacity(0.88)
        } else {
            LinearGradient(stops: stops, startPoint: .top, endPoint: .bottom)
        }
    }
}

extension PlayerClock {
    /// A time in words, as VoiceOver reads the seek line: "12 minutes 4 seconds", "1 hour 2
    /// minutes".
    public static func spoken(_ seconds: Double) -> String {
        let total = Int(max(0, seconds.isFinite ? seconds : 0).rounded(.down))
        let parts = [(total / 3600, "hour"), (total / 60 % 60, "minute"), (total % 60, "second")]
            .filter { $0.0 > 0 }
            .map { "\($0.0) \($0.1)\($0.0 == 1 ? "" : "s")" }
        return parts.isEmpty ? "0 seconds" : parts.joined(separator: " ")
    }
}
