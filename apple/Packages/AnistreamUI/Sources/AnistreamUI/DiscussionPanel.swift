// An episode's discussion: the header, the comments and the field, in whatever holds them (an
// iPhone sheet, the iPad's side panel, the Mac's window). Follows the Discussions board.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// What the viewer is writing, and which comment it answers or replaces.
@Observable @MainActor
final class DiscussionDraft {
    var text = ""
    var replying: CommentView?
    var editing: CommentView?
    var deleting: CommentView?
    /// Spoilers opened for this visit, by comment and run.
    var revealed: Set<String> = []
    /// Replies unfolded under "3 more replies", by the reply they hang from.
    var expanded: Set<UInt32> = []
    /// Long comments opened with More.
    var unfolded: Set<UInt32> = []

    var hasText: Bool { !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    func reply(to comment: CommentView) {
        editing = nil
        replying = comment
    }

    func edit(_ comment: CommentView) {
        replying = nil
        editing = comment
        text = comment.markdown
    }

    func cancel() {
        if editing != nil { text = "" }
        replying = nil
        editing = nil
    }

    func toggle(spoiler id: String) {
        if revealed.contains(id) { revealed.remove(id) } else { revealed.insert(id) }
    }

    /// Wrap the words in AniList's spoiler marks: the whole field, as SwiftUI gives no selection.
    func markSpoiler() {
        guard hasText else { return }
        text = "~!\(text)!~"
    }
}

/// Where the panel sits, which sets its header and its sizes.
enum DiscussionChrome {
    case sheet, inspector, window
}

struct DiscussionPanel: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let key: DiscussionKey
    let chrome: DiscussionChrome
    var close: (() -> Void)?

    @State private var draft = DiscussionDraft()
    /// Nothing shows for 600 ms, then placeholders at final size.
    @State private var silent = true
    @State private var slow = false
    @FocusState private var focused: Bool

