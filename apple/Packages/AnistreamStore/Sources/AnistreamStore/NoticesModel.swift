// Notices: what the work tells the user beyond the answer it was asked for.

import AnistreamKit
import Foundation
import Observation

/// One notice as the app keeps it: when it came, and a number to tell it from one with the same
/// words.
public struct NoticeItem: Identifiable, Equatable, Sendable {
    public let id: Int
    public let notice: Notice
    public let at: Date

    public var isAlert: Bool { notice.kind == .alert }
}

/// The notice capsule's current notice, and Messages, the log of every notice this session.
@Observable @MainActor
public final class NoticesModel {
    /// How many notices Messages keeps.
    public static let kept = 200

    /// What the capsule shows, until it is dismissed or another arrives.
    public private(set) var current: NoticeItem?
    /// Every notice, newest first.
    public private(set) var log: [NoticeItem] = []
    /// The engine's status line, when it last set one.
    public private(set) var status: String?

    @ObservationIgnored private var count = 0
    @ObservationIgnored private let clock: @Sendable () -> Date

    public init(clock: @escaping @Sendable () -> Date = { Date() }) {
        self.clock = clock
    }

    func post(_ notice: Notice) {
        count += 1
        let item = NoticeItem(id: count, notice: notice, at: clock())
        current = item
        log.insert(item, at: 0)
        if log.count > Self.kept { log.removeLast(log.count - Self.kept) }
    }

    func status(_ text: String) {
        status = text.isEmpty ? nil : text
    }

    /// Take the capsule down, if it still shows `item`.
    public func dismiss(_ item: NoticeItem) {
        if current?.id == item.id { current = nil }
    }
}
