// Where streams come from, as Providers and Settings show it: each provider, why there are none,
// the VPN guard's verdict, and the source pinned for the title on screen.

import AnistreamKit
import Foundation
import Observation

/// The providers, as the engine last said them.
@Observable @MainActor
public final class ProvidersModel {
    /// Each provider, in the order failover tries them.
    public private(set) var rows: [ProviderRow] = []
    /// Whether the engine has said the providers at all.
    public private(set) var loaded = false
    /// Why there are no sources, when there are none.
    public private(set) var note: String?
    /// Whether the sources are being rebuilt, because a setting changed which ones exist: from
    /// `SourcesChanged` until the rebuilt list.
    public private(set) var rebuilding = false
    /// The VPN guard's verdict for torrents, `nil` while torrents are off or before it said.
    public private(set) var guardView: GuardView?
    /// When the guard last said it.
    public private(set) var guardAt: Date?
    /// The source pinned for the title on screen, `nil` for automatic.
    public private(set) var preference: String?
    /// The metadata ladder's rungs.
    public private(set) var meta: [MetaSourceRow] = []

    public init() {}

    func apply(_ rows: [ProviderRow]) {
        self.rows = rows
        loaded = true
        rebuilding = false
        if !rows.isEmpty { note = nil }
    }

    func note(_ text: String) {
        note = text
    }

    func sourcesChanged() {
        rebuilding = true
    }

    func guardSaid(_ view: GuardView?, at date: Date = .now) {
        guardView = view
        guardAt = view == nil ? nil : date
    }

    func preference(_ provider: String?) {
        preference = provider
    }

    func meta(_ rows: [MetaSourceRow]) {
        meta = rows
    }

    public func row(_ id: String) -> ProviderRow? {
        rows.first { $0.id == id }
    }
}

extension ProviderRow {
    /// How it stands, as the Providers list says it beside the name.
    public var standing: ProviderStanding {
        if disabled { return .off }
        if heldBack { return .heldBack }
        if !usable || state == "down" { return .failing }
        if state == "degraded" { return .slow }
        return .working
    }
}

/// How a provider stands, in the words and the dot the Providers list gives it.
public enum ProviderStanding: Equatable, Sendable {
    case working, slow, failing, heldBack
    /// Switched off by name: kept, and not tried until it is on again.
    case off
}

extension GuardView {
    /// Whether torrents may run: the guard passed, or the config accepts no protection.
    public var passes: Bool {
        switch self {
        case .protected, .unprotected: true
        case .checking, .leaking, .unreachable: false
        }
    }
}
