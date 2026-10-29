// The menus a title, an episode and a list status have, wherever they appear: context menus,
// the title page's More, the menu bar.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// A title's context menu in a grid or a calendar.
struct TitleMenu: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    var body: some View {
        let action = TitleAction(entry, episodes: store.titles.episodes[entry.id])
        if action.playable, store.titles.unlisted(entry.id) {
            Button(AddSourceWords.menu) { navigator.openProviders() }
        } else {
            Button(action.title) {
                store.send(.play(id: entry.id, episode: String(action.episode)))
            }
            // Still to air: when, said and not offered.
            .disabled(!action.playable)
        }
        Button(L.titleOpen) { navigator.openTitle(entry.id) }
        Divider()
        Menu(L.filterStatus) { StatusItems(id: entry.id) }
        Button(L.titleOpenAnilist) { store.openOnAniList(entry.id) }
    }
}

/// The list a title is on, a choice each, checked where it is.
struct StatusItems: View {
    @Environment(AppStore.self) private var store
    let id: AnilistId

    var body: some View {
        let current = store.titles[id]?.listStatus
        ForEach(LibraryModel.order, id: \.self) { segment in
            Toggle(segment.title, isOn: Binding(
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
        Button(L.episodeMarkPreviousWatched) {
            store.send(.setWatched(id: entry.id, episodes: (1..<next).map(String.init), watched: true))
            navigator.watchedByHand += 1
        }
        .disabled(next <= 1)
        Button(L.continueHide) { store.hideFromContinue(entry.id) }
            .disabled(entry.progress == nil)
        Divider()
        let action = TitleAction(entry, episodes: store.titles.episodes[entry.id])
        // The episode Play would play, with a party: the form asks for the server and room.
        Button(L.partyOpen) {
            store.party.open(.episode(id: entry.id, episode: String(action.episode)))
        }
        .disabled(!action.playable || store.titles.unlisted(entry.id))
        Button(L.titlePlayTrailer) { navigator.playTrailer(entry, store: store) }
        .disabled(entry.trailerUrl == nil)
        Button(L.titleOpenAnilist) { store.openOnAniList(entry.id) }
        #if os(macOS)
        Button(L.titleMoreAbout) { navigator.moreAbout = entry.id }
        #else
        if UIDevice.current.userInterfaceIdiom == .pad {
            Button(L.titleMoreAbout) { navigator.moreAbout = entry.id }
        }
        #endif
        Divider()
        Button(L.sourcePickerTitle) { navigator.sourcePicker = entry.id }
        Button(L.shellShowSettings) {
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
        } else if row.unsourced {
            // AniList says it is out and no source lists it: the picker, to try another.
            Button(L.sourcePickerTitle) { navigator.sourcePicker = entry.id }
        } else {
            Button(resuming ? L.shellResumeEpisode(episode: Int(number)) : L.titleButtonPlay(episode: Int(number))) {
                store.send(.play(id: entry.id, episode: row.number))
            }
            // A part watched: the episode again from its first second, its place let go.
            if resuming {
                Button(L.episodePlayFromBeginning) {
                    store.send(.play(id: entry.id, episode: row.number, fromStart: true))
                }
            }
            if store.tv.available {
                Button(L.tvPlayOn, systemImage: "tv") {
                    store.playOnTV(entry.id, episode: row.number, from: resuming ? entry.resume?.position : nil)
                }
            }
        }
        Divider()
        if row.completed {
            Button(L.episodeMarkUnwatched) {
                store.send(.setWatched(id: entry.id, episodes: [row.number], watched: false))
            }
        } else {
            Button(L.episodeMarkWatched) {
                store.send(.setWatched(id: entry.id, episodes: [row.number], watched: true))
                navigator.watchedByHand += 1
            }
        }
        Button(L.episodeMarkPreviousWatched) {
            store.send(.setWatched(id: entry.id, episodes: (1...max(number, 1)).map(String.init), watched: true))
            navigator.watchedByHand += 1
        }
        if !row.metadataOnly, !row.unsourced {
            Divider()
            Button(L.shellDownload) { store.send(.downloadEpisode(id: entry.id, episode: row.number)) }
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
