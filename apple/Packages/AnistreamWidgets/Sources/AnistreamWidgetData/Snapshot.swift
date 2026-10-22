// The snapshot: what the app writes for its widgets, as JSON in the App Group container.

import Foundation

/// What the widgets show, as the app last wrote it: the engine's `WidgetData`, with its art
/// fetched into files beside it.
///
/// The JSON keeps the engine record's field names, and its `version` is the record's:
///
/// - 1: Continue's titles and today's broadcasts.
/// - 2: adds who is signed in, the next broadcast due, the six days after today and, per row, the
///   resume point and time left, the watched mark, the translation and the banner.
///
/// A widget reads both, and refuses a newer version, which it could misread.
public struct WidgetSnapshot: Codable, Sendable, Equatable {
    /// The newest version this build reads and writes.
    public static let newestVersion = 2

    public var version: Int
    /// When the engine made it.
    public var generatedAt: Date
    /// Whether a tracker is signed in. Unknown in a version 1 file.
    public var signedIn: Bool?
    /// Which tracker, `anilist` say, while one is.
    public var tracker: String?
    /// What to continue, most recently watched first.
    public var continueWatching: [SnapshotContinue]
    /// What airs on the day it was made, in the device's time zone, in the order it airs.
    public var airingToday: [SnapshotAiring]
    /// What airs on the six days after that. Empty in a version 1 file.
    public var nextDays: [SnapshotAiring]
    /// The next broadcast due of a show being followed.
    public var nextAiring: SnapshotNextAiring?

    public init(
        version: Int = WidgetSnapshot.newestVersion,
        generatedAt: Date,
        signedIn: Bool? = nil,
        tracker: String? = nil,
        continueWatching: [SnapshotContinue] = [],
        airingToday: [SnapshotAiring] = [],
        nextDays: [SnapshotAiring] = [],
        nextAiring: SnapshotNextAiring? = nil
    ) {
        self.version = version
        self.generatedAt = generatedAt
        self.signedIn = signedIn
        self.tracker = tracker
        self.continueWatching = continueWatching
        self.airingToday = airingToday
        self.nextDays = nextDays
        self.nextAiring = nextAiring
    }

    enum CodingKeys: String, CodingKey {
        case version
        case generatedAt = "generated_at"
        case signedIn = "signed_in"
        case tracker
        case continueWatching = "continue_watching"
        case airingToday = "airing_today"
        case nextDays = "next_days"
        case nextAiring = "next_airing"
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        version = try c.decode(Int.self, forKey: .version)
        generatedAt = try c.decode(Date.self, forKey: .generatedAt)
        signedIn = try c.decodeIfPresent(Bool.self, forKey: .signedIn)
        tracker = try c.decodeIfPresent(String.self, forKey: .tracker)
        continueWatching = try c.decodeIfPresent([SnapshotContinue].self, forKey: .continueWatching) ?? []
        airingToday = try c.decodeIfPresent([SnapshotAiring].self, forKey: .airingToday) ?? []
        nextDays = try c.decodeIfPresent([SnapshotAiring].self, forKey: .nextDays) ?? []
        nextAiring = try c.decodeIfPresent(SnapshotNextAiring.self, forKey: .nextAiring)
    }

    /// Every broadcast it has, today's and the next days', in the order they air.
    public var schedule: [SnapshotAiring] {
        (airingToday + nextDays).sorted { ($0.airsAt, $0.id) < ($1.airsAt, $1.id) }
    }

    /// Every art file it names.
    public var artFiles: Set<String> {
        var names = Set<String>()
        for item in continueWatching { names.formUnion(item.art.files) }
        for item in airingToday + nextDays { names.formUnion(item.art.files) }
        return names
    }
}