    private var view: DiscussionView? {
        store.discussion.open.flatMap { $0.media == key.media && $0.episode == key.episode ? $0 : nil }
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            content
            composer
        }
        .background(.ground)
        .foregroundStyle(.silk)
        .environment(\.openURL, OpenURLAction { url in
            if url.scheme == DiscussionText.spoilerScheme {
                withAnimation(.quick) { draft.toggle(spoiler: url.host() ?? "") }
                return .handled
            }
            return .systemAction
        })
        .task(id: key) {
            silent = true
            slow = false
            store.openDiscussion(key)
            try? await Task.sleep(for: .milliseconds(600))
            silent = false
            try? await Task.sleep(for: .milliseconds(3400))
            slow = true
        }
        .onDisappear { store.closeDiscussion() }
        .alert(L.discussionDeleteTitle, isPresented: Binding(
            get: { draft.deleting != nil }, set: { if !$0 { draft.deleting = nil } }
        )) {
            Button(L.actionDelete, role: .destructive) {
                if let comment = draft.deleting { store.deleteComment(comment.id) }
                draft.deleting = nil
            }
            Button(L.actionCancel, role: .cancel) { draft.deleting = nil }
        } message: {
            Text(L.discussionDeleteBody)
        }
        #if os(macOS)
        .onExitCommand { draft.cancel() }
        #endif
    }

    // MARK: Header

    private var count: UInt32 {
        view.map(\.commentCount) ?? store.discussion.count(key.media, episode: key.episode) ?? 0
    }

    private var showTitle: String? { store.titles[key.media]?.showTitle }

    @ViewBuilder
    private var header: some View {
        #if os(macOS)
        let touch = false
        #else
        let touch = true
        #endif
        VStack(spacing: 0) {
            if touch {
                ZStack {
                    VStack(spacing: 0) {
                        Text(L.discussionTitleEpisode(episode: Int(key.episode)))
                            .font(.ui(17, weight: .semibold))
                        if let showTitle {
                            Text(showTitle).font(.showTitle(size: 12)).foregroundStyle(.haze)
                        }
                    }
                    .lineLimit(1)
                    .accessibilityElement(children: .combine)
                    .accessibilityAddTraits(.isHeader)
                    HStack {
                        if chrome != .inspector { closeButton }
                        Spacer(minLength: 0)
                        if chrome == .inspector { closeButton }
                    }
                }
                .frame(height: 44)
                .padding(.horizontal, 16)
                .padding(.top, chrome == .sheet ? 12 : 8)
            }
            HStack {
                Text(count > 0 ? L.discussionCount(count: Int(count)) : " ")
                    .font(.plat(mac: 13, touch: 15))
                    .foregroundStyle(.haze)
                    .monospacedDigit()
                Spacer(minLength: 8)
                if let view, let url = URL(string: view.url) {
                    Button(L.titleOpenAnilist) { openURL(url) }
                        .buttonStyle(.plain)
                        .font(.plat(mac: 13, touch: 15, weight: .semibold))
                        .foregroundStyle(.silk)
                        .frame(minHeight: touch ? 44 : 28)
                        .contentShape(.rect)
                        .pointerHover()
                }
            }
            .padding(.horizontal, touch ? 16 : 20)
            .frame(height: touch ? 40 : 40)
            if let view, case .offline(let minutes) = view.state, !view.comments.isEmpty {
                Text(L.discussionOfflineCached(age: DiscussionText.age(minutes: minutes)))
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, touch ? 16 : 20)
                    .padding(.bottom, 6)
            }
            if !touch { Rectangle().fill(.faintRule).frame(height: 1) }
        }
    }

    private var closeButton: some View {
        Button {
            close?()
        } label: {
            Image(systemName: "xmark")
                .font(.system(size: 14, weight: .semibold))
                .frame(width: 44, height: 44)
                .glassEffect(.regular.interactive(), in: .circle)
                .contentShape(.circle)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(L.discussionAppleClose)
    }

    // MARK: Content

    @ViewBuilder
    private var content: some View {
        Group {
            switch view?.state ?? .loading {
            case .loading:
                placeholders(note: slow ? L.discussionStillLoading : nil)
            case .rateLimited(let seconds):
                placeholders(note: L.discussionAppleWaiting(time: "\(seconds) s"))
            case .ready(let more):
                if let view, view.comments.isEmpty {
                    EmptyLine(title: L.discussionNoComments)
                } else if let view {
                    thread(view, more: more)
                }
            case .offline:
                if let view, !view.comments.isEmpty {
                    thread(view, more: false)
                } else {
                    unavailable(title: L.discussionAppleOffline, symbol: "wifi.slash", retry: true)
                }
            case .noThread:
                noThread
            case .failed:
                ScreenFailed(title: L.discussionFailed, reason: "") { store.openDiscussion(key) }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .animation(reduceMotion ? nil : .fadeIn, value: phase)
    }

    /// What the content area shows, so a change between them crossfades.
    private var phase: Int {
        switch view?.state ?? .loading {
        case .loading, .rateLimited: silent ? 0 : 1
        case .ready: 2
        case .offline: 3
        case .noThread: 4
        case .failed: 5
        }
    }

    @ViewBuilder
    private func placeholders(note: String?) -> some View {
        if silent {
            Color.clear
        } else {
            VStack(spacing: 0) {
                if let note {
                    Text(note)
                        .font(.plat(mac: 13, touch: 15))
                        .foregroundStyle(.haze)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.horizontal, chrome == .window ? 20 : 16)
                        .padding(.bottom, 8)
                        .transition(.opacity)
                }
                ForEach(0..<4, id: \.self) { _ in CommentPlaceholder() }
                Spacer(minLength: 0)
            }
            .transition(.opacity)
            .accessibilityHidden(true)
        }
    }

    private var noThread: some View {
        ContentUnavailableView {
            Label(L.discussionAppleNoThread, systemImage: "bubble.left.and.bubble.right")
                .foregroundStyle(.silk)
        } actions: {
            if let view, let url = URL(string: view.url) {
                Button(L.titleOpenAnilist) { openURL(url) }
                    .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
            }
        }
    }

    private func unavailable(title: String, symbol: String, retry: Bool) -> some View {
        ContentUnavailableView {
            Label(title, systemImage: symbol).foregroundStyle(.silk)
        } actions: {
            if retry {
                Button(L.actionTryAgain) { store.openDiscussion(key) }
                    .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
            }
        }
    }

    // MARK: The thread

    /// One line of the flattened thread.
    private struct Line: Identifiable {
        enum Kind { case comment(CommentView), more(CommentView) }
        let id: String
        let kind: Kind
        let depth: Int
        let parent: UInt32?
        let to: String?
    }

    private func lines(_ view: DiscussionView) -> [Line] {
        var out: [Line] = []
        for top in view.comments {
            out.append(Line(id: "c\(top.id)", kind: .comment(top), depth: 0, parent: nil, to: nil))
            for reply in top.replies {
                out.append(Line(id: "c\(reply.id)", kind: .comment(reply), depth: 1, parent: top.id, to: nil))
                if reply.hiddenReplies > 0 || !reply.replies.isEmpty {
                    if draft.expanded.contains(reply.id) {
                        for deep in reply.replies {
                            out.append(Line(id: "c\(deep.id)", kind: .comment(deep), depth: 2, parent: reply.id, to: reply.author))
                        }
                    } else {
                        out.append(Line(id: "m\(reply.id)", kind: .more(reply), depth: 1, parent: top.id, to: nil))
                    }
                }
            }
        }
        return out
    }

    private func thread(_ view: DiscussionView, more: Bool) -> some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    let all = lines(view)
                    ForEach(Array(all.enumerated()), id: \.element.id) { index, line in
                        switch line.kind {
                        case .comment(let comment):
                            if line.depth == 0, index > 0 {
                                Rectangle().fill(.faintRule).frame(height: 1)
                                    .padding(.leading, DiscussionMetrics.column)
                            }
                            CommentRow(
                                comment: comment, depth: line.depth, to: line.to, parent: line.parent,
                                view: view, draft: draft, thread: view.url
                            )
                            .id(line.id)
                            .transition(.opacity)
                        case .more(let reply):
                            MoreReplies(count: Int(reply.hiddenReplies)) {
                                withAnimation(reduceMotion ? nil : .standard) { _ = draft.expanded.insert(reply.id) }
                            }
                        }
                    }
                    if more {
                        VStack(spacing: 0) { CommentPlaceholder() }
                            .onAppear { store.moreComments() }
                            .accessibilityHidden(true)
                    }
                }
                .animation(reduceMotion ? nil : .fadeIn, value: view.comments.map(\.id))
                .padding(.bottom, 8)
            }
            .scrollIndicators(.hidden)
            .onChange(of: draft.replying?.id) { _, id in
                if let id { withAnimation(.standard) { proxy.scrollTo("c\(id)", anchor: .center) } }
            }
        }
    }

    // MARK: The field

    @ViewBuilder
    private var composer: some View {
        if let view, view.state != .noThread, view.state != .failed {
            VStack(spacing: 2) {
                if !view.signedIn {
                    Button(L.discussionSignIn) { store.connect("anilist") }
                        .buttonStyle(SecondaryCapsuleStyle(height: ScreenFailed.buttonHeight, fontSize: ScreenFailed.buttonFont))
                        .frame(maxWidth: .infinity)
                } else if !view.canPost {
                    Text(L.discussionOfflinePost)
                        .font(.plat(mac: 13, touch: 15))
                        .foregroundStyle(.haze)
                        .frame(maxWidth: .infinity, minHeight: 44)
                } else {
                    fieldRow
                }
            }
            .padding(.horizontal, chrome == .window ? 16 : 16)
            .padding(.top, 8)
            .padding(.bottom, chrome == .window ? 12 : 10)
            .background(.ground)
            .overlay(alignment: .top) { Rectangle().fill(.faintRule).frame(height: 1) }
        }
    }

    private var fieldRow: some View {
        VStack(spacing: 2) {
            if let context {
                HStack {
                    Text(context)
                        .font(.plat(mac: 12, touch: 13))
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                    Spacer(minLength: 0)
                    Button { draft.cancel() } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(.haze)
                            .frame(width: 44, height: 28)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(L.discussionCancelReply)
                }
                .padding(.leading, 12)
                .transition(.opacity)
            }
            HStack(alignment: .bottom, spacing: 8) {
                TextField(
                    draft.editing != nil ? L.discussionEditComment : L.discussionAddComment,
                    text: $draft.text, axis: .vertical
                )
                .textFieldStyle(.plain)
                .lineLimit(1...5)
                .focused($focused)
                .font(.plat(mac: 13, touch: 17))
                .padding(.horizontal, DiscussionMetrics.fieldInset)
                .padding(.vertical, DiscussionMetrics.fieldPad)
                .frame(minHeight: DiscussionMetrics.control)
                .background(fieldBackground)
                .contextMenu {
                    Button(L.discussionAppleMarkSpoiler) { draft.markSpoiler() }
                        .disabled(!draft.hasText)
                }
                .onChange(of: draft.replying?.id) { _, id in if id != nil { focused = true } }
                .onChange(of: draft.editing?.id) { _, id in if id != nil { focused = true } }
                Button(draft.editing != nil ? L.actionSave : L.discussionPost) { submit() }
                    .buttonStyle(PostStyle(enabled: draft.hasText, height: DiscussionMetrics.control))
                    .disabled(!draft.hasText)
                    #if os(macOS)
                    .keyboardShortcut(.return, modifiers: .command)
                    #endif
            }
            #if os(macOS)
            Text(L.discussionAppleKeyHint)
                .font(.mac(11))
                .foregroundStyle(.haze)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.top, 2)
            #endif
        }
        .animation(.quick, value: context)
    }

    @ViewBuilder
    private var fieldBackground: some View {
        #if os(macOS)
        RoundedRectangle(cornerRadius: 6).fill(Palette.raised)
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Palette.rule, lineWidth: 1))
        #else
        Capsule().fill(Palette.glass)
            .overlay(Capsule().stroke(Palette.glassEdge, lineWidth: 1))
        #endif
    }

    private var context: String? {
        if draft.editing != nil { return L.discussionAppleEditing }
        if let replying = draft.replying { return L.discussionReplyingTo(name: replying.author) }
        return nil
    }

    private func submit() {
        let text = draft.text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        if let editing = draft.editing {
            store.editComment(editing.id, text: text)
        } else {
            store.postComment(text, parent: draft.replying?.id)
        }
        draft.text = ""
        draft.replying = nil
        draft.editing = nil
    }
}

