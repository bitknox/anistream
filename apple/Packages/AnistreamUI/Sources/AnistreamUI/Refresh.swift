// Pull to refresh on iPhone and iPad: the screen asks again, and the spinner stays until the
// engine's job for it ends, at most 15 s.

import AnistreamKit
import AnistreamStore
import Foundation
import SwiftUI

/// What a pull refreshes.
enum Refreshable {
    case home, continueWatching, calendar, seasonal, downloads
    /// The Library's list on screen, which also syncs.
    case library(LibrarySegment)
}

extension AppStore {
    /// The longest a pull's spinner stays.
    static var refreshLimit: Duration { .seconds(15) }

    /// Ask again for `what`, and return once its answer is in and its jobs are done, or after
    /// 15 s, whichever is first.
    func refresh(_ what: Refreshable) async {
        let activities: [String]
        let list: ListModel?
        switch what {
        case .home:
            loadHome(force: true)
            (activities, list) = (["loading home"], nil)
        case .continueWatching:
            loadContinue(force: true)
            (activities, list) = (["loading continue watching"], continuing.list)
        case .calendar:
            loadCalendar(force: true)
            (activities, list) = (["loading the calendar"], calendar.list)
        case .seasonal:
            loadSeasonal()
            (activities, list) = (["loading this season"], seasonal)
        case .library(let segment):
            syncNow()
            loadLibrary(segment)
            (activities, list) = (["syncing", "loading your \(segment.label) list"], library.segment(segment))
        case .downloads:
            loadDownloads()
            (activities, list) = ([], nil)
        }
        let deadline = ContinuousClock.now + Self.refreshLimit
        // The jobs start as the engine takes the commands; give them that moment first.
        try? await Task.sleep(for: .milliseconds(150))
        while ContinuousClock.now < deadline, !Task.isCancelled {
            let loading = list.map { !$0.state.isAnswer } ?? false
            let working = jobs.running.contains { activities.contains($0.activity) }
            if !loading, !working { return }
            try? await Task.sleep(for: .milliseconds(100))
        }
    }
}

extension View {
    /// Pull to refresh `what`, on iPhone and iPad; the Mac has no pull.
    @ViewBuilder
    func pullToRefresh(_ store: AppStore, _ what: Refreshable) -> some View {
        #if os(iOS)
        refreshable { await store.refresh(what) }
        #else
        self
        #endif
    }
}
