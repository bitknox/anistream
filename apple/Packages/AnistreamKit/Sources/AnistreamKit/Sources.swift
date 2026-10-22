// Where streams come from, as Providers moves them: ports of `anistream_model::sources`'s
// order rules, so a drag or ⌥⌘↑ writes the order the terminal's keys would.

import Foundation

public enum SourceOrder {
    /// `order` with the source `id` moved one place up the chain (`step` below zero) or down it,
    /// or `nil` when it is already at that end or has no row. `order_with_source_moved`.
    ///
    /// One place is past the next row on screen. An entry with no row — `"plugins"`, torrents
    /// while they are off, a plugin named but not installed — is stepped over rather than
    /// counted, and kept all the same. `"plugins"` is first spelled out as the plugins on screen
    /// it stands for, in the order they are shown, and stays right after them.
    public static func moved(_ order: [String], rows: [ProviderRow], id: String, step: Int) -> [String]? {
        var order = order
        let unnamed = rows.filter { $0.kind == "plugin" && !order.contains($0.id) }.map(\.id)
        let at = order.firstIndex(of: "plugins") ?? order.count
        order.insert(contentsOf: unnamed, at: at)

        guard let from = order.firstIndex(of: id) else { return nil }
        let shown = { (entry: String) in entry != "plugins" && rows.contains { $0.id == entry } }
        let to: Int
        if step < 0 {
            guard let before = order[..<from].lastIndex(where: shown) else { return nil }
            to = before
        } else {
            guard let after = order[(from + 1)...].firstIndex(where: shown) else { return nil }
            to = after
        }
        let entry = order.remove(at: from)
        order.insert(entry, at: to)
        return order
    }

    /// `order` with `id` moved to where the row at `target` is on screen: one place at a time,
    /// by the rule above, so a drag writes what the same number of key presses would.
    public static func moved(_ order: [String], rows: [ProviderRow], id: String, to target: Int) -> [String]? {
        guard let from = rows.firstIndex(where: { $0.id == id }), target != from,
              rows.indices.contains(target)
        else { return nil }
        var order = order
        for _ in 0..<abs(target - from) {
            guard let next = moved(order, rows: rows, id: id, step: target < from ? -1 : 1) else { break }
            order = next
        }
        return order
    }

    /// The rows in the order failover tries them: by where each stands in `order`, a plugin
    /// `order` does not name where `"plugins"` stands, or last when it is not there either.
    public static func sorted(_ rows: [ProviderRow], by order: [String]) -> [ProviderRow] {
        func rank(_ row: ProviderRow) -> Int {
            if let at = order.firstIndex(of: row.id) { return at }
            if row.kind == "plugin", let at = order.firstIndex(of: "plugins") { return at }
            return order.count
        }
        return rows.enumerated()
            .sorted { (rank($0.element), $0.offset) < (rank($1.element), $1.offset) }
            .map(\.element)
    }
}
