using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>One title's downloads.</summary>
public sealed record DownloadGroup(uint Id, DownloadRow[] Rows)
{
    public string Title => Rows.FirstOrDefault()?.Title ?? "";
    public ulong BytesOnDisk => Rows.Aggregate(0UL, (sum, row) => sum + row.OnDisk());
}

/// <summary>The download queue as the Downloads page reads it: by title, with speeds from one event to the next.</summary>
public sealed class DownloadQueue
{
    readonly Dictionary<long, (ulong Bytes, DateTimeOffset At)> last = [];

    public DownloadRow[] Rows { get; private set; } = [];

    /// <summary>Bytes a second for each running download.</summary>
    public Dictionary<long, double> Rates { get; private set; } = [];

    public bool Loaded { get; private set; }

    public void Apply(DownloadRow[] rows, DateTimeOffset now)
    {
        var rates = new Dictionary<long, double>();
        var seen = new Dictionary<long, (ulong, DateTimeOffset)>();
        foreach (var row in rows.Where(r => r.State == DownloadState.Active))
        {
            seen[row.Id] = (row.Downloaded, now);
            if (!last.TryGetValue(row.Id, out var before))
                continue;
            var seconds = (now - before.At).TotalSeconds;
            if (seconds <= 0.2 || row.Downloaded < before.Bytes)
            {
                if (Rates.TryGetValue(row.Id, out var kept))
                    rates[row.Id] = kept;
                seen[row.Id] = before;
                continue;
            }
            var rate = (row.Downloaded - before.Bytes) / seconds;
            // Half the last reading and half this one: steady enough to read, quick to follow.
            rates[row.Id] = Rates.TryGetValue(row.Id, out var was) ? (was + rate) / 2 : rate;
        }
        last.Clear();
        foreach (var (id, value) in seen)
            last[id] = value;
        Rows = rows;
        Rates = rates;
        Loaded = true;
    }

    /// <summary>The queue by title, each title where its first row is.</summary>
    public DownloadGroup[] Groups() =>
        [.. Rows.GroupBy(r => r.AnilistId).Select(g => new DownloadGroup(g.Key, [.. g]))];

    /// <summary>What the downloads take on disk: a finished one's size, and what the rest have so far.</summary>
    public ulong BytesOnDisk => Rows.Aggregate(0UL, (sum, row) => sum + row.OnDisk());

    /// <summary>Seconds until a running download finishes, at its speed.</summary>
    public long? SecondsLeft(DownloadRow row) =>
        row.State == DownloadState.Active && row.Total > row.Downloaded && Rates.TryGetValue(row.Id, out var rate) && rate > 0
            ? (long)Math.Ceiling((row.Total - row.Downloaded) / rate)
            : null;
}

public static class DownloadRows
{
    /// <summary>What it takes on disk now.</summary>
    public static ulong OnDisk(this DownloadRow row) =>
        row.State == DownloadState.Done ? Math.Max(row.Total, row.Downloaded) : row.Downloaded;

    /// <summary>A seeding file's size: what has gone up over the ratio that makes.</summary>
    public static ulong? Size(this SeedRow row) =>
        row.Ratio > 0 ? (ulong)Math.Round(row.UploadedBytes / row.Ratio) : null;
}
