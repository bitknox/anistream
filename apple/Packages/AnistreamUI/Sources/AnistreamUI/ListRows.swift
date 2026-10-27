// Rows in a List drawn as the scroll views drew them, so they can take swipe actions (native-ux,
// the swipe table) and nothing else about them changes: no background, no separators, the insets
// and the spacing each screen sets itself.

import AnistreamKit
import AnistreamStore
import SwiftUI

extension View {
    /// A row of a bare list set in by `insets`, on iPhone and iPad; with none, as it is.
    @ViewBuilder
    func listed(_ insets: EdgeInsets?) -> some View {
        #if os(iOS)
        if let insets { bareRow(insets) } else { self }
        #else
        self
        #endif
    }

    /// Continue's swipes, on iPhone and iPad; the Mac has none.
    @ViewBuilder
    func swiped(_ item: ContinueItem) -> some View {
        #if os(iOS)
        continueSwipes(item)
        #else
        self
        #endif
    }
}

#if os(iOS)
extension View {
    /// A List that draws nothing of its own: plain, on the screen's ground, its rows as short as
    /// what is in them.
    func bareList() -> some View {
        listStyle(.plain)
            .scrollContentBackground(.hidden)
            .environment(\.defaultMinListRowHeight, 0)
    }

    /// A row of a bare list, set in by `insets` and nothing more.
    func bareRow(_ insets: EdgeInsets = EdgeInsets()) -> some View {
        listRowInsets(insets)
            .listRowBackground(Color.clear)
            .listRowSeparator(.hidden)
    }

    /// Continue's swipes: Watched, the episode the row plays, from the leading edge, and Remove
    /// from the trailing one. Untinted, as Downloads' are: the screen's one amber is not spent on
    /// a swipe. Remove takes no full swipe while it has no Undo to go with it.
    func continueSwipes(_ item: ContinueItem) -> some View {
        modifier(ContinueSwipes(item: item))
    }

    /// An episode's swipes: Watched or Unwatched from the leading edge, the full swipe among them,
    /// and Download from the trailing one, where the episode menu offers it.
    func episodeSwipes(_ entry: Entry, _ row: EpisodeRow) -> some View {
        modifier(EpisodeSwipes(entry: entry, row: row))
    }
}

private struct ContinueSwipes: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let item: ContinueItem

    func body(content: Content) -> some View {
        content
            .swipeActions(edge: .leading, allowsFullSwipe: false) {
                if item.playable {
                    Button("Watched") {
                        store.send(.setWatched(id: item.id, episodes: [String(item.episode)], watched: true))
                        navigator.watchedByHand += 1
                    }
                }
            }
            .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                Button("Remove", role: .destructive) { store.hideFromContinue(item.id) }
            }
    }
}

private struct EpisodeSwipes: ViewModifier {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let row: EpisodeRow

    func body(content: Content) -> some View {
        content
            .swipeActions(edge: .leading, allowsFullSwipe: true) {
                if row.completed {
                    Button("Unwatched") {
                        store.send(.setWatched(id: entry.id, episodes: [row.number], watched: false))
                    }
                } else {
                    Button("Watched") {
                        store.send(.setWatched(id: entry.id, episodes: [row.number], watched: true))
                        navigator.watchedByHand += 1
                    }
                }
            }
            .swipeActions(edge: .trailing, allowsFullSwipe: false) {
                if !row.metadataOnly, !row.unsourced, !store.hasDownload(entry.id, episode: row.number) {
                    Button("Download") { store.send(.downloadEpisode(id: entry.id, episode: row.number)) }
                }
            }
    }
}
#endif
