// The watched episodes folded away at the top of a title's episode list: the list starts at the
// last one watched, under a quiet row, "Episodes 1–5 watched", that opens them in place and then
// reads "Hide watched episodes". Opening or closing them leaves the rows on screen where they
// were: the page scrolls by exactly what came or went above them.

import AnistreamDesign
import AnistreamKit
import SwiftUI

/// The watched run a list folds away: every episode of the watched run it starts with but the
/// last, when that is two episodes or more. Nothing folds when nothing is watched, or when only
/// the first episode would go.
struct WatchedFold: Equatable {
    /// How many rows at the top of the list fold away.
    let count: Int
    /// The first and the last of them, by their numbers.
    let first: String
    let last: String

    init?(_ rows: [EpisodeRow]) {
        let run = rows.prefix { $0.completed }.count
        guard run - 1 >= 2 else { return nil }
        count = run - 1
        first = rows[0].number
        last = rows[count - 1].number
    }

    /// "Episodes 1–5 watched".
    var label: String {
        "Episodes \(first)–\(last) watched"
    }

    static let hide = "Hide watched episodes"
}

extension Navigator {
    /// Whether `title`'s watched episodes are open, for as long as the app runs.
    func watchedShown(_ title: AnilistId) -> Bool {
        watchedOpen.contains(title)
    }

    func toggleWatched(_ title: AnilistId) {
        if !watchedOpen.insert(title).inserted { watchedOpen.remove(title) }
    }
}

/// The row that opens and closes the fold: plain text and a chevron, in the list's own rhythm,
/// starting where the rows' stills do.
struct WatchedFoldRow: View {
    let fold: WatchedFold
    let shown: Bool
    let phone: Bool
    let toggle: () -> Void

    var body: some View {
        Button(action: toggle) {
            HStack(spacing: 6) {
                Text(shown ? WatchedFold.hide : fold.label)
                    .monospacedDigit()
                Image(systemName: shown ? "chevron.up" : "chevron.down")
                    .font(phone ? .ui(12, weight: .semibold) : .mac(10, weight: .semibold))
                Spacer(minLength: 0)
            }
            .font(phone ? .ui(15) : .mac(13))
            .foregroundStyle(.haze)
            // Where the stills start: the number's column and the gap after it.
            .padding(.leading, phone ? 34 : 44)
            .frame(minHeight: phone ? 44 : 36)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityHint(shown ? "Folds the watched episodes away" : "Shows the watched episodes")
    }
}

/// The folded rows and the row under them. As the rows open or close, the page scrolls by the
/// height they gain or lose, so what is below them stays where it was on screen.
struct WatchedFoldSection<Rows: View>: View {
    @Environment(\.scrollNudge) private var nudge
    let fold: WatchedFold
    let shown: Bool
    let phone: Bool
    let toggle: () -> Void
    @ViewBuilder let rows: () -> Rows
    /// Set by a tap, so only the change it makes moves the page: not the rows' own changes, nor
    /// the page opening with them open.
    @State private var armed = false

    var body: some View {
        VStack(spacing: 0) {
            if shown { rows() }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { old, new in
            guard armed else { return }
            armed = false
            nudge?.by(new - old)
        }
        WatchedFoldRow(fold: fold, shown: shown, phone: phone) {
            armed = true
            toggle()
        }
    }
}

extension View {
    /// A scroll view the rows inside it can move, by `ScrollNudge`.
    func nudgeableScroll() -> some View {
        modifier(NudgeableScroll())
    }
}

/// Moves a page's scroll view by a distance, without animation.
struct ScrollNudge {
    let by: @MainActor (CGFloat) -> Void
}

extension EnvironmentValues {
    @Entry var scrollNudge: ScrollNudge?
}

private struct NudgeableScroll: ViewModifier {
    /// Where the scroll view stands: kept out of the view's state, so scrolling redraws nothing.
    @MainActor private final class Geometry {
        /// How far down its content it is, from the content's top.
        var offset: CGFloat = 0
        var content: CGFloat = 0
        /// Its own height, less the room.
        var container: CGFloat = 0
    }

    @State private var position = ScrollPosition()
    @State private var geometry = Geometry()
    /// Room under the content, so a page too short to scroll by what opened above can still keep
    /// the rows below it in place. Gone again once the rows close.
    @State private var room: CGFloat = 0

    func body(content: Content) -> some View {
        content
            .environment(\.scrollNudge, ScrollNudge { [geometry, $position, $room] distance in
                // The content has its new height by now. The scroll view goes no further than
                // its content's height less its own, and the room is taken from its own.
                let target = max(0, geometry.offset + distance)
                let base = geometry.container + $room.wrappedValue
                var quiet = Transaction()
                quiet.disablesAnimations = true
                withTransaction(quiet) {
                    $room.wrappedValue = target > 0 ? max(0, target + base - geometry.content) : 0
                    $position.wrappedValue.scrollTo(y: target)
                }
            })
            .contentMargins(.bottom, room, for: .scrollContent)
            .scrollPosition($position)
            .onScrollGeometryChange(for: [CGFloat].self) { scroll in
                [scroll.contentOffset.y + scroll.contentInsets.top, scroll.contentSize.height,
                 scroll.containerSize.height]
            } action: { [geometry] _, now in
                geometry.offset = now[0]
                geometry.content = now[1]
                geometry.container = now[2]
            }
    }
}
