// Settings › Playback › Language's first row, Title language (MacSettingsTitles,
// PhoneSettingsTitles): AniList's choice, English or Romaji, with what each does said under the
// label and each choice's menu item showing a title the way it would read.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// Which row of the Playback pane the column beside it is about.
public enum PlaybackFocus: Sendable {
    case upscaling, titleLanguage
}

/// The three choices, as the settings file names them.
enum TitleChoice: String, CaseIterable, Sendable {
    case anilist, english, romaji

    /// As the menu says it.
    var label: String {
        switch self {
        case .anilist: "AniList's choice"
        case .english: "English"
        case .romaji: "Romaji"
        }
    }
}

/// What the title-language row says.
enum TitleLanguageWords {
    /// The line under the label: "Romaji, from your AniList account" for AniList's choice, or
    /// what English and Romaji each mean.
    static func line(_ choice: TitleChoice, signedIn: Bool, viewer: TitleLanguage?) -> String {
        switch choice {
        case .anilist:
            guard signedIn, let viewer else { return "English while signed out of AniList" }
            return "\(name(viewer)), from your AniList account"
        case .english: return "Romaji where there is no English title"
        case .romaji: return "Japanese titles in Latin letters"
        }
    }

    /// The preview column's note under the examples.
    static func note(_ choice: TitleChoice, signedIn: Bool, viewer: TitleLanguage?, examples: [Entry]) -> String {
        switch choice {
        case .anilist:
            let signedOut = "While you are signed out of AniList, titles show in English."
            guard signedIn, let viewer else { return "Follows your AniList account. " + signedOut }
            return "Follows your AniList account, which is set to \(name(viewer).lowercased()). " + signedOut
        case .english:
            if let kept = examples.first(where: { ($0.titleEnglish ?? "").isEmpty }) {
                return "\(kept.titleRomaji ?? kept.title) has no English title on AniList, so it keeps its romaji one."
            }
            return "English titles as AniList lists them, and romaji where there is none."
        case .romaji:
            return "Japanese titles written in Latin letters, as AniList lists them."
        }
    }

    static func name(_ language: TitleLanguage) -> String {
        switch language {
        case .romaji: "Romaji"
        case .english: "English"
        case .native: "Native"
        }
    }

    /// `entry`'s title as `choice` would name it, the way the engine picks: the form asked for,
    /// else English, else romaji, else native. AniList's choice follows the signed-in account,
    /// and English while nobody is.
    static func example(_ entry: Entry, _ choice: TitleChoice, signedIn: Bool, viewer: TitleLanguage?) -> String {
        let language: TitleLanguage = switch choice {
        case .anilist: signedIn ? (viewer ?? .english) : .english
        case .english: .english
        case .romaji: .romaji
        }
        let chosen = switch language {
        case .romaji: entry.titleRomaji
        case .english: entry.titleEnglish
        case .native: entry.titleNative
        }
        return [chosen, entry.titleEnglish, entry.titleRomaji, entry.titleNative]
            .compactMap { $0 }
            .first { !$0.isEmpty } ?? entry.title
    }
}

extension AppStore {
    /// Whether AniList is signed in, for the title-language row's words.
    var anilistSignedIn: Bool {
        accounts.summary.state != .signedOut
    }

    /// The choice in effect, AniList's until the settings are here.
    var titleChoice: TitleChoice {
        if case .str(let name)? = settings.row(.titles)?.option?.value, let choice = TitleChoice(rawValue: name) {
            return choice
        }
        return .anilist
    }

    /// Choose how every title is named.
    func choose(_ choice: TitleChoice) {
        guard choice != titleChoice else { return }
        setSetting(.titles, to: .str(choice.rawValue))
    }

    /// Three titles to show the choice on: Continue's, else the Library's.
    var titleLanguageExamples: [Entry] {
        let continuing = continuing.queue(titles).all.map(\.entry)
        let library = LibraryModel.order.flatMap { library.segment($0).state.ids }.compactMap { titles[$0] }
        return Array((continuing + library).filter { $0.coverUrl != nil }.reduce(into: [Entry]()) { kept, entry in
            if !kept.contains(where: { $0.id == entry.id }) { kept.append(entry) }
        }.prefix(3))
    }

