// One comment in a discussion: avatar, name and age, the text, media lines, and the heart and Reply.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

struct CommentRow: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let comment: CommentView
    /// 0 for a top-level comment, 1 for a reply, 2 for a reply to a reply.
    let depth: Int
    let to: String?
    let parent: UInt32?
    let view: DiscussionView
    let draft: DiscussionDraft
    /// The thread's address, which a comment's own is made from.
    let thread: String
    @State private var hovering = false

    private var small: CGFloat { depth == 0 ? DiscussionMetrics.avatar : DiscussionMetrics.replyAvatar }
    private var gap: CGFloat { depth == 0 ? 12 : 10 }
    private var selected: Bool { draft.replying?.id == comment.id || draft.editing?.id == comment.id }
    private var dimmed: Bool { comment.pending != .posted }

    var body: some View {
        HStack(alignment: .top, spacing: gap) {
            CommentAvatar(url: comment.avatar, name: comment.author, size: small)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 0) {
                nameLine
                text
                actions
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.leading, depth == 0 ? 16 : DiscussionMetrics.column)
        .padding(.trailing, 16)
        .padding(.top, 10)
        .background(selected ? Palette.raised : .clear)
        .opacity(dimmed ? 0.5 : 1)
        .animation(.quick, value: dimmed)
        .animation(.quick, value: selected)
        .contentShape(.rect)
        .contextMenu { menu }
        #if os(macOS)
        .onHover { hovering = $0 }
        #endif
        .accessibilityElement(children: .contain)
    }

    private var nameLine: some View {
        HStack(alignment: .firstTextBaseline, spacing: 6) {
            Text(comment.author).fontWeight(.semibold)
            Text(comment.pending == .sending ? L.discussionAppleSending : comment.age.text)
                .foregroundStyle(.haze)
                .monospacedDigit()
            if let to {
                Text(L.discussionAppleAnswering(name: to)).foregroundStyle(.haze)
            }
        }
        .font(.plat(mac: 12, touch: 15))
        .lineLimit(1)
        .padding(.top, 2)
    }

    @ViewBuilder
    private var text: some View {
        let folded = DiscussionText.isLong(comment) && !draft.unfolded.contains(comment.id)
        VStack(alignment: .leading, spacing: 6) {
            ForEach(Array(comment.body.enumerated()), id: \.offset) { index, block in
                let key = "\(comment.id)-\(index)"
                let words = DiscussionText.attributed(block, key: key, revealed: draft.revealed)
                if !words.characters.isEmpty {
                    Text(words)
                        .font(.plat(mac: 13, touch: 17))
                        .foregroundStyle(block.quote ? .haze : .silk)
                        .tint(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.leading, block.quote ? 10 : 0)
                        .overlay(alignment: .leading) {
                            if block.quote { Rectangle().fill(.rule).frame(width: 1) }
                        }
                }
                ForEach(Array(DiscussionText.media(block).enumerated()), id: \.offset) { _, span in
                    MediaLine(span: span, inline: view.loadImages)
                }
            }
        }
        .lineLimit(folded ? 10 : nil)
        .padding(.top, 4)
        if DiscussionText.isLong(comment) {
            Button(draft.unfolded.contains(comment.id) ? L.shellLess : L.actionMore) {
                withAnimation(reduceMotion ? nil : .standard) {
                    if draft.unfolded.contains(comment.id) { draft.unfolded.remove(comment.id) } else { draft.unfolded.insert(comment.id) }
                }
            }
            .buttonStyle(.plain)
            .font(.plat(mac: 11, touch: 13, weight: .semibold))
            .foregroundStyle(.haze)
            .frame(minHeight: 28)
        }
    }

    private var actions: some View {
        HStack(spacing: 20) {
            like
            if view.canPost, comment.pending == .posted {
                Button(L.discussionReply) { draft.reply(to: comment) }
                    .buttonStyle(.plain)
                    .fontWeight(.semibold)
                    .foregroundStyle(.haze)
            }
            if comment.pending == .failed {
                Label {
                    Text(L.discussionPostFailed.components(separatedBy: ". ").first ?? L.discussionPostFailed)
                } icon: {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.vermilion)
                }
                .foregroundStyle(.silk)
                Button(L.actionTryAgain) { store.postComment(comment.markdown, parent: parent) }
                    .buttonStyle(.plain)
                    .fontWeight(.semibold)
                    .foregroundStyle(.silk)
            }
            Spacer(minLength: 0)
            if comment.pending == .posted, comment.own || hovering {
                Menu { menu } label: {
                    Image(systemName: "ellipsis")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.haze)
                        .frame(width: 28, height: 28)
                        .background(hovering ? Palette.silk.opacity(0.14) : .clear, in: .circle)
                        .contentShape(.circle)
                }
                .menuStyle(.button)
                .menuIndicator(.hidden)
                .buttonStyle(.plain)
                .accessibilityLabel(L.discussionAppleCommentMenu)
            }
        }
        .font(.plat(mac: 11, touch: 13))
        .foregroundStyle(.haze)
        .frame(minHeight: DiscussionMetrics.actions)
    }

    @ViewBuilder
    private var like: some View {
        let heart = HStack(spacing: 6) {
            Image(systemName: comment.liked ? "heart.fill" : "heart")
                .font(.system(size: DiscussionMetrics.actions > 30 ? 14 : 12))
            Text("\(comment.likeCount)")
                .monospacedDigit()
                .contentTransition(.numericText(value: Double(comment.likeCount)))
                .animation(.standard, value: comment.likeCount)
        }
        .foregroundStyle(comment.liked ? Color.silk : Color.haze)
        if view.canPost, comment.pending == .posted {
            Button { store.likeComment(comment.id) } label: {
                heart.frame(minHeight: DiscussionMetrics.actions).contentShape(.rect)
            }
            .buttonStyle(.plain)
            #if os(iOS)
            .sensoryFeedback(.impact(weight: .light), trigger: comment.liked)
            #endif
            .accessibilityLabel(comment.liked ? L.discussionAppleLiked : L.discussionLike)
            .accessibilityValue("\(comment.likeCount)")
        } else {
            heart.accessibilityElement(children: .combine)
        }
    }

    @ViewBuilder
    private var menu: some View {
        if view.canPost, comment.pending == .posted {
            Button(L.discussionReply) { draft.reply(to: comment) }
        }
        Button(L.discussionAppleCopyText) { Pasteboard.copy(DiscussionText.plain(comment)) }
        if let url = URL(string: "\(thread)/comment/\(comment.id)") {
            Button(L.titleOpenAnilist) { openURL(url) }
        }
        if comment.own, comment.pending == .posted {
            Divider()
            Button(L.actionEdit) { draft.edit(comment) }
            Button(L.actionDelete, role: .destructive) { draft.deleting = comment }
        }
    }
}

