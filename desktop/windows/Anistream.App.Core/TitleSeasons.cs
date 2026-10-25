using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// The title page's season picker: the franchise's seasons in watch order, as the engine walked
/// them, each named "Season 2 · Spring 2025". One season, or a walk still under way, is no picker.
/// </summary>
public static class TitleSeasons
{
    public sealed record Choice(uint Id, string Label, bool IsThis);

    public static IReadOnlyList<Choice> Of(Entry entry) =>
        entry.Detail?.Seasons is { Length: > 1 } seasons
            ? [.. seasons.Select((s, i) => new Choice(s.Id, Label(i, s), s.Id == entry.Id))]
            : [];

    /// <summary>"Season 2 · Spring 2025", "Season 3 · 2026", or "Season 4" with no date.</summary>
    public static string Label(int index, LinkedTitle season)
    {
        var name = $"Season {index + 1}";
        if (season.Year is not { } year)
            return name;
        return season.Season is { } s ? $"{name} · {AnistreamModelMethods.SearchSeasonLabel(s)} {year}" : $"{name} · {year}";
    }

    /// <summary>The related titles the picker does not already hold.</summary>
    public static RelatedTitle[] Related(Entry entry)
    {
        var picked = Of(entry).Select(c => c.Id).ToHashSet();
        return [.. (entry.Related ?? []).Where(r => !picked.Contains(r.Id))];
    }
}
