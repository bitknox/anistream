// The metadata on a title's page and in its "More about this show" sheet (native-spec/metadata.md;
// MacTitleMeta, MacMoreAbout, PadTitleMeta, PadMoreAbout, PhoneTitleMeta, PhoneMoreAbout).

import AnistreamDesign
import AnistreamKit
import AnistreamStore
import AnistreamStrings
import SwiftUI

// MARK: - On the page

/// The other forms of the title, under it, in haze. On Mac and iPad one line, cut at the end; on
/// iPhone both forms when they fit on one line, else the first, cut.
struct OtherTitlesLine: View {
    let entry: Entry
    var phone = false

    var body: some View {
        let forms = entry.otherTitles
        if !forms.isEmpty {
            Group {
                if phone, forms.count > 1 {
                    ViewThatFits(in: .horizontal) {
                        Text(forms.joined(separator: " · ")).fixedSize(horizontal: true, vertical: false)
                        Text(forms[0])
                    }
                } else {
                    Text(forms.joined(separator: " · "))
                }
            }
            .font(phone ? .ui(15) : .mac(15))
            .foregroundStyle(.haze)
            .lineLimit(1)
        }
    }
}

/// Text cut at `lines` with "More" at the end of its last line, which shows the rest in place.
/// No "Less": the page goes on below it. The "More" takes a 44 pt target on touch.
struct InlineMoreText: View {
    let text: String
    let lines: Int
    let font: Font
    @State private var expanded = false
    @State private var full: CGFloat = 0
    @State private var shown: CGFloat = 0

    var body: some View {
        Text(text)
            .font(font)
            .foregroundStyle(.haze)
            .lineLimit(expanded ? nil : lines)
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { shown = $0 }
            .background(alignment: .topLeading) {
                Text(text)
                    .font(font)
                    .fixedSize(horizontal: false, vertical: true)
                    .hidden()
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { full = $0 }
            }
            .overlay(alignment: .bottomTrailing) {
                if !expanded, full > shown + 1 {
                    Button(L.actionMore) { withAnimation(.standard) { expanded = true } }
                        .buttonStyle(.plain)
                        .font(font.weight(.semibold))
                        .foregroundStyle(.silk)
                        .padding(.leading, 28)
                        .background(LinearGradient(
                            stops: [.init(color: Palette.ground.opacity(0), location: 0),
                                    .init(color: Palette.ground, location: 0.45)],
                            startPoint: .leading, endPoint: .trailing))
                        .contentShape(.rect.inset(by: -12))
                        .accessibilityHint(L.titleSynopsisHint)
                }
            }
    }
}

/// The details grid beside the episodes on Mac and iPad: Studio, Based on, Season, the airing
/// fact, Also on, and the top three tags, which wrap to two lines and are then cut.
struct MetaFactsGrid: View {
    let entry: Entry

    var body: some View {
        let tags = entry.topTags
        Grid(alignment: .leading, horizontalSpacing: 12, verticalSpacing: 10) {
            ForEach(entry.facts(), id: \.label) { fact in
                GridRow {
                    Text(fact.label).foregroundStyle(.haze).frame(width: 124, alignment: .leading)
                    Text(fact.value).foregroundStyle(.silk).lineLimit(2)
                }
            }
            if !tags.isEmpty {
                GridRow(alignment: .top) {
                    Text(L.titleTags).foregroundStyle(.haze).frame(width: 124, alignment: .leading)
                    Text(tags.joined(separator: ", ")).foregroundStyle(.silk).lineLimit(2)
                }
            }
        }
        .font(.mac(12))
    }
}

/// What opens the sheet: on the Mac "More About This Show" at the end of the details column, on
/// iPad the same in sentence case, 44 high.
struct MoreAboutButton: View {
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    var body: some View {
        #if os(macOS)
        let label = L.titleMoreAbout
        let height: CGFloat = 28
        #else
        let label = L.titleMoreAbout
        let height: CGFloat = 44
        #endif
        Button {
            navigator.moreAbout = entry.id
        } label: {
            HStack(spacing: 6) {
                Text(label)
                Image(systemName: "chevron.right").font(.mac(10, weight: .semibold))
            }
        }
        .buttonStyle(SecondaryCapsuleStyle(height: height))
    }
}

/// The iPhone's opener: a row under the tags line, 44 high, silk semibold with a chevron.
struct MoreAboutRow: View {
    @Environment(Navigator.self) private var navigator
    let entry: Entry

