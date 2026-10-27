// The source picker: which source a title's episodes come from, or which one release of a
// torrent source. Automatic first, then each source in the order they are tried, with what it
// has of the title. Mac opens it from the Episode menu and the palette, iPhone and iPad from the
// title page's More menu; one list serves all three.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

/// A title as the picker sheet's item.
struct SourcePickerItem: Identifiable {
    let id: AnilistId
}

extension View {
    /// The source picker for whichever title the navigator names: a sheet over the window on the
    /// Mac and iPad, the medium detent on iPhone.
    func sourcePickerSheet(_ navigator: Navigator) -> some View {
        sheet(item: Binding<SourcePickerItem?>(
            get: { navigator.sourcePicker.map(SourcePickerItem.init) },
            set: { navigator.sourcePicker = $0?.id }
        )) { item in
            SourcePickerSheet(id: item.id)
            #if os(iOS)
                .presentationDetents([.medium, .large])
            #endif
        }
    }
}

/// The sheet's chrome around the list: a header with the title on the Mac, a navigation bar with
/// Done on iPhone and iPad.
struct SourcePickerSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    let id: AnilistId

    var body: some View {
        let entry = store.titles[id]
        #if os(macOS)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 14) {
                if let entry { CoverArt(entry: entry, width: 36) }
                VStack(alignment: .leading, spacing: 2) {
                    Text(entry?.showTitle ?? "").font(.macShowTitle(size: 15)).foregroundStyle(.silk).lineLimit(1)
                    Text(L.sourcePickerTitle).font(.mac(13)).foregroundStyle(.haze)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 24)
            .padding(.top, 24)
            .padding(.bottom, 12)
            SourcePickerList(id: id, done: { dismiss() })
            HStack {
                Spacer()
                Button("Done") { dismiss() }
                    .buttonStyle(PrimaryCapsuleStyle(height: 34))
                    .keyboardShortcut(.defaultAction)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
        }
        .frame(width: 480)
        .frame(minHeight: 360, idealHeight: 520)
        .background(.raised)
        .closesOnEscape { dismiss() }
        #else
        NavigationStack {
            SourcePickerList(id: id, done: { dismiss() })
                .navigationTitle(L.sourcePickerTitle)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Done") { dismiss() }.fontWeight(.semibold).tint(.silk)
                    }
                }
        }
        #endif
    }
}

/// Automatic, then the sources. Picking one sends the choice and closes the picker.
struct SourcePickerList: View {
    @Environment(AppStore.self) private var store
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let id: AnilistId
    let done: () -> Void

    var body: some View {
        let options = store.sourceChoices.options[id]
        let loading = store.sourceChoices.isLoading(id)
        let automatic = options.map { $0.allSatisfy { !$0.pinned } } ?? false
        List {
            Section {
                SourcePickerRow(
                    title: L.sourcePickerAutomatic, detail: L.sourcePickerAutomaticDetail,
                    chosen: automatic
                ) {
                    store.chooseSource(id, provider: nil)
                    done()
                }
                if let options {
                    ForEach(options, id: \.providerId) { option in
                        sourceRows(option)
                    }
                    if options.isEmpty {
                        Text(L.sourcesEmpty)
                            .font(PickerType.detail)
                            .foregroundStyle(.haze)
                            .frame(minHeight: PickerType.row, alignment: .leading)
                            .pickerRow()
                    }
                } else if loading {
                    ProgressView()
                        .controlSize(.small)
                        .frame(maxWidth: .infinity, minHeight: PickerType.row)
                        .pickerRow()
                }
            }
        }
        .pickerListStyle()
        .animation(reduceMotion ? nil : .standard, value: options?.count)
        .task(id: id) { store.loadSourceOptions(id) }
        .onDisappear { store.sourceChoices.giveUp(id) }
    }

