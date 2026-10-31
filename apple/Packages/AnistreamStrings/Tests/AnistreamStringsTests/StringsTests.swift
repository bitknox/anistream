import Foundation
import Testing

@testable import AnistreamStrings

/// The generated catalogue, compiled by Xcode's own tools and read through `L`.
@Suite struct StringsTests {
    @Test func plainStringsComeFromTheCatalogue() {
        #expect(L.settingsLanguage == "Language")
        #expect(L.sourcesTitle == "Sources")
        #expect(L.legalHostsNoContent == "anistream hosts no content.")
    }

    @Test func termsAreWrittenIn() {
        #expect(L.pluginRiskWarning == "Plugins are written by other people and run on your device. Install only from people you trust.")
        #expect(L.audioSubbed == "Subbed")
    }

    @Test func aPluralFollowsItsCount() {
        #expect(L.queueEpisodesLeft(count: 1, title: "Sample") == "One episode left in Sample")
        #expect(L.queueEpisodesLeft(count: 5, title: "Sample") == "5 episodes left in Sample")
        #expect(L.episodesCount(count: 1) == "1 episode")
    }

    @Test func theCountThatPicksTheFormNeedNotBeTheFirstArgument() {
        #expect(L.progressEpisodes(watched: 6, total: 13) == "6 of 13 episodes")
        #expect(L.progressEpisodes(watched: 6, total: 1) == "6 of 1 episode")
        #expect(L.progressEpisodes(watched: 1, total: 13) == "1 of 13 episodes")
    }

    @Test func textArguments() {
        #expect(L.noticeSaved(label: "Language") == "Saved Language.")
        #expect(L.durationDaysHours(days: 6, hours: 17) == "6 d 17 h")
    }

    @Test func macMenuItemsAreTitleCaseAndOnlyThere() {
        #expect(L.menuShowLibrary == "Show Library")
    }

    @Test func languagesListSourceFirst() {
        #expect(L.languages.first == L.Language(tag: "en", name: "English", flag: "gb", draft: false))
    }

    @Test func everyBundledFlagIsFoundAndEveryLanguageFlagIsBundled() {
        let codes = ["gb", "se", "jp", "mx", "br", "fr", "de", "it", "pl", "ru", "tr", "id", "vn", "th", "kr", "cn", "tw"]
        for code in codes { #expect(L.flagURL(code) != nil, "flag-\(code).png") }
        for language in L.languages {
            if let flag = language.flag { #expect(L.flagURL(flag) != nil, "\(language.tag)") }
        }
        #expect(L.flagURL("xx") == nil)
    }
}
