using System.Globalization;
using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// One value a setting can be set to, as <c>SetSetting</c> takes it. <paramref name="Label"/> is
/// the engine's own word for it, which its row's value says and which finds the option again;
/// <paramref name="Shown"/> is what a person reads, from the catalogue.
/// </summary>
public sealed record SettingOption(SettingValue Value, string Label, string Shown);

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
        SettingKey.PresenceRich,
        SettingKey.Torrents, SettingKey.Syncplay, SettingKey.UpdateCheck, SettingKey.SpoilerProtection,
        SettingKey.DownloadsOnCellular, SettingKey.TorrentsOnCellular, SettingKey.EpisodeNotifications,
    ];

    /// <summary>
    /// The switches a row waits on, each to be on before the row means anything: the party's
    /// server and room on Watch party, the title on Discord, rich presence on the title too.
    /// The Mac's integrations pane greys the same rows.
    /// </summary>
    public static IReadOnlyList<SettingKey> Needs(SettingKey key) => key switch
    {
        SettingKey.SyncplayServer or SettingKey.SyncplayRoom => [SettingKey.Syncplay],
        SettingKey.PresenceTitle => [SettingKey.Presence],
        SettingKey.PresenceRich => [SettingKey.Presence, SettingKey.PresenceTitle],
        _ => [],
    };

    /// <summary>The settings with two states, on and off, that a switch shows.</summary>
    public static bool IsSwitch(SettingKey key) => Switches.Contains(key);

    static SettingOption Str(string value, string label, string shown) => new(new SettingValue.Str(value), label, shown);
    static SettingOption Int(long value, string label, string shown) => new(new SettingValue.Int(value), label, shown);

    /// <summary>
    /// The values <paramref name="key"/> takes, each with the engine's word and the shown one.
    /// Empty for one that is typed or read-only. The engine's words come from its own wording
    /// (plurals and sizes included), only to match its row; they are never shown.
    /// </summary>
    public static IReadOnlyList<SettingOption> Options(SettingKey key) => key switch
    {
        SettingKey.Titles =>
        [
            Str("anilist", "AniList's choice", T.SettingsTitleTracker(AnistreamFfiMethods.TrackerName("anilist"))),
            Str("english", "English", T.SettingsTitleEnglish),
            Str("romaji", "Romaji", T.SettingsTitleRomaji),
        ],
        SettingKey.Trailers =>
            [Str("always", "always", T.SettingsValueTrailersAlways), Str("wifi-only", "Wi-Fi only", T.SettingsValueTrailersWifiOnly), Str("off", "off", T.ValueOff)],
        SettingKey.Quality => [.. new long[] { 480, 720, 1080, 2160 }.Select(q => Int(q, $"{q}p", T.SettingsQualityLines(q)))],
        SettingKey.CommitThreshold =>
            [.. new[] { 70, 80, 85, 90, 95 }.Select(p => new SettingOption(new SettingValue.Float(p / 100.0), $"{p}%", Wording.Percent(p)))],
        SettingKey.Upscaling =>
            [Str("off", "off", T.ValueOff), Str("natural", "natural · ArtCNN", T.SettingsUpscalingNatural), Str("sharp", "sharp · Anime4K", T.SettingsUpscalingSharp)],
        SettingKey.Translation => [Str("sub", "sub", T.AudioSubbed), Str("dub", "dub", T.AudioDubbed)],
        SettingKey.Theme => [Str("adaptive", "adaptive", T.SettingsThemeAdaptive), Str("immersive", "immersive", T.SettingsThemeImmersive)],
        SettingKey.PlayerUi => [Str("anistream", "anistream", T.SettingsPlayerUiApp), Str("mpv", "your mpv setup", T.SettingsPlayerUiMpv)],
        SettingKey.UpNextCountdown =>
            [.. new long[] { 0, 5, 10, 15, 20, 30 }.Select(s => Int(s, s == 0 ? "off" : $"{s} s", s == 0 ? T.ValueOff : T.DurationSeconds(s)))],
        SettingKey.SeedRatio =>
            [.. new[] { 0, 5, 10, 15, 20 }.Select(tenths => new SettingOption(
                new SettingValue.Float(tenths / 10.0),
                tenths > 0 ? "until ratio " + (tenths / 10.0).ToString("0.0", CultureInfo.InvariantCulture) : "off",
                tenths > 0 ? T.SettingsSeedUntilRatio(Wording.Number(tenths / 10.0, 1)) : T.ValueOff))],
        SettingKey.SeedHours => [.. new long[] { 1, 6, 12, 24, 72 }.Select(h => Int(h, $"{h} h", T.DurationHours(h)))],
        SettingKey.UploadLimit =>
            [.. new long[] { 0, 256, 512, 1024, 2048, 5120 }.Select(k => Int(k, UploadLabel((ulong)k), k == 0 ? T.SettingsUploadUnlimited : Wording.Rate((ulong)k * 1024)))],
        SettingKey.DownloadQuality =>
            [Str("standard", "standard", T.SettingsDownloadStandard), Str("higher", "higher", T.SettingsDownloadHigher)],
        SettingKey.KeepNextEpisodes =>
            [.. new long[] { 0, 1, 2, 3, 5, 10 }.Select(n => Int(n, n == 0 ? "off" : AnistreamFfiMethods.Plural((ulong)n, "episode"), n == 0 ? T.ValueOff : T.EpisodesCount(n)))],
        // Warn only is asked for with its acknowledgement, through the torrent protection command.
        SettingKey.VpnOnFailure => [Str("pause", "pause torrents", T.TorrentsFailPause), Str("stop", "stop torrents", T.TorrentsFailStop)],
        _ when IsSwitch(key) => [new(new SettingValue.Bool(false), "off", T.ValueOff), new(new SettingValue.Bool(true), "on", T.ValueOn)],
        _ => [],
    };

    /// <summary>An upload cap in the engine's words, "unlimited" or "524 kB/s", to match its row: <c>upload_label</c>.</summary>
    public static string UploadLabel(ulong kib) =>
        kib == 0 ? "unlimited" : AnistreamFfiMethods.HumanBytes(kib * 1024) + "/s";

    /// <summary>
    /// A row's value as a person reads it: the shown words of the option it stands on, or the
    /// engine's own, capitalised, for one the catalogue does not list. None when empty.
    /// </summary>
    public static string Shown(SettingKey key, string value)
    {
        if (value.Length == 0)
            return T.ValueNone;
        var saved = key is SettingKey.Translation or SettingKey.Theme ? value.Split(" · ")[0] : value;
        if (Options(key).FirstOrDefault(o => o.Label == saved) is { } option)
            return option.Shown;
        return char.ToUpper(value[0], CultureInfo.CurrentCulture) + value[1..];
    }

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
