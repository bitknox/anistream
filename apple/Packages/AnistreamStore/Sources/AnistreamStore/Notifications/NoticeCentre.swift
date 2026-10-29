// The system's notification centre as the app uses it, behind a protocol: the app schedules
// through the system's, while the tests and the Fixtures app use one that never reaches it, so
// nothing they do ever shows a banner on the machine.

import AnistreamKit
import AnistreamStrings
import CoreGraphics
import Foundation
import ImageIO
import UserNotifications

/// Whether the app may notify.
public enum NoticePermission: Sendable, Equatable {
    /// Not asked yet: turning a switch on asks.
    case notAsked
    case allowed
    /// Declined, here or in the system's settings.
    case denied
}

/// Where the app's new-episode notifications go.
public protocol NoticeCentre: Sendable {
    func permission() async -> NoticePermission
    /// Ask for permission, the first time a switch is turned on. True when it was given.
    func askPermission() async -> Bool
    /// The identifiers of the notifications waiting to fire.
    func pending() async -> [String]
    /// Schedule `notice`, replacing one with its identifier, with `picture` attached when there
    /// is one: a file the centre takes over.
    func schedule(_ notice: PlannedNotice, picture: URL?) async throws
    /// Take back the notifications waiting under `identifiers`.
    func cancel(_ identifiers: [String]) async
    /// Say what the notifications' actions are: once, before any is scheduled.
    func registerCategories()
}

/// The system's centre.
public struct SystemNoticeCentre: NoticeCentre {
    public init() {}

    private var centre: UNUserNotificationCenter { .current() }

    public func permission() async -> NoticePermission {
        switch await centre.notificationSettings().authorizationStatus {
        case .notDetermined: .notAsked
        case .denied: .denied
        default: .allowed
        }
    }

    public func askPermission() async -> Bool {
        (try? await centre.requestAuthorization(options: [.alert, .sound, .badge])) ?? false
    }

    public func pending() async -> [String] {
        await centre.pendingNotificationRequests().map(\.identifier)
    }

    public func schedule(_ notice: PlannedNotice, picture: URL?) async throws {
        let content = Self.content(notice)
        if let picture, let attachment = try? Self.attachment(picture) {
            content.attachments = [attachment]
        }
        try await centre.add(Self.request(notice, content: content))
    }

    /// What `notice` says, and how the system groups it: every one in the new-episodes thread,
    /// named in a group's summary by its show.
    public static func content(_ notice: PlannedNotice) -> UNMutableNotificationContent {
        let content = UNMutableNotificationContent()
        content.title = notice.title
        content.body = notice.body
        content.threadIdentifier = NoticePlan.thread
        content.summaryArgument = notice.title
        content.categoryIdentifier = notice.category.rawValue
        content.userInfo = NoticeRoute.userInfo(notice)
        content.sound = .default
        return content
    }

    /// `notice` as a request: at the minute and second it airs, in the device's calendar, or now.
    public static func request(_ notice: PlannedNotice, content: UNNotificationContent) -> UNNotificationRequest {
        let trigger = notice.fireAt.map {
            UNCalendarNotificationTrigger(
                dateMatching: Calendar.current.dateComponents(
                    [.year, .month, .day, .hour, .minute, .second], from: $0),
                repeats: false)
        }
        return UNNotificationRequest(identifier: notice.identifier, content: content, trigger: trigger)
    }

    public func cancel(_ identifiers: [String]) async {
        guard !identifiers.isEmpty else { return }
        centre.removePendingNotificationRequests(withIdentifiers: identifiers)
    }

    public func registerCategories() {
        centre.setNotificationCategories(Self.categories())
    }

    /// The two kinds, by their actions: caught up, Play and Mark watched; behind, Play alone.
    /// Play brings the app forward; Mark watched runs in the background. Hidden previews say
    /// "New episode", and a group "3 more new episodes".
    public static func categories() -> Set<UNNotificationCategory> {
        let (play, mark) = (L.actionPlay, L.episodeMarkWatched)
        let playAction = UNNotificationAction(identifier: NoticeAction.play.rawValue, title: play, options: [.foreground])
        let markAction = UNNotificationAction(identifier: NoticeAction.markWatched.rawValue, title: mark, options: [])
        let category = { (kind: NoticeCategory, actions: [UNNotificationAction]) in
            UNNotificationCategory(
                identifier: kind.rawValue, actions: actions, intentIdentifiers: [],
                hiddenPreviewsBodyPlaceholder: NoticePlan.hiddenPreview,
                categorySummaryFormat: NoticePlan.summaryFormat, options: [])
        }
        return [
            category(.newEpisode, [playAction, markAction]),
            category(.behind, [playAction]),
        ]
    }

