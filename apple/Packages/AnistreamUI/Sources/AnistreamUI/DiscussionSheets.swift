// Where a discussion opens: a sheet on iPhone, a 380 pt side panel on a regular-width iPad, and a
// window of its own on the Mac. Over the player, the player's cover presents it.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

public struct DiscussionSheets: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(\.playerUp) private var playerUp
    @Environment(Navigator.self) private var navigator
    #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    #else
    @Environment(\.openWindow) private var openWindow
    /// A discussion a launch argument asks for, in a sheet: the dev harness has no second window.
    @State private var preview: DiscussionKey?
    #endif
    let overPlayer: Bool

    /// The window's, or with `overPlayer` the player's cover's.
    public init(overPlayer: Bool = false) {
        self.overPlayer = overPlayer
    }

    private var here: Bool {
        #if os(iOS)
        overPlayer || !playerUp
        #else
        !overPlayer
        #endif
    }

    private var target: DiscussionKey? { here ? store.discussion.target : nil }

    private func close() { store.discussion.target = nil }

    /// `-AnistreamSheet discussion-7` opens episode 7 of the launch title's discussion once its
    /// thread is found, as the engine would say after the episode counts as watched; `discussion-row`
    /// only says the thread is found, for the episode row's quiet line. Screenshots only.
    private func launched() async {
        guard !overPlayer, let name = navigator.launchSheet, name.hasPrefix("discussion") else { return }
        let media = LaunchRoute.current.title ?? 900_001
        let episode = UInt32(name.dropFirst("discussion-".count)) ?? 7
        try? await Task.sleep(for: .milliseconds(500))
        navigator.launchSheet = nil
        store.apply(.discussionFound(media: media, episode: episode, comments: 6))
        guard name != "discussion-row" else { return }
        let key = DiscussionKey(media: media, episode: episode)
        #if os(iOS)
        store.discussion.show(key)
        #else
        preview = key
        #endif
        try? await Task.sleep(for: .milliseconds(700))
        store.apply(.discussion(Self.sample(media: media, episode: episode)))
    }

    /// A made-up thread for the shots, since the testing engine has no forum threads.
    static func sample(media: AnilistId, episode: UInt32) -> DiscussionView {
        func span(_ text: String, bold: Bool = false, spoiler: Bool = false) -> CommentSpan {
            CommentSpan(text: text, bold: bold, spoiler: spoiler)
        }
        func comment(_ id: UInt32, _ author: String, _ age: Int64, _ likes: UInt32, _ spans: [CommentSpan],
                     liked: Bool = false, own: Bool = false, replies: [CommentView] = []) -> CommentView {
            CommentView(id: id, author: author, createdAt: 0, age: .discussionAgeHours(hours: age),
                        body: [CommentBlock(spans: spans)], likeCount: likes, liked: liked, own: own,
                        pending: .posted, replies: replies)
        }
        let reply = comment(2, "tamsin", 3, 4, [span("Same. The quiet scene at the harbour stayed with me.")], liked: true)
        let comments = [
            comment(1, "lampwick", 5, 12, [span("That ending. I did not see the lantern coming.")], replies: [reply]),
            comment(3, "marlowe", 4, 2, [span("The soundtrack in the second half is "), span("so good", bold: true), span(".")]),
            comment(4, "juniper", 3, 0, [span("Does the ferry captain come back? "), span("She sails home in the finale.", spoiler: true)]),
            comment(5, "ferrylog", 2, 7, [span("Adding this to my rewatch list.")]),
            comment(6, "sorrel", 1, 1, [span("Rewatching it tonight.")], own: true),
        ]
        return DiscussionView(media: media, episode: episode, title: "Harbour Lights, episode \(episode)",
                              commentCount: 6, url: "https://forum.example/thread/1", comments: comments,
                              state: .ready(more: false), canPost: true, signedIn: true, loadImages: true)
    }

    public func body(content: Content) -> some View {
        #if os(iOS)
        let regular = sizeClass == .regular
        content
            .task { await launched() }
            .inspector(isPresented: Binding(
                get: { regular && target != nil },
                set: { if !$0, regular, here { close() } }
            )) {
                if let key = target {
                    DiscussionPanel(key: key, chrome: .inspector, close: close)
                        .id(key)
                        .inspectorColumnWidth(380)
                        .environment(store)
                }
            }
            .sheet(item: Binding(
                get: { regular ? nil : target },
                set: { if $0 == nil, here, !regular { close() } }
            )) { key in
                DiscussionPanel(key: key, chrome: .sheet, close: close)
                    .presentationDetents([.medium, .large])
                    .presentationBackground(Palette.ground)
                    .presentationDragIndicator(.visible)
                    .preferredColorScheme(.dark)
                    .environment(store)
            }
        #else
        content
            .task { await launched() }
            .sheet(item: $preview) { key in DiscussionWindowView(key: key).environment(store) }
            .onChange(of: store.discussion.target) { _, key in
                guard here, let key else { return }
                openWindow(id: DiscussionKey.windowId, value: key)
                store.discussion.target = nil
            }
        #endif
    }
}

extension DiscussionKey: Identifiable {
    public var id: String { "\(media)-\(episode)" }

    /// The Mac's discussion windows, one per episode.
    public static let windowId = "discussion"
}

#if os(macOS)
/// The Mac's discussion window: 480 by 720 pt, the title in its bar.
public struct DiscussionWindowView: View {
    @Environment(AppStore.self) private var store
    let key: DiscussionKey

    public init(key: DiscussionKey) {
        self.key = key
    }

    public var body: some View {
        DiscussionPanel(key: key, chrome: .window)
            .navigationTitle(L.discussionTitleEpisode(episode: Int(key.episode)))
            .navigationSubtitle(store.titles[key.media]?.showTitle ?? "")
            .frame(minWidth: 400, idealWidth: 480, minHeight: 480, idealHeight: 720)
            .preferredColorScheme(.dark)
    }
}
#endif

/// Whether an episode offers its discussion.
enum DiscussionEntry {
    /// Discussions on, and the episode at or before the viewer's progress; or spoiler protection
    /// off for the show. The engine refuses the rest itself.
    @MainActor
    static func offered(entry: Entry, number: UInt32, row: EpisodeRow, store: AppStore) -> Bool {
        guard store.discussionsOn else { return false }
        let next = entry.progress?.next ?? 1
        return row.completed || number < next || !store.spoilerProtection(for: entry.id)
    }
}

/// The quiet line under a watched episode that has a discussion: "Discussion · 214 comments".
/// Nothing for an episode with none, or with discussions off.
struct DiscussionLine: View {
    @Environment(AppStore.self) private var store
    let entry: Entry
    let row: EpisodeRow
    /// How far in the text column starts, so the line sits under the title.
    let inset: CGFloat

    var body: some View {
        if store.discussionsOn, let number = UInt32(row.number),
           let count = store.discussion.count(entry.id, episode: number) {
            Button {
                store.discussion.show(DiscussionKey(media: entry.id, episode: number))
            } label: {
                Text(L.discussionRow(count: Int(count)))
                    .font(.plat(mac: 12, touch: 13))
                    .foregroundStyle(.haze)
                    .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
                    .padding(.leading, inset)
                    .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .pointerHover()
            .transition(.opacity)
        }
    }
}
