// What the playback and airing Live Activities are shown through: ActivityKit on iPhone, with
// local updates only (no push, which needs a paid team), or a fake in the tests. And the covers
// they show, fetched once a title and written into the App Group for the extension to read.

import AnistreamKit
import AnistreamWidgetData
import Foundation
#if os(iOS) && canImport(ActivityKit)
import ActivityKit
#endif

/// Where one kind of activity is shown.
@MainActor
public protocol LiveActivityHost<State>: AnyObject {
    associatedtype State
    /// Whether the system lets the app show activities now.
    var isEnabled: Bool { get }
    /// Whether one is up.
    var isShowing: Bool { get }
    /// Start one; false when the system does not allow it, as in the background.
    func start(_ state: State, staleAt: Date?) -> Bool
    func update(_ state: State, staleAt: Date?)
    /// End it: at once, or showing `state` until `dismissAt`.
    func end(_ state: State?, dismissAt: Date?)
}

/// Which activity leads when several are up: the Dynamic Island shows the highest. Playback is
/// what the viewer is doing now; an episode airing is due at a time; downloads run on, and in the
/// background the system shows their progress itself. Downloads keep the system's default of 0.
public enum LiveActivityRelevance {
    public static let playback: Double = 100
    public static let airing: Double = 50
}

#if os(iOS) && canImport(ActivityKit)
/// ActivityKit's activity of type `Attributes`. It keeps the activity's id, and finds the
/// activity by it for each change, off the main actor.
@MainActor
final class SystemLiveActivity<Attributes: ActivityAttributes>: LiveActivityHost
where Attributes.ContentState: Sendable {
    typealias State = Attributes.ContentState

    private let attributes: Attributes
    private let relevance: Double
    private let name: String
    private var id: String?
    private var refused = false

    init(_ attributes: Attributes, relevance: Double, name: String) {
        self.attributes = attributes
        self.relevance = relevance
        self.name = name
        // An activity a previous run left up says nothing true any more.
        Task.detached { await Self.endAll() }
    }

    var isEnabled: Bool { ActivityAuthorizationInfo().areActivitiesEnabled }

    var isShowing: Bool { id != nil }

    func start(_ state: State, staleAt: Date?) -> Bool {
        guard isEnabled else { return false }
        do {
            let activity = try Activity.request(
                attributes: attributes,
                content: ActivityContent(state: state, staleDate: staleAt, relevanceScore: relevance),
                pushType: nil)
            id = activity.id
            refused = false
            return true
        } catch {
            // Said once until one starts: a start is tried again at each change.
            if !refused { AppLog.info(name, "no Live Activity: \(error.localizedDescription)") }
            refused = true
            return false
        }
    }

    func update(_ state: State, staleAt: Date?) {
        guard let id else { return }
        let content = ActivityContent(state: state, staleDate: staleAt, relevanceScore: relevance)
        Task.detached { await Self.update(id, content) }
    }

    func end(_ state: State?, dismissAt: Date?) {
        guard let id else { return }
        self.id = nil
        let content = state.map { ActivityContent(state: $0, staleDate: nil, relevanceScore: relevance) }
        Task.detached { await Self.end(id, content, dismissAt: dismissAt) }
    }

    nonisolated private static func activity(_ id: String) -> Activity<Attributes>? {
        Activity<Attributes>.activities.first { $0.id == id }
    }

    nonisolated private static func update(_ id: String, _ content: ActivityContent<State>) async {
        await activity(id)?.update(content)
    }

    nonisolated private static func end(_ id: String, _ content: ActivityContent<State>?, dismissAt: Date?) async {
        await activity(id)?.end(content, dismissalPolicy: dismissAt.map { .after($0) } ?? .immediate)
    }

    nonisolated private static func endAll() async {
        for activity in Activity<Attributes>.activities {
            await activity.end(nil, dismissalPolicy: .immediate)
        }
    }
}
#endif

/// The covers the activities show, as files in the App Group's `widgets/live/`, by title.
@MainActor
final class LiveActivityCovers {
    private let files: WidgetFiles?
    private var names: [AnilistId: String] = [:]
    private var asked: Set<AnilistId> = []

    init(files: WidgetFiles?) {
        self.files = files
    }

    /// The cover's file for `id`, fetching it from `url` the first time: nil until it is there,
    /// and `ready` told its name once it is.
    func file(
        for id: AnilistId, url: String?, artwork: (any WidgetArtSource)?, ready: @escaping @MainActor (String) -> Void
    ) -> String? {
        if let name = names[id] { return name }
        guard let files, let artwork, let url, asked.insert(id).inserted else { return nil }
        Task { [weak self] in
            let images = await artwork.images(for: [url], maxPixelSize: 400)
            guard let image = images[url], let jpeg = ArtThumbnail.jpeg(from: image, kind: .poster) else { return }
            let name = WidgetFiles.artName(for: url, kind: .poster)
            do {
                try FileManager.default.createDirectory(at: files.liveDirectory, withIntermediateDirectories: true)
                try jpeg.write(to: files.liveDirectory.appending(path: name), options: .atomic)
                self?.names[id] = name
                ready(name)
            } catch {}
        }
        return nil
    }
}
