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
    /// The window's width as it is without the panel, which the panel itself takes from the page.
    @State private var windowWidth: CGFloat = .infinity
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

    private static let panelWidth: CGFloat = 380
    private static let pageMinimum: CGFloat = 560

    #if os(iOS)
    private var panelFits: Bool {
        sizeClass == .regular && windowWidth >= Self.panelWidth + Self.pageMinimum
    }
    #endif

    private var target: DiscussionKey? { here ? store.discussion.target : nil }

    private func close() { store.discussion.target = nil }

    /// `-AnistreamSheet discussion-7` opens episode 7's discussion, which the testing engine's
    /// fixture thread answers. Screenshots only: the sheet name comes from the launch route, which
    /// a Release build ignores.
    private func launched() async {
        guard !overPlayer, let name = navigator.launchSheet, name.hasPrefix("discussion") else { return }
        let media = LaunchRoute.current.title ?? 900_001
        let episode = UInt32(name.dropFirst("discussion-".count)) ?? 7
        try? await Task.sleep(for: .milliseconds(500))
        navigator.launchSheet = nil
        let key = DiscussionKey(media: media, episode: episode)
        #if os(iOS)
        store.discussion.show(key)
        #else
        preview = key
        #endif
    }

    public func body(content: Content) -> some View {
        #if os(iOS)
        // The side panel only where the page keeps room beside it (the title's own column needs
        // about 560 pt); a narrower window gets the sheet.
        let regular = panelFits
        content
            .task { await launched() }
            .inspector(isPresented: Binding(
                get: { regular && target != nil },
                set: { if !$0, regular, here { close() } }
            )) {
                if let key = target {
                    DiscussionPanel(key: key, chrome: .inspector, close: close)
                        .id(key)
                        .inspectorColumnWidth(Self.panelWidth)
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
            .onGeometryChange(for: CGFloat.self, of: { $0.size.width }) { width in
                // Read while the panel is shut; open, the page is narrower by its width.
                windowWidth = panelFits && target != nil ? width + Self.panelWidth : width
            }
        #else
        content
            .task { await launched() }
            .sheet(item: $preview) { key in DiscussionWindowView(key: key).environment(store) }
            .onChange(of: store.discussion.target) { _, key in
                guard here, let key else { return }
                store.discussion.reading = key
                openWindow(id: DiscussionKey.windowId)
                store.discussion.target = nil
            }
        #endif
    }
}

extension DiscussionKey: Identifiable {
    public var id: String { "\(media)-\(episode)" }

    /// The Mac's discussion window: one, because the engine keeps one discussion open.
    public static let windowId = "discussion"
}

#if os(macOS)
/// The Mac's discussion window: 480 by 720 pt, the title in its bar.
public struct DiscussionWindowHost: View {
    @Environment(AppStore.self) private var store

    public init() {}

    public var body: some View {
        if let key = store.discussion.reading {
            DiscussionWindowView(key: key).id(key)
        } else {
            Color.clear
        }
    }
}

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
