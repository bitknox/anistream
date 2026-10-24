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
        Button("Go to Title") { navigator.openTitle(entry.id) }
        Divider()
        Menu("Status") { StatusItems(id: entry.id) }
        Button("Open on AniList") { store.openOnAniList(entry.id) }
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
        Button("Mark All Previous Watched") {
            store.send(.setWatched(id: entry.id, episodes: (1..<next).map(String.init), watched: true))
        }
        .disabled(next <= 1)
        Button("Hide from Continue") { store.hideFromContinue(entry.id) }
            .disabled(entry.progress == nil)
        Divider()
        Button("Play Trailer") {
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
        Button("Show Settings for This Show…") { navigator.showSettings = entry.id }
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
            Button(resuming ? "Resume Episode \(number)" : "Play Episode \(number)") {
                store.send(.play(id: entry.id, episode: row.number))
            }
            if store.tv.available {
                Button("Play on TV", systemImage: "tv") {
                    store.playOnTV(entry.id, episode: row.number, from: resuming ? entry.resume?.position : nil)
                }
            }
        }
        Divider()
        if row.completed {
            Button("Mark Unwatched") {
                store.send(.setWatched(id: entry.id, episodes: [row.number], watched: false))
            }
        } else {
            Button("Mark Watched") {
                store.send(.setWatched(id: entry.id, episodes: [row.number], watched: true))
            }
        }
        Button("Mark All Previous Watched") {
            store.send(.setWatched(id: entry.id, episodes: (1...max(number, 1)).map(String.init), watched: true))
        }
        if !row.metadataOnly {
            Divider()
            Button("Download") { store.send(.downloadEpisode(id: entry.id, episode: row.number)) }
                .disabled(true)
        }
    }
}

extension AppStore {
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