/// The author's picture, round, with their initial on a muted tone until it arrives or when there
/// is none.
struct CommentAvatar: View {
    let url: String?
    let name: String
    let size: CGFloat

    var body: some View {
        ZStack {
            Text(name.first.map { String($0).uppercased() } ?? "")
                .font(.system(size: size * 0.4, weight: .semibold))
                .foregroundStyle(.silk)
                .frame(width: size, height: size)
                .background(Palette.raisedSelected, in: .circle)
            if let url {
                ArtworkView(url: url, maxPixelSize: Int(size * 3), flat: .clear)
                    .frame(width: size, height: size)
                    .clipShape(.circle)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// An image or video in a comment: a line naming where it is, which opens it in the browser. With
/// Load images on, an image shows itself, at most 240 high.
struct MediaLine: View {
    @Environment(\.openURL) private var openURL
    let span: CommentSpan
    let inline: Bool

    var body: some View {
        if let link = span.link, let url = URL(string: link) {
            if span.media == .image, inline {
                AsyncImage(url: url) { image in
                    image.resizable().scaledToFit()
                } placeholder: {
                    Rectangle().fill(Palette.artPlaceholder).frame(height: 120)
                }
                .frame(maxHeight: 240, alignment: .leading)
                .clipShape(.rect(cornerRadius: 8))
                .onTapGesture { openURL(url) }
            } else {
                Button { openURL(url) } label: {
                    HStack(spacing: 8) {
                        Image(systemName: span.media == .video ? "play.rectangle" : "photo")
                        Text(words(url))
                    }
                    .font(.plat(mac: 12, touch: 15))
                    .foregroundStyle(.haze)
                    .frame(minHeight: 32, alignment: .leading)
                    .contentShape(.rect)
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func words(_ url: URL) -> String {
        guard let host = url.host() else { return span.media == .video ? L.discussionVideo : L.discussionImage }
        return span.media == .video ? L.discussionAppleVideoHost(host: host) : L.discussionAppleImageHost(host: host)
    }
}