    /// The first example as `choice` would name it, for a menu item's second line.
    func titleExample(_ choice: TitleChoice) -> String? {
        titleLanguageExamples.first.map {
            TitleLanguageWords.example($0, choice, signedIn: anilistSignedIn, viewer: accounts.viewerTitleLanguage)
        }
    }
}

/// The menu's items: each choice ticked when in effect, with a title as it would read under it.
private struct TitleChoiceItems: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let current = store.titleChoice
        ForEach(TitleChoice.allCases, id: \.self) { choice in
            Toggle(isOn: Binding(get: { current == choice }, set: { if $0 { store.choose(choice) } })) {
                Text(choice.label)
                if let example = store.titleExample(choice) { Text(example) }
            }
        }
    }
}

/// The Mac's and iPad's row: a popup and the line under the label.
struct TitleLanguageRow: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator

    var body: some View {
        let selected = navigator.playbackFocus == .titleLanguage
        let choice = store.titleChoice
        SettingsRow(
            title: SettingKey.titles.title,
            note: TitleLanguageWords.line(choice, signedIn: store.anilistSignedIn, viewer: store.accounts.viewerTitleLanguage),
            selected: selected
        ) {
            Menu {
                TitleChoiceItems()
            } label: {
                MenuCapsule(text: choice.label)
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .fixedSize()
            .disabled(store.settings.row(.titles) == nil)
            .accessibilityLabel(SettingKey.titles.title)
        }
        .onTapGesture { navigator.playbackFocus = .titleLanguage }
        .task { store.send(.loadTitleLanguage) }
    }
}

/// The column beside the pane while the row is selected: three covers with their titles as the
/// current choice names them, and what the choice does.
struct TitleLanguagePreview: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let choice = store.titleChoice
        let examples = store.titleLanguageExamples
        let viewer = store.accounts.viewerTitleLanguage
        VStack(alignment: .leading, spacing: 12) {
            Text("Title language").groupHeaderStyle().macLineHeight(14)
                .accessibilityAddTraits(.isHeader)
            HStack(alignment: .top, spacing: 12) {
                ForEach(examples, id: \.id) { entry in
                    VStack(alignment: .leading, spacing: 6) {
                        CoverArt(entry: entry, width: 88)
                        Text(TitleLanguageWords.example(entry, choice, signedIn: store.anilistSignedIn, viewer: viewer))
                            .font(.macShowTitle(size: 12))
                            .foregroundStyle(.silk)
                            .lineLimit(2)
                    }
                    .frame(width: 88, alignment: .leading)
                }
            }
            Text(TitleLanguageWords.note(choice, signedIn: store.anilistSignedIn, viewer: viewer, examples: examples))
                .font(.mac(13))
                .macLineHeight(18)
                .foregroundStyle(.silk)
                .fixedSize(horizontal: false, vertical: true)
        }
        // The examples are what Continue lists, asked for when Settings opens first.
        .task { store.loadContinue() }
    }
}

#if os(iOS)
/// The iPhone's row: a menu whose items each carry the example title under their label, in place
/// of a preview column.
struct PhoneTitleLanguageRow: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let choice = store.titleChoice
        Menu {
            TitleChoiceItems()
        } label: {
            HStack {
                PhoneRowTitle(
                    title: SettingKey.titles.title,
                    note: TitleLanguageWords.line(choice, signedIn: store.anilistSignedIn, viewer: store.accounts.viewerTitleLanguage)
                )
                Spacer()
                HStack(spacing: 6) {
                    Text(choice.label)
                    Image(systemName: "chevron.up.chevron.down").font(.ui(12, weight: .semibold))
                }
                .font(.ui(17))
                .foregroundStyle(.haze)
            }
            // A menu's label centres its lines; the row's title and note sit flush left.
            .multilineTextAlignment(.leading)
        }
        .disabled(store.settings.row(.titles) == nil)
        .listRowBackground(Palette.raised)
        .task {
            store.send(.loadTitleLanguage)
            store.loadContinue()
        }
    }
}
#endif
