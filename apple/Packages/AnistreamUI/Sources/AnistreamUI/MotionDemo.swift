// Transitions started by a launch argument, for frame sequences of the motion audit: the Fixtures
// app and the dev harness run one once the store has settled, and the shots record it.

import AnistreamKit
import AnistreamStore
import SwiftUI

/// A transition from motion.md the app can be asked to run, with `-AnistreamMotion <name>`.
public enum MotionDemo: String, CaseIterable, Sendable {
    /// T1: a cover zooms into its title's page (iPhone and iPad).
    case t1
    /// M2: Continue's selection moves to the next title, the preview crossfading.
    case m2
    /// M3: the preview's banner grows into the title page.
    case m3
    /// M3 in reverse: Back from a title page opened from the preview.
    case back
    /// T9: the title's next episode is marked watched.
    case t9
    /// The title page's next season opened from its season picker, as a click on its cell does.
    case season

    /// The one the launch arguments name.
    public static func from(arguments: [String] = ProcessInfo.processInfo.arguments) -> MotionDemo? {
        guard let index = arguments.firstIndex(of: "-AnistreamMotion"), arguments.indices.contains(index + 1)
        else { return nil }
        return MotionDemo(rawValue: arguments[index + 1])
    }

    /// Run it on the screen the launch opened.
    @MainActor
    public func run(navigator: Navigator, store: AppStore) {
        let queue = store.continuing.queue(store.titles).all.map(\.id)
        switch self {
        case .t1:
            if let id = queue.first { navigator.openTitle(id) }
        case .m2:
            if queue.count > 1 { navigator.continueSelection = queue[1] }
        case .m3:
            if let id = navigator.focusedTitle ?? queue.first { navigator.openTitle(id, fromPreview: true) }
        case .back:
            navigator.back()
        case .t9:
            guard let id = navigator.focusedTitle, let entry = store.titles[id] else { return }
            let next = entry.progress?.next ?? 1
            store.send(.setWatched(id: id, episodes: [String(next)], watched: true))
        case .season:
            guard let id = navigator.titleOnMac ?? navigator.paths[navigator.section]?.last,
                  let seasons = store.titles[id]?.detail?.seasons,
                  let at = seasons.firstIndex(where: { $0.id == id }), seasons.indices.contains(at + 1)
            else { return }
            navigator.openTitle(seasons[at + 1].id)
        }
    }
}
