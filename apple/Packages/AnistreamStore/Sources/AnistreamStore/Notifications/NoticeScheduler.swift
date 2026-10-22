// Keeps the system's pending new-episode notifications in step with the engine's week: what no
// longer notifies is taken back, what is new or changed is scheduled with its picture, and what
// was scheduled is remembered, so the background check never says an episode a second time.

import AnistreamKit
import Foundation

/// What was scheduled, by identifier, with the version scheduled and when it fires: kept on the
/// device, for a week after it fires.
public final class NoticeLedger: @unchecked Sendable {
    private let defaults: UserDefaults
    private let key: String
    private let lock = NSLock()
    /// How long after it fires an entry is kept: longer than the check looks back.
    static let keptFor: TimeInterval = 8 * 24 * 3_600

    public init(defaults: UserDefaults = .standard, key: String = "anistream.notices") {
        self.defaults = defaults
        self.key = key
    }

    private struct Entry: Codable {
        var fingerprint: String
        var fires: Date
    }

    private func read() -> [String: Entry] {
        guard let data = defaults.data(forKey: key) else { return [:] }
        return (try? JSONDecoder().decode([String: Entry].self, from: data)) ?? [:]
    }

    private func write(_ entries: [String: Entry]) {
        defaults.set(try? JSONEncoder().encode(entries), forKey: key)
    }

    /// Every notification scheduled or posted that is still remembered at `now`.
    public func identifiers(at now: Date = .now) -> Set<String> {
        lock.withLock { Set(read().filter { $0.value.fires > now.addingTimeInterval(-Self.keptFor) }.keys) }
    }

    /// The version of `identifier` last scheduled.
    func fingerprint(_ identifier: String) -> String? {
        lock.withLock { read()[identifier]?.fingerprint }
    }

    /// Remember `notices`, scheduled at `now`, and forget what fired over a week ago.
    func record(_ notices: [PlannedNotice], at now: Date) {
        lock.withLock {
            var entries = read().filter { $0.value.fires > now.addingTimeInterval(-Self.keptFor) }
            for notice in notices {
                entries[notice.identifier] = Entry(fingerprint: notice.fingerprint, fires: notice.fireAt ?? now)
            }
            write(entries)
        }
    }

    /// Forget `identifiers`: taken back before they fired.
    func forget(_ identifiers: some Sequence<String>) {
        lock.withLock {
            var entries = read()
            for identifier in identifiers { entries[identifier] = nil }
            write(entries)
        }
    }
}

/// Schedules the week's notifications and posts what the background check found.
public actor NoticeScheduler {
    public let centre: any NoticeCentre
    let ledger: NoticeLedger
    /// The bytes at a picture's address, from the engine's image cache: `nil` when it could not
    /// be had, and the notification goes without.
    let fetch: @Sendable (String) async -> Data?
    /// Where pictures wait for the centre to take them.
    let staging: URL

    public init(
        centre: any NoticeCentre,
        ledger: NoticeLedger = NoticeLedger(),
        staging: URL = FileManager.default.temporaryDirectory.appending(path: "notice-pictures", directoryHint: .isDirectory),
        fetch: @escaping @Sendable (String) async -> Data?
    ) {
        self.centre = centre
        self.ledger = ledger
        self.staging = staging
        self.fetch = fetch
    }

    /// Make the pending notifications the week's: take back what no longer notifies, and
    /// schedule what is new or has changed. Nothing without permission. What is scheduled.
    @discardableResult
    public func reschedule(_ week: UpcomingNotices, now: Date = .now) async -> [PlannedNotice] {
        let pending = Set(await centre.pending().filter { $0.hasPrefix(NoticePlan.prefix) })
        guard await centre.permission() == .allowed else {
            await centre.cancel(Array(pending))
            return []
        }
        let plan = NoticePlan.schedule(week, now: now)
        let gone = pending.subtracting(plan.map(\.identifier))
        await centre.cancel(Array(gone))
        ledger.forget(gone)
        var scheduled: [PlannedNotice] = []
        for notice in plan {
            if pending.contains(notice.identifier), ledger.fingerprint(notice.identifier) == notice.fingerprint {
                continue
            }
            if await post(notice) { scheduled.append(notice) }
        }
        ledger.record(scheduled, at: now)
        return plan
    }

    /// Post now what the background check found aired that no notification said. What was posted.
    @discardableResult
    public func catchUp(_ said: [NewEpisode], week: UpcomingNotices?, now: Date = .now) async -> [PlannedNotice] {
        guard !said.isEmpty, await centre.permission() == .allowed else { return [] }
        let posts = NoticePlan.catchUp(said, scheduled: ledger.identifiers(at: now), week: week)
        var posted: [PlannedNotice] = []
        for notice in posts where await post(notice) {
            posted.append(notice)
        }
        ledger.record(posted, at: now)
        return posted
    }

    /// Take back every new-episode notification waiting to fire.
    public func clear() async {
        let pending = await centre.pending().filter { $0.hasPrefix(NoticePlan.prefix) }
        await centre.cancel(pending)
        ledger.forget(pending)
    }

    private func post(_ notice: PlannedNotice) async -> Bool {
        var picture: URL?
        if let url = notice.artURL, let bytes = await fetch(url) {
            picture = NoticePictures.stage(bytes, in: staging)
        }
        do {
            try await centre.schedule(notice, picture: picture)
            return true
        } catch {
            if let picture { try? FileManager.default.removeItem(at: picture) }
            AppLog.error("notices", "could not schedule \(notice.identifier): \(error.localizedDescription)")
            return false
        }
    }
}
