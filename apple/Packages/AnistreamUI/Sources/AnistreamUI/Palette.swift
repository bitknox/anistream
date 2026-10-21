// ⌘K: titles and commands by name, on the Mac and an iPad with a keyboard.

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

/// One line in the palette.
struct PaletteItem: Identifiable {
    enum Kind {
        case title(Entry)
        case command(symbol: String, detail: String?, shortcut: String?, enabled: Bool, run: @MainActor () -> Void)
        case recent(String)
    }

    let id: String
    let name: String
    let kind: Kind
}

/// What the palette lists for `query`: titles the app knows whose names match, the commands that
/// match, and recent searches.
@MainActor
func paletteItems(query: String, store: AppStore, navigator: Navigator) -> (titles: [PaletteItem], commands: [PaletteItem], recent: [PaletteItem]) {
    let q = query.trimmingCharacters(in: .whitespaces).lowercased()
    func matches(_ text: String) -> Bool { q.isEmpty || text.lowercased().contains(q) }

    let titles = store.titles.entries.values
        .filter { !q.isEmpty && (matches($0.title) || matches($0.secondary ?? "")) }
        .sorted { ($0.progress != nil ? 0 : 1, $0.showTitle) < ($1.progress != nil ? 0 : 1, $1.showTitle) }
        .prefix(5)
        .map { PaletteItem(id: "title-\($0.id)", name: $0.showTitle, kind: .title($0)) }

    var commands: [PaletteItem] = []
    /// The focused title's commands are listed whatever the words, the rest when they match.
    var always = false
    func command(_ name: String, _ symbol: String, detail: String? = nil, shortcut: String? = nil,
                 enabled: Bool = true, _ run: @escaping @MainActor () -> Void) {
        guard always || matches(name) else { return }
        commands.append(PaletteItem(
            id: "command-\(name)", name: name,
            kind: .command(symbol: symbol, detail: detail, shortcut: shortcut, enabled: enabled, run: run)
        ))
    }
    if let id = navigator.focusedTitle, let entry = store.titles[id] {
        always = true
        defer { always = false }
        let next = entry.progress?.next ?? 1
        let episodeName = store.titles.episodes[id]?.first { $0.number == String(next) }?.title
        command("Mark Watched", "checkmark",
                detail: episodeName.map { "Episode \(next) · \($0)" } ?? "Episode \(next)",
                shortcut: "⇧⌘U") {
            store.send(.setWatched(id: id, episodes: [String(next)], watched: true))
        }
        command("Hide from Continue", "eye.slash", detail: "Progress is kept", shortcut: "⌘⌫",
                enabled: entry.progress != nil) { store.hideFromContinue(id) }
        command("Show Settings for This Show…", "slider.horizontal.3",
                detail: "Audio, subtitles, auto-skip, source", shortcut: "⌘I", enabled: false) {}
    }
    for (index, section) in [AppSection.continue, .calendar, .seasonal, .library, .downloads].enumerated() {
        command(section.title, section.symbol, shortcut: "⌘\(index + 1)") { navigator.open(section) }
    }
    command("Search", "magnifyingglass", shortcut: "⌘F") { navigator.open(.search) }
    command("Providers", AppSection.providers.symbol, shortcut: "⌘6") { navigator.open(.providers) }
    command("Plugin Stores", AppSection.pluginStores.symbol, shortcut: "⌘7") { navigator.open(.pluginStores) }
    command("Sync Now", "arrow.triangle.2.circlepath", shortcut: "⌘R") { store.send(.syncNow) }
    command("Messages", "text.bubble", shortcut: "⌥⌘L") { navigator.messagesShown = true }
    command("Settings", AppSection.settings.symbol, shortcut: "⌘,") { navigator.open(.settings) }
    // What cannot run now comes last, dimmed.
    commands.sort { lhs, rhs in
        let enabled = { (item: PaletteItem) -> Bool in
            if case .command(_, _, _, let on, _) = item.kind { on } else { true }
        }
        return enabled(lhs) && !enabled(rhs)
    }

    let recent = RecentSearches.shared.queries
        .filter { matches($0) }
        .prefix(3)
        .map { PaletteItem(id: "recent-\($0)", name: $0, kind: .recent($0)) }
    return (titles, Array(commands.prefix(q.isEmpty ? 4 : 6)), recent)
}

