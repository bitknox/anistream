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
        if connected { return "Connected · \(tv.address)" }
        let paired = Date(timeIntervalSince1970: TimeInterval(tv.pairedAt))
        let pairedWords = "Paired " + paired.formatted(.dateTime.day().month(.abbreviated))
        guard let seen = tv.lastSeen else { return pairedWords }
        let ago = Int64(now.timeIntervalSince1970) - seen
        return pairedWords + " · seen " + (ago < 60 ? "just now" : Wording.ago(ago))
    }

    static func footnote(_ tv: TVRemoteModel) -> String {
        "A TV lists this \(Device.name) with the code \(tv.shortFingerprint). Forget a TV here and it forgets this \(Device.name) too."
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
            SettingsGroup(title: "Play on TV", footnote: TVSettingsWords.footnote(tv)) {
                ForEach(tv.paired, id: \.fingerprint) { paired in
                    SettingsRow(title: paired.name, note: TVSettingsWords.note(paired, connected: tv.connected == paired.fingerprint)) {
                        Button(L.actionForget) { forgetting = paired }
                            .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                    }
                }
                SettingsRow(title: tv.paired.isEmpty ? "No TVs paired yet" : "Another TV", note: L.tvNonePairedDetail) {
                    Button("Pair a TV…") { tv.openPicker() }
                        .buttonStyle(SecondaryCapsuleStyle(raisedSurface: true))
                }
            }
            .confirmationDialog(
                "Forget \(forgetting?.name ?? "this TV")?",
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
                Button("Pair a TV") { tv.openPicker() }
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
