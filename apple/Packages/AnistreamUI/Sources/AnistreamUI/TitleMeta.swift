// AniList's metadata as the title page and its sheet say it (native-spec/metadata.md): the other
// forms of the title, the facts, the airing state from the dates, the related cards in their
// order, the rankings, cast, staff and tags, and the episode still to air.

import AnistreamKit
import AnistreamStrings
import Foundation

// MARK: - Titles

extension Entry {
    /// The line under the title: Romaji, English, Native in that order, without the form shown
    /// above it, a missing one or one that repeats another ("Stardust Wayfarer · 星屑の旅人").
    var otherTitles: [String] {
        var seen: Set<String> = [showTitle]
        return [titleRomaji, titleEnglish, titleNative].compactMap { form in
            guard let form, !form.isEmpty, seen.insert(form).inserted else { return nil }
            return form
        }
    }
}

extension LinkedTitle {
    /// The title a card shows: the engine's, in the form Settings › Title language chose.
    var shownTitle: String {
        title
    }

    /// "Movie · 2025": its format and year, as a recommendation says them.
    var formatYear: String {
        [format.map(TitleWords.format), year.map(String.init)].compactMap { $0 }.joined(separator: " · ")
    }

    /// "Side story · Movie · 2025": how it relates, then its format and year, as a related card
    /// says them.
    var relationLine: String {
        [relation.map(TitleWords.relation), format.map(TitleWords.format), year.map(String.init)]
            .compactMap { $0 }.joined(separator: " · ")
    }
}

// MARK: - Words

/// AniList's kinds in the words the screens use.
enum TitleWords {
    static func season(_ season: Season) -> String {
        switch season {
        case .winter: L.seasonWinter
        case .spring: L.seasonSpring
        case .summer: L.seasonSummer
        case .fall: L.seasonAutumn
        }
    }

    /// What a title was adapted from, or `nil` for Other and none, which are left out.
    static func source(_ source: SourceMaterial) -> String? {
        switch source {
        case .original: L.materialOriginal
        case .manga: L.materialManga
        case .lightNovel: L.materialLightNovel
        case .webNovel: L.materialWebNovel
        case .visualNovel: L.materialVisualNovel
        case .videoGame: L.materialVideoGame
        case .novel: L.materialNovel
        case .doujinshi: L.materialDoujinshi
        case .anime: L.materialAnime
        case .liveAction: L.materialLiveAction
        case .game: L.materialGame
        case .comic: L.materialComic
        case .multimediaProject: L.materialMultimedia
        case .pictureBook: L.materialPictureBook
        case .other: nil
        }
    }

    static func relation(_ relation: RelationKind) -> String {
        switch relation {
        case .parent: L.relationParent
        case .prequel: L.relationPrequel
        case .sequel: L.relationSequel
        case .sideStory: L.relationSideStory
        case .spinOff: L.relationSpinOff
        case .alternative: L.relationAlternative
        case .summary: L.relationSummary
        case .compilation: L.relationCompilation
        case .contains: L.relationContains
        case .sameUniverse: L.relationSameUniverse
        case .character: L.relationSharesCharacters
        case .adaptation: L.relationAdaptation
        case .source: L.showSettingsSource
        case .other: L.relationOther
        }
    }

    /// AniList's format as a card says it: TV, TV short, Movie, Special, OVA, ONA, Music.
    static func format(_ format: String) -> String {
        switch format.uppercased() {
        case "TV": L.formatTv
        case "TV_SHORT", "TV SHORT": L.formatTvShort
        case "MOVIE": L.formatMovie
        case "SPECIAL": L.formatSpecial
        case "OVA": L.formatOva
        case "ONA": L.formatOna
        case "MUSIC": L.formatMusic
        default: format.capitalizedSentence
        }
    }

    static func role(_ role: CastRole) -> String {
        switch role {
        case .main: L.moreRoleMain
        case .supporting: L.moreRoleSupporting
        case .background: L.moreRoleBackground
        }
    }
}

// MARK: - Facts

/// One fact about a title, as the details grid and the sheet's About list them.
struct TitleFact: Equatable {
    let label: String
    let value: String
}

/// Where a title stands, from its dates until the engine says it outright.
enum AiringState: Equatable {
    case finished, airing, upcoming
}

