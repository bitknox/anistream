// What the screens ask the engine for, beyond the lists: settings, accounts, sources, plugin
// stores and downloads. Each sends one of the engine's own commands; where a screen waits on the
// answer, the job it runs as is kept by the model that shows it.

import AnistreamKit
import Foundation

extension AppStore {
    // MARK: Settings

    /// Step a setting one rung, or over to its other state: an arrow on a row, a switch.
    public func stepSetting(_ key: SettingKey, by delta: Int64) {
        send(.stepSetting(key: key, delta: delta))
    }

    /// Set a setting to `value`: a menu's choice, a typed value, the source order.
    public func setSetting(_ key: SettingKey, to value: SettingValue) {
        send(.setSetting(key: key, value: value))
    }

    /// Turn a switch on or off, when it is not already.
    public func setSwitch(_ key: SettingKey, on: Bool) {
        guard settings.isOn(key) != on else { return }
        setSetting(key, to: .bool(on))
    }

    // MARK: Accounts

    public func syncNow() {
        accounts.started("sync", job: send(.syncNow))
    }

    /// Start a tracker's own sign-in: the system's sheet for AniList and MyAnimeList, a device
    /// code for Simkl and Trakt.
    public func connect(_ tracker: String) {
        accounts.started(tracker, job: send(.connect(tracker: tracker)))
    }

    /// The system's sheet came back from `request` with `callback`, the redirect that carries
    /// the code or the token, so the engine finishes signing in. Never logged.
    public func finishSignIn(_ request: SignInRequest, callback: URL) {
        accounts.signInEnded(request)
        send(.finishSignIn(tracker: request.tracker, callback: callback.absoluteString))
    }

    /// The system's sheet for `request` was closed without signing in.
    public func cancelSignIn(_ request: SignInRequest) {
        accounts.signInEnded(request)
        send(.cancelSignIn(tracker: request.tracker))
    }

    public func connectKitsu(username: String, password: String) {
        accounts.started("kitsu", job: send(.connectKitsu(username: username, password: password)))
    }

    public func disconnect(_ tracker: String) {
        send(.disconnect(tracker: tracker))
    }

    public func resolveConflict(_ id: AnilistId, keepLocal: Bool) {
        send(.resolveConflict(id: id, keepLocal: keepLocal))
    }

    // MARK: Sources

    public func checkProviders() {
        send(.checkProviders)
    }

    /// Move a source one row up the chain (`step` below zero) or down it.
    public func moveSource(_ id: String, by step: Int) {
        guard let order = SourceOrder.moved(settings.sources.order, rows: providers.rows, id: id, step: step)
        else { return }
        setSetting(.sourceOrder, to: .strings(order))
    }

    /// Move a source to where the row at `index` is: a drag.
    public func moveSource(_ id: String, to index: Int) {
        guard let order = SourceOrder.moved(settings.sources.order, rows: providers.rows, id: id, to: index)
        else { return }
        setSetting(.sourceOrder, to: .strings(order))
    }

    /// The providers in the order failover tries them, as the config now has it.
    public var orderedProviders: [ProviderRow] {
        SourceOrder.sorted(providers.rows, by: settings.sources.order)
    }

    /// Pin a source for one title, or give it back to automatic with `nil`.
    public func setProviderPreference(_ id: AnilistId, to provider: String?) {
        send(.setProviderPreference(id: id, provider: provider))
    }

    // MARK: Plugin stores

    public func loadStores() {
        send(.loadStores)
    }

    /// Check a pasted invite link against the store it names.
    public func previewStore(_ link: String) {
        let link = link.trimmingCharacters(in: .whitespacesAndNewlines)
        stores.checking(link)
        send(.previewStore(link: link))
    }

    public func joinStore(_ link: String) {
        stores.started(.join, job: send(.joinStore(link: link.trimmingCharacters(in: .whitespacesAndNewlines))))
    }

    public func leaveStore(_ store: String) {
        stores.started(.leave(store: store), job: send(.leaveStore(store: store)))
    }

    /// Install a store's plugin, or update it.
    public func installPlugin(_ plugin: String, from store: String) {
        stores.started(.install(store: store, plugin: plugin), job: send(.installPlugin(store: store, plugin: plugin)))
    }

    public func removePlugin(_ plugin: String) {
        stores.started(.remove(plugin: plugin), job: send(.removePlugin(plugin: plugin)))
    }

    /// An invite link: for someone new with a name, for another device of yours without.
    public func createInvite(_ store: String, for name: String?) {
        stores.started(.invite(store: store), job: send(.createInvite(store: store, name: name)))
    }

    public func loadMembers(_ store: String) {
        send(.loadMembers(store: store))
    }

    public func removeMember(_ member: String, from store: String) {
        send(.removeMember(store: store, member: member))
    }

    public func cancelInvite(_ invite: String, in store: String) {
        send(.cancelInvite(store: store, invite: invite))
    }

    public func loadLocalPlugins(for store: String) {
        send(.loadLocalPlugins(store: store))
    }

    public func publishPlugin(_ plugin: String, to store: String) {
        send(.publishPlugin(store: store, plugin: plugin))
    }

    public func openAdmin(_ store: String) {
        send(.openAdmin(store: store))
    }

    /// Let anyone with the store's link join, or only people you invite. Opening makes a new link.
    public func setOpenJoin(_ store: String, open: Bool) {
        stores.started(.openJoin(store: store), job: send(.setOpenJoin(store: store, open: open)))
    }

    /// Remove one device that joined with the store's link, or all of them when `device` is nil.
    public func removeAnonymous(_ device: String?, from store: String) {
        stores.started(.removeAnonymous(store: store), job: send(.removeAnonymous(store: store, device: device)))
    }

    // MARK: Downloads

    public func loadDownloads() {
        send(.loadDownloads)
    }

    /// Pause a download, or resume it.
    public func toggleDownload(_ id: Int64) {
        send(.downloadPause(id: id))
    }

    /// Take a download off the queue, keeping a finished file.
    public func cancelDownload(_ id: Int64) {
        send(.downloadCancel(id: id))
    }

    /// Take a download off the queue and delete its file.
    public func deleteDownload(_ id: Int64) {
        send(.downloadDelete(id: id))
    }

    public func clearCompletedDownloads() {
        send(.downloadClearCompleted)
    }

    /// Pause every download that is running or waiting.
    public func pauseAllDownloads() {
        for row in downloads.rows where row.state == .active || row.state == .queued {
            toggleDownload(row.id)
        }
    }

    /// Resume every paused download.
    public func resumeAllDownloads() {
        for row in downloads.rows where row.state == .paused {
            toggleDownload(row.id)
        }
    }

    public func playDownload(_ row: DownloadRow) {
        guard let path = row.path else { return }
        send(.playLocal(id: row.anilistId, episode: row.episode, title: row.title, path: path))
    }

    /// Queue a failed download again: the engine retries it in place.
    public func retryDownload(_ row: DownloadRow) {
        send(.downloadEpisode(id: row.anilistId, episode: row.episode))
    }
}