    var body: some View {
        Button {
            navigator.moreAbout = entry.id
        } label: {
            HStack {
                Text(L.titleMoreAbout).font(.ui(17, weight: .semibold)).foregroundStyle(.silk)
                Spacer()
                Image(systemName: "chevron.right").font(.ui(15, weight: .semibold)).foregroundStyle(.haze)
            }
            .frame(minHeight: 44)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
    }
}

/// A related title: its cover, its title, and how it relates, with its format and year. Opens
/// its page.
struct RelatedCard: View {
    @Environment(Navigator.self) private var navigator
    let linked: LinkedTitle
    /// The cover's size: 36 by 54 on the Mac, 40 by 60 on iPad and iPhone.
    let cover: CGSize
    var phone = false

    var body: some View {
        Button {
            navigator.openTitle(linked.id)
        } label: {
            HStack(spacing: 10) {
                ArtworkView(url: linked.coverUrl, maxPixelSize: Int(cover.width * 3), flat: Palette.artPlaceholder)
                    .frame(width: cover.width, height: cover.height)
                    .clipShape(.rect(cornerRadius: 6))
                VStack(alignment: .leading, spacing: 0) {
                    Text(linked.shownTitle)
                        .font(phone ? .showTitle(size: 17) : .macShowTitle(size: 13))
                        .foregroundStyle(.silk)
                        .lineLimit(1)
                    Text(linked.relationLine)
                        .font(phone ? .ui(15) : .mac(12))
                        .foregroundStyle(.haze)
                        .lineLimit(1)
                }
            }
            .padding(EdgeInsets(top: 4, leading: 4, bottom: 4, trailing: 14))
            .contentShape(.rect(cornerRadius: 10))
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }
}

// MARK: - The sheet

/// "More about this show": the rankings, cast, staff, recommendations and tags AniList has for a
/// title, and on iPhone its synopsis and facts first. A window sheet on the Mac and iPad, two
/// columns; a sheet at the large detent on iPhone, one.
struct MoreAboutSheet: View {
    @Environment(AppStore.self) private var store
    @Environment(Navigator.self) private var navigator
    @Environment(\.dismiss) private var dismiss
    let id: AnilistId
    /// The spoiler tags, shown for as long as the sheet is open and never remembered.
    @State private var spoilersShown = false
    @State private var asked = ContinuousClock.now
    @State private var now = ContinuousClock.now

    private var phone: Bool {
        #if os(iOS)
        UIDevice.current.userInterfaceIdiom == .phone
        #else
        false
        #endif
    }

    var body: some View {
        let entry = store.titles[id] ?? Entry.placeholder(id)
        VStack(spacing: 0) {
            header(entry)
            ScrollView {
                Group {
                    if phone { phoneBody(entry) } else { wideBody(entry) }
                }
                .padding(EdgeInsets(top: 4, leading: phone ? 16 : 40, bottom: 32, trailing: phone ? 16 : 40))
            }
            .scrollIndicators(.automatic)
        }
        .background(.raised)
        .task(id: id) {
            if store.titles[id]?.detail == nil {
                asked = .now
                store.loadDetail(id)
            }
            // The failed state's clock.
            while !Task.isCancelled, store.titles[id]?.detail == nil {
                try? await Task.sleep(for: .seconds(1))
                now = .now
            }
        }
    }

    // MARK: Header

    private func header(_ entry: Entry) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            if phone {
                VStack(alignment: .leading, spacing: 2) {
                    Text(entry.showTitle)
                        .font(.showTitle(size: 22, relativeTo: .title2))
                        .foregroundStyle(.silk)
                        .lineLimit(2)
                    OtherTitlesLine(entry: entry, phone: true)
                }
            } else {
                Text(entry.showTitle)
                    .font(.macShowTitle(size: 22, relativeTo: .title2))
                    .foregroundStyle(.silk)
                    .lineLimit(1)
                Text(sheetName).font(.mac(15)).foregroundStyle(.haze).lineLimit(1)
            }
            Spacer(minLength: 12)
            Button(L.actionDone) { close() }
                .buttonStyle(SecondaryCapsuleStyle(height: phone ? 44 : 32, raisedSurface: true, fontSize: phone ? 17 : 13))
                .keyboardShortcut(.cancelAction)
        }
        .padding(EdgeInsets(top: phone ? 20 : 28, leading: phone ? 16 : 40, bottom: 16, trailing: phone ? 16 : 28))
        .accessibilityElement(children: .contain)
    }

