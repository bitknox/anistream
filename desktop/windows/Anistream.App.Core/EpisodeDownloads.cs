using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>Where an episode stands in the download queue, as its row on a title page shows it.</summary>
public enum EpisodeDownloadKind { None, Queued, Going, Paused, Done, Failed }

/// <summary>
/// An episode row's download cell: nothing yet (Download, shown on hover or focus), on its way
/// (a ring with how far), downloaded (a check), or failed (Download again). The words are the
/// cell's tooltip and its name for a screen reader.
/// </summary>
public sealed record EpisodeDownload(EpisodeDownloadKind Kind, double Fraction, string Say)
{
    /// <summary>Whether a click asks for the download: when there is none, or the last one failed.</summary>
    public bool Asks => Kind is EpisodeDownloadKind.None or EpisodeDownloadKind.Failed;

    /// <summary>
    /// The cell for <paramref name="episode"/> of <paramref name="show"/>: a finished file over any
    /// other row for it, else the newest row, since a retry adds one.
    /// </summary>
    public static EpisodeDownload Of(IEnumerable<DownloadRow> rows, uint show, string episode)
    {
        var mine = rows.Where(r => r.AnilistId == show && r.Episode == episode).ToList();
        var row = mine.FirstOrDefault(r => r.State == DownloadState.Done) ?? mine.MaxBy(r => r.Id);
        var percent = row is null ? 0 : (int)Math.Floor(Math.Clamp(row.Fraction, 0, 1) * 100);
        return row?.State switch
        {
            null => new(EpisodeDownloadKind.None, 0, $"Download episode {episode}"),
            DownloadState.Queued => new(EpisodeDownloadKind.Queued, 0, "Queued to download"),
            DownloadState.Active when row.Total == 0 => new(EpisodeDownloadKind.Queued, 0, "Starting the download"),
            DownloadState.Active => new(EpisodeDownloadKind.Going, row.Fraction, $"Downloading · {percent}%"),
            DownloadState.Paused => new(EpisodeDownloadKind.Paused, row.Fraction, $"Download paused at {percent}%"),
            DownloadState.Done => new(EpisodeDownloadKind.Done, 1, "Downloaded"),
            _ => new(EpisodeDownloadKind.Failed, 0, $"Couldn't download episode {episode}. Download again"),
        };
    }
}