    @ViewBuilder
    private func sourceRows(_ option: SourceOption) -> some View {
        let releaseShown = option.pinned ? option.pinnedRelease.flatMap { pin in option.releases.first { $0.id == pin } } : nil
        SourcePickerRow(
            title: option.name,
            detail: SourcePickerWords.detail(option),
            chosen: option.pinned && releaseShown == nil,
            dimmed: option.note != nil,
            available: option.note != .gone
        ) {
            store.chooseSource(id, provider: option.providerId)
            done()
        }
        if option.releases.count > 1 {
            Text(L.sourcePickerReleases)
                .font(PickerType.detail.weight(.medium))
                .foregroundStyle(.haze)
                .padding(.leading, 20)
                .padding(.top, 8)
                .accessibilityAddTraits(.isHeader)
                .pickerRow()
            ForEach(option.releases, id: \.id) { release in
                SourcePickerRow(
                    title: release.title, detail: SourcePickerWords.detail(release),
                    chosen: option.pinned && option.pinnedRelease == release.id, inset: true
                ) {
                    store.chooseRelease(id, provider: option.providerId, release: release)
                    done()
                }
            }
        }
    }
}

/// The picker's type: the Mac's sizes on the Mac, the touch sizes elsewhere.
enum PickerType {
    #if os(macOS)
    static let title = Font.mac(14)
    static let detail = Font.mac(12)
    static let check = Font.mac(13, weight: .semibold)
    static let row: CGFloat = 44
    #else
    static let title = Font.ui(17)
    static let detail = Font.ui(13)
    static let check = Font.ui(15, weight: .semibold)
    static let row: CGFloat = 52
    #endif
}

/// What a row says under its name.
enum SourcePickerWords {
    /// A source's coverage ("6 of 13 episodes"), or why it has none. A failure says what the
    /// source itself said, as it said it.
    static func detail(_ option: SourceOption) -> String {
        switch option.note {
        case nil: L.sourcePickerCoverage(covered: Int(option.covered), of: Int(option.of))
        case .off: L.sourceNoteOff
        case .resting: L.sourceNoteResting
        case .gone: L.sourceNoteGone
        case .ambiguous: L.sourceNoteAmbiguous
        case .failed(let reason): reason
        }
    }

    /// A release's quality, size and seeders: "1080p · 14.2 GiB · 84 seeders".
    static func detail(_ release: SourceCandidate) -> String {
        [
            release.quality.map { "\($0)p" },
            release.size,
            release.seeders.map { L.sourceReleaseSeeders(count: Int($0)) },
        ]
        .compactMap { $0 }
        .joined(separator: " · ")
    }
}

/// One choice: its name, what it has under it, and a check where it is the one chosen. A
/// dimmed row stays a button; only a source no longer installed cannot be picked.
struct SourcePickerRow: View {
    let title: String
    let detail: String
    let chosen: Bool
    var dimmed = false
    var available = true
    var inset = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(PickerType.title)
                        .foregroundStyle(dimmed ? .haze : .silk)
                        .lineLimit(2)
                    if !detail.isEmpty {
                        Text(detail)
                            .font(PickerType.detail)
                            .foregroundStyle(.haze)
                            .lineLimit(2)
                    }
                }
                Spacer(minLength: 8)
                if chosen {
                    Image(systemName: "checkmark")
                        .font(PickerType.check)
                        .foregroundStyle(.silk)
                        .accessibilityHidden(true)
                }
            }
            .padding(.leading, inset ? 20 : 0)
            .frame(minHeight: PickerType.row)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .disabled(!available)
        .pointerHover()
        .pickerRow()
        .accessibilityElement(children: .combine)
        .accessibilityLabel(title)
        .accessibilityValue([detail, chosen ? L.sourcePickerChosen : nil].compactMap { $0 }.joined(separator: ", "))
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

private extension View {
    /// A row of the picker's list: on the sheet's own surface, with no rule between rows.
    func pickerRow() -> some View {
        listRowBackground(Color.clear)
            .listRowSeparator(.hidden)
            .listRowInsets(EdgeInsets(top: 0, leading: 20, bottom: 0, trailing: 20))
    }

    func pickerListStyle() -> some View {
        #if os(macOS)
        listStyle(.plain)
            .scrollContentBackground(.hidden)
        #else
        listStyle(.plain)
            .scrollContentBackground(.hidden)
            .background(.raised)
        #endif
    }
}