/// The palette over the window: a glass panel at the top, the window dimmed behind it.
struct PaletteOverlay: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @State private var query = LaunchRoute.current.palette ?? ""
    @State private var selected = 0
    @FocusState private var focused: Bool

    var body: some View {
        let lists = paletteItems(query: query, store: store, navigator: navigator)
        let all = lists.titles + lists.commands + lists.recent
        ZStack(alignment: .top) {
            Palette.ground.opacity(0.35)
                .ignoresSafeArea()
                .onTapGesture { close() }
            VStack(spacing: 0) {
                HStack(spacing: 12) {
                    Image(systemName: "magnifyingglass").font(.system(size: 17)).foregroundStyle(.haze)
                    TextField("Titles and commands", text: $query)
                        .textFieldStyle(.plain)
                        .font(.system(size: 20))
                        .foregroundStyle(.silk)
                        .focused($focused)
                        .onSubmit { run(all, play: false) }
                }
                .padding(.horizontal, 20)
                .frame(height: 64)
                Rectangle().fill(Palette.glassRule).frame(height: 1)
                VStack(alignment: .leading, spacing: 0) {
                    group("Titles", lists.titles, offset: 0)
                    group("Commands", lists.commands, offset: lists.titles.count)
                    group("Recent", lists.recent, offset: lists.titles.count + lists.commands.count)
                    if all.isEmpty {
                        Text("Return searches for \u{201c}\(query)\u{201d}")
                            .font(.system(size: 13))
                            .foregroundStyle(.haze)
                            .padding(.horizontal, 28)
                            .frame(height: 40)
                    }
                }
                .padding(.vertical, 8)
                Rectangle().fill(Palette.glassRule).frame(height: 1)
                HStack(spacing: 20) {
                    Spacer()
                    hint("↩", "Open")
                    hint("⌘↩", "Play")
                    hint("esc", "Close")
                }
                .font(.system(size: 13))
                .padding(.horizontal, 20)
                .frame(height: 44)
            }
            .frame(width: 640)
            .chromeGlass(in: .rect(cornerRadius: 16))
            .padding(.top, 112)
        }
        .ignoresSafeArea()
        .onAppear { focused = true }
        .onChange(of: query) { selected = 0 }
        .onKeyPress(.downArrow) { selected = min(selected + 1, max(all.count - 1, 0)); return .handled }
        .onKeyPress(.upArrow) { selected = max(selected - 1, 0); return .handled }
        .onKeyPress(.escape) { close(); return .handled }
        .onKeyPress(keys: [.return], phases: .down) { press in
            guard press.modifiers.contains(.command) else { return .ignored }
            run(all, play: true)
            return .handled
        }
        .accessibilityAddTraits(.isModal)
    }

    @ViewBuilder
    private func group(_ name: String, _ items: [PaletteItem], offset: Int) -> some View {
        if !items.isEmpty {
            Text(name)
                .groupHeaderStyle()
                .padding(EdgeInsets(top: 10, leading: 28, bottom: 6, trailing: 28))
            ForEach(Array(items.enumerated()), id: \.element.id) { index, item in
                PaletteRow(item: item, selected: offset + index == selected) {
                    selected = offset + index
                    run([item], play: false)
                }
            }
        }
    }

    private func hint(_ key: String, _ name: String) -> some View {
        HStack(spacing: 6) {
            Text(key).fontWeight(.semibold).foregroundStyle(.silk)
            Text(name).foregroundStyle(.haze)
        }
    }

    private func run(_ items: [PaletteItem], play: Bool) {
        let item = items.count == 1 ? items[0] : (items.indices.contains(selected) ? items[selected] : nil)
        guard let item else {
            if !query.isEmpty { navigator.search(query, in: store) }
            close()
            return
        }
        switch item.kind {
        case .title(let entry):
            if play, entry.progress == nil, entry.listStatus == nil {
                store.setStatus(entry.id, .planning)
            } else if play {
                store.send(.play(id: entry.id, episode: String(TitleAction(entry).episode)))
            } else {
                navigator.openTitle(entry.id)
            }
        case .command(_, _, _, let enabled, let action):
            guard enabled else { return }
            action()
        case .recent(let query):
            navigator.search(query, in: store)
        }
        close()
    }

    private func close() {
        navigator.paletteShown = false
    }
}

/// A line in the palette: a title with its cover and what its action plays, a command with its
/// shortcut, a recent search with when.
struct PaletteRow: View {
    let item: PaletteItem
    let selected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 14) {
                switch item.kind {
                case .title(let entry):
                    ArtworkView(url: entry.coverUrl, color: entry.plateColor, maxPixelSize: 84)
                        .frame(width: 28, height: 42)
                        .clipShape(.rect(cornerRadius: 4))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(entry.showTitle)
                            .font(.showTitle(size: 15))
                            .foregroundStyle(selected ? .amber : .silk)
                        Text([entry.alternateTitle, entry.listStatus?.label.capitalizedSentence]
                            .compactMap { $0 }.joined(separator: " · "))
                            .font(.system(size: 13))
                            .foregroundStyle(.haze)
                    }
                    Spacer()
                    Text(paletteAction(entry))
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.silk)
                case .command(let symbol, let detail, let shortcut, _, _):
                    Image(systemName: symbol)
                        .font(.system(size: 15))
                        .frame(width: 28)
                        .foregroundStyle(.haze)
                    Text(item.name).font(.system(size: 15)).foregroundStyle(.silk)
                    if let detail { Text(detail).font(.system(size: 13)).foregroundStyle(.haze) }
                    Spacer()
                    if let shortcut { Text(shortcut).font(.system(size: 13)).foregroundStyle(.haze) }
                case .recent(let query):
                    Image(systemName: "clock")
                        .font(.system(size: 15))
                        .frame(width: 28)
                        .foregroundStyle(.haze)
                    Text(query).font(.system(size: 15)).foregroundStyle(.silk)
                    Spacer()
                }
            }
            .opacity(isEnabled ? 1 : 0.45)
            .padding(.horizontal, 16)
            .frame(height: item.isTitle ? 56 : 40)
            .background(selected ? Palette.selection : .clear, in: .rect(cornerRadius: 10))
            .contentShape(.rect(cornerRadius: 10))
            .padding(.horizontal, 12)
        }
        .buttonStyle(.plain)
    }

    private var isEnabled: Bool {
        if case .command(_, _, _, let enabled, _) = item.kind { enabled } else { true }
    }

    private func paletteAction(_ entry: Entry) -> String {
        guard let progress = entry.progress else {
            return entry.listStatus == nil ? "Add to Planning" : "Start E1"
        }
        return entry.resume != nil ? "Resume E\(progress.next)" : "Start E\(progress.next)"
    }
}

extension PaletteItem {
    var isTitle: Bool {
        if case .title = kind { true } else { false }
    }
}
