// The menus a title, an episode and a list status have, wherever they appear: context menus,
// the title page's More, the menu bar.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// A title's context menu in a grid or a calendar.
struct TitleMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    var body: some View {
        let action = TitleAction(entry)
        if action.playable, store.titles.unlisted(entry.id) {
            Button(AddSourceWords.menu) { navigator.openProviders() }
        } else {
            Button(action.title) {
                store.send(.play(id: entry.id, episode: String(action.episode)))
            }
            // Still to air: when, said and not offered.
            .disabled(!action.playable)
        }
        Button(cased(mac: "Go to Title", touch: "Go to title")) { navigator.openTitle(entry.id) }
        Divider()
        Menu("Status") { StatusItems(id: entry.id) }
        Button(cased(mac: "Open on AniList", touch: "Open on AniList")) { store.openOnAniList(entry.id) }
    }
}

/// The list a title is on, a choice each, checked where it is.
struct StatusItems: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId

    var body: some View {
        let current = store.titles[id]?.listStatus
        ForEach(LibraryModel.order, id: \.self) { segment in
            Toggle(segment.label.capitalizedSentence, isOn: Binding(
                get: { current == segment },
                set: { on in if on { store.setStatus(id, segment) } }
            ))
        }
    }
}

/// The title page's More menu.
struct TitleMoreItems: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    var body: some View {
        let next = entry.progress?.next ?? 1
        Button(cased(mac: "Mark All Previous Watched", touch: "Mark all previous watched")) {
            store.send(.setWatched(id: entry.id, episodes: (1..<next).map(String.init), watched: true))
            navigator.watchedByHand += 1
        }
        .disabled(next <= 1)
        Button(cased(mac: "Hide from Continue", touch: "Hide from Continue")) { store.hideFromContinue(entry.id) }
            .disabled(entry.progress == nil)
        Divider()
        let action = TitleAction(entry)
        // The episode Play would play, with a party: the form asks for the server and room.
        Button(cased(mac: "Watch Together…", touch: "Watch together…")) {
            store.party.open(.episode(id: entry.id, episode: String(action.episode)))
        }
        .disabled(!action.playable || store.titles.unlisted(entry.id))
        Button(cased(mac: "Play Trailer", touch: "Play trailer")) {
            if let trailer = entry.trailerUrl { store.send(.playTrailer(url: trailer, title: entry.title)) }
        }
        .disabled(entry.trailerUrl == nil)
        Button("Open on AniList") { store.openOnAniList(entry.id) }
        #if os(macOS)
        Button("More About This Show…") { navigator.moreAbout = entry.id }
        #else
        if UIDevice.current.userInterfaceIdiom == .pad {
            Button("More about this show") { navigator.moreAbout = entry.id }
        }
        #endif
        Divider()
        Button(cased(mac: "Show Settings for This Show…", touch: "Show settings for this show")) {
            navigator.showSettings = entry.id
        }
    }
}

/// An episode's context menu.
struct EpisodeMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry
    let number: UInt32
    let row: EpisodeRow

    var body: some View {
        let resuming = entry.progress?.next == number && entry.resume != nil
        // One no source has: the way to a source in Play's and Download's place.
        if row.metadataOnly {
            Button(AddSourceWords.menu) { navigator.openProviders() }
        } else {
            Button(resuming ? cased(mac: "Resume Episode \(number)", touch: "Resume episode \(number)")
                            : cased(mac: "Play Episode \(number)", touch: "Play episode \(number)")) {
                store.send(.play(id: entry.id, episode: row.number))
            }
            // A part watched: the episode again from its first second, its place let go.
            if resuming {
                Button(cased(mac: "Play from the Beginning", touch: "Play from the beginning")) {
                    store.send(.play(id: entry.id, episode: row.number, fromStart: true))
                }
            }
            if store.tv.available {
                Button(cased(mac: "Play on TV", touch: "Play on TV"), systemImage: "tv") {
                    store.playOnTV(entry.id, episode: row.number, from: resuming ? entry.resume?.position : nil)
                }
            }
        }
        Divider()
        if row.completed {
            Button(cased(mac: "Mark Unwatched", touch: "Mark unwatched")) {
                store.send(.setWatched(id: entry.id, episodes: [row.number], watched: false))
            }
        } else {
            Button(cased(mac: "Mark Watched", touch: "Mark watched")) {
                store.send(.setWatched(id: entry.id, episodes: [row.number], watched: true))
                navigator.watchedByHand += 1
            }
        }
        Button(cased(mac: "Mark All Previous Watched", touch: "Mark all previous watched")) {
            store.send(.setWatched(id: entry.id, episodes: (1...max(number, 1)).map(String.init), watched: true))
            navigator.watchedByHand += 1
        }
        if !row.metadataOnly {
            Divider()
            Button("Download") { store.send(.downloadEpisode(id: entry.id, episode: row.number)) }
                .disabled(store.hasDownload(entry.id, episode: row.number))
        }
    }
}

extension AppStore {
    /// Whether the episode is downloaded or on its way: Download has nothing more to do.
    func hasDownload(_ id: AnilistId, episode: String) -> Bool {
        downloads.rows.contains { $0.anilistId == id && $0.episode == episode && $0.state != .failed }
    }

    /// Put a title on a list, the change shown at once and the tracker told.
    func setStatus(_ id: AnilistId, _ segment: LibrarySegment) {
        titles.setStatus(id, segment)
        send(.setStatus(id: id, status: segment))
    }

    /// Open a title's AniList page in the browser.
    func openOnAniList(_ id: AnilistId) {
        send(.openExternal(url: "https://anilist.co/anime/\(id)"))
    }
}

/// A capsule track with a raised capsule under the choice: the boards' one segmented shape, for
/// a choice on the ground.
struct SegmentedCapsule<Value: Hashable>: View {
    @Binding var selection: Value
    let options: [(Value, String)]

    var body: some View {
        HStack(spacing: 0) {
            ForEach(options, id: \.0) { value, name in
                let selected = selection == value
                Button(name) {
                    withAnimation(.quick) { selection = value }
                }
                .buttonStyle(.plain)
                .font(.mac(12))
                .foregroundStyle(selected ? .silk : .haze)
                .padding(.horizontal, 12)
                .frame(height: 24)
                .background(selected ? Palette.raisedSelected : .clear, in: .capsule)
                .contentShape(.capsule)
                .accessibilityAddTraits(selected ? .isSelected : [])
            }
        }
        .padding(2)
        .background(.raised, in: .capsule)
    }
}