/// Post: amber once there is text, the only amber in the discussion, raised and dim before.
struct PostStyle: ButtonStyle {
    let enabled: Bool
    let height: CGFloat

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.plat(mac: 13, touch: 17, weight: .semibold))
            .foregroundStyle(enabled ? Color.ground : Color.silk.opacity(0.4))
            .padding(.horizontal, 18)
            .frame(minHeight: height)
            .background(
                (enabled ? Palette.amber.opacity(configuration.isPressed ? 0.85 : 1) : Palette.raised),
                in: .capsule
            )
            .animation(.quick, value: enabled)
            .contentShape(.capsule)
    }
}

enum DiscussionMetrics {
    #if os(macOS)
    static let avatar: CGFloat = 28
    static let replyAvatar: CGFloat = 18
    static let column: CGFloat = 56
    static let control: CGFloat = 28
    static let fieldInset: CGFloat = 8
    static let fieldPad: CGFloat = 5
    static let actions: CGFloat = 28
    #else
    static let avatar: CGFloat = 32
    static let replyAvatar: CGFloat = 24
    static let column: CGFloat = 60
    static let control: CGFloat = 44
    static let fieldInset: CGFloat = 16
    static let fieldPad: CGFloat = 11
    static let actions: CGFloat = 36
    #endif
}