extension FuzzyDate {
    /// "20 Aug 2026", "Aug 2026" without the day, "2026" with the year alone.
    var words: String {
        guard let month, (1...12).contains(month), let date = Self.date(Int(year), Int(month), day.map(Int.init) ?? 1) else {
            return String(year)
        }
        let style = Date.FormatStyle(locale: AirTime.locale, timeZone: Self.utc).month(.abbreviated).year()
        return day == nil ? date.formatted(style) : date.formatted(style.day())
    }

    /// "20 Aug", without the year, for the start of a range within one year.
    fileprivate var dayMonth: String? {
        guard let month, (1...12).contains(month), let date = Self.date(Int(year), Int(month), day.map(Int.init) ?? 1) else {
            return nil
        }
        let style = Date.FormatStyle(locale: AirTime.locale, timeZone: Self.utc).month(.abbreviated)
        return day == nil ? date.formatted(style) : date.formatted(style.day())
    }

    /// The first moment it can mean, and the last: a year alone runs from its first day to its
    /// last.
    fileprivate func bounds(_ calendar: Calendar) -> (start: Date, end: Date)? {
        let start = DateComponents(year: Int(year), month: month.map(Int.init) ?? 1, day: day.map(Int.init) ?? 1)
        guard let first = calendar.date(from: start) else { return nil }
        let length: Calendar.Component = day != nil ? .day : month != nil ? .month : .year
        guard let next = calendar.date(byAdding: length, value: 1, to: first) else { return nil }
        return (first, next)
    }

    private static let utc = TimeZone(identifier: "UTC") ?? .gmt

    /// A day with no time zone, so the date reads the same wherever the person is.
    private static func date(_ year: Int, _ month: Int, _ day: Int) -> Date? {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = utc
        return calendar.date(from: DateComponents(year: year, month: month, day: day))
    }
}

extension Entry {
    /// "Summer 2026", or "2026" with only the year.
    var seasonWords: String? {
        guard let year else { return nil }
        return season.map { L.seasonWithYear(season: TitleWords.season($0), year: String(year)) } ?? String(year)
    }

    var basedOn: String? {
        source.flatMap(TitleWords.source)
    }

    /// Where the title stands by its dates: finished once its end has passed, airing once its
    /// start has, upcoming before. `nil` with no dates at all.
    func airingState(now: Date = .now, calendar: Calendar = .current) -> AiringState? {
        if let end = endDate?.bounds(calendar), end.end <= now { return .finished }
        guard let start = startDate?.bounds(calendar) else { return endDate == nil ? nil : .airing }
        return start.start <= now ? .airing : .upcoming
    }

    /// The airing fact: "Aired" with "4 Apr – 22 Jun 2024", "Airing" with "Since 20 Aug 2026",
    /// "Airs" with "From Jan 2027".
    func airingFact(now: Date = .now, calendar: Calendar = .current) -> TitleFact? {
        switch airingState(now: now, calendar: calendar) {
        case .finished:
            guard let end = endDate else { return nil }
            guard let start = startDate, start != end else { return TitleFact(label: L.mediaStatusAired, value: end.words) }
            let from = start.year == end.year ? (start.dayMonth ?? start.words) : start.words
            return TitleFact(label: L.mediaStatusAired, value: "\(from) – \(end.words)")
        case .airing:
            return TitleFact(label: L.mediaStatusAiring, value: startDate.map { L.mediaAiringSince(date: $0.words) } ?? L.titleAiringNow)
        case .upcoming:
            return startDate.map { TitleFact(label: L.titleFactAirs, value: L.mediaAiringFrom(date: $0.words)) }
        case nil:
            return nil
        }
    }

    /// "Hikari Stream", or "Hikari Stream and 1 more" with more than one.
    var alsoOnFact: String? {
        guard let first = availableOn.first else { return nil }
        return availableOn.count > 1 ? L.titleAlsoOnMoreApple(first: first, count: availableOn.count - 1) : first
    }

    /// The top three tags by rank that give nothing away.
    var topTags: [String] {
        Array(safeTags.prefix(3))
    }

    /// Every tag that gives nothing away, most relevant first.
    var safeTags: [String] {
        (detail?.tags ?? []).filter { !$0.mediaSpoiler && !$0.generalSpoiler }.map(\.name)
    }

