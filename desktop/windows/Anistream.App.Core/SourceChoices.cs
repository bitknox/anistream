using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>One row of the source picker.</summary>
public abstract record PickerRow
{
    /// <summary>A stable key for lists and focus.</summary>
    public abstract string Key { get; }

    /// <summary>Let the engine pick per episode.</summary>
    public sealed record Automatic(bool Chosen) : PickerRow
    {
        public override string Key => "automatic";
    }

    /// <summary>One source with what it has of the title. Dimmed when it has a note instead of a count.</summary>
    public sealed record Source(SourceOption Option, bool Chosen) : PickerRow
    {
        public override string Key => $"source/{Option.ProviderId}";
        public bool Dimmed => Option.Note is not null;

        /// <summary>A source that is no longer installed cannot be picked.</summary>
        public bool Selectable => Option.Note is not SourceNote.Gone;
    }

    /// <summary>The heading over one source's releases.</summary>
    public sealed record ReleasesHeading(string ProviderId) : PickerRow
    {
        public override string Key => $"releases/{ProviderId}";
    }

    /// <summary>One release of a source.</summary>
    public sealed record Release(SourceOption Option, SourceCandidate Candidate, bool Chosen) : PickerRow
    {
        public override string Key => $"release/{Option.ProviderId}/{Candidate.Id}";
    }
}

/// <summary>What the source picker lists and how it words each row.</summary>
public static class SourcePicker
{
    /// <summary>
    /// The picker's rows: Automatic, then each source in the order they are tried, and under a
    /// source with several releases a heading and its releases. Just Automatic while nothing has
    /// come.
    /// </summary>
    public static PickerRow[] Rows(SourceOption[]? options)
    {
        var rows = new List<PickerRow> { new PickerRow.Automatic(options?.Any(o => o.Pinned) == false) };
        foreach (var option in options ?? [])
        {
            // A pinned release the listing no longer holds leaves the source itself marked.
            var releases = option.Releases ?? [];
            var shown = option.Pinned && option.PinnedRelease is { } pin && releases.Any(r => r.Id == pin);
            rows.Add(new PickerRow.Source(option, option.Pinned && !shown));
            if (releases.Length > 1)
            {
                rows.Add(new PickerRow.ReleasesHeading(option.ProviderId));
                foreach (var release in releases)
                    rows.Add(new PickerRow.Release(option, release, option.Pinned && option.PinnedRelease == release.Id));
            }
        }
        return [.. rows];
    }

    /// <summary>A source's coverage ("6 of 13 episodes"), or why it has none. A failure is the source's own words.</summary>
    public static string Detail(SourceOption option) => option.Note switch
    {
        null => T.SourcePickerCoverage(option.Covered, option.Of),
        SourceNote.Off => T.SourceNoteOff,
        SourceNote.Resting => T.SourceNoteResting,
        SourceNote.Gone => T.SourceNoteGone,
        SourceNote.Ambiguous => T.SourceNoteAmbiguous,
        SourceNote.Failed failed => failed.Reason,
        _ => "",
    };

    /// <summary>A release's quality, size and the plugin's own label, as written: "1080p · 14.2 GiB · 84 seeders".</summary>
    public static string? ReleaseDetail(SourceCandidate release)
    {
        var parts = new List<string>();
        if (release.Quality is { } quality)
            parts.Add($"{quality}p");
        if (release.Size is { Length: > 0 } size)
            parts.Add(size);
        if (release.Label is { Length: > 0 } label)
            parts.Add(label);
        return parts.Count == 0 ? null : string.Join(" · ", parts);
    }

    /// <summary>The row's text: its name, then what it has under it.</summary>
    public static (string Title, string? Detail) Words(PickerRow row) => row switch
    {
        PickerRow.Automatic => (T.SourcePickerAutomatic, T.SourcePickerAutomaticDetail),
        PickerRow.Source s => (s.Option.Name, Detail(s.Option)),
        PickerRow.Release r => (r.Candidate.Title, ReleaseDetail(r.Candidate)),
        PickerRow.ReleasesHeading => (T.SourcePickerReleases, null),
        _ => ("", null),
    };

    /// <summary>
    /// What a screen reader says for the row: "Hotaru Open Archive, 6 of 13 episodes, Chosen".
    /// </summary>
    public static string Spoken(PickerRow row)
    {
        var (title, detail) = Words(row);
        var chosen = row switch
        {
            PickerRow.Automatic a => a.Chosen,
            PickerRow.Source s => s.Chosen,
            PickerRow.Release r => r.Chosen,
            _ => false,
        };
        return string.Join(", ", new[] { title, detail, chosen ? T.SourcePickerChosen : null }.Where(p => !string.IsNullOrEmpty(p)));
    }
}

/// <summary>A pinned source that could not be used, for the notice that says so.</summary>
public sealed record PinUnavailableNotice(uint Id, string ProviderId, string Name);

/// <summary>
/// What the source picker shows: the options the engine listed for each title, which of them is
/// pinned, and the one notice a pin the engine could not use is owed. Only the UI thread touches it.
/// </summary>
public sealed class SourceChoices
{
    readonly Dictionary<uint, SourceOption[]> options = [];
    readonly HashSet<uint> asked = [];
    readonly HashSet<(uint, string)> said = [];

