// Updates: what the engine's update check found, and an update the viewer started.
//
// The engine looks once a day and whenever Check Now asks, and says what it found with
// `Event.update`. On the Mac the app installs a newer release itself, through its updater
// (`UpdateInstaller`, Sparkle in the app); on iPhone and iPad the viewer updates in SideStore or
// AltStore, from the source each release publishes. Nothing installs until the viewer presses
// Update: `update()` is the one way an install starts.

import AnistreamKit
import AnistreamStrings
import Foundation
import Observation

/// What installs a release on this device: the Mac's updater. iPhone and iPad have none.
@MainActor
public protocol UpdateInstaller: AnyObject {
    /// Download `release`, check it, install it and relaunch, saying how it goes through
    /// `progress`. Called only when the viewer presses Update.
    func install(_ release: Release, progress: @escaping @MainActor (UpdateProgress) -> Void)
    /// Stop an update that has not started installing.
    func cancel()
}

/// How far an update the viewer started has come.
public enum UpdateProgress: Equatable, Sendable {
    case idle
    /// Finding the release in the update feed.
    case preparing
    /// Downloading: bytes so far, and of how many when the server said.
    case downloading(received: UInt64, expected: UInt64?)
    /// Checking and unpacking what was downloaded, from nought to one.
    case unpacking(Double)
    /// Installed; the app quits and opens again.
    case relaunching
    /// It stopped, and why, in words to show.
    case failed(String)

    /// Whether an update is under way.
    public var isActive: Bool {
        switch self {
        case .preparing, .downloading, .unpacking, .relaunching: true
        case .idle, .failed: false
        }
    }

    /// How far along, from nought to one, when that is known.
    public var fraction: Double? {
        switch self {
        case .downloading(let received, let expected?) where expected > 0:
            min(1, Double(received) / Double(expected))
        case .unpacking(let done): done
        case .relaunching: 1
        default: nil
        }
    }
}

/// The update check's word, and an update the viewer started.
@Observable @MainActor
public final class UpdatesModel {
    /// What the last check the viewer asked for found, when it had nothing to offer.
    public enum Outcome: Equatable, Sendable {
        case upToDate
        /// This build is a beta ahead of the newest stable release, and beta updates are off:
        /// the version, so the line can say updates resume with the next stable.
        case stableBehind(String)
        case failed(String)
    }

    /// The newest release, while it is newer than this build.
    public private(set) var latest: Release?
    /// Whether this build is a beta, a release not yet made the stable one, as the last check
    /// found. About then shows "2.3.4 beta".
    public private(set) var runningBeta = false
    /// Whether a check is under way that the viewer asked for.
    public private(set) var checking = false
    /// What the last check said when it found no release to offer.
    public private(set) var outcome: Outcome?
    /// The release the notice tells of, until it is dismissed or taken.
    public private(set) var notice: Release?
    /// How far an update the viewer started has come.
    public private(set) var progress: UpdateProgress = .idle
    /// What installs a release here: the Mac's updater, set by the app. None on iPhone and iPad.
    @ObservationIgnored public var installer: (any UpdateInstaller)?
    /// How a command reaches the engine.
    @ObservationIgnored var send: (Command) -> Void = { _ in }

    public init() {}

    /// The source SideStore and AltStore read the iPhone and iPad app from, which every release
    /// publishes beside its files.
    public static let sourceURL =
        URL(string: "https://github.com/bitknox/anistream/releases/latest/download/sidestore.json")!

    /// The Sparkle feed of one release: its own appcast, signed like every other, so a beta
    /// the engine offered installs even though `releases/latest` is the stable.
    public static func feedURL(forVersion version: String) -> URL? {
        URL(string: "https://github.com/bitknox/anistream/releases/download/v\(version)/appcast.xml")
    }

    func apply(_ status: UpdateStatus) {
        switch status {
        case .checking:
            checking = true
        case .upToDate(let stableBehind, let beta):
            checking = false
            latest = nil
            notice = nil
            runningBeta = beta
            outcome = stableBehind.map { .stableBehind($0) } ?? .upToDate
        case .available(let release, let notify):
            checking = false
            latest = release
            outcome = nil
            if notify, !progress.isActive { notice = release }
        case .failed(let reason):
            checking = false
            outcome = .failed(reason)
        }
    }

