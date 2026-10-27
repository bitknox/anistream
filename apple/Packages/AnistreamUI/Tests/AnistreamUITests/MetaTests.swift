// AniList's metadata as the screens say it (native-spec/metadata.md).

import AnistreamKit
@testable import AnistreamUI
import Foundation
import Testing

func detail(
    tags: [TitleTag] = [], schedule: [EpisodeAiring] = [], relations: [LinkedTitle] = [],
    seasons: [LinkedTitle]? = nil, recommendations: [LinkedTitle] = [], rankings: [Ranking] = [],
    characters: [CastMember] = [], staff: [StaffCredit] = []
) -> TitleDetail {
    TitleDetail(tags: tags, schedule: schedule, relations: relations, seasons: seasons,
                recommendations: recommendations, rankings: rankings, characters: characters, staff: staff)
}

func linked(_ id: AnilistId, _ relation: RelationKind?, _ format: String, _ year: UInt16?) -> LinkedTitle {
    LinkedTitle(id: id, title: "Title \(id)", titleRomaji: "Romaji \(id)", titleEnglish: "Title \(id)",
                titleNative: nil, relation: relation, format: format, coverUrl: nil, coverColor: nil, season: nil,
                year: year, listStatus: nil)
}

func tag(_ name: String, spoiler: Bool = false) -> TitleTag {
    TitleTag(name: name, rank: 80, mediaSpoiler: spoiler, generalSpoiler: false, category: nil)
}

