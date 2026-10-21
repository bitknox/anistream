// What moves when a watch moves something (motion.md): the check drawn on as an episode becomes
// watched (T9), and progress held while the player is up, to run on once it has gone (T3).

import AnistreamDesign
import SwiftUI

extension EnvironmentValues {
    /// Whether the player the app hosts is up: the app's shell says so from the player.
    @Entry public var playerUp = false
}

/// The check of a watched episode: drawn on when the episode becomes watched, at once under
/// Reduce Motion, with a light impact on iPhone (T9). It holds its place while unwatched, so
/// nothing in the row moves.
struct WatchedCheck: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let watched: Bool
    var size: CGFloat = 11

    var body: some View {
        Image(systemName: "checkmark")
            .font(.system(size: size, weight: .semibold))
            .foregroundStyle(.haze)
            .symbolEffect(.drawOn, isActive: !watched)
            .symbolEffectsRemoved(reduceMotion)
            .opacity(watched ? 1 : 0)
            #if os(iOS)
            .sensoryFeedback(.impact(weight: .light), trigger: watched) { _, now in now }
            #endif
            .accessibilityHidden(true)
    }
}

/// A value held while the player is up and let go once it has gone, so a watch's progress runs
/// on the page the viewer comes back to rather than behind the player (T3).
struct Held<Value: Equatable, Content: View>: View {
    @Environment(\.playerUp) private var playerUp
    let value: Value
    @ViewBuilder let content: (Value) -> Content
    @State private var held: Value?

    var body: some View {
        content(held ?? value)
            .onChange(of: playerUp) { _, up in
                held = up ? value : nil
            }
    }
}

/// "14 min left", its changed digits rolling as it changes (T3).
struct MinutesLeft: View {
    let minutes: Int
    var size: CGFloat = 13

    var body: some View {
        Text("\(minutes) min left")
            .font(.system(size: size))
            .monospacedDigit()
            .foregroundStyle(.haze)
            .contentTransition(.numericText(value: Double(minutes)))
            .animation(.standard, value: minutes)
    }
}