private struct EmptyLine: View {
    let title: String

    var body: some View {
        ContentUnavailableView {
            Label(title, systemImage: "bubble.left").foregroundStyle(.silk)
        }
    }
}

struct MoreReplies: View {
    let count: Int
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Text(L.discussionMoreReplies(count: count))
                Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold))
            }
            .font(.plat(mac: 11, touch: 13, weight: .semibold))
            .foregroundStyle(.haze)
            .frame(minHeight: 32)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .padding(.leading, DiscussionMetrics.column + DiscussionMetrics.replyAvatar + 10)
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A comment before it arrives: the avatar's circle and three lines of text, at final size.
struct CommentPlaceholder: View {
    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Circle().fill(Palette.artPlaceholder).frame(width: DiscussionMetrics.avatar, height: DiscussionMetrics.avatar)
            VStack(alignment: .leading, spacing: 8) {
                bar(width: 90)
                bar(width: nil)
                bar(width: nil)
                bar(width: 160)
            }
            .padding(.top, 6)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
    }

    private func bar(width: CGFloat?) -> some View {
        RoundedRectangle(cornerRadius: 4).fill(Palette.artPlaceholder)
            .frame(width: width, height: 10)
            .frame(maxWidth: width == nil ? .infinity : width, alignment: .leading)
    }
}
