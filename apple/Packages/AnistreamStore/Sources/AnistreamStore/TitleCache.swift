// Every title the app has been told of, once, so every list and page shows a title as it now
// stands: a mark or a watch moves it everywhere at once.

import AnistreamKit
import AnistreamStrings
import Observation

/// The titles by id, merged from every list, page and progress the engine sends. Lists hold ids
/// and read their rows from here.
@Observable @MainActor
public final class TitleCache {
    public private(set) var entries: [AnilistId: Entry] = [:]
    /// The titles whose page has been loaded, so their detail-only fields are known.
    public private(set) var detailed: Set<AnilistId> = []
    /// Each title's episode table, as last loaded.
    public private(set) var episodes: [AnilistId: [EpisodeRow]] = [:]
    /// Each title's line on its dub ("Dub to episode 6"), where the source could say one.
    public private(set) var dubLines: [AnilistId: String] = [:]

    public init() {}

    public subscript(id: AnilistId) -> Entry? {
        entries[id]
    }

    /// A list's rows. A list carries no detail-only fields, so a page's stay where one was
    /// loaded, and so does a known list status the list does not know. Only Continue's list
    /// speaks for this device's watch history: any other list may carry a tracker's progress, or
    /// none, so for a title already known it leaves the local progress and resume point alone.
    func merge(_ list: [Entry], local: Bool = false) {
        for entry in list {
            guard var kept = entries[entry.id] else {
                entries[entry.id] = entry
                continue
            }
            let related = kept.related, recommended = kept.recommended
            let trailer = kept.trailerUrl, availableOn = kept.availableOn, detail = kept.detail
            let status = kept.listStatus, lastAired = kept.lastAired
            let watched = kept.progress, resume = kept.resume
            kept = entry
            if !local {
                kept.progress = watched
                kept.resume = resume
            }
            if detailed.contains(entry.id) {
                kept.related = related
                kept.recommended = recommended
                kept.trailerUrl = trailer
                kept.availableOn = availableOn
                kept.detail = detail
            }
            kept.listStatus = entry.listStatus ?? status
            // A list arrives ahead of the broadcast line its own follow-up brings.
            kept.lastAired = entry.lastAired ?? lastAired
            entries[entry.id] = kept
        }
    }

    /// A title's page, whole.
    func merge(detail entry: Entry) {
        var entry = entry
        if let kept = entries[entry.id] {
            entry.listStatus = entry.listStatus ?? kept.listStatus
            entry.lastAired = entry.lastAired ?? kept.lastAired
        }
        entries[entry.id] = entry
        detailed.insert(entry.id)
    }

    /// Where a title stands after something moved it.
    func progress(
        id: AnilistId,
        progress: TitleProgress?,
        resume: ResumePoint?,
        episodes rows: [EpisodeProgress]
    ) {
        entries[id]?.progress = progress
        entries[id]?.resume = resume
        guard !rows.isEmpty, var table = episodes[id] else { return }
        for row in rows {
            guard let index = table.firstIndex(where: { $0.number == row.number }) else { continue }
            table[index].watched = row.watched
            table[index].completed = row.completed
        }
        episodes[id] = table
    }

    /// The latest broadcast of each airing title, said after the list it annotates.
    func lastAired(_ rows: [TitleAired]) {
        for row in rows {
            entries[row.id]?.lastAired = row.aired
        }
    }

    func episodes(id: AnilistId, rows: [EpisodeRow]) {
        episodes[id] = rows
    }

    /// How far a title's dub goes, as the page words it; nothing when there is none to say.
    func dub(id: AnilistId, extent: DubExtent?) {
        guard let extent else {
            dubLines[id] = nil
            return
        }
        dubLines[id] = extent.complete ? L.audioDubbed : L.titleDubbedTo(episode: Int(extent.through))
    }

    /// Whether title `id`'s episodes, as last loaded, are AniList's alone: no source set up has
    /// it, so it can be marked watched but there is nothing to play or download it from.
    public func unlisted(_ id: AnilistId) -> Bool {
        episodes[id]?.first?.metadataOnly == true
    }

    /// The titles whose episodes, as last loaded, are AniList's alone.
    var unlistedTitles: [AnilistId] {
        episodes.compactMap { id, rows in rows.first?.metadataOnly == true ? id : nil }
    }

    /// A title put on a list here, ahead of the tracker's word.
    public func setStatus(_ id: AnilistId, _ segment: LibrarySegment) {
        entries[id]?.listStatus = segment
    }
}
