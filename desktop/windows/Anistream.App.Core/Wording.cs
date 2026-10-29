using System.Globalization;
using Anistream.Engine.Core;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// The model's wording rules (anistream-model's present.rs and wording.rs) in the person's
/// language: the engine decides how much of a span or a size to show and what a row is about, and
/// the words come from the catalogue (<see cref="T"/>), the digits and dates from .NET's formatters
/// in the current culture. Every number a person reads goes through here or through T.
/// </summary>
public static class Wording
{
    static CultureInfo Culture => CultureInfo.CurrentCulture;

    /// <summary>A whole number with the culture's grouping: "1,204".</summary>
    public static string Number(long n) => n.ToString("N0", Culture);

    /// <summary>A decimal as the culture writes it, at most two places: "1.5".</summary>
    public static string Number(double n) => n.ToString("0.##", Culture);

    /// <summary>A decimal at a fixed number of places: "0.8".</summary>
    public static string Number(double n, int decimals) => n.ToString("F" + decimals, Culture);

    /// <summary>A percentage from 0 to 100, as the culture writes one: "82%".</summary>
    public static string Percent(double percent) => (percent / 100.0).ToString("P0", Culture);

    /// <summary>A Unix time as a short date: "12 Oct 2026" or as the culture writes it.</summary>
    public static string Date(long unix) =>
        DateTimeOffset.FromUnixTimeSeconds(unix).ToLocalTime().ToString("d MMM yyyy", Culture);

    /// <summary>A span the engine split: "6d 17h", "1h 5m", "7m", "1m 05s".</summary>
    public static string Span(Span s)
    {
        long days = s.Days, hours = s.Hours, minutes = s.Minutes, seconds = s.Seconds;
        if (days > 0)
            return hours > 0 ? T.DurationDaysHours(days, hours) : T.DurationDays(days);
        if (hours > 0)
            return minutes > 0 ? T.DurationHoursMinutes(hours, minutes) : T.DurationHours(hours);
        if (minutes > 0)
            return seconds > 0 ? T.DurationMinutesSeconds(minutes, seconds.ToString("00", Culture)) : T.DurationMinutes(minutes);
        return seconds > 0 ? T.DurationSeconds(seconds) : T.DurationMinutes(0);
    }

    /// <summary>
    /// A wait in the largest units that still read at a glance, the model's <c>Span::countdown</c>:
    /// days and hours under a week, then days; hours and minutes; minutes.
    /// </summary>
    public static string Countdown(long seconds)
    {
        seconds = Math.Max(0, seconds);
        long minutes = seconds / 60, hours = minutes / 60, days = hours / 24;
        if (days >= 1)
            return Span(new Span((uint)days, days < 7 ? (byte)(hours % 24) : (byte)0, 0, 0));
        if (hours >= 1)
            return Span(new Span(0, (byte)hours, (byte)(minutes % 60), 0));
        return Span(new Span(0, 0, (byte)minutes, 0));
    }

    /// <summary>"2d ago", or "just now" under a minute.</summary>
    public static string Ago(long seconds) => seconds < 60 ? T.TimeJustNowInline : T.TimeAgo(Countdown(seconds));

    /// <summary>"in 4d 20h".</summary>
    public static string In(long seconds) => T.TimeIn(Countdown(seconds));

    /// <summary>A size the engine split, its number in the culture's digits: "9.4 MB".</summary>
    public static string Size(Size s)
    {
        var value = s.Value.ToString("F" + s.Decimals, Culture);
        return s.Unit switch
        {
            SizeUnit.Byte => T.SizeByte(value),
            SizeUnit.Kilobyte => T.SizeKilobyte(value),
            SizeUnit.Megabyte => T.SizeMegabyte(value),
            _ => T.SizeGigabyte(value),
        };
    }

    /// <summary>Bytes at human scale in decimal units, two figures: the model's <c>Size::of</c>.</summary>
    public static string Bytes(ulong bytes)
    {
        (SizeUnit Unit, ulong Scale)[] units =
            [(SizeUnit.Gigabyte, 1_000_000_000), (SizeUnit.Megabyte, 1_000_000), (SizeUnit.Kilobyte, 1_000), (SizeUnit.Byte, 1)];
        foreach (var (unit, scale) in units)
        {
            if (bytes < scale)
                continue;
            var value = (double)bytes / scale;
            return value < 10 && scale > 1
                ? Size(new Size(value, unit, 1))
                : Size(new Size(Math.Round(value), unit, 0));
        }
        return Size(new Size(0, SizeUnit.Byte, 0));
    }

    /// <summary>A rate: "1.2 MB/s".</summary>
    public static string Rate(ulong bytesPerSecond) => T.RatePerSecond(Bytes(bytesPerSecond));

