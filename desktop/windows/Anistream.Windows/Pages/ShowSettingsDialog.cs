using System.Globalization;
using Anistream.Engine.Core;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Anistream.App;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// One show's own settings, from the title page's More menu (Ctrl+I): its audio and the
/// subtitles the player remembers for it (read-only), its skips and spoiler protection,
/// new-episode notifications, the source it keeps, and a reset that puts it all back. Each row
/// follows Settings until the show sets its own. The rows are the Mac's and Linux's.
/// </summary>
sealed class ShowSettingsDialog
{
    static readonly (string Heading, ShowSettingKey[] Keys)[] Groups =
    [
        ("Audio and subtitles", [ShowSettingKey.Translation]),
        ("Episodes", [ShowSettingKey.SkipOpening, ShowSettingKey.SkipFiller, ShowSettingKey.SpoilerProtection]),
        ("Notifications", [ShowSettingKey.NotifyNewEpisodes]),
    ];

    readonly Ctx ctx;
    readonly uint id;
    readonly ContentDialog dialog;
    readonly List<(ShowSettingKey Key, ComboBox Box)> rows = [];
    readonly TextBlock subtitles = Ui.Text("Use default", "HazeBodyStyle", wrap: false);
    readonly ComboBox source = new() { MinWidth = 200 };
    readonly string[] sources;
    ShowSettingsView? view;
    bool quiet = true;

