// iPhone and iPad: the tab bar, Search on its own, and Continue as PhoneHome draws it.

#if os(iOS)
import AnistreamDesign
import AnistreamKit
import AnistreamStore
import SwiftUI

struct TabShell: View {
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var section: AppSection = .continue

    var body: some View {
        TabView(selection: $section) {
            ForEach([AppSection.continue, .calendar, .seasonal, .library]) { section in
                Tab(section.title, systemImage: section.symbol, value: section) {
                    NavigationStack {
                        screen(section)
                            .navigationTitle(section.title)
                            .background(.ground)
                    }
                }
            }
            Tab(value: AppSection.search, role: .search) {
                NavigationStack {
                    NotBuiltYet(section: .search)
                        .navigationTitle("Search")
                        .background(.ground)
                }
            }
        }
        .tabViewStyle(.sidebarAdaptable)
        .tabBarMinimizeBehavior(.onScrollDown)
    }

    @ViewBuilder
    private func screen(_ section: AppSection) -> some View {
        switch section {
        case .continue:
            if sizeClass == .regular {
                ContinueScreen()
            } else {
                PhoneContinue()
            }
        default:
            NotBuiltYet(section: section)
        }
    }
}

/// Continue on an iPhone: the next episode as the hero, the rest of the queue as a rail of
/// banner cards, then New this week.
struct PhoneContinue: View {
    @Environment(AppStore.self) private var store

    var body: some View {
        let queue = store.continuing.queue(store.titles)
        ScrollView {
            switch store.continuing.list.state {
            case .empty:
                NothingWatched().padding(.top, 80)
            case .failed(let reason):
                ListFailed(reason: reason)
            default:
                VStack(alignment: .leading, spacing: 0) {
                    if let hero = queue.inProgress.first ?? queue.newThisWeek.first {
                        PhoneHero(item: hero)
                            .padding(.top, 8)
                    }
                    let rail = Array(queue.inProgress.dropFirst())
                    if !rail.isEmpty {
                        Text("In progress")
                            .font(.system(size: 20, weight: .semibold))
                            .foregroundStyle(.silk)
                            .accessibilityAddTraits(.isHeader)
                            .padding(.top, 18)
                        ScrollView(.horizontal) {
                            HStack(alignment: .top, spacing: 12) {
                                ForEach(rail) { PhoneCard(item: $0) }
                            }
                            .padding(.horizontal, 16)
                        }
                        .scrollIndicators(.never)
                        .padding(.horizontal, -16)
                        .padding(.top, 10)
                    }
                    let fresh = queue.inProgress.isEmpty ? Array(queue.newThisWeek.dropFirst()) : queue.newThisWeek
                    if !fresh.isEmpty {
                        Text("New this week")
                            .font(.system(size: 20, weight: .semibold))
                            .foregroundStyle(.silk)
                            .accessibilityAddTraits(.isHeader)
                            .padding(.top, 16)
                        ForEach(fresh) { PhoneNewRow(item: $0) }
                            .padding(.top, 2)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 24)
            }
        }
        .scrollContentBackground(.hidden)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button("Accounts and settings", systemImage: "person.crop.circle") {}
            }
        }
        .task { store.loadContinue() }
        .task(id: queue.inProgress.first?.id) {
            if let id = queue.inProgress.first?.id, store.titles.episodes[id] == nil {
                store.loadEpisodes(id)
            }
        }
    }
}

/// The next episode: its banner, the title, the episode, how far into it and the amber action.
struct PhoneHero: View {
    @Environment(AppStore.self) private var store
    let item: ContinueItem

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ArtworkView(url: item.entry.bannerUrl ?? item.entry.coverUrl, maxPixelSize: 1100)
                .aspectRatio(16 / 9, contentMode: .fit)
                .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
            Text(item.entry.showTitle)
                .font(.showTitle(size: 22, relativeTo: .title2))
                .foregroundStyle(.silk)
                .padding(.top, 8)
            Text(item.episodeLine(store.titles))
                .font(.system(size: 15))
                .foregroundStyle(.haze)
                .padding(.top, 2)
            HStack(spacing: 14) {
                VStack(alignment: .leading, spacing: 6) {
                    if let fraction = item.fraction {
                        Meter(fraction: fraction, height: 4)
                    }
                    if let left = item.timeLeft {
                        Text(left).font(.system(size: 13)).foregroundStyle(.haze)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Button {
                    store.send(.play(id: item.id, episode: String(item.episode)))
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "play.fill").font(.system(size: 12))
                        Text(item.action)
                    }
                    .lineLimit(1)
                    .fixedSize()
                }
                .buttonStyle(PrimaryCapsuleStyle(height: 44, fontSize: 17))
                .layoutPriority(1)
            }
            .padding(.top, 8)
        }
    }
}

/// A title under way in the rail: its banner, how far into the episode, the title and what is
/// left.
struct PhoneCard: View {
    let item: ContinueItem

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ArtworkView(url: item.entry.bannerUrl ?? item.entry.coverUrl, maxPixelSize: 640)
                .frame(width: 208, height: 117)
                .clipShape(.rect(cornerRadius: ArtShape.bannerRadius))
            Group {
                if let fraction = item.fraction {
                    Meter(fraction: fraction, height: 3)
                } else {
                    Color.clear.frame(height: 3)
                }
            }
            .padding(.top, 6)
            Text(item.entry.showTitle)
                .font(.showTitle(size: 17, relativeTo: .body))
                .foregroundStyle(.silk)
                .lineLimit(1)
                .padding(.top, 6)
            Text(item.timeLeft.map { "E\(item.episode) · \($0)" } ?? item.action)
                .font(.system(size: 15))
                .foregroundStyle(.haze)
        }
        .frame(width: 208, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

/// A new episode: the cover, the title and what it plays, with the water dot.
struct PhoneNewRow: View {
    let item: ContinueItem

    var body: some View {
        HStack(spacing: 12) {
            ArtworkView(url: item.entry.coverUrl, maxPixelSize: 120)
                .frame(width: 40, height: 60)
                .clipShape(.rect(cornerRadius: ArtShape.coverRadius(width: 40)))
            VStack(alignment: .leading, spacing: 2) {
                Text(item.entry.showTitle)
                    .font(.showTitle(size: 17, relativeTo: .body))
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                HStack(spacing: 6) {
                    Circle().fill(.water).frame(width: 6, height: 6)
                    Text(item.action)
                }
                .font(.system(size: 15))
                .foregroundStyle(.haze)
            }
            Spacer(minLength: 0)
            Image(systemName: "chevron.right")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.haze)
        }
        .frame(height: 64)
        .accessibilityElement(children: .combine)
    }
}
#endif