    /// The picture, its thumbnail the middle square of it: the cover cropped to a square.
    static func attachment(_ picture: URL) throws -> UNNotificationAttachment {
        var options: [String: Any] = [:]
        if let square = NoticePictures.squareCrop(of: picture) {
            options[UNNotificationAttachmentOptionsThumbnailClippingRectKey] = square.dictionaryRepresentation
        }
        return try UNNotificationAttachment(identifier: "cover", url: picture, options: options)
    }
}

/// A centre that never reaches the system: it says notifications are allowed, keeps what is
/// scheduled in memory, and shows nothing. The Fixtures app's, so a run of it never posts a
/// banner, and the tests'.
public final class QuietNoticeCentre: NoticeCentre, @unchecked Sendable {
    private let lock = NSLock()
    private var permissionNow: NoticePermission
    private let answer: Bool
    private var scheduled: [String: PlannedNotice] = [:]
    private var order: [String] = []
    private var asked = 0

    /// - Parameters:
    ///   - permission: what the centre says before it is asked.
    ///   - answer: what it answers when asked.
    public init(permission: NoticePermission = .notAsked, answer: Bool = true) {
        permissionNow = permission
        self.answer = answer
    }

    /// What is waiting to fire, in the order it was scheduled.
    public var notices: [PlannedNotice] {
        lock.withLock { order.compactMap { scheduled[$0] } }
    }

    /// How many times permission was asked for.
    public var timesAsked: Int { lock.withLock { asked } }

    public func permission() async -> NoticePermission { lock.withLock { permissionNow } }

    public func askPermission() async -> Bool {
        lock.withLock {
            asked += 1
            permissionNow = answer ? .allowed : .denied
            return answer
        }
    }

    public func pending() async -> [String] { lock.withLock { order } }

    public func schedule(_ notice: PlannedNotice, picture: URL?) async throws {
        lock.withLock {
            if scheduled[notice.identifier] == nil { order.append(notice.identifier) }
            scheduled[notice.identifier] = notice
        }
        if let picture { try? FileManager.default.removeItem(at: picture) }
    }

    public func cancel(_ identifiers: [String]) async {
        lock.withLock {
            for identifier in identifiers { scheduled[identifier] = nil }
            order.removeAll { scheduled[$0] == nil }
        }
    }

    public func registerCategories() {}
}

/// The pictures notifications carry, staged as files for the centre to take.
enum NoticePictures {
    /// Write `bytes` to a file of its own in `folder`, named for its kind so the system reads it.
    static func stage(_ bytes: Data, in folder: URL) -> URL? {
        let kind: String
        if bytes.starts(with: [0xFF, 0xD8]) {
            kind = "jpg"
        } else if bytes.starts(with: [0x89, 0x50, 0x4E, 0x47]) {
            kind = "png"
        } else if bytes.count > 12, bytes[8..<12].elementsEqual("WEBP".utf8) {
            kind = "webp"
        } else {
            return nil
        }
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let file = folder.appending(path: "\(UUID().uuidString).\(kind)")
            try bytes.write(to: file, options: .atomic)
            return file
        } catch {
            return nil
        }
    }

    /// The middle square of the picture at `url`, in the unit square the attachment's clipping
    /// rectangle is given in.
    static func squareCrop(of url: URL) -> CGRect? {
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = (properties[kCGImagePropertyPixelWidth] as? NSNumber)?.doubleValue,
              let height = (properties[kCGImagePropertyPixelHeight] as? NSNumber)?.doubleValue,
              width > 0, height > 0
        else { return nil }
        return square(width: width, height: height)
    }

    /// The middle square of a `width` by `height` picture, in the unit square.
    static func square(width: Double, height: Double) -> CGRect {
        let side = min(width, height)
        let (w, h) = (side / width, side / height)
        return CGRect(x: (1 - w) / 2, y: (1 - h) / 2, width: w, height: h)
    }
}
