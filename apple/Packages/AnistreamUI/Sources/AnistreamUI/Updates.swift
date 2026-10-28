// Updates in Settings and in the notice: this version and what the update check found, with the
// one action that fits. The Mac updates in place, iPhone and iPad in SideStore or AltStore, and
// a release with no file for this device on its page. Nothing installs until Update is pressed.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// What Settings says of updates under this version, and the action it offers.
struct UpdateWords: Equatable {
    enum Action: Equatable {
        case checkNow
        /// Install it here: the Mac.
        case update(version: String)
        /// Open SideStore or AltStore: iPhone and iPad.
        case updateInStore
        /// Its page, for a release with no file for this device.
        case releasePage
        case cancel
        case none
    }

    var note: String?
    /// Whether the note says something went wrong.
    var problem = false
    var action: Action
    var disabled = false

    init(
        latest: Release?, checking: Bool, outcome: UpdatesModel.Outcome?, progress: UpdateProgress,
        installsHere: Bool, mobile: Bool
    ) {
        switch progress {
        case .preparing:
            self.init(note: "Preparing the update", action: .cancel)
            return
        case .downloading(let received, let expected):
            let of = expected.map { " of \(Wording.bytes($0))" } ?? ""
            self.init(note: "Downloading \(Wording.bytes(received))\(of)", action: .cancel)
            return
        case .unpacking:
            self.init(note: L.updatesCheckingDownloadShort, action: .cancel)
            return
        case .relaunching:
            self.init(note: "Restarting to finish the update", action: .none)
            return
        case .failed:
            if let latest {
                self.init(note: "The update did not finish.", problem: true,
                          action: .update(version: latest.version))
                return
            }
        case .idle:
            break
        }
        if checking {
            self.init(note: "Checking…", action: .checkNow, disabled: true)
        } else if let latest {
            let action: Action = installsHere ? .update(version: latest.version) : mobile ? .updateInStore : .releasePage
            self.init(note: "Version \(latest.version) is available", action: action)
        } else {
            switch outcome {
            case .upToDate: self.init(note: L.updatesUpToDate, action: .checkNow)
            case .failed: self.init(note: "Could not check.", problem: true, action: .checkNow)
            case nil: self.init(note: nil, action: .checkNow)
            }
        }
    }

    init(note: String?, problem: Bool = false, action: Action, disabled: Bool = false) {
        self.note = note
        self.problem = problem
        self.action = action
        self.disabled = disabled
    }

    @MainActor
    init(_ updates: UpdatesModel) {
        #if os(iOS)
        let mobile = true
        #else
        let mobile = false
        #endif
        self.init(latest: updates.latest, checking: updates.checking, outcome: updates.outcome,
                  progress: updates.progress, installsHere: updates.installsHere, mobile: mobile)
    }

    /// The action's label.
    var label: String? {
        switch action {
        case .checkNow: L.actionCheckNow
        case .update(let version): "Update to v\(version)"
        case .updateInStore: cased(mac: "Update in SideStore", touch: "Update in SideStore")
        case .releasePage: cased(mac: "Release Page", touch: "Release page")
        case .cancel: "Cancel"
        case .none: nil
        }
    }
}

/// Carries out what an update's button or notice asks.
@MainActor
struct UpdateActions {
    let store: AppStore
    let openURL: OpenURLAction

    func run(_ action: UpdateWords.Action) {
        let updates = store.updates
        switch action {
        case .checkNow: updates.checkNow()
        case .update: updates.update()
        case .updateInStore:
            if let release = updates.latest { open(UpdatesModel.storeLinks(for: release)) }
        case .releasePage:
            if let page = updates.latest.flatMap({ URL(string: $0.pageUrl) }) { openURL(page) }
        case .cancel: updates.cancel()
        case .none: break
        }
    }

    /// The first of `links` an app takes: SideStore, AltStore, then the page in the browser.
    private func open(_ links: [URL]) {
        guard let first = links.first else { return }
        let rest = Array(links.dropFirst())
        openURL(first) { accepted in
            guard !accepted else { return }
            Task { @MainActor in open(rest) }
        }
    }
}

/// Updates (MacSettingsIntegrations): the daily check's switch, this version with what the check
/// found and its action, and what is new in a release on offer.
struct UpdatesGroup: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        SettingsGroup(title: L.updatesTitle) {
            SettingsRow(title: SettingKey.updateCheck.title, note: L.updatesSchedule) {
                SettingSwitch(key: .updateCheck)
            }
            UpdateVersionRow()
            if let release = store.updates.latest, !release.notes.isEmpty {
                ReleaseNotesRow(release: release)
            }
        }
        // Seen here, the release needs no notice over it.
        .onAppear { store.updates.dismissNotice() }
    }
}

