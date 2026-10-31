// Home: the engine's whole view of it, kept as the engine sent it, with the few things a screen
// holds beside it (a card on its way out, the sources check's answers, the hidden titles).

import AnistreamKit
import AnistreamStrings
import Foundation
import Observation

/// What Home shows. The engine sends it whole and decides everything on it, so this keeps the last
/// one and the small amount of state the screen needs around it.
@Observable @MainActor
public final class HomeModel {
    /// Home as last sent. `nil` until the first part arrives.
    public private(set) var view: HomeView?
    /// The titles taken off Home in this session, ahead of the engine's next view: their cards
    /// fade out at once, and the shelves close the gap.
    public private(set) var leaving: Set<AnilistId> = []
    /// What the sources check found, by title.
    public private(set) var playable: [AnilistId: Playable] = [:]
    /// The titles hidden with Not interested, newest first, once Settings has asked.
    public private(set) var hidden: [HiddenTitle]?
    /// Bumped each time the spotlight changes title or case, for the crossfade.
    public private(set) var spotlightChanges = 0
    /// When Home was last asked for, and when a view last arrived.
    @ObservationIgnored public private(set) var askedAt: ContinuousClock.Instant?
    @ObservationIgnored public private(set) var answeredAt: ContinuousClock.Instant?
    /// Titles the sources check was asked about, so a spotlight or card is asked once.
    @ObservationIgnored private var probed: Set<AnilistId> = []

    public init() {}

    /// Whether Home was asked for and has not yet been given its first view.
    public var isLoading: Bool { askedAt != nil && view == nil }

    /// Whether anything is on its way: a part with nothing kept, or Continue's own titles.
    public var isSettled: Bool {
        guard askedAt != nil else { return true }
        guard let view else { return false }
        return view.pending.isEmpty && view.end != nil
    }

    /// Whether Home was answered less than `window` ago.
    public func answered(within window: Duration) -> Bool {
        answeredAt.map { ContinuousClock.now - $0 < window } ?? false
    }

    func asked() {
        askedAt = .now
    }

    /// Take the engine's view. A title that left the view is no longer on its way out.
    func apply(_ next: HomeView, titles: TitleCache) {
        if let before = view, next.spotlightMoved(from: before) { spotlightChanges += 1 }
        var continuing: [Entry] = []
        var others: [Entry] = []
        if let spot = next.spotlight {
            if spot.case == .continuing { continuing.append(spot.entry) } else { others.append(spot.entry) }
        }
        for section in next.sections {
            let entries = section.items.map(\.entry)
            if section.kind == .continue { continuing += entries } else { others += entries }
        }
        titles.merge(others)
        // Continue speaks for this device's watch history.
        titles.merge(continuing, local: true)
        let present = Set(next.ids)
        leaving.formIntersection(present)
        view = next
        answeredAt = .now
    }

    /// The sources check's answer for a title.
    func playable(_ id: AnilistId, _ found: Playable) {
        playable[id] = found
    }

    /// What the check found for `id`, falling back on what Home was sent with.
    public func playable(_ id: AnilistId, sent: Playable) -> Playable {
        switch playable[id] {
        case .some(let found) where found != .unknown: found
        default: sent
        }
    }

    /// Whether `id` was asked after already, and now is.
    func probing(_ id: AnilistId) -> Bool {
        probed.insert(id).inserted
    }

    func leaves(_ id: AnilistId) {
        leaving.insert(id)
    }

    func returns(_ id: AnilistId) {
        leaving.remove(id)
    }

    func hidden(_ rows: [HiddenTitle]) {
        hidden = rows
    }

    /// A restored title leaves the Settings list at once.
    func unhide(_ id: AnilistId) {
        hidden?.removeAll { $0.entry.id == id }
    }
}

extension HomeView {
    /// Every title Home shows, the spotlight's first.
    var ids: [AnilistId] {
        (spotlight.map { [$0.entry.id] } ?? []) + sections.flatMap { $0.items.map(\.entry.id) }
    }

    /// Whether the spotlight is another title, or the same one for another reason, than in
    /// `before`: what a front end crossfades on. `HomeView::spotlight_moved`.
    func spotlightMoved(from before: HomeView) -> Bool {
        let key = { (home: HomeView) in home.spotlight.map { SpotKey(id: $0.entry.id, case: $0.case) } }
        return key(self) != key(before)
    }

    /// The section of `kind`, when Home has one.
    public func section(_ kind: SectionKind) -> HomeSection? {
        sections.first { $0.kind == kind }
    }
}

private struct SpotKey: Equatable {
    let id: AnilistId
    let `case`: SpotlightCase
}

extension AppStore {
    /// Home, unless it was answered within `listFresh`; `force` asks for it again, dropping what
    /// the engine kept for half an hour (pull to refresh, Refresh, Try again).
    public func loadHome(force: Bool = false) {
        guard force || !discovery.answered(within: Self.listFresh) else { return }
        discovery.asked()
        send(.loadHome(refresh: force))
    }

    /// Ask whether a source has `id`, once for each title, when someone shows interest in it.
    public func probe(_ id: AnilistId) {
        guard discovery.probing(id) else { return }
        send(.probeTitle(id: id))
    }

    /// Not interested in `entry`: it fades off every shelf at once, the engine keeps it hidden,
    /// and the notice offers Undo for as long as notices hold a button.
    ///
    /// - Parameters:
    ///   - title: the title as the screens head it.
    ///   - unaired: it has not started airing, so it comes back when it does.
    public func notInterested(_ entry: Entry, title: String, unaired: Bool = false) {
        discovery.leaves(entry.id)
        send(.dismissUpNext(id: entry.id))
        let line = unaired ? L.upNextHiddenUntilAirs(title: title) : L.upNextHidden(title: title)
        notices.post(
            Notice(kind: .info, text: line, hint: nil),
            action: NoticeButton(label: L.actionUndo) { [weak self] in self?.restore(entry.id) })
    }

    /// One notice for however many parts failed: raw errors are in Messages already.
    func homeFailed() {
        notices.post(
            Notice(kind: .alert, text: L.homeFailed, hint: nil),
            action: NoticeButton(label: L.homeTryAgain) { [weak self] in self?.loadHome(force: true) })
    }

    /// Bring a hidden title back: Undo in the notice, or Restore in Settings.
    public func restore(_ id: AnilistId) {
        discovery.returns(id)
        discovery.unhide(id)
        send(.restoreUpNext(id: id))
    }

    /// Ask for the hidden titles, for Settings.
    public func loadHiddenTitles() {
        send(.loadHiddenTitles)
    }
}
