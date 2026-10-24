using System.Globalization;
using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>One value a setting can be set to, as <c>SetSetting</c> takes it, and the words its row then says.</summary>
public sealed record SettingOption(SettingValue Value, string Label);

/// <summary>
/// The values each stepped setting takes, in the order the engine's arrows walk them, and the
/// words the row says once one is chosen. The same table the Apple app (AnistreamKit's
/// <c>SettingKey.options</c>) and the Android app (<c>SettingsCatalog</c>) keep, ports of
/// <c>anistream_model::settings</c>'s ladders and wording, so a menu here offers exactly what the
/// engine steps through. A row's control follows from the key, not from guessing at its value.
/// </summary>
public static class SettingsCatalog
{
    static readonly HashSet<SettingKey> Switches =
    [
        SettingKey.Motion, SettingKey.AutoNext, SettingKey.SkipOpening, SettingKey.SkipFiller,
        SettingKey.MarkChapters, SettingKey.SkipButton, SettingKey.Fullscreen, SettingKey.NightMode,
        SettingKey.MergeSubtitles, SettingKey.KeepSeeding, SettingKey.Presence, SettingKey.PresenceTitle,
        SettingKey.Torrents, SettingKey.Syncplay, SettingKey.UpdateCheck, SettingKey.SpoilerProtection,
        SettingKey.DownloadsOnCellular, SettingKey.TorrentsOnCellular, SettingKey.EpisodeNotifications,
    ];

    /// <summary>The settings with two states, on and off, that a switch shows.</summary>
    public static bool IsSwitch(SettingKey key) => Switches.Contains(key);

    static SettingOption Str(string value, string label) => new(new SettingValue.Str(value), label);
    static SettingOption Int(long value, string label) => new(new SettingValue.Int(value), label);

    /// <summary>The values <paramref name="key"/> takes, each as its row will say it. Empty for one that is typed or read-only.</summary>
    public static IReadOnlyList<SettingOption> Options(SettingKey key) => key switch
    {
        SettingKey.Titles => [Str("anilist", "AniList's choice"), Str("english", "English"), Str("romaji", "Romaji")],
        SettingKey.Quality => [.. new long[] { 480, 720, 1080, 2160 }.Select(q => Int(q, $"{q}p"))],
        SettingKey.CommitThreshold =>
            [.. new[] { 70, 80, 85, 90, 95 }.Select(p => new SettingOption(new SettingValue.Float(p / 100.0), $"{p}%"))],
        SettingKey.Upscaling => [Str("off", "off"), Str("natural", "natural · ArtCNN"), Str("sharp", "sharp · Anime4K")],
        SettingKey.Translation => [Str("sub", "sub"), Str("dub", "dub")],
        SettingKey.Theme => [Str("adaptive", "adaptive"), Str("immersive", "immersive")],
        SettingKey.PlayerUi => [Str("anistream", "anistream"), Str("mpv", "your mpv setup")],
        SettingKey.UpNextCountdown =>
            [.. new long[] { 0, 5, 10, 15, 20, 30 }.Select(s => Int(s, s == 0 ? "off" : $"{s} s"))],
        SettingKey.SeedRatio =>
            [.. new[] { 0, 5, 10, 15, 20 }.Select(tenths => new SettingOption(
                new SettingValue.Float(tenths / 10.0),
                tenths > 0 ? "until ratio " + (tenths / 10.0).ToString("0.0", CultureInfo.InvariantCulture) : "off"))],
        SettingKey.SeedHours => [.. new long[] { 1, 6, 12, 24, 72 }.Select(h => Int(h, $"{h} h"))],
        SettingKey.UploadLimit => [.. new long[] { 0, 256, 512, 1024, 2048, 5120 }.Select(k => Int(k, UploadLabel((ulong)k)))],
        SettingKey.DownloadQuality => [Str("standard", "standard"), Str("higher", "higher")],
        SettingKey.KeepNextEpisodes =>
            [.. new long[] { 0, 1, 2, 3, 5, 10 }.Select(n => Int(n, n == 0 ? "off" : AnistreamFfiMethods.Plural((ulong)n, "episode")))],
        // Warn only is asked for with its acknowledgement, through the torrent protection command.
        SettingKey.VpnOnFailure => [Str("pause", "pause torrents"), Str("stop", "stop torrents")],
        _ when IsSwitch(key) => [new(new SettingValue.Bool(false), "off"), new(new SettingValue.Bool(true), "on")],
        _ => [],
    };

    /// <summary>An upload cap as people read it: "unlimited", "524 kB/s". The engine's <c>upload_label</c>.</summary>
    public static string UploadLabel(ulong kib) =>
        kib == 0 ? "unlimited" : AnistreamFfiMethods.HumanBytes(kib * 1024) + "/s";

    /// <summary>
    /// The option a row stands on, found by its words. Sub or dub and the theme say the saved
    /// choice first, with the session's beside it when that differs, and stand on the saved one.
    /// </summary>
    public static SettingOption? Current(SettingRow? row)
    {
        if (row is null)
            return null;
        var saved = row.Key is SettingKey.Translation or SettingKey.Theme
            ? row.Value.Split(" · ")[0]
            : row.Value;
        return Options(row.Key).FirstOrDefault(o => o.Label == saved);
    }

    /// <summary>Whether a switch's row says it is on.</summary>
    public static bool IsOn(SettingRow? row) => row?.Value == "on";

    /// <summary>Whether the row can be changed from here at all: <c>SettingChange::editable</c>.</summary>
    public static bool Editable(SettingRow? row) => row is not null && row.Change is not SettingChange.Fixed;

    /// <summary>The text a typed setting starts editing from, when it is typed.</summary>
    public static string? TypedText(SettingRow? row) => (row?.Change as SettingChange.Typed)?.Text;

    /// <summary>How a row's control looks: from the key and the engine's change kind.</summary>
    public static SettingControl ControlFor(SettingRow row) => row.Change switch
    {
        SettingChange.Fixed => SettingControl.ReadOnly,
        SettingChange.Typed => SettingControl.Text,
        _ when IsSwitch(row.Key) => SettingControl.Switch,
        _ when Options(row.Key).Count > 0 => SettingControl.Choice,
        _ => SettingControl.ReadOnly,
    };
}

public enum SettingControl
{
    Switch,
    Choice,
    Text,
    ReadOnly,
}
