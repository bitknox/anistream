// AniList's public airing schedule, which Airing today fetches itself when the app has written
// no snapshot it can read: the times are right, and nothing is marked.

import Foundation

public enum PublicSchedule {
    /// AniList's GraphQL endpoint.
    public static let endpoint = URL(string: "https://graphql.anilist.co")!

    /// At most this many pages of 50 are asked for a day.
    static let pages = 2

    static let query = """
        query ($from: Int, $to: Int, $page: Int) {
          Page(page: $page, perPage: 50) {
            pageInfo { hasNextPage }
            airingSchedules(airingAt_greater: $from, airingAt_lesser: $to, sort: TIME) {
              airingAt
              episode
              media { id isAdult title { romaji english } coverImage { color } }
            }
          }
        }
        """

    /// The request for one page of the broadcasts between `from` and `to`.
    public static func request(from: Date, to: Date, page: Int) -> URLRequest {
        var request = URLRequest(url: endpoint, timeoutInterval: 10)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        let body: [String: Any] = [
            "query": query,
            "variables": [
                // AniList's bounds are exclusive.
                "from": Int(from.timeIntervalSince1970) - 1,
                "to": Int(to.timeIntervalSince1970),
                "page": page,
            ],
        ]
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)
        return request
    }

    /// One page of AniList's answer, as broadcasts: no adult titles, titles in English where
    /// AniList has one, as anistream names them signed out. Whether there is another page.
    public static func decode(_ data: Data) throws -> (rows: [SnapshotAiring], more: Bool) {
        let answer = try JSONDecoder().decode(Answer.self, from: data)
        guard let page = answer.data?.page else { throw ScheduleError.noData }
        let rows = page.airingSchedules.compactMap { entry -> SnapshotAiring? in
            guard let media = entry.media, media.isAdult != true else { return nil }
            let english = media.title.english?.trimmingCharacters(in: .whitespaces)
            let title = (english?.isEmpty == false ? english : nil) ?? media.title.romaji ?? ""
            guard !title.isEmpty else { return nil }
            return SnapshotAiring(
                id: media.id, title: title, episode: entry.episode,
                airsAt: Date(timeIntervalSince1970: TimeInterval(entry.airingAt)),
                art: Art(color: media.coverImage?.color))
        }
        return (rows, page.pageInfo?.hasNextPage ?? false)
    }

    /// The broadcasts of the day `now` is in, asked of AniList.
    public static func fetch(
        day now: Date, calendar: Calendar = .current, session: URLSession = .shared
    ) async throws -> [SnapshotAiring] {
        let start = calendar.startOfDay(for: now)
        guard let end = calendar.date(byAdding: .day, value: 1, to: start) else { return [] }
        var rows: [SnapshotAiring] = []
        for page in 1...pages {
            let (data, response) = try await session.data(for: request(from: start, to: end, page: page))
            if let http = response as? HTTPURLResponse, http.statusCode != 200 {
                throw ScheduleError.status(http.statusCode)
            }
            let decoded = try decode(data)
            rows += decoded.rows
            if !decoded.more { break }
        }
        return rows
    }

    public enum ScheduleError: Error, Equatable {
        case noData
        case status(Int)
    }

    // AniList's wire shape.
    struct Answer: Decodable {
        var data: Payload?
    }

    struct Payload: Decodable {
        var page: PageData?

        enum CodingKeys: String, CodingKey {
            case page = "Page"
        }
    }

    struct PageData: Decodable {
        var pageInfo: PageInfo?
        var airingSchedules: [Entry]
    }

    struct PageInfo: Decodable {
        var hasNextPage: Bool?
    }

    struct Entry: Decodable {
        var airingAt: Int
        var episode: Int
        var media: Media?
    }

    struct Media: Decodable {
        var id: Int
        var isAdult: Bool?
        var title: Title
        var coverImage: Cover?
    }

    struct Title: Decodable {
        var romaji: String?
        var english: String?
    }

    struct Cover: Decodable {
        var color: String?
    }
}

/// The extension's copy of a day's public schedule, kept so every reload does not ask AniList.
public struct CachedSchedule: Codable, Sendable, Equatable {
    public var fetchedAt: Date
    public var rows: [SnapshotAiring]

    public init(fetchedAt: Date, rows: [SnapshotAiring]) {
        self.fetchedAt = fetchedAt
        self.rows = rows
    }

    enum CodingKeys: String, CodingKey {
        case fetchedAt = "fetched_at"
        case rows
    }

    /// Whether it still answers for `now`: fetched on the same day, within the lifetime.
    public func answers(at now: Date, calendar: Calendar = .current) -> Bool {
        calendar.isDate(fetchedAt, inSameDayAs: now)
            && now.timeIntervalSince(fetchedAt) < WidgetTimeline.publicScheduleLifetime
            && now >= fetchedAt
    }
}

extension WidgetFiles {
    /// The public schedule kept for `now`'s day, fetching it again when it is older than its
    /// lifetime. The copy kept, however old, when AniList cannot be reached; nil with none.
    public func publicSchedule(
        at now: Date, calendar: Calendar = .current,
        fetch: (Date) async throws -> [SnapshotAiring] = { try await PublicSchedule.fetch(day: $0) }
    ) async -> [SnapshotAiring]? {
        let kept = (try? Data(contentsOf: publicScheduleFile)).flatMap {
            try? WidgetSnapshot.decoder.decode(CachedSchedule.self, from: $0)
        }
        if let kept, kept.answers(at: now, calendar: calendar) { return kept.rows }
        do {
            let rows = try await fetch(now)
            let cached = CachedSchedule(fetchedAt: now, rows: rows)
            try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try? WidgetSnapshot.encoder.encode(cached).write(to: publicScheduleFile, options: .atomic)
            return rows
        } catch {
            guard let kept, calendar.isDate(kept.fetchedAt, inSameDayAs: now) else { return nil }
            return kept.rows
        }
    }
}
