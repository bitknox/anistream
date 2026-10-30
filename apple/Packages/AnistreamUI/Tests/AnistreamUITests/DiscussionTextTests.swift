// A comment's spans become one sentence, with spoilers kept inside it.

import AnistreamKit
@testable import AnistreamUI
import Foundation
import Testing

struct DiscussionTextTests {
    private func span(_ text: String, spoiler: Bool = false, media: MediaKind? = nil) -> CommentSpan {
        CommentSpan(text: text, bold: false, italic: false, strike: false, spoiler: spoiler, link: nil, media: media)
    }

    @Test func aSpoilerStaysInTheSentenceAndRevealsInPlace() {
        let block = CommentBlock(spans: [span("Read ahead: "), span("the ferryman is her brother", spoiler: true)], quote: false)
        let hidden = DiscussionText.attributed(block, key: "1-0", revealed: [])
        #expect(String(hidden.characters) == "Read ahead: the ferryman is her brother")
        let run = hidden.runs.first { $0.link?.scheme == DiscussionText.spoilerScheme }
        #expect(run?.link?.host() == "1-0-0")
        let shown = DiscussionText.attributed(block, key: "1-0", revealed: ["1-0-0"])
        #expect(shown != hidden)
    }

    @Test func neighbouringSpoilerSpansAreOneSpoiler() {
        let block = CommentBlock(spans: [span("a", spoiler: true), span("b", spoiler: true), span(" c"), span("d", spoiler: true)], quote: false)
        let text = DiscussionText.attributed(block, key: "k", revealed: [])
        let hosts = text.runs.compactMap { $0.link?.host() }
        #expect(hosts == ["k-0", "k-1"])
    }

    @Test func mediaSpansAreLinesOfTheirOwn() {
        let block = CommentBlock(spans: [span("see "), CommentSpan(text: "", bold: false, italic: false, strike: false, spoiler: false, link: "https://pics.example/a.png", media: .image)], quote: false)
        #expect(String(DiscussionText.attributed(block, key: "k", revealed: []).characters) == "see ")
        #expect(DiscussionText.media(block).count == 1)
    }

    @Test func ageOfKeptCommentsIsWorded() {
        #expect(DiscussionText.age(minutes: 0).contains("1"))
        #expect(DiscussionText.age(minutes: 120).contains("2"))
    }
}
