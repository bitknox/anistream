// What the source picker shows: the options the engine listed for a title, which of them is
// pinned, and the one notice a pin the engine could not use is owed.

import AnistreamKit
import Foundation
import Observation

/// A pinned source that could not be used, for the notice that says so.
public struct PinUnavailableNotice: Equatable, Sendable {
    public let id: AnilistId
    public let name: String
    /// Tells two of the same words apart, so each one is said.
    public let serial: Int
}

/// The picker's options per title, from `Event::SourceOptions`.
@Observable @MainActor
public final class SourceChoiceModel {
    /// The latest listing of each title, in the order its sources are tried. A listing stays
    /// until the next one replaces it, so reopening the picker shows it at once.
    public private(set) var options: [AnilistId: [SourceOption]] = [:]
    /// Titles whose listing was asked for and has not come.
    public private(set) var asked: Set<AnilistId> = []
    /// The newest pin the engine could not use. The shells say it once.
    public private(set) var pinNotice: PinUnavailableNotice?

    @ObservationIgnored private var said: Set<String> = []
    @ObservationIgnored private var serial = 0

    public init() {}

    /// Whether the picker for `id` has nothing to show yet.
    public func isLoading(_ id: AnilistId) -> Bool {
        options[id] == nil && asked.contains(id)
    }

    func asking(_ id: AnilistId) {
        asked.insert(id)
    }

    /// The picker closed: an answer that never came is not waited for.
    public func giveUp(_ id: AnilistId) {
        asked.remove(id)
    }

    func apply(id: AnilistId, options listed: [SourceOption]) {
        asked.remove(id)
        options[id] = listed
    }

    /// A choice made, shown at once: the pinned flags follow it, so the picker reads right when
    /// it opens again before the engine's next listing.
    func chose(_ id: AnilistId, provider: String?, release: String? = nil) {
        // A new choice may meet the same unavailable source again, and says so again.
        said = said.filter { !$0.hasPrefix("\(id):") }
        guard let listed = options[id] else { return }
        options[id] = listed.map { option in
            var option = option
            option.pinned = option.providerId == provider
            option.pinnedRelease = option.pinned ? release : nil
            return option
        }
    }

    func pinUnavailable(id: AnilistId, providerId: String, name: String) {
        guard said.insert("\(id):\(providerId)").inserted else { return }
        serial += 1
        pinNotice = PinUnavailableNotice(id: id, name: name, serial: serial)
    }
}
