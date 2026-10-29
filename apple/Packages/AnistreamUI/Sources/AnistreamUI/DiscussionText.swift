// A comment's text from the engine's spans: paragraphs, bold, italic, strike-through, links and
// spoilers that stay in the sentence. No markdown or HTML is read here; the engine parsed it.

import AnistreamDesign
import AnistreamKit
import AnistreamStrings
import SwiftUI

enum DiscussionText {
    /// The scheme a spoiler's run carries so a tap on it can be told from a link.
    static let spoilerScheme = "anistream-spoiler"

    /// The text of `block`, its media spans left out (they are lines of their own). A spoiler is
    /// a run of its neighbours, hidden until its key is in `revealed`.
    static func attributed(_ block: CommentBlock, key: String, revealed: Set<String>) -> AttributedString {
        var out = AttributedString()
        var run = -1
        var previousSpoiler = false
        for span in block.spans where span.media == nil {
            var text = AttributedString(span.text)
            var intent: InlinePresentationIntent = []
            if span.bold { intent.insert(.stronglyEmphasized) }
            if span.italic { intent.insert(.emphasized) }
            if !intent.isEmpty { text.inlinePresentationIntent = intent }
            if span.strike { text.strikethroughStyle = .single }
            if span.spoiler {
                if !previousSpoiler { run += 1 }
                let id = "\(key)-\(run)"
                text.link = URL(string: "\(spoilerScheme)://\(id)")
                if revealed.contains(id) {
                    text.backgroundColor = Palette.raisedSelected.opacity(0.55)
                    text.foregroundColor = Palette.silk
                } else {
                    text.backgroundColor = Palette.raisedSelected
                    text.foregroundColor = .clear
                }
                text.underlineStyle = nil
            } else if let link = span.link, let url = URL(string: link) {
                text.link = url
                text.underlineStyle = .single
                text.foregroundColor = Palette.silk
            }
            previousSpoiler = span.spoiler
            out += text
        }
        return out
    }

    /// The media spans of `block`, in order.
    static func media(_ block: CommentBlock) -> [CommentSpan] {
        block.spans.filter { $0.media != nil }
    }

    /// The comment's plain text, for Copy.
    static func plain(_ comment: CommentView) -> String {
        comment.markdown.isEmpty
            ? comment.body.map { $0.spans.map(\.text).joined() }.joined(separator: "\n\n")
            : comment.markdown
    }

    /// Whether a comment is long enough to fold behind More, by its characters: about ten lines.
    static func isLong(_ comment: CommentView) -> Bool {
        comment.body.reduce(0) { $0 + $1.spans.reduce(0) { $0 + $1.text.count } } > 520
            || comment.body.count > 6
    }

    /// "2 h" worded from the minutes the comments were kept for.
    static func age(minutes: UInt32) -> String {
        switch minutes {
        case ..<60: L.discussionAgeMinutes(minutes: Int(max(minutes, 1)))
        case ..<1440: L.discussionAgeHours(hours: Int(minutes / 60))
        default: L.discussionAgeDays(days: Int(minutes / 1440))
        }
    }
}