/// One title to continue.
public struct SnapshotContinue: Codable, Sendable, Equatable, Identifiable {
    /// The AniList id.
    public var id: Int
    /// The shown title, in the form the title setting picks.
    public var title: String
    /// The episode Continue starts: the one left part-way, or the next one.
    public var episode: Int
    /// How many episodes have been watched.
    public var watched: Int
    /// How many episodes the title has, when that is known.
    public var episodes: Int?
    /// How far into the episode it was left, from 0 to 1.
    public var fraction: Double?
    /// The episode's name, when it is known. Only the large size shows it, as Continue does.
    public var episodeTitle: String?
    /// Whether the episode was left part-way, rather than one to start. Unknown in version 1.
    public var resume: Bool?
    public var positionSecs: Int?
    public var durationSecs: Int?
    /// Whole minutes still to watch: "14 min left".
    public var minutesLeft: Int?
    /// When the title's next episode airs, while one is still to come.
    public var nextAiring: Date?
    public var nextAiringEpisode: Int?
    public var art: Art

    public init(
        id: Int, title: String, episode: Int, watched: Int = 0, episodes: Int? = nil,
        fraction: Double? = nil, episodeTitle: String? = nil, resume: Bool? = nil,
        positionSecs: Int? = nil, durationSecs: Int? = nil, minutesLeft: Int? = nil,
        nextAiring: Date? = nil, nextAiringEpisode: Int? = nil, art: Art = Art()
    ) {
        self.id = id
        self.title = title
        self.episode = episode
        self.watched = watched
        self.episodes = episodes
        self.fraction = fraction
        self.episodeTitle = episodeTitle
        self.resume = resume
        self.positionSecs = positionSecs
        self.durationSecs = durationSecs
        self.minutesLeft = minutesLeft
        self.nextAiring = nextAiring
        self.nextAiringEpisode = nextAiringEpisode
        self.art = art
    }

    enum CodingKeys: String, CodingKey {
        case id, title, episode, watched, episodes, fraction
        case episodeTitle = "episode_title"
        case resume
        case positionSecs = "position_secs"
        case durationSecs = "duration_secs"
        case minutesLeft = "minutes_left"
        case nextAiring = "next_airing"
        case nextAiringEpisode = "next_airing_episode"
        case art
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(Int.self, forKey: .id)
        title = try c.decode(String.self, forKey: .title)
        episode = try c.decode(Int.self, forKey: .episode)
        watched = try c.decodeIfPresent(Int.self, forKey: .watched) ?? 0
        episodes = try c.decodeIfPresent(Int.self, forKey: .episodes)
        fraction = try c.decodeIfPresent(Double.self, forKey: .fraction)
        episodeTitle = try c.decodeIfPresent(String.self, forKey: .episodeTitle)
        resume = try c.decodeIfPresent(Bool.self, forKey: .resume)
        positionSecs = try c.decodeIfPresent(Int.self, forKey: .positionSecs)
        durationSecs = try c.decodeIfPresent(Int.self, forKey: .durationSecs)
        minutesLeft = try c.decodeIfPresent(Int.self, forKey: .minutesLeft)
        nextAiring = try c.decodeIfPresent(Date.self, forKey: .nextAiring)
        nextAiringEpisode = try c.decodeIfPresent(Int.self, forKey: .nextAiringEpisode)
        art = try c.decodeIfPresent(Art.self, forKey: .art) ?? Art()
    }

    /// Whether it plays from a place part-way through: the record says so from version 2, and a
    /// version 1 record by having a fraction.
    public var resumes: Bool {
        resume ?? ((fraction ?? 0) > 0)
    }

    /// The progress bar's fill, from 0 to 1, for an episode left part-way.
    public var progress: Double? {
        guard resumes, let fraction else { return nil }
        return min(max(fraction, 0), 1)
    }
}

/// One broadcast.
public struct SnapshotAiring: Codable, Sendable, Equatable, Identifiable {
    public var id: Int
    public var title: String
    public var episode: Int
    /// When it airs, or aired.
    public var airsAt: Date
    /// Whether the show is one being followed, which the widget marks.
    public var onList: Bool
    /// Whether this episode has been watched here. Unknown, so false, in version 1.
    public var watched: Bool
    /// Sub or dub, when the broadcast is one of them alone.
    public var translation: SnapshotTranslation?
    public var art: Art