    /// The tags that would spoil something, which the sheet hides until asked.
    var spoilerTags: [String] {
        (detail?.tags ?? []).filter { $0.mediaSpoiler || $0.generalSpoiler }.map(\.name)
    }

    /// The facts, in their order, each left out when AniList does not have it: Studio, Based on,
    /// Season, the airing fact, Also on.
    func facts(now: Date = .now) -> [TitleFact] {
        var facts: [TitleFact] = []
        if let studio { facts.append(TitleFact(label: L.titleStudio, value: studio)) }
        if let basedOn { facts.append(TitleFact(label: L.titleBasedOn, value: basedOn)) }
        if let seasonWords { facts.append(TitleFact(label: L.filterSeason, value: seasonWords)) }
        if let airing = airingFact(now: now) { facts.append(airing) }
        if let alsoOnFact { facts.append(TitleFact(label: L.titleFactAlsoOn, value: alsoOnFact)) }
        return facts
    }

    /// The facts on one line, as a phone shows them: "Manga · Atelier Hotaru · Airing since 20
    /// Aug 2026", the dates last, so they are the first to go.
    func factsLine(now: Date = .now) -> String? {
        var parts = [basedOn, studio].compactMap { $0 }
        if let airing = airingFact(now: now) {
            parts.append("\(airing.label) \(airing.value.prefix(1).lowercased() + airing.value.dropFirst())")
        }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// The synopsis without AniList's inline spoilers (`~!…!~`), which nothing reveals yet.
    var shownSynopsis: String {
        guard synopsis.contains("~!") else { return synopsis }
        let stripped = synopsis.replacing(/~!.*?!~/.dotMatchesNewlines(), with: "")
        return stripped.replacing(/\ {2,}/, with: " ").trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

// MARK: - Related

extension Entry {
    /// The Related row's cards: every related title but the series' own seasons, the parent
    /// story first, then the prequels and sequels that are not TV in air order, then the rest in
    /// the relations' order, each relation by year.
    func relatedCards(excluding seasons: Set<AnilistId>) -> [LinkedTitle] {
        let rank: [RelationKind: Int] = [
            .parent: 0, .prequel: 1, .sequel: 1, .sideStory: 2, .spinOff: 3, .alternative: 4, .summary: 5,
            .compilation: 6, .contains: 7, .sameUniverse: 8, .character: 9, .adaptation: 10, .source: 11,
            .other: 12,
        ]
        let cards = (detail?.relations ?? []).filter { !seasons.contains($0.id) && $0.id != id }
        return cards.enumerated().sorted { a, b in
            let (ra, rb) = (rank[a.element.relation ?? .other] ?? 12, rank[b.element.relation ?? .other] ?? 12)
            if ra != rb { return ra < rb }
            let (ya, yb) = (a.element.year ?? .max, b.element.year ?? .max)
            if ya != yb { return ya < yb }
            return a.offset < b.offset
        }.map(\.element)
    }
}

// MARK: - The sheet

extension Ranking {
    /// "Highest rated, Summer 2026", "Most popular, all time": what follows the rank.
    var words: String {
        func said(_ scope: String) -> String {
            kind == .rated ? L.rankingHighestRated(scope: scope) : L.rankingMostPopular(scope: scope)
        }
        if allTime { return said(L.rankingAllTime) }
        if let season, let year {
            return said(L.seasonWithYear(season: TitleWords.season(season), year: String(year)))
        }
        return year.map { said(String($0)) }
            ?? (kind == .rated ? L.titleRankingRated : L.titleRankingPopular)
    }

    /// Season before year before all time; rated before popular within each.
    fileprivate var order: (Int, Int) {
        (allTime ? 2 : season != nil ? 0 : 1, kind == .rated ? 0 : 1)
    }
}

extension TitleDetail {
    /// All the rankings AniList gives, in the sheet's order.
    var orderedRankings: [Ranking] {
        rankings.enumerated().sorted { a, b in
            a.element.order != b.element.order ? a.element.order < b.element.order : a.offset < b.offset
        }.map(\.element)
    }

    /// The first twelve of the cast, in the engine's order.
    var shownCast: [CastMember] { Array(characters.prefix(12)) }
    var shownStaff: [StaffCredit] { Array(staff.prefix(8)) }
    var shownRecommendations: [LinkedTitle] { Array(recommendations.prefix(8)) }

    /// Nothing to show beyond the page's own: no rankings, cast, staff, recommendations or tags.
    var isEmpty: Bool {
        rankings.isEmpty && characters.isEmpty && staff.isEmpty && recommendations.isEmpty && tags.isEmpty
    }
}

extension CastMember {
    /// "Main · Ren's voice": the role, and the first Japanese voice when there is one.
    var roleLine: String {
        ([TitleWords.role(role)] + [voiceActors.first?.name].compactMap { $0 }).joined(separator: " · ")
    }
}

enum SpoilerWords {
    /// "Show 1 spoiler tag", "Show 3 spoiler tags", or with `shown`, "Hide spoiler tags".
    static func reveal(_ count: Int, shown: Bool) -> String {
        shown ? L.moreHideSpoilerTags : L.moreShowSpoilerTags(count: count)
    }
}

// MARK: - Episodes

extension Entry {
    /// When `episode` airs, by the title's broadcasts on its page.
    func airsAt(_ episode: UInt32) -> Date? {
        detail?.schedule.first { $0.episode == episode }.map { Date(timeIntervalSince1970: TimeInterval($0.airsAt)) }
    }

    /// The next broadcast still to air, by the schedule: the one episode the table adds a row
    /// for, which no source lists before it is out.
    func nextUnaired(now: Date = .now) -> EpisodeAiring? {
        detail?.schedule.first { TimeInterval($0.airsAt) > now.timeIntervalSince1970 }
    }

    /// The episode table, with the next episode still to air at its end, dated, when the source
    /// does not have it yet: titled "Episode 10" unless a row the source has names it.
    func episodeTable(_ rows: [EpisodeRow], now: Date = .now) -> [EpisodeRow] {
        var rows = rows.map { row -> EpisodeRow in
            guard row.airsAt == nil, let number = UInt32(row.number), let at = airsAt(number) else { return row }
            var dated = row
            dated.airsAt = Int64(at.timeIntervalSince1970)
            return dated
        }
        if let next = nextUnaired(now: now), !rows.contains(where: { $0.number == String(next.episode) }) {
            rows.append(EpisodeRow(
                number: String(next.episode), title: nil, thumbnail: nil, description: nil, durationSecs: nil,
                watched: 0, completed: false, kind: nil, skippable: false, airsAt: next.airsAt))
        } else if detail == nil, let next = nextEpisode, let seconds = nextAiringIn, seconds > 0,
                  !rows.isEmpty, !rows.contains(where: { $0.number == String(next) }) {
            // Before the page's schedule is here, the title's next broadcast from the list.
            rows.append(EpisodeRow(
                number: String(next), title: nil, thumbnail: nil, description: nil, durationSecs: nil,
                watched: 0, completed: false, kind: nil, skippable: false,
                airsAt: Int64(now.timeIntervalSince1970) + seconds))
        }
        return rows
    }
}

extension EpisodeRow {
    /// Whether it is still to air: dated later than now. Undated counts as out.
    func isUnaired(now: Date = .now) -> Bool {
        airsAt.map { TimeInterval($0) > now.timeIntervalSince1970 } ?? false
    }

    var airsDate: Date? {
        airsAt.map { Date(timeIntervalSince1970: TimeInterval($0)) }
    }
}

/// When an episode airs, in words: "Airs today 16:30", "Airs Thursday 16:30", "Airs 29 Oct,
/// 16:30" (`AirTime`).
enum AirsWords {
    static func airs(_ date: Date, now: Date = .now) -> String {
        L.episodeRowAirs(when: AirTime.words(date, now: now))
    }

    /// "Aired Thursday 15 October 2026", the Mac's help tag on an aired row.
    static func aired(_ date: Date) -> String {
        L.episodeAiredHelp(day: AirTime.day(date))
    }

    static func clock(_ date: Date) -> String {
        AirTime.clock(date)
    }
}

/// An AniList score out of 100, written as a percentage in the person's language: "82%".
enum Score {
    static func text(_ score: some BinaryInteger) -> String {
        (Double(score) / 100).formatted(.percent.precision(.fractionLength(0)).locale(AirTime.locale))
    }
}