    /// <summary>A runtime in whole minutes: "24 min".</summary>
    public static string Minutes(ulong seconds) => T.TimeMinutes((long)((seconds + 30) / 60));

    /// <summary>A season by name: "Autumn".</summary>
    public static string SeasonName(Season season) => season switch
    {
        Season.Winter => T.SeasonWinter,
        Season.Spring => T.SeasonSpring,
        Season.Summer => T.SeasonSummer,
        _ => T.SeasonAutumn,
    };

    /// <summary>A season and its year: "Autumn 2026".</summary>
    public static string SeasonWithYear(SeasonYear season) => SeasonWithYear(season.Season, season.Year);

    public static string SeasonWithYear(Season season, long year) => T.SeasonWithYear(SeasonName(season), year.ToString(CultureInfo.InvariantCulture));

    /// <summary>A format as a caption says it: "TV", "Movie", "OVA". AniList's wire name, or a name it has not got a word for, as it came.</summary>
    public static string Format(string wire) => wire.ToUpperInvariant() switch
    {
        "TV" => T.FormatTv,
        "TV_SHORT" => T.FormatTvShort,
        "MOVIE" => T.FormatMovie,
        "SPECIAL" => T.FormatSpecial,
        "OVA" => T.FormatOva,
        "ONA" => T.FormatOna,
        "MUSIC" => T.FormatMusic,
        _ => wire,
    };

    /// <summary>The title page's metadata line: "TV · Summer 2026 · 24 episodes · 79%".</summary>
    public static string MetaLine(Entry entry)
    {
        var parts = new List<string>();
        if (entry.Format is { Length: > 0 } f)
            parts.Add(Format(f));
        if (entry.Season is { } s && entry.Year is { } y)
            parts.Add(SeasonWithYear(s, y));
        else if (entry.Year is { } year)
            parts.Add(year.ToString(CultureInfo.InvariantCulture));
        if (entry.Episodes is > 0 and var n)
            parts.Add(T.EpisodesCount(n));
        if (entry.Score is { } score)
            parts.Add(Percent(score));
        return string.Join(" · ", parts);
    }

    /// <summary>The line under a cover: "Atelier Hotaru · 12 episodes", or the format and year when the studio is not known.</summary>
    public static string CoverCaption(Entry entry)
    {
        var parts = new List<string>();
        if (entry.Studio is { Length: > 0 } studio)
            parts.Add(studio);
        else if (entry.Format is { Length: > 0 } f)
            parts.Add(Format(f));
        if (entry.Episodes is > 0 and var n)
            parts.Add(T.EpisodesCount(n));
        else if (entry.Year is { } y)
            parts.Add(y.ToString(CultureInfo.InvariantCulture));
        return string.Join(" · ", parts);
    }

    /// <summary>The title page's line on its dub: "Dubbed to episode 6", "Dubbed", or nothing.</summary>
    public static string? DubLine(DubExtent? dub) =>
        dub is null ? null : dub.Complete ? T.AudioDubbed : T.TitleDubbedTo(dub.Through);

    /// <summary>An episode as a line names it, "Dubbed" after it for a dub: "Episode 10 · Dubbed".</summary>
    public static string EpisodeLine(string label, Translation? translation) =>
        translation == Translation.Dub ? T.EpisodeLineDubbed(label) : label;

    /// <summary>How long a code has left: "Expires in 15 min", "Expires in 45s", "Code expired".</summary>
    public static string CodeExpiry(long secondsLeft) => secondsLeft switch
    {
        <= 0 => T.AccountsCodeEnded,
        < 60 => T.AccountsCodeExpires(T.DurationSeconds(secondsLeft)),
        _ => T.AccountsCodeExpires(T.TimeMinutes((secondsLeft + 59) / 60)),
    };

    /// <summary>
    /// A subtitle or audio track: its language by name in the person's language, and its title's
    /// own words past the language it repeats: "English (Signs)".
    /// </summary>
    public static string TrackName(string? languageCode, string title)
    {
        var language = languageCode is { Length: > 0 } code ? LanguageName(code) : null;
        var english = languageCode is { Length: > 0 } c2 ? EnglishLanguageName(c2) : null;
        var words = title.Split(' ', StringSplitOptions.RemoveEmptyEntries)
            .Where(w => (language is null || !w.Equals(language, StringComparison.CurrentCultureIgnoreCase))
                && (english is null || !w.Equals(english, StringComparison.OrdinalIgnoreCase)));
        var own = string.Join(" ", words);
        if (own.Length > 0)
            own = char.ToUpper(own[0], Culture) + own[1..];
        return (language, own.Length == 0) switch
        {
            (null, true) => T.PlayerTrackUnnamed,
            (null, false) => own,
            (_, true) => language!,
            _ => T.ShowSettingsTrackWith(language!, own),
        };
    }

