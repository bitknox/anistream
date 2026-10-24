// "Report a bug": the address of a pre-filled GitHub issue, which the app opens in the browser.
// The engine writes the text, the same on every front end, and strips what is private from it;
// this adds what only the app knows about itself.

import AnistreamKit
import Foundation
#if canImport(UIKit)
import UIKit
#endif

/// What a bug report says about this app and this device.
public struct BugReportDevice: Equatable, Sendable {
    public var appVersion: String
    public var platform: String
    public var osVersion: String
    public var device: DeviceClass
    public var build: String

    public init(appVersion: String, platform: String, osVersion: String, device: DeviceClass, build: String) {
        self.appVersion = appVersion
        self.platform = platform
        self.osVersion = osVersion
        self.device = device
        self.build = build
    }

    /// The device this process runs on, with the version its bundle says.
    public static func current(bundle: Bundle = .main) -> BugReportDevice {
        let system = ProcessInfo.processInfo.operatingSystemVersion
        let version = "\(system.majorVersion).\(system.minorVersion)"
        #if os(macOS)
        let (platform, device) = ("macOS", DeviceClass.computer)
        #else
        let pad = UIDevice.current.userInterfaceIdiom == .pad
        let (platform, device) = pad ? ("iPadOS", DeviceClass.tablet) : ("iOS", DeviceClass.phone)
        #endif
        #if DEBUG
        var build = "debug"
        #else
        var build = "release"
        #endif
        if bundle.bundleIdentifier?.hasSuffix(".fixtures") == true { build = "fixtures" }
        return BugReportDevice(
            appVersion: bundle.infoDictionary?["CFBundleShortVersionString"] as? String ?? "",
            platform: platform, osVersion: version, device: device, build: build
        )
    }
}

/// An error a report starts from: what kind of problem, and what it said.
public struct BugContext: Equatable, Sendable {
    public var kind: String
    public var message: String
    /// The title of the show it was about, if any, which the report removes wherever it appears.
    public var title: String?

    public init(kind: String, message: String, title: String? = nil) {
        self.kind = kind
        self.message = message
        self.title = title
    }
}

extension AppStore {
    /// The address of a new issue for this device, with `context` as its last error.
    public func bugReportURL(context: BugContext? = nil, device: BugReportDevice = .current()) -> URL {
        BugReports.url(device: device, providers: providers.rows, context: context)
    }
}

public enum BugReports {
    /// The address for `device`, the engine's sources and an optional error.
    public static func url(device: BugReportDevice, providers: [ProviderRow], context: BugContext?) -> URL {
        let report = BugReport(
            appVersion: device.appVersion, platform: device.platform, osVersion: device.osVersion,
            device: device.device, build: device.build, providers: providers,
            error: context.map { BugError(kind: $0.kind, message: $0.message) },
            private: context?.title.map { [$0] } ?? []
        )
        // The engine's address is always well formed; the project's tracker is the fallback.
        return URL(string: bugReportUrl(report: report)) ?? URL(string: "https://github.com/bitknox/anistream/issues/new")!
    }
}
