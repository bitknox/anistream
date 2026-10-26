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
    /// A protection checked before it is saved, `nil` until a sheet asks and once it is done.
    public private(set) var trial: ProtectionTrial?
    /// Whether a check went to the engine and its verdict is still to come.
    @ObservationIgnored private var checkSent = false
    /// The check asked for while another was out, sent once that one's verdict is in.
    @ObservationIgnored private var waiting: ProtectionCheck?
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

    /// A check asked for: the check to send now, or `nil` while an earlier one's verdict is still
    /// to come. The engine checks off its loop, so verdicts can come back in any order: one check
    /// at a time is the only way to know which check a verdict answers. The newest waits for its
    /// turn, and one it replaces is never sent.
    func trialStarted(_ check: ProtectionCheck) -> ProtectionCheck? {
        trial = .checking
        guard !checkSent else {
            waiting = check
            return nil
        }
        checkSent = true
        return check
    }

    /// The verdict on the check sent: the check waiting to go next, if any. A verdict is kept
    /// only when it answers the check a sheet still waits for, never an earlier one.
    func trialSaid(_ view: GuardView) -> ProtectionCheck? {
        checkSent = false
        if let next = waiting {
            waiting = nil
            checkSent = true
            return next
        }
        if trial == .checking { trial = .said(view) }
        return nil
    }

    /// Done with the check: its sheet closed, or what it checked changed. A check still waiting
    /// to be sent is dropped.
    public func clearTrial() {
        trial = nil
        waiting = nil
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

/// What a check is asked about: a protection, through a proxy when it is one.
struct ProtectionCheck {
    let mode: TorrentProtection
    let proxy: TorrentProxy?
}

/// A protection checked before anything is saved: under way, then the check's verdict on it.
public enum ProtectionTrial: Equatable, Sendable {
    case checking
    case said(GuardView)

    /// The check passed: what was checked may be saved.
    public var passed: Bool {
        if case .said(.protected) = self { true } else { false }
    }

    /// The check failed, and nothing was saved.
    public var failed: GuardView? {
        if case .said(let view) = self, !passed { view } else { nil }
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