    public init(
        id: Int, title: String, episode: Int, airsAt: Date, onList: Bool = false,
        watched: Bool = false, translation: SnapshotTranslation? = nil, art: Art = Art()
    ) {
        self.id = id
        self.title = title
        self.episode = episode
        self.airsAt = airsAt
        self.onList = onList
        self.watched = watched
        self.translation = translation
        self.art = art
    }

    enum CodingKeys: String, CodingKey {
        case id, title, episode
        case airsAt = "airs_at"
        case onList = "on_list"
        case watched, translation, art
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(Int.self, forKey: .id)
        title = try c.decode(String.self, forKey: .title)
        episode = try c.decode(Int.self, forKey: .episode)
        airsAt = try c.decode(Date.self, forKey: .airsAt)
        onList = try c.decodeIfPresent(Bool.self, forKey: .onList) ?? false
        watched = try c.decodeIfPresent(Bool.self, forKey: .watched) ?? false
        // A translation this build does not know is read as none, rather than failing the file.
        translation = (try? c.decodeIfPresent(SnapshotTranslation.self, forKey: .translation)) ?? nil
        art = try c.decodeIfPresent(Art.self, forKey: .art) ?? Art()
    }

    /// Identifies the row among the day's: a show can air twice in a day, as sub and dub.
    public var rowID: String {
        "\(id).\(episode).\(translation?.rawValue ?? "-").\(Int(airsAt.timeIntervalSince1970))"
    }
}

public enum SnapshotTranslation: String, Codable, Sendable, Equatable {
    case sub, dub
}

/// The next broadcast due of a show being followed: "Episode 10 airs Thursday 16:30".
public struct SnapshotNextAiring: Codable, Sendable, Equatable {
    public var id: Int
    public var title: String
    public var episode: Int
    public var airsAt: Date

    public init(id: Int, title: String, episode: Int, airsAt: Date) {
        self.id = id
        self.title = title
        self.episode = episode
        self.airsAt = airsAt
    }

    enum CodingKeys: String, CodingKey {
        case id, title, episode
        case airsAt = "airs_at"
    }
}

/// A row's art, as file names in the group's `widgets/art/`, and the cover's colour.
public struct Art: Codable, Sendable, Equatable {
    /// AniList's banner, scaled to cover 680 × 240 px.
    public var wide: String?
    /// The cover, scaled to cover 96 × 144 px.
    public var poster: String?
    /// The cover's colour, `#5d9be4`.
    public var color: String?

    public init(wide: String? = nil, poster: String? = nil, color: String? = nil) {
        self.wide = wide
        self.poster = poster
        self.color = color
    }

    var files: [String] { [wide, poster].compactMap { $0 } }
}

// MARK: - Reading and writing

extension WidgetSnapshot {
    /// What a snapshot file holds, read.
    public enum Read: Sendable, Equatable {
        case snapshot(WidgetSnapshot)
        /// A version this build does not know: written by a newer app.
        case newer(Int)
        /// Not a snapshot at all.
        case unreadable
    }

    /// `data` read as a snapshot of any version this build knows.
    public static func read(_ data: Data) -> Read {
        struct Header: Decodable { var version: Int }
        let decoder = Self.decoder
        guard let header = try? decoder.decode(Header.self, from: data) else { return .unreadable }
        guard header.version >= 1 else { return .unreadable }
        guard header.version <= newestVersion else { return .newer(header.version) }
        guard let snapshot = try? decoder.decode(WidgetSnapshot.self, from: data) else {
            return .unreadable
        }
        return .snapshot(snapshot)
    }

    /// The snapshot as its file holds it.
    public func encoded() throws -> Data {
        try Self.encoder.encode(self)
    }

    static var decoder: JSONDecoder {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .secondsSince1970
        return decoder
    }

    static var encoder: JSONEncoder {
        let encoder = JSONEncoder()
        // Whole seconds, as the engine counts them.
        encoder.dateEncodingStrategy = .custom { date, encoder in
            var container = encoder.singleValueContainer()
            try container.encode(Int64(date.timeIntervalSince1970.rounded()))
        }
        encoder.outputFormatting = [.sortedKeys]
        return encoder
    }
}