/// This version, what the check found, and the action: Check Now, Update, or Cancel while an
/// update downloads, with how far it has come on a thin amber line beneath.
struct UpdateVersionRow: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL

    var body: some View {
        let updates = store.updates
        let words = UpdateWords(updates)
        VStack(alignment: .leading, spacing: 0) {
            SettingsRow(title: AppVersion.long, note: words.note, noteColor: .haze) {
                UpdateButton(words: words) { UpdateActions(store: store, openURL: openURL).run($0) }
            }
            if updates.progress.isActive {
                Meter(fraction: updates.progress.fraction ?? 0, height: 2)
                    .padding(.horizontal, 14)
                    .padding(.bottom, 8)
                    .transition(.opacity)
            }
        }
        .animation(.standard, value: updates.progress.isActive)
    }
}

/// An update's action as a capsule: amber for the update itself, the quiet capsule otherwise.
struct UpdateButton: View {
    let words: UpdateWords
    var height: CGFloat = 28
    var fontSize: CGFloat = 13
    let run: (UpdateWords.Action) -> Void

    var body: some View {
        if let label = words.label {
            switch words.action {
            case .update, .updateInStore:
                Button(label) { run(words.action) }
                    .buttonStyle(PrimaryCapsuleStyle(height: height, fontSize: fontSize))
                    .disabled(words.disabled)
            default:
                Button(label) { run(words.action) }
                    .buttonStyle(SecondaryCapsuleStyle(height: height, raisedSurface: true, fontSize: fontSize))
                    .disabled(words.disabled)
            }
        }
    }
}

/// What is new in the release on offer, and its page.
struct ReleaseNotesRow: View {
    let release: Release
    @Environment(\.openURL) private var openURL

    var body: some View {
        SettingsRow(title: "What’s new in \(release.version)", note: release.notes) {
            if let page = URL(string: release.pageUrl) {
                Button(cased(mac: "Release Notes", touch: "Release notes")) { openURL(page) }
                    .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
            }
        }
    }
}

/// The update notice, in the notice capsule's place: the release, Update, and a way to say not
/// now. What Update does is what Settings' button does; on the Mac, Settings then shows how it
/// goes.
struct UpdateNotice: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.openURL) private var openURL
    let release: Release

    var body: some View {
        let words = UpdateWords(store.updates)
        HStack(spacing: 12) {
            Text("anistream \(release.version) is available")
                .font(.mac(13))
                .foregroundStyle(.silk)
                .lineLimit(2)
            if let label = noticeLabel(words) {
                Button(label) {
                    UpdateActions(store: store, openURL: openURL).run(words.action)
                    #if os(macOS)
                    if case .update = words.action { navigator.openSettings(SettingsResults.updatesPane) }
                    #endif
                    store.updates.dismissNotice()
                }
                .buttonStyle(.plain)
                .font(.mac(13, weight: .semibold))
                .foregroundStyle(.amber)
            }
            Button {
                store.updates.dismissNotice()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.haze)
                    .frame(width: 24, height: 24)
                    .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Not Now")
        }
        .padding(.leading, 16)
        .padding(.trailing, 8)
        .frame(minHeight: 36)
        .chromeGlass(in: .capsule)
        .frame(maxWidth: 520)
        .accessibilityElement(children: .contain)
    }

    /// The notice's own word for the action: shorter than Settings'.
    private func noticeLabel(_ words: UpdateWords) -> String? {
        switch words.action {
        case .update: "Update"
        case .updateInStore: "Update"
        case .releasePage: cased(mac: "Release Page", touch: "Release page")
        default: nil
        }
    }
}

#if os(iOS)
/// Updates on iPhone, in About: the daily check's switch, as the Mac and iPad have it, then what
/// the check found, what is new, and the action. With the switch off nothing is checked on its
/// own, and Check Now still asks.
struct PhoneUpdatesCard: View {
    @Environment(AppStore.self) private var store
    @Environment(\.openURL) private var openURL

    var body: some View {
        let words = UpdateWords(store.updates)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 16) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(SettingKey.updateCheck.title)
                        .font(.ui(17))
                        .foregroundStyle(.silk)
                    Text(L.updatesSchedule)
                        .font(.ui(15))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                SettingSwitch(key: .updateCheck)
            }
            .padding(16)
            .accessibilityElement(children: .combine)
            Rectangle().fill(.faintRule).frame(height: 1).padding(.leading, 16)
            VStack(alignment: .leading, spacing: 12) {
                if let note = words.note {
                    Text(note)
                        .font(.ui(17))
                        .foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if let release = store.updates.latest, !release.notes.isEmpty {
                    Text(release.notes)
                        .font(.ui(15))
                        .foregroundStyle(.haze)
                        .fixedSize(horizontal: false, vertical: true)
                }
                UpdateButton(words: words, height: 44, fontSize: 17) {
                    UpdateActions(store: store, openURL: openURL).run($0)
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(.raised, in: .rect(cornerRadius: 12))
        .frame(maxWidth: 480)
        // Seen here, the release needs no notice over it.
        .onAppear { store.updates.dismissNotice() }
    }
}
#endif