    private var sheetName: String { L.titleMoreAbout }

    private func close() {
        navigator.moreAbout = nil
        dismiss()
    }

    // MARK: Layouts

    /// Mac and iPad: rankings and cast on the left, staff and tags on the right, recommendations
    /// across the foot.
    @ViewBuilder
    private func wideBody(_ entry: Entry) -> some View {
        if let detail = entry.detail {
            if detail.isEmpty {
                emptyState
            } else {
                VStack(alignment: .leading, spacing: 32) {
                    HStack(alignment: .top, spacing: 48) {
                        VStack(alignment: .leading, spacing: 28) {
                            rankings(detail)
                            cast(detail, entry: entry)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        VStack(alignment: .leading, spacing: 28) {
                            staff(detail)
                            tags(entry)
                        }
                        .frame(width: 300, alignment: .leading)
                    }
                    recommendations(detail)
                }
            }
        } else {
            waiting
        }
    }

    /// iPhone: About first, then each section under the other, the cast and recommendations as
    /// rails.
    @ViewBuilder
    private func phoneBody(_ entry: Entry) -> some View {
        VStack(alignment: .leading, spacing: 32) {
            about(entry)
            if let detail = entry.detail {
                if detail.isEmpty {
                    emptyState
                } else {
                    rankings(detail)
                    cast(detail, entry: entry)
                    staff(detail)
                    recommendations(detail)
                    tags(entry)
                }
            } else {
                waiting
            }
        }
    }

    // MARK: Sections

    private func heading(_ name: String, trailing: String? = nil) -> some View {
        HStack(alignment: .firstTextBaseline) {
            if phone {
                Text(name).font(.ui(22, weight: .bold)).foregroundStyle(.silk)
            } else {
                Text(name).groupHeaderStyle()
            }
            Spacer()
            if let trailing {
                Text(trailing).font(phone ? .ui(15) : .mac(12)).foregroundStyle(.haze)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }

    @ViewBuilder
    private func about(_ entry: Entry) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            if !entry.shownSynopsis.isEmpty {
                Text(entry.shownSynopsis).font(.ui(17)).foregroundStyle(.silk)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Grid(alignment: .leading, horizontalSpacing: 16, verticalSpacing: 10) {
                ForEach(entry.facts(), id: \.label) { fact in
                    GridRow {
                        Text(fact.label).foregroundStyle(.haze).frame(width: 104, alignment: .leading)
                        Text(fact.value).foregroundStyle(.silk)
                    }
                }
            }
            .font(.ui(17))
        }
    }

    @ViewBuilder
    private func rankings(_ detail: TitleDetail) -> some View {
        let rankings = detail.orderedRankings
        if !rankings.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                heading(L.moreRankings)
                FlowLayout(spacing: 36, lineSpacing: 6) {
                    ForEach(Array(rankings.enumerated()), id: \.offset) { _, ranking in
                        Text("\(Text("#\(ranking.rank)").fontWeight(.semibold).foregroundStyle(.silk)) \(ranking.words)")
                            .foregroundStyle(.haze)
                            .font(phone ? .ui(17) : .mac(13))
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func cast(_ detail: TitleDetail, entry: Entry) -> some View {
        let cast = detail.shownCast
        if !cast.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                heading(L.moreCast, trailing: L.moreCastVoices)
                if phone {
                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 12) {
                            ForEach(Array(cast.enumerated()), id: \.offset) { _, member in
                                VStack(alignment: .leading, spacing: 4) {
                                    art(member, entry: entry, size: CGSize(width: 84, height: 126))
                                    Text(member.name).font(.ui(17)).foregroundStyle(.silk).lineLimit(2)
                                    Text(TitleWords.role(member.role)).font(.ui(15)).foregroundStyle(.haze)
                                    if let voice = member.voiceActors.first?.name {
                                        Text(voice).font(.ui(15)).foregroundStyle(.haze).lineLimit(1)
                                    }
                                }
                                .frame(width: 84, alignment: .leading)
                                .accessibilityElement(children: .combine)
                            }
                        }
                    }
                    .scrollIndicators(.never)
                    .padding(.horizontal, -16)
                    .contentMargins(.horizontal, 16, for: .scrollContent)
                } else {
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 24, alignment: .topLeading),
                                        GridItem(.flexible(), alignment: .topLeading)], alignment: .leading, spacing: 12) {
                        ForEach(Array(cast.enumerated()), id: \.offset) { _, member in
                            HStack(spacing: 12) {
                                art(member, entry: entry, size: onMac ? CGSize(width: 32, height: 48) : CGSize(width: 36, height: 54))
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(member.name).font(.mac(13)).foregroundStyle(.silk).lineLimit(1)
                                    Text(member.roleLine).font(.mac(12)).foregroundStyle(.haze).lineLimit(1)
                                }
                            }
                            .accessibilityElement(children: .combine)
                        }
                    }
                }
            }
        }
    }

    /// Whether this is the Mac: its cast art is 32 by 48 where the iPad's is 36 by 54, and its words are in title case.
    private var onMac: Bool {
        #if os(macOS)
        true
        #else
        false
        #endif
    }

    private func art(_ member: CastMember, entry: Entry, size: CGSize) -> some View {
        ArtworkView(url: member.imageUrl, color: entry.plateColor, maxPixelSize: Int(size.width * 3),
                    flat: entry.plateColor == nil ? Palette.artPlaceholder : nil)
            .frame(width: size.width, height: size.height)
            .clipShape(.rect(cornerRadius: phone ? 8 : 4))
            .accessibilityHidden(true)
    }

    @ViewBuilder
    private func staff(_ detail: TitleDetail) -> some View {
        let staff = detail.shownStaff
        if !staff.isEmpty {
            VStack(alignment: .leading, spacing: 10) {
                heading(L.moreStaff)
                let columns = phone
                    ? [GridItem(.flexible(), spacing: 16, alignment: .topLeading), GridItem(.flexible(), alignment: .topLeading)]
                    : [GridItem(.flexible(), alignment: .topLeading)]
                LazyVGrid(columns: columns, alignment: .leading, spacing: phone ? 14 : 8) {
                    ForEach(Array(staff.enumerated()), id: \.offset) { _, credit in
                        VStack(alignment: .leading, spacing: 1) {
                            Text(credit.name).font(phone ? .ui(17) : .mac(13)).foregroundStyle(.silk).lineLimit(1)
                            Text(credit.role).font(phone ? .ui(15) : .mac(12)).foregroundStyle(.haze).lineLimit(1)
                        }
                        .accessibilityElement(children: .combine)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func recommendations(_ detail: TitleDetail) -> some View {
        let shown = detail.shownRecommendations
        if !shown.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                heading(L.moreRecommendations)
                if phone {
                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 12) {
                            ForEach(shown, id: \.id) { recommendationCard($0, width: 112) }
                        }
                    }
                    .scrollIndicators(.never)
                    .padding(.horizontal, -16)
                    .contentMargins(.horizontal, 16, for: .scrollContent)
                } else if onMac {
                    // One row of eight.
                    HStack(alignment: .top, spacing: 16) {
                        ForEach(shown, id: \.id) { recommendationCard($0, width: nil) }
                        ForEach(shown.count..<8, id: \.self) { _ in Color.clear.frame(maxWidth: .infinity) }
                    }
                } else {
                    // The iPad's larger type needs wider cards: eight in a row where the sheet is
                    // wide, a second row where a window is narrow.
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 104), spacing: 16, alignment: .top)],
                              alignment: .leading, spacing: 16) {
                        ForEach(shown, id: \.id) { recommendationCard($0, width: nil) }
                    }
                }
            }
        }
    }

    private func recommendationCard(_ linked: LinkedTitle, width: CGFloat?) -> some View {
        Button {
            // The sheet goes, and that title's page opens.
            close()
            navigator.openTitle(linked.id)
        } label: {
            VStack(alignment: .leading, spacing: 6) {
                ArtworkView(url: linked.coverUrl, maxPixelSize: 300, flat: Palette.artPlaceholder)
                    .aspectRatio(2 / 3, contentMode: .fit)
                    .clipShape(.rect(cornerRadius: 6))
                Text(linked.shownTitle)
                    .font(phone ? .showTitle(size: 15) : .macShowTitle(size: 13))
                    .foregroundStyle(.silk)
                    .lineLimit(2, reservesSpace: false)
                Text(linked.formatYear).font(phone ? .ui(13) : .mac(12)).foregroundStyle(.haze).lineLimit(1)
            }
            .frame(width: width)
            .frame(maxWidth: width == nil ? .infinity : nil, alignment: .leading)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .pointerHover()
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }

    @ViewBuilder
    private func tags(_ entry: Entry) -> some View {
        let safe = entry.safeTags
        let spoilers = entry.spoilerTags
        if !safe.isEmpty || !spoilers.isEmpty {
            VStack(alignment: .leading, spacing: 12) {
                heading(L.titleTags)
                if !safe.isEmpty {
                    Text(safe.joined(separator: " · ")).font(phone ? .ui(17) : .mac(13)).foregroundStyle(.silk)
                        .fixedSize(horizontal: false, vertical: true)
                }
                if spoilersShown {
                    FlowLayout(spacing: 14, lineSpacing: 6) {
                        ForEach(spoilers, id: \.self) { name in
                            Text("\(Text(name).foregroundStyle(.silk)) \(L.moreSpoiler)")
                                .foregroundStyle(.haze)
                                .font(phone ? .ui(17) : .mac(13))
                        }
                    }
                }
                if !spoilers.isEmpty {
                    Button(SpoilerWords.reveal(spoilers.count, shown: spoilersShown)) {
                        withAnimation(.standard) { spoilersShown.toggle() }
                    }
                    .buttonStyle(.plain)
                    .font(phone ? .ui(17, weight: .semibold) : .mac(13, weight: .semibold))
                    .foregroundStyle(.silk)
                    .frame(minHeight: phone ? 44 : nil)
                    .accessibilityValue(spoilersShown ? L.stateExpanded : L.stateCollapsed)
                }
            }
        }
    }

    // MARK: States

    /// While the page's detail is on its way: placeholders where the sections will be, then
    /// "AniList did not answer." once it has waited long enough.
    @ViewBuilder
    private var waiting: some View {
        if now - asked > .seconds(25) {
            VStack(alignment: .leading, spacing: 12) {
                Text(L.titleNoAnswerShort).font(phone ? .ui(17) : .mac(13)).foregroundStyle(.silk)
                Button(L.actionTryAgain) {
                    asked = .now
                    now = .now
                    store.loadDetail(id, force: true)
                }
                .buttonStyle(SecondaryCapsuleStyle(height: phone ? 44 : 28, raisedSurface: true, fontSize: phone ? 17 : 13))
            }
        } else {
            VStack(alignment: .leading, spacing: 28) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("#4 Highest rated, Summer 2026") // i18n-skip
                    Text("#19 Highest rated, 2026") // i18n-skip
                }
                HStack(spacing: 12) {
                    ForEach(0..<4, id: \.self) { _ in
                        RoundedRectangle(cornerRadius: 4).fill(.raisedSelected)
                            .frame(width: phone ? 84 : 32, height: phone ? 126 : 48)
                    }
                }
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(0..<4, id: \.self) { _ in Text("Director of Stardust Wayfarer") } // i18n-skip
                }
                HStack(spacing: 12) {
                    ForEach(0..<4, id: \.self) { _ in
                        RoundedRectangle(cornerRadius: 6).fill(.raisedSelected).frame(width: 80, height: 120)
                    }
                }
            }
            .font(phone ? .ui(17) : .mac(13))
            .foregroundStyle(.haze)
            .redacted(reason: .placeholder)
            .accessibilityHidden(true)
            .transition(.opacity)
        }
    }

    private var emptyState: some View {
        VStack(spacing: 16) {
            Nagare(cell: 4)
            Text(L.moreEmpty)
                .font(phone ? .ui(17) : .mac(13))
                .foregroundStyle(.haze)
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
    }
}

// MARK: - Flow

/// Items left to right, onto the next line when the line is full: the rankings, the spoiler tags.
struct FlowLayout: Layout {
    var spacing: CGFloat
    var lineSpacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, line: CGFloat = 0, widest: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width {
                y += line + lineSpacing
                x = 0
                line = 0
            }
            x += size.width + spacing
            line = max(line, size.height)
            widest = max(widest, x - spacing)
        }
        return CGSize(width: proposal.width ?? widest, height: y + line)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, line: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x > bounds.minX, x + size.width > bounds.maxX {
                y += line + lineSpacing
                x = bounds.minX
                line = 0
            }
            subview.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            line = max(line, size.height)
        }
    }
}
