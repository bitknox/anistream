using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>What Stats for nerds shows now: its rows, the waiting line, and Copy stats' state.</summary>
public sealed record StatsView(DetailRow[] Rows, bool WaitingLine, bool CanCopy, bool Copied);

/// <summary>
/// Stats for nerds (player-stats.md), as the panel's states have it. The engine words every row
/// and picks the one amber value; this keeps what the panel shows from one second to the next:
/// nothing for a moment after it opens on a file not yet playing, then the rows with dashes,
/// then "Waiting for the stream" too, and the rows once they come. Up or away is kept for the
/// window's later playbacks; the mini player has no room for it.
/// </summary>
public sealed class StatsPanel(Func<TimeSpan> clock)
{
    /// <summary>Nothing shows for this long after the panel opens on a file not yet playing.</summary>
    public static readonly TimeSpan Dashes = TimeSpan.FromMilliseconds(600);
    /// <summary>"Waiting for the stream" under the title once the dashes have lasted this long.</summary>
    public static readonly TimeSpan WaitingLineAfter = TimeSpan.FromSeconds(4);
    /// <summary>"Copied" stands in for the button this long.</summary>
    public static readonly TimeSpan CopiedFor = TimeSpan.FromMilliseconds(1500);

    DetailRow[] rows = [];
    TimeSpan? waitingSince, copiedAt;
    /// <summary>A shot's rows, held: the engine's are passed over.</summary>
    bool held;

    /// <summary>The viewer has the panel up.</summary>
    public bool Shown { get; private set; }

    /// <summary>The mini player is up, where the panel has no room.</summary>
    public bool Mini { get; set; }

    /// <summary>The panel up or away, waiting for the file's figures as it comes up.</summary>
    public void Show(bool shown)
    {
        Shown = shown;
        held = false;
        rows = [];
        copiedAt = null;
        waitingSince = shown ? clock() : null;
    }

    /// <summary>
    /// I, the close button or the Settings menu's check: the panel up if it was away and away if
    /// it was up. Returns what the engine's player is told, so its figures start or stop with it.
    /// </summary>
    public PlayerCommand Toggle()
    {
        Show(!Shown);
        return new PlayerCommand.Stats(Shown);
    }

    /// <summary>A new playback: the engine's player starts with its stats off, so the panel waits again.</summary>
    public void Restart()
    {
        if (!Shown || held)
            return;
        rows = [];
        copiedAt = null;
        waitingSince = clock();
    }

    /// <summary>The engine's figures, once a second while the panel is up. False when passed over.</summary>
    public bool Took(PlayerStats stats)
    {
        if (!Shown || held || stats.Rows is not { Length: > 0 } now)
            return false;
        rows = now;
        if (!stats.Waiting)
            waitingSince = null;
        else
            waitingSince ??= clock();
        return true;
    }

    /// <summary>Copy stats' lines are on the clipboard: "Copied" in the button's place a moment.</summary>
    public bool Copied()
    {
        if (!Shown)
            return false;
        copiedAt = clock();
        return true;
    }

    /// <summary>
    /// For a shot: <paramref name="stats"/> held, <paramref name="waited"/> before the file
    /// opened for the dashes and the waiting line, and "Copied" up, for a minute, when
    /// <paramref name="copied"/>.
    /// </summary>
    public void Stage(PlayerStats stats, TimeSpan? waited, bool copied)
    {
        Shown = true;
        held = true;
        rows = stats.Rows ?? [];
        var now = clock();
        waitingSince = waited is { } w ? now - w : null;
        copiedAt = copied ? now + TimeSpan.FromMinutes(1) : null;
    }

    /// <summary>What shows now, or null for nothing.</summary>
    public StatsView? View()
    {
        if (!Shown || Mini)
            return null;
        var now = clock();
        var copied = copiedAt is { } at && now - at < CopiedFor;
        if (waitingSince is not { } since)
            return rows.Length == 0 ? null : new StatsView(rows, WaitingLine: false, CanCopy: true, copied);
        var waited = now - since;
        if (waited < Dashes)
            return null;
        // The engine's dashes when it has said them, which know a file on the device has no
        // Network row; the fixture's before it has.
        var dashes = rows.Length > 0 ? rows : AnistreamFfiMethods.PlayerStatsFixture(StatsFixture.Waiting).Rows ?? [];
        return new StatsView(dashes, waited >= WaitingLineAfter, CanCopy: false, copied);
    }
}

/// <summary>What Copy stats says of this app, system and device: nothing that names the viewer.</summary>
public static class StatsAbouts
{
    /// <summary>
    /// "Windows 11 24H2 (build 26100.2894)": the name by the build, since Windows 11 still says
    /// 10.0 as its version, then the release and the build as Settings' About shows them.
    /// </summary>
    public static string System(int build, int revision, string? release)
    {
        var name = build >= 22000 ? "Windows 11" : "Windows 10";
        if (release is { Length: > 0 } r)
            name += $" {r.Trim()}";
        return revision > 0 ? $"{name} (build {build}.{revision})" : $"{name} (build {build})";
    }

    static readonly string[] Placeholders =
    [
        "System Product Name", "System Version", "To be filled by O.E.M.", "To Be Filled By O.E.M.",
        "Default string", "Not Applicable", "None", "Not Specified", "0", "1.0",
    ];

    /// <summary>
    /// "21HMCTO1WW (ThinkPad X1 Carbon Gen 11)": the machine's model as its firmware names it,
    /// never the computer's own name. The version holds the marketing name on some makers'
    /// machines, where the product is a part number, so it follows when it reads as words:
    /// letters and a space, not a code such as "124I:00036T".
    /// Firmware's fill-in values ("To be filled by O.E.M.") say nothing and are left out.
    /// </summary>
    public static string Device(string? product, string? version)
    {
        static string? Real(string? value) =>
            value?.Trim() is { Length: > 0 } v && !Placeholders.Contains(v, StringComparer.OrdinalIgnoreCase) ? v : null;
        var p = Real(product);
        var v = Real(version);
        if (p is null)
            return "";
        return v is not null && v.Any(char.IsAsciiLetter) && v.Contains(' ') && !p.Contains(v, StringComparison.OrdinalIgnoreCase) ? $"{p} ({v})" : p;
    }
}