    /// <summary>The latest listing of a title, in the order its sources are tried, or null before one has come.</summary>
    public SourceOption[]? For(uint id) => options.GetValueOrDefault(id);

    /// <summary>Whether a listing was asked for and has not come.</summary>
    public bool Loading(uint id) => asked.Contains(id) && !options.ContainsKey(id);

    /// <summary>The picker for a title opens: the listing is asked for.</summary>
    public void Asking(uint id) => asked.Add(id);

    /// <summary>The picker closed: an answer that never came is not waited for.</summary>
    public void GiveUp(uint id) => asked.Remove(id);

    /// <summary>The engine's listing replaces any earlier one.</summary>
    public void Apply(uint id, SourceOption[] listed)
    {
        asked.Remove(id);
        options[id] = listed;
    }

    /// <summary>
    /// A choice made, shown at once: the pinned flags follow it, so the picker reads right when
    /// it opens again before the engine's next listing. A new choice may meet the same
    /// unavailable source again, and says so again.
    /// </summary>
    public void Chose(uint id, string? provider, string? release = null)
    {
        said.RemoveWhere(s => s.Item1 == id);
        if (!options.TryGetValue(id, out var listed))
            return;
        options[id] = [.. listed.Select(o =>
        {
            var pinned = provider is not null && o.ProviderId == provider;
            return o with { Pinned = pinned, PinnedRelease = pinned ? release : null };
        })];
    }

    /// <summary>Whether the notice for a pin the engine could not use is new, and so said.</summary>
    public bool Say(PinUnavailableNotice notice) => said.Add((notice.Id, notice.ProviderId));

    public void Take(Event e)
    {
        switch (e)
        {
            case Event.SourceOptions listed:
                Apply(listed.Id, listed.Options);
                break;
        }
    }
}

/// <summary>The episode a title's main button plays.</summary>
/// <param name="Episode">The episode.</param>
/// <param name="Unsourced">Every episode from the next one on is one no source lists: there is nothing to play.</param>
public readonly record struct PlayableNext(uint Episode, bool Unsourced)
{
    /// <summary>
    /// The next episode to play, skipping those no source lists, in <paramref name="rows"/> when
    /// the table is loaded. A part-watched episode is resumed as it is. With every later episode
    /// unsourced too the answer is the engine's own next one, flagged <see cref="Unsourced"/>.
    /// </summary>
    public static PlayableNext Of(Entry entry, EpisodeRow[]? rows)
    {
        var next = entry.Progress?.Next ?? 1;
        if (entry.Resume is not null || rows is null)
            return new(next, false);
        var numbered = rows.Select(r => (Ok: uint.TryParse(r.Number, out var n), N: n, Row: r)).Where(r => r.Ok).ToArray();
        if (!numbered.Any(r => r.N == next && r.Row.Unsourced))
            return new(next, false);
        var later = numbered.Where(r => r.N > next && !r.Row.Unsourced).Select(r => r.N).DefaultIfEmpty().Min();
        return later > 0 ? new(later, false) : new(next, true);
    }

    /// <summary>
    /// The row for the title's main button: the model's own, worked out for the episode that can
    /// be played. With nothing that can, the row says so in place of its button.
    /// </summary>
    public static QueueItem Item(Entry entry, EpisodeRow[]? rows, Casing casing)
    {
        var target = Of(entry, rows);
        if (target.Episode == (entry.Progress?.Next ?? 1) && !target.Unsourced)
            return Wording.Said(AnistreamFfiMethods.QueueItem(entry, casing));
        if (target.Unsourced)
            return Wording.Said(AnistreamFfiMethods.QueueItem(entry, casing)) with
            {
                Playable = false,
                Action = T.EpisodeUnsourced,
                Button = null,
                Waiting = T.EpisodeUnsourced,
            };
        return Wording.Said(AnistreamFfiMethods.QueueItem(entry with { Progress = new TitleProgress(entry.Progress?.Done ?? 0, target.Episode) }, casing));
    }

    /// <summary>A Continue row, worked out again for the episode that can be played.</summary>
    public static QueueItem Item(QueueItem item, EpisodeRow[]? rows, Casing casing)
    {
        var target = Of(item.Entry, rows);
        return target.Episode == (item.Entry.Progress?.Next ?? 1) && !target.Unsourced ? item : Item(item.Entry, rows, casing);
    }
}

/// <summary>What a source-less episode can and cannot do.</summary>
public static class EpisodeSources
{
    /// <summary>Whether an episode can be played or downloaded: not one only AniList lists, nor one no source lists.</summary>
    public static bool Playable(EpisodeRow row) => !row.MetadataOnly && !row.Unsourced;

    /// <summary>The episodes a download can queue, in table order; a range skips the unsourced.</summary>
    public static string[] Downloadable(IEnumerable<EpisodeRow> rows) => [.. rows.Where(Playable).Select(r => r.Number)];
}
