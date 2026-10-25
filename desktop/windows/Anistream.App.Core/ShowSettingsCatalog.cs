using Anistream.Engine.Core;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// What a show's own settings sheet offers for each row, beside following Settings: the audio as
/// Japanese or English, the switches as On or Off, and the stepped rows (quality, download quality,
/// the episodes kept ahead) as the Settings row's own ladder, so a show can set nothing Settings
/// could not. The Mac's <c>ShowWords</c> for the rows it has; the rest after
/// <see cref="SettingsCatalog"/>.
/// </summary>
public static class ShowSettingsCatalog
{
    /// <summary>The Settings row a show's row takes the place of: <c>ShowSettingKey::setting</c>.</summary>
    public static SettingKey Setting(ShowSettingKey key) => key switch
    {
        ShowSettingKey.Translation => SettingKey.Translation,
        ShowSettingKey.Quality => SettingKey.Quality,
        ShowSettingKey.SkipOpening => SettingKey.SkipOpening,
        ShowSettingKey.SkipFiller => SettingKey.SkipFiller,
        ShowSettingKey.SpoilerProtection => SettingKey.SpoilerProtection,
        ShowSettingKey.NotifyNewEpisodes => SettingKey.EpisodeNotifications,
        ShowSettingKey.DownloadQuality => SettingKey.DownloadQuality,
        _ => SettingKey.KeepNextEpisodes,
    };

    /// <summary>What a show can set <paramref name="key"/> to, each as the sheet says it.</summary>
    public static IReadOnlyList<SettingOption> Choices(ShowSettingKey key) => key switch
    {
        ShowSettingKey.Translation => [new(new SettingValue.Str("sub"), "Japanese"), new(new SettingValue.Str("dub"), "English")],
        ShowSettingKey.SkipOpening or ShowSettingKey.SkipFiller or ShowSettingKey.SpoilerProtection or ShowSettingKey.NotifyNewEpisodes =>
            [new(new SettingValue.Bool(true), "On"), new(new SettingValue.Bool(false), "Off")],
        _ => [.. SettingsCatalog.Options(Setting(key)).Select(o => o with { Label = Capital(o.Label) })],
    };

    /// <summary>A row's value in the sheet's words: Japanese or English for the audio, On or Off, "1080p", "2 episodes".</summary>
    public static string Shown(ShowSettingKey key, string value) => key switch
    {
        ShowSettingKey.Translation => value.StartsWith("dub", StringComparison.Ordinal) ? "English" : "Japanese",
        ShowSettingKey.SkipOpening or ShowSettingKey.SkipFiller or ShowSettingKey.SpoilerProtection or ShowSettingKey.NotifyNewEpisodes =>
            value is "on" or "true" ? "On" : "Off",
        _ => Capital(value),
    };

    /// <summary>The choice a show's own value stands on, or -1 when it follows Settings or sets one the sheet does not offer.</summary>
    public static int IndexOf(ShowSettingKey key, SettingValue? own)
    {
        if (own is null)
            return -1;
        var choices = Choices(key);
        for (var i = 0; i < choices.Count; i++)
            if (Same(choices[i].Value, own))
                return i;
        return -1;
    }

    /// <summary>Whether two values are the same value, whatever record instance carries them.</summary>
    public static bool Same(SettingValue a, SettingValue b) => (a, b) switch
    {
        (SettingValue.Str x, SettingValue.Str y) => x.V1 == y.V1,
        (SettingValue.Bool x, SettingValue.Bool y) => x.V1 == y.V1,
        (SettingValue.Int x, SettingValue.Int y) => x.V1 == y.V1,
        (SettingValue.Float x, SettingValue.Float y) => Math.Abs(x.V1 - y.V1) < 1e-9,
        _ => false,
    };

    static string Capital(string text) => text.Length == 0 ? text : char.ToUpperInvariant(text[0]) + text[1..];
}