    ShowSettingsDialog(Ctx ctx, uint id, string title, XamlRoot root)
    {
        this.ctx = ctx;
        this.id = id;
        var body = new StackPanel { Spacing = 20, MinWidth = 420 };
        foreach (var (heading, keys) in Groups)
        {
            var group = Group(heading);
            foreach (var key in keys)
            {
                var box = new ComboBox { MinWidth = 200 };
                AutomationProperties.SetName(box, TitleOf(key));
                var k = key;
                box.SelectionChanged += (_, _) => Chosen(k, box);
                group.Children.Add(Row(TitleOf(key), key == ShowSettingKey.SpoilerProtection ? "Blurs stills after the next episode." : null, box));
                rows.Add((key, box));
            }
            if (keys is [ShowSettingKey.Translation])
                group.Children.Add(Row("Subtitles", "Remembered from your last episode.", subtitles));
            body.Children.Add(group);
        }

        var ordered = ctx.Known.OrderedProviders;
        sources = [.. ordered.Select(p => p.Id)];
        source.Items.Add("Automatic");
        foreach (var p in ordered)
            source.Items.Add(ProviderInfo.Of(p, ctx.Known.Stores).Name);
        AutomationProperties.SetName(source, "Source");
        source.SelectionChanged += (_, _) =>
        {
            if (quiet)
                return;
            var at = source.SelectedIndex;
            ctx.Send(new Command.SetProviderPreference(id, at > 0 ? sources[at - 1] : null));
            ctx.Send(new Command.LoadShowSettings(id));
        };
        var sourceGroup = Group("Source");
        sourceGroup.Children.Add(Row("Source", "Automatic tries your sources in order.", source));
        body.Children.Add(sourceGroup);

        var reset = new Button { Content = "Reset to defaults", Margin = new Thickness(0, 4, 0, 0) };
        reset.Click += (_, _) => ctx.Send(new Command.ResetShowSettings(id));
        var resetGroup = new StackPanel { Spacing = 6 };
        resetGroup.Children.Add(reset);
        resetGroup.Children.Add(Ui.Text("Also clears the remembered audio and subtitles.", "HazeCaptionStyle"));
        body.Children.Add(resetGroup);

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = new ScrollViewer { Content = body, MaxHeight = 560, Padding = new Thickness(0, 0, 12, 0) },
            CloseButtonText = "Done",
            DefaultButton = ContentDialogButton.Close,
        };
        AutomationProperties.SetName(dialog, $"Settings for {title}");
    }

    /// <summary>Opens the sheet for show <paramref name="id"/> and asks the engine for its values.</summary>
    public static async Task Open(Ctx ctx, uint id, string title, XamlRoot root)
    {
        var sheet = new ShowSettingsDialog(ctx, id, title, root);
        ctx.Batch += sheet.Heard;
        ctx.Send(new Command.LoadShowSettings(id));
        try
        {
            await sheet.dialog.ShowAsync();
        }
        finally
        {
            ctx.Batch -= sheet.Heard;
        }
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.ShowSettings s when s.V1.Id == id:
                    Fill(s.V1);
                    break;
                case Event.Settings when view is not null:
                    Fill(view);
                    break;
            }
        }
    }

    void Fill(ShowSettingsView sheet)
    {
        quiet = true;
        view = sheet;
        foreach (var (key, box) in rows)
        {
            if (sheet.Rows.FirstOrDefault(r => r.Key == key) is not { } row)
                continue;
            box.Items.Clear();
            box.Items.Add($"Use default · {Shown(key, row.Default)}");
            foreach (var (_, label) in Choices(key))
                box.Items.Add(label);
            var at = row.Own is { } own ? Array.FindIndex(Choices(key), c => Same(c.Value, own)) : -1;
            box.SelectedIndex = at + 1;
        }
        subtitles.Text = Subtitles(sheet, ctx.Known.Setting(SettingKey.Subtitles)?.Value ?? "");
        var chosen = sheet.Source is { } s ? Array.IndexOf(sources, s) : -1;
        source.SelectedIndex = chosen + 1;
        quiet = false;
    }

    void Chosen(ShowSettingKey key, ComboBox box)
    {
        if (quiet || box.SelectedIndex < 0)
            return;
        SettingValue? value = box.SelectedIndex == 0 ? null : Choices(key)[box.SelectedIndex - 1].Value;
        ctx.Send(new Command.SetShowSetting(id, key, value));
    }

    static StackPanel Group(string heading)
    {
        var group = new StackPanel { Spacing = 10 };
        var text = Ui.Text(heading, "BodyStrongTextBlockStyle", wrap: false);
        AutomationProperties.SetHeadingLevel(text, AutomationHeadingLevel.Level2);
        group.Children.Add(text);
        return group;
    }

    static Grid Row(string title, string? line, FrameworkElement control)
    {
        var grid = new Grid { ColumnSpacing = 16 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        words.Children.Add(Ui.Text(title, "BodyTextBlockStyle", wrap: false));
        if (line is not null)
            words.Children.Add(Ui.Text(line, "HazeCaptionStyle"));
        grid.Children.Add(words);
        control.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(control, 1);
        grid.Children.Add(control);
        return grid;
    }

    static string TitleOf(ShowSettingKey key) => key switch
    {
        ShowSettingKey.Translation => "Audio",
        ShowSettingKey.SkipOpening => "Skip opening",
        ShowSettingKey.SkipFiller => "Skip filler",
        ShowSettingKey.SpoilerProtection => "Spoiler protection",
        ShowSettingKey.NotifyNewEpisodes => "New episodes",
        ShowSettingKey.Quality => "Quality",
        ShowSettingKey.DownloadQuality => "Download quality",
        _ => "Keep next episodes",
    };

    /// <summary>A row's value in the sheet's words: Japanese or English for the audio, On or Off.</summary>
    internal static string Shown(ShowSettingKey key, string value) => key switch
    {
        ShowSettingKey.Translation => value.StartsWith("dub", StringComparison.Ordinal) ? "English" : "Japanese",
        _ => value is "on" or "true" ? "On" : "Off",
    };

    static (SettingValue Value, string Label)[] Choices(ShowSettingKey key) => key == ShowSettingKey.Translation
        ? [(new SettingValue.Str("sub"), "Japanese"), (new SettingValue.Str("dub"), "English")]
        : [(new SettingValue.Bool(true), "On"), (new SettingValue.Bool(false), "Off")];

    static bool Same(SettingValue a, SettingValue b) => (a, b) switch
    {
        (SettingValue.Str x, SettingValue.Str y) => x.V1 == y.V1,
        (SettingValue.Bool x, SettingValue.Bool y) => x.V1 == y.V1,
        _ => false,
    };

    /// <summary>The subtitles remembered for the audio in effect, or following Settings when none are.</summary>
    static string Subtitles(ShowSettingsView sheet, string settings)
    {
        var dub = sheet.Rows.FirstOrDefault(r => r.Key == ShowSettingKey.Translation)?.Value.StartsWith("dub", StringComparison.Ordinal) == true;
        var translation = dub ? Translation.Dub : Translation.Sub;
        var remembered = sheet.Remembered.FirstOrDefault(r => r.Translation == translation)?.Subtitles;
        return remembered switch
        {
            RememberedSubtitles.Off => "Off",
            RememberedSubtitles.Track t => Track(t.TrackValue),
            _ when settings.Length == 0 => "Use default",
            _ => $"Use default · {Language(settings)}",
        };
    }

    static string Track(RememberedTrack track)
    {
        var name = track.Language is { Length: > 0 } lang ? Language(lang) : null;
        var title = track.Title.Length > 0 ? CultureInfo.CurrentCulture.TextInfo.ToTitleCase(track.Title) : null;
        return (name, title) switch
        {
            ({ } n, { } t) when !t.Equals(n, StringComparison.OrdinalIgnoreCase) => $"{n} ({t.Replace(n, "", StringComparison.OrdinalIgnoreCase).Trim()})".Replace(" ()", ""),
            ({ } n, _) => n,
            (_, { } t) => t,
            _ => "Unnamed track",
        };
    }

    static string Language(string code)
    {
        try
        {
            return new CultureInfo(code.Split(',')[0].Trim()).EnglishName.Split(" (")[0];
        }
        catch (CultureNotFoundException)
        {
            return code;
        }
    }
}