@Suite(.britishEnglish)
struct MetaTests {
    /// Wednesday 21 October 2026, at noon where the test runs.
    let now: Date = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        return calendar.date(from: DateComponents(year: 2026, month: 10, day: 21, hour: 12))!
    }()

    @Test func theLineUnderTheTitleHoldsTheOtherFormsInOrder() {
        // The engine names the title in the chosen form, here romaji.
        let e = entry {
            $0.title = "Hoshikuzu no Tabibito"
            $0.titleRomaji = "Hoshikuzu no Tabibito"
            $0.titleEnglish = "Stardust Wayfarer"
            $0.titleNative = "星屑の旅人"
        }
        #expect(e.otherTitles == ["Stardust Wayfarer", "星屑の旅人"])
        let english = entry {
            $0.titleRomaji = "Hoshikuzu no Tabibito"
            $0.titleEnglish = "Stardust Wayfarer"
            $0.titleNative = "星屑の旅人"
        }
        #expect(english.otherTitles == ["Hoshikuzu no Tabibito", "星屑の旅人"], "English chosen, romaji first beneath")
        let same = entry {
            $0.title = "Hoshikuzu no Tabibito"
            $0.titleRomaji = "Hoshikuzu no Tabibito"
            $0.titleEnglish = "Hoshikuzu no Tabibito"
            $0.titleNative = nil
        }
        #expect(same.otherTitles.isEmpty, "the shown form, a repeat and a missing form go")
    }

    @Test func factsComeInOrderAndLeaveOutWhatAniListLacks() {
        let e = entry {
            $0.studio = "Atelier Hotaru"
            $0.source = .manga
            $0.season = .summer
            $0.year = 2026
            $0.startDate = FuzzyDate(year: 2026, month: 8, day: 20)
            $0.availableOn = ["Hikari Stream", "Lantern Stream"]
        }
        #expect(e.facts(now: now).map(\.label) == ["Studio", "Based on", "Season", "Airing", "Also on"])
        #expect(e.facts(now: now).map(\.value) == [
            "Atelier Hotaru", "Manga", "Summer 2026", "Since 20 Aug 2026", "Hikari Stream and 1 more",
        ])
        #expect(e.factsLine(now: now) == "Manga · Atelier Hotaru · Airing since 20 Aug 2026")
        #expect(entry { $0.source = .other }.basedOn == nil, "Other is left out")
        #expect(entry { $0.year = 2026; $0.season = nil }.seasonWords == "2026")
    }

    @Test func theAiringStateComesFromTheDates() {
        let finished = entry {
            $0.startDate = FuzzyDate(year: 2024, month: 4, day: 4)
            $0.endDate = FuzzyDate(year: 2024, month: 6, day: 22)
        }
        #expect(finished.airingFact(now: now) == TitleFact(label: "Aired", value: "4 Apr – 22 Jun 2024"))
        let film = entry {
            $0.startDate = FuzzyDate(year: 2025, month: 3, day: 7)
            $0.endDate = FuzzyDate(year: 2025, month: 3, day: 7)
        }
        #expect(film.airingFact(now: now)?.value == "7 Mar 2025", "one day")
        let upcoming = entry { $0.startDate = FuzzyDate(year: 2027, month: 1, day: nil) }
        #expect(upcoming.airingFact(now: now) == TitleFact(label: "Airs", value: "From Jan 2027"))
        #expect(entry { _ in }.airingFact(now: now) == nil, "no dates, no row")
        let acrossYears = entry {
            $0.startDate = FuzzyDate(year: 2023, month: 10, day: 1)
            $0.endDate = FuzzyDate(year: 2024, month: 3, day: nil)
        }
        #expect(acrossYears.airingFact(now: now)?.value == "1 Oct 2023 – Mar 2024")
    }

    @Test func tagsOnThePageGiveNothingAway() {
        let e = entry {
            $0.detail = detail(tags: [tag("Travel"), tag("Twist Ending", spoiler: true), tag("Found Family"),
                                      tag("Lamplighters"), tag("Episodic")])
        }
        #expect(e.topTags == ["Travel", "Found Family", "Lamplighters"])
        #expect(e.safeTags.count == 4)
        #expect(e.spoilerTags == ["Twist Ending"])
        #expect(SpoilerWords.reveal(1, shown: false, titleCase: false) == "Show 1 spoiler tag")
        #expect(SpoilerWords.reveal(3, shown: false, titleCase: true) == "Show 3 Spoiler Tags")
        #expect(SpoilerWords.reveal(3, shown: true, titleCase: false) == "Hide spoiler tags")
    }

    @Test func theSynopsisLosesItsInlineSpoilers() {
        let e = entry { $0.synopsis = "A lamplighter walks. ~!She is the last one.!~ The lantern runs low." }
        #expect(e.shownSynopsis == "A lamplighter walks. The lantern runs low.")
    }

    @Test func relatedCardsGoParentFirstAndLeaveTheSeasonsToThePicker() {
        let e = entry {
            $0.detail = detail(relations: [
                linked(2, .sequel, "TV", 2027),
                linked(3, .sideStory, "MOVIE", 2025),
                linked(4, .spinOff, "ONA", 2026),
                linked(5, .sideStory, "OVA", 2024),
                linked(6, .parent, "TV", 2020),
                linked(7, .other, "MUSIC", 2026),
            ])
        }
        let cards = e.relatedCards(excluding: [2])
        #expect(cards.map(\.id) == [6, 5, 3, 4, 7])
        #expect(cards[1].relationLine == "Side story · OVA · 2024")
        #expect(cards.last?.relationLine == "Related · Music · 2026")
        #expect(cards[0].shownTitle == "Title 6", "the engine's, in the chosen form")
    }

    @Test func rankingsGoSeasonThenYearThenAllTime() {
        let d = detail(rankings: [
            Ranking(rank: 31, kind: .popular, context: "", allTime: true, season: nil, year: nil),
            Ranking(rank: 19, kind: .rated, context: "", allTime: false, season: nil, year: 2026),
            Ranking(rank: 6, kind: .popular, context: "", allTime: false, season: .summer, year: 2026),
            Ranking(rank: 4, kind: .rated, context: "", allTime: false, season: .summer, year: 2026),
        ])
        #expect(d.orderedRankings.map { "#\($0.rank) \($0.words)" } == [
            "#4 Highest rated, Summer 2026", "#6 Most popular, Summer 2026", "#19 Highest rated, 2026",
            "#31 Most popular, all time",
        ])
        let ren = CastMember(name: "Ren Amagiri", imageUrl: nil, role: .main,
                             voiceActors: [Person(name: "Ren's voice", imageUrl: nil)])
        #expect(ren.roleLine == "Main · Ren's voice")
        #expect(CastMember(name: "Ferry Passenger", imageUrl: nil, role: .background, voiceActors: []).roleLine == "Background")
    }

    @Test func theNextEpisodeStillToAirGetsARowFromTheSchedule() {
        let at = Int64(now.timeIntervalSince1970)
        let e = entry {
            $0.detail = detail(schedule: [
                EpisodeAiring(episode: 9, airsAt: at - 6 * 86_400),
                EpisodeAiring(episode: 10, airsAt: at + 2 * 86_400),
                EpisodeAiring(episode: 11, airsAt: at + 9 * 86_400),
            ])
        }
        func row(_ number: String) -> EpisodeRow {
            EpisodeRow(number: number, title: "Episode title \(number)", thumbnail: nil, description: nil,
                       durationSecs: 1440, watched: 0, completed: false, kind: nil, skippable: false, airsAt: nil)
        }
        let table = e.episodeTable([row("8"), row("9")], now: now)
        #expect(table.map(\.number) == ["8", "9", "10"], "only the next one")
        #expect(table[1].airsAt == at - 6 * 86_400, "a source's row is dated by the schedule")
        #expect(!table[1].isUnaired(now: now))
        #expect(table[2].isUnaired(now: now))
        #expect(table[2].title == nil)
        #expect(!row("8").isUnaired(now: now), "undated counts as out")
    }

    @Test func anAirTimeIsTodayTomorrowAWeekdayOrADate() {
        let calendar = Calendar.current
        let at = { (days: Int, hour: Int) in
            calendar.date(bySettingHour: hour, minute: 30, second: 0, of: calendar.date(byAdding: .day, value: days, to: now)!)!
        }
        let clock = AirsWords.clock(at(0, 16))
        #expect(AirsWords.airs(at(0, 16), now: now) == "Airs today \(clock)")
        #expect(AirsWords.airs(at(1, 16), now: now) == "Airs tomorrow \(clock)")
        #expect(AirsWords.airs(at(2, 16), now: now) == "Airs Friday \(clock)")
        #expect(AirsWords.airs(at(8, 16), now: now) == "Airs 29 Oct, \(clock)")
        #expect(AirsWords.aired(at(-6, 16)) == "Aired Thursday, 15 October 2026")
    }
}
