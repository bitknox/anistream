// Notices: what the work tells the user beyond the answer it was asked for.

import AnistreamKit
import Foundation
import Observation

/// A button the app puts on a notice of its own, beside the engine's hints: Undo, Try again.
public struct NoticeButton: Sendable {
    public let label: String
    public let run: @MainActor @Sendable () -> Void

    public init(label: String, run: @escaping @MainActor @Sendable () -> Void) {
        self.label = label
        self.run = run
    }
}

/// One notice as the app keeps it: when it came, and a number to tell it from one with the same
/// words.
public struct NoticeItem: Identifiable, Equatable, Sendable {
    public let id: Int
    public let notice: Notice
    public let at: Date
    /// The app's own button on it, when it has one.
    public let action: NoticeButton?

    public static func == (a: NoticeItem, b: NoticeItem) -> Bool {
        a.id == b.id && a.notice == b.notice && a.at == b.at
    }

    public var isAlert: Bool { notice.kind == .alert }
}

extension Notice {
    /// What the notice says in the person's language: the engine's message through the
    /// catalogue, or for a notice the app made itself, its text (already from the catalogue).
    public var words: String {
        msg?.text ?? text
    }
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

    @ObservationIgnored private var count = 0
    @ObservationIgnored private let clock: @Sendable () -> Date

    public init(clock: @escaping @Sendable () -> Date = { Date() }) {
        self.clock = clock
    }

    func post(_ notice: Notice, action: NoticeButton? = nil) {
        count += 1
        let item = NoticeItem(id: count, notice: notice, at: clock(), action: action)
        current = item
        log.insert(item, at: 0)
        if log.count > Self.kept { log.removeLast(log.count - Self.kept) }
    }

    /// Keep `notice` in Messages without putting it up: the whole reason a screen failed for,
    /// which the screen says in a few plain words.
    func record(_ notice: Notice) {
        count += 1
        log.insert(NoticeItem(id: count, notice: notice, at: clock(), action: nil), at: 0)
        if log.count > Self.kept { log.removeLast(log.count - Self.kept) }
    }

    /// Take the capsule down, if it still shows `item`.
    public func dismiss(_ item: NoticeItem) {
        if current?.id == item.id { current = nil }
    }
}