    /// <summary>A language code ("eng", "ja") by its name in the person's language, or the code when .NET has no name for it.</summary>
    public static string LanguageName(string code)
    {
        var culture = Lookup(code);
        if (culture is null)
            return code;
        var name = culture.DisplayName;
        return name.Length == 0 ? code : char.ToUpper(name[0], Culture) + name[1..];
    }

    static string? EnglishLanguageName(string code) => Lookup(code)?.EnglishName;

    static CultureInfo? Lookup(string code)
    {
        var two = code.ToLowerInvariant() switch
        {
            "eng" => "en",
            "jpn" or "jp" => "ja",
            "spa" => "es",
            "por" => "pt",
            "fre" or "fra" => "fr",
            "ger" or "deu" => "de",
            "ita" => "it",
            "rus" => "ru",
            "ara" => "ar",
            "ind" => "id",
            "kor" => "ko",
            "chi" or "zho" => "zh",
            var other => other,
        };
        try
        {
            var culture = CultureInfo.GetCultureInfo(two, predefinedOnly: true);
            return culture.Name.Length == 0 ? null : culture;
        }
        catch (CultureNotFoundException)
        {
            return null;
        }
    }

    /// <summary>
    /// A Continue row with its words in the person's language: the model's <c>QueueItem::of</c>
    /// decides what the row is about, and this says it. "Continue E7 · 10:06", "Play E4",
    /// "Play E9 · out 2h ago", "Caught up · E9 in 2d 3h"; the preview's button
    /// "Resume episode 7 · 10:06" or "Play episode 8"; with nothing out, "Episode 9 airs in 2d 3h".
    /// </summary>
    public static QueueItem Said(QueueItem item)
    {
        var entry = item.Entry;
        var episode = (long)item.Episode;
        var dub = entry.WatchedIn == Translation.Dub;
        var airs = entry.NextAiringIn is { } at && entry.NextEpisode == item.Episode ? In(at) : null;
        var clock = item.Resume is { } r ? Clock(r.Position) : null;
        var action = (clock, item.Playable, airs, item.AiredAgo) switch
        {
            ({ } c, _, _, _) => dub ? T.ContinueRowResumeDubbed(episode, c) : T.ContinueRowResume(episode, c),
            (null, false, { } a, _) => T.ContinueRowCaughtUp(episode, a),
            (null, false, null, _) => T.ContinueRowCaughtUp(episode, T.EpisodeRowNotOut),
            (null, true, _, { } ago) => T.ContinueRowStartAired(episode, Ago(ago)),
            _ => dub ? T.ContinueRowStartDubbed(episode) : T.ContinueRowStart(episode),
        };
        string? button = clock is { } c2
            ? EpisodeButton(T.TitleButtonResume(episode, c2), dub)
            : item.Playable ? EpisodeButton(WatchedAll(entry) ? T.TitleButtonWatchAgain : T.TitleButtonPlay(episode), dub) : null;
        string? waiting = item.Playable ? null : airs is { } w ? T.EpisodeAirs(episode, w) : T.EpisodeNotOut(episode);
        string? left = null;
        if (item.Resume is { Fraction: > 0 and var fraction } resume)
        {
            var rest = resume.Position / fraction - resume.Position;
            left = T.TimeMinutesLeft(Math.Max(1, (long)Math.Round(rest / 60)));
        }
        return item with { Action = action, Button = button, Waiting = waiting, Left = left, Mark = dub ? T.AudioDubbed : null };
    }

    /// <summary>Continue's queue, every row <see cref="Said(QueueItem)"/>.</summary>
    public static Queue Said(Queue queue) =>
        queue with { InProgress = [.. queue.InProgress.Select(Said)], NewThisWeek = [.. queue.NewThisWeek.Select(Said)] };

    /// <summary>Every episode of a show with a known count is watched, so the engine offers episode 1 again.</summary>
    static bool WatchedAll(Entry entry) =>
        entry.Episodes is > 0 and var total && entry.Progress is { } p && p.Done >= total && p.Next == 1;

    static string EpisodeButton(string text, bool dub) => dub ? T.EpisodeLineDubbed(text) : text;

    /// <summary>A playhead as people read it: "4:05", "1:02:03". The same in every language.</summary>
    public static string Clock(double seconds)
    {
        var s = (long)Math.Max(0, seconds);
        return s >= 3600 ? $"{s / 3600}:{s / 60 % 60:00}:{s % 60:00}" : $"{s / 60}:{s % 60:00}";
    }
}