    /// Settings' Check Now.
    public func checkNow() {
        guard !checking, !progress.isActive else { return }
        checking = true
        send(.checkForUpdates)
    }

    /// Whether this device installs the release itself: the Mac, when the release has its file.
    public var installsHere: Bool {
        installer != nil && latest?.macos != nil
    }

    /// The viewer pressed Update: the one way an install starts. Does nothing without a release
    /// for this device, or while an update is under way.
    public func update() {
        guard let release = latest, release.macos != nil, let installer, !progress.isActive else { return }
        notice = nil
        progress = .preparing
        installer.install(release) { [weak self] progress in
            self?.progress = progress
        }
    }

    /// Stop an update before it installs.
    public func cancel() {
        guard progress.isActive, progress != .relaunching else { return }
        installer?.cancel()
        progress = .idle
    }

    /// Take the notice down. The release stays on offer in Settings.
    public func dismissNotice() {
        notice = nil
    }

    /// Where iPhone and iPad update `release`, in the order to try: SideStore's link that adds
    /// the source, AltStore's, then the release's page.
    public static func storeLinks(for release: Release) -> [URL] {
        var links: [URL] = []
        for scheme in ["sidestore", "altstore"] {
            var link = URLComponents()
            link.scheme = scheme
            link.host = "source"
            link.queryItems = [URLQueryItem(name: "url", value: sourceURL.absoluteString)]
            if let url = link.url { links.append(url) }
        }
        if let page = URL(string: release.pageUrl) { links.append(page) }
        return links
    }
}

/// What the Mac's updater answers when its feed shows an update: install only the release the
/// viewer pressed Update for, or a newer one, and never one the updater found on its own.
public struct UpdateGate: Equatable, Sendable {
    /// The version the viewer pressed Update for, until the update ends.
    public private(set) var armed: String?

    public init() {}

    public enum Choice: Equatable, Sendable {
        case install
        case dismiss
    }

    /// The viewer pressed Update for `version`.
    public mutating func arm(_ version: String) {
        armed = version
    }

    /// The update ended, or was stopped.
    public mutating func disarm() {
        armed = nil
    }

    /// What to answer the feed's update `found`: install it when the viewer asked for it or an
    /// older release, as when a newer one came out since; dismiss anything else, and an update
    /// that is only information, with no file.
    public func choice(found: String, informationOnly: Bool) -> Choice {
        guard let armed, !informationOnly, !Self.older(found, than: armed) else { return .dismiss }
        return .install
    }

    /// Whether the version `a` is older than `b`, compared number by number: 2.0.10 is newer
    /// than 2.0.9.
    static func older(_ a: String, than b: String) -> Bool {
        let parts = { (version: String) in version.split(separator: ".").map { Int($0) ?? 0 } }
        let (x, y) = (parts(a), parts(b))
        for index in 0..<max(x.count, y.count) {
            let (p, q) = (index < x.count ? x[index] : 0, index < y.count ? y[index] : 0)
            if p != q { return p < q }
        }
        return false
    }
}

#if FIXTURES
/// The Fixtures app's updater: it goes through a download as the Mac's would, and installs
/// nothing.
@MainActor
public final class RehearsedInstaller: UpdateInstaller {
    private var task: Task<Void, Never>?

    public init() {}

    public func install(_ release: Release, progress: @escaping @MainActor (UpdateProgress) -> Void) {
        let size = release.macos?.size ?? 0
        task = Task { @MainActor in
            progress(.preparing)
            try? await Task.sleep(for: .milliseconds(400))
            for step in 0...10 {
                guard !Task.isCancelled else { return }
                progress(.downloading(received: size * UInt64(step) / 10, expected: size))
                try? await Task.sleep(for: .milliseconds(300))
            }
            progress(.failed(L.updatesFixturesNoInstall))
        }
    }

    public func cancel() {
        task?.cancel()
    }
}
#endif
