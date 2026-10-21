// The download queue and what is still seeding, as Downloads shows them: grouped by title, with
// each running download's speed worked out from how far it moved between two updates.

import AnistreamKit
import Foundation
import Observation

/// The downloads, as the engine last said them.
@Observable @MainActor
public final class DownloadsModel {
    /// The queue, in the engine's order.
    public private(set) var rows: [DownloadRow] = []
    /// Watched episodes still seeding.
    public private(set) var seeding: [SeedRow] = []
    /// Whether the engine has said the queue at all.
    public private(set) var loaded = false
    /// Bytes a second for each running download.
    public private(set) var rates: [Int64: Double] = [:]
    @ObservationIgnored private var last: [Int64: (bytes: UInt64, at: Date)] = [:]

    public init() {}

    func apply(_ rows: [DownloadRow], at now: Date = .now) {
        var rates: [Int64: Double] = [:]
        var last: [Int64: (bytes: UInt64, at: Date)] = [:]
        for row in rows where row.state == .active {
            last[row.id] = (row.downloaded, now)
            guard let before = self.last[row.id] else { continue }
            let seconds = now.timeIntervalSince(before.at)
            guard seconds > 0.2, row.downloaded >= before.bytes else {
                rates[row.id] = self.rates[row.id]
                last[row.id] = before
                continue
            }
            let rate = Double(row.downloaded - before.bytes) / seconds
            // Half the last reading and half this one: steady enough to read, quick to follow.
            rates[row.id] = self.rates[row.id].map { ($0 + rate) / 2 } ?? rate
        }
        self.rows = rows
        self.rates = rates
        self.last = last
        loaded = true
    }

    func seeding(_ rows: [SeedRow]) {
        seeding = rows
    }

    /// The queue by title, each title where its first row is.
    public var groups: [DownloadGroup] {
        var order: [AnilistId] = []
        var byTitle: [AnilistId: [DownloadRow]] = [:]
        for row in rows {
            if byTitle[row.anilistId] == nil { order.append(row.anilistId) }
            byTitle[row.anilistId, default: []].append(row)
        }
        return order.map { DownloadGroup(id: $0, rows: byTitle[$0] ?? []) }
    }

    /// What the downloads take on disk: a finished one's size, and what the rest have so far.
    public var bytesOnDisk: UInt64 {
        rows.reduce(0) { $0 + $1.bytesOnDisk }
    }

    /// Seconds until a running download finishes, at its speed.
    public func secondsLeft(_ row: DownloadRow) -> Int64? {
        guard row.state == .active, row.total > row.downloaded, let rate = rates[row.id], rate > 0
        else { return nil }
        return Int64((Double(row.total - row.downloaded) / rate).rounded(.up))
    }
}

/// One title's downloads.
public struct DownloadGroup: Identifiable, Equatable, Sendable {
    public let id: AnilistId
    public let rows: [DownloadRow]

    /// The title, as the queue names it.
    public var title: String { rows.first?.title ?? "" }

    public var bytesOnDisk: UInt64 {
        rows.reduce(0) { $0 + $1.bytesOnDisk }
    }
}

extension DownloadRow {
    /// What it takes on disk now.
    public var bytesOnDisk: UInt64 {
        state == .done ? max(total, downloaded) : downloaded
    }
}

extension SeedRow {
    /// The file's size: what has gone up over the ratio that makes.
    public var size: UInt64? {
        ratio > 0 ? UInt64((Double(uploadedBytes) / ratio).rounded()) : nil
    }
}
