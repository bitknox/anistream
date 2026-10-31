// Settings, Hidden titles: the titles marked Not interested, each with Restore.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// The hidden titles as rows with Restore, and the line when there are none. Asks the engine once
/// it is on screen.
struct HiddenTitlesRows: View {
    @Environment(AppStore.self) private var store
    var phone = false

    var body: some View {
        Group {
            if let rows = store.discovery.hidden {
                if rows.isEmpty {
                    Text(L.settingsHiddenEmpty)
                        .font(.ui(phone ? 15 : 13))
                        .foregroundStyle(.haze)
                        .padding(.vertical, 6)
                        .padding(.horizontal, phone ? 0 : 10)
                } else {
                    ForEach(rows, id: \.entry.id) { row in
                        HiddenTitleRow(row: row, phone: phone)
                    }
                }
            }
        }
        .task { store.loadHiddenTitles() }
    }
}

private struct HiddenTitleRow: View {
    @Environment(AppStore.self) private var store
    let row: HiddenTitle
    let phone: Bool

    var body: some View {
        HStack(spacing: 12) {
            ArtworkView(url: row.entry.coverUrl, color: row.entry.plateColor, maxPixelSize: 120)
                .frame(width: 32, height: 48)
                .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: 32)))
            VStack(alignment: .leading, spacing: 2) {
                Text(row.entry.showTitle)
                    .font(.showTitle(size: phone ? 17 : 14, relativeTo: .body))
                    .foregroundStyle(.silk)
                    .lineLimit(2)
                if row.untilAirs {
                    Text(L.settingsHiddenUntilAirs)
                        .font(.ui(phone ? 13 : 12))
                        .foregroundStyle(.haze)
                }
            }
            Spacer(minLength: 8)
            Button(L.settingsRestore) { store.restore(row.entry.id) }
                .buttonStyle(SecondaryCapsuleStyle(height: phone ? 36 : 28, fontSize: phone ? 15 : 13))
        }
        .padding(.vertical, 4)
        .padding(.horizontal, phone ? 0 : 10)
        .accessibilityElement(children: .combine)
        .accessibilityAction(named: L.settingsRestore) { store.restore(row.entry.id) }
    }
}

#if os(iOS)
/// The iPhone's page for it.
struct PhoneHiddenTitlesPage: View {
    var body: some View {
        PhoneList {
            Section {
                HiddenTitlesRows(phone: true)
            } footer: {
                Text(L.settingsHiddenTitlesBody)
            }
        }
        .navigationTitle(L.settingsHiddenTitles)
        .navigationBarTitleDisplayMode(.inline)
    }
}
#endif
