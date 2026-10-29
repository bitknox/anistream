// The TVs paired for Play on TV, in Settings: each with when it was paired and where it was last
// reached, Forget, and a way to pair another.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// What Settings says about a paired TV under its name.
@MainActor
enum TVSettingsWords {
    static func note(_ tv: FfiPairedTv, connected: Bool, now: Date = .now) -> String {
        if connected { return L.tvConnectedAt(address: tv.address) }
        let paired = Date(timeIntervalSince1970: TimeInterval(tv.pairedAt))
        let pairedWords = L.tvPairedOn(when: paired.formatted(.dateTime.day().month(.abbreviated).locale(AirTime.locale)))
        guard let seen = tv.lastSeen else { return pairedWords }
        let ago = Int64(now.timeIntervalSince1970) - seen
        return L.tvPairedSeen(paired: pairedWords, ago: ago < 60 ? L.tvSeenJustNow : Wording.ago(ago))
    }

    static func footnote(_ tv: TVRemoteModel) -> String {
        L.tvSettingsFootnote(code: tv.shortFingerprint)
    }
}

/// Mac: a group of the Integrations pane. iPad: a group of the Playback pane, under Picture and
/// sound. The iPhone has its own page for them (PhoneTVsPage).
struct TVsGroup: View {
    @Environment(AppStore.self) private var store
    @State private var forgetting: FfiPairedTv?

    var body: some View {
        let tv = store.tv
        if tv.available {
            SettingsGroup(title: L.tvPlayOn, footnote: TVSettingsWords.footnote(tv)) {
                ForEach(tv.paired, id: \.fingerprint) { paired in
                    SettingsRow(title: paired.name, note: TVSettingsWords.note(paired, connected: tv.connected == paired.fingerprint)) {
                        Button(L.actionForget) { forgetting = paired }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
                SettingsRow(title: tv.paired.isEmpty ? L.tvNonePaired : L.tvAnother, note: L.tvNonePairedDetail) {
                    Button(L.tvPairMenu) { tv.openPicker() }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
            }
            .confirmationDialog(
                L.tvForgetTitle(name: forgetting?.name ?? L.tvGenericName),
                isPresented: Binding(get: { forgetting != nil }, set: { if !$0 { forgetting = nil } }),
                presenting: forgetting
            ) { paired in
                Button(L.actionForget, role: .destructive) { tv.forget(paired.fingerprint) }
            } message: { _ in
                Text(L.tvForgetDetail)
            }
        }
    }
}

#if os(iOS)
/// iPhone: a page of the Settings sheet.
struct PhoneTVsPage: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let tv = store.tv
        PhoneList {
            Section {
                if tv.paired.isEmpty {
                    PhoneRowTitle(title: L.tvNonePaired, note: L.tvNonePairedDetail)
                        .listRowBackground(Palette.raised)
                }
                ForEach(tv.paired, id: \.fingerprint) { paired in
                    HStack {
                        PhoneRowTitle(title: paired.name, note: TVSettingsWords.note(paired, connected: tv.connected == paired.fingerprint))
                        Spacer()
                        Button(L.actionForget) { tv.forget(paired.fingerprint) }
                            .font(.ui(17, weight: .semibold))
                            .foregroundStyle(.silk)
                            .buttonStyle(.borderless)
                    }
                    .listRowBackground(Palette.raised)
                    .swipeActions {
                        Button(L.actionForget, role: .destructive) { tv.forget(paired.fingerprint) }
                    }
                }
                Button(L.tvPairAction) { tv.openPicker() }
                    .font(.ui(17, weight: .semibold))
                    .foregroundStyle(.silk)
                    .listRowBackground(Palette.raised)
            } header: {
                PhoneSectionHeader(L.tvPairedTvs)
            } footer: {
                Text(TVSettingsWords.footnote(tv))
            }
        }
        .navigationTitle(L.tvTitleList)
        .navigationBarTitleDisplayMode(.inline)
    }
}
#endif
