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
/// subtitles the player remembers for it (read-only), the quality it plays in, its skips and
/// spoiler protection, its downloads' quality and how many episodes it keeps ahead,
/// new-episode notifications, the source it keeps, and a reset that puts it all back. Each row
/// follows Settings until the show sets its own. The rows are the engine's (<c>ShowSettingKey</c>),
/// the Mac's and Android's among them. Until the engine answers, every row stands on "–" and
/// cannot be changed; a line under the title, in a slot of its own, says it is loading after
/// 600 ms and that it failed after 8 s, with Try again, so nothing moves when the answer lands.
/// </summary>
sealed class ShowSettingsDialog
{
    static readonly TimeSpan Silence = TimeSpan.FromMilliseconds(600), GiveUp = TimeSpan.FromSeconds(8);

    /// <summary>The groups, each read from the catalogue when the sheet opens.</summary>
    static (string Heading, ShowSettingKey[] Keys)[] Groups =>
    [
        (T.ShowSettingsAudioSubtitles, [ShowSettingKey.Translation]),
        (T.SettingsGroupPlayback, [ShowSettingKey.Quality, ShowSettingKey.SkipOpening, ShowSettingKey.SkipFiller, ShowSettingKey.SpoilerProtection]),
        (T.NavDownloads, [ShowSettingKey.DownloadQuality, ShowSettingKey.KeepNext]),
        (T.SettingsGroupNotifications, [ShowSettingKey.NotifyNewEpisodes]),
    ];

    readonly Ctx ctx;
    readonly uint id;
    readonly ContentDialog dialog;
    readonly List<(ShowSettingKey Key, ComboBox Box)> rows = [];
    readonly TextBlock subtitles = Ui.Text("–", "HazeBodyStyle", wrap: false);
    readonly ComboBox source = new() { MinWidth = 200 };
    readonly Button reset = new() { Content = T.ShowSettingsReset, Margin = new Thickness(0, 4, 0, 0), IsEnabled = false };
    readonly string[] sources;
    readonly Grid status = new() { Height = 20, Margin = new Thickness(0, 0, 0, -8) };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer silence, giveUp;
    ShowSettingsView? view;
    bool quiet = true;

    ShowSettingsDialog(Ctx ctx, uint id, string title, XamlRoot root)
    {
        this.ctx = ctx;
        this.id = id;
        var body = new StackPanel { Spacing = 20, MinWidth = 440 };
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);
        body.Children.Add(status);
        foreach (var (heading, keys) in Groups)
        {
            var group = Group(heading);
            foreach (var key in keys)
            {
                var box = new ComboBox { MinWidth = 200, IsEnabled = false };
                box.Items.Add("–");
                box.SelectedIndex = 0;
                AutomationProperties.SetName(box, TitleOf(key));
                var k = key;
                box.SelectionChanged += (_, _) => Chosen(k, box);
                group.Children.Add(Row(TitleOf(key), NoteOf(key), box));
                rows.Add((key, box));
            }
            if (keys is [ShowSettingKey.Translation, ..])
                group.Children.Add(Row(T.PlayerSubtitles, T.ShowSettingsSubsRemembered, subtitles));
            body.Children.Add(group);
        }

        var ordered = ctx.Known.OrderedProviders;
        sources = [.. ordered.Select(p => p.Id)];
        source.Items.Add(T.ValueAutomatic);
        foreach (var p in ordered)
            source.Items.Add(ProviderInfo.Of(p, ctx.Known.Stores).Name);
        source.SelectedIndex = 0;
        source.IsEnabled = false;
        AutomationProperties.SetName(source, T.ShowSettingsSource);
        source.SelectionChanged += (_, _) =>
        {
            if (quiet)
                return;
            var at = source.SelectedIndex;
            ctx.Send(new Command.SetProviderPreference(id, at > 0 ? sources[at - 1] : null));
            ctx.Send(new Command.LoadShowSettings(id));
        };
        var sourceGroup = Group(T.ShowSettingsSource);
        sourceGroup.Children.Add(Row(T.ShowSettingsSource, sources.Length > 0 ? T.ShowSettingsSourceDetail : T.ShowSettingsNoSource, source));
        body.Children.Add(sourceGroup);

        reset.Click += (_, _) => ctx.Send(new Command.ResetShowSettings(id));
        var resetGroup = new StackPanel { Spacing = 6 };
        resetGroup.Children.Add(reset);
        resetGroup.Children.Add(Ui.Text(T.ShowSettingsResetDetail, "HazeCaptionStyle"));
        body.Children.Add(resetGroup);

        var queue = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
        silence = queue.CreateTimer();
        silence.Interval = Silence;
        silence.IsRepeating = false;
        silence.Tick += (_, _) => Say(Loading());
        giveUp = queue.CreateTimer();
        giveUp.Interval = GiveUp;
        giveUp.IsRepeating = false;
        giveUp.Tick += (_, _) => Say(Failed());

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = title,
            Content = new ScrollViewer { Content = body, MaxHeight = 560, Padding = new Thickness(0, 0, 12, 0) },
            CloseButtonText = T.ActionDone,
            DefaultButton = ContentDialogButton.Close,
        };
        AutomationProperties.SetName(dialog, T.ShowSettingsLabel(title));
    }

    /// <summary>Asks the engine for the show's values, and starts the clocks that say it is taking a while.</summary>
    void Ask()
    {
        silence.Stop();
        giveUp.Stop();
        if (view is null)
        {
            silence.Start();
            giveUp.Start();
        }
        ctx.Send(new Command.LoadShowSettings(id));
    }

    static UIElement Loading()
    {
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        line.Children.Add(new ProgressRing { Width = 14, Height = 14, IsActive = true });
        line.Children.Add(Ui.Text(T.ShowSettingsLoading, "HazeCaptionStyle", wrap: false));
        return line;
    }

    UIElement Failed()
    {
        var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        line.Children.Add(States.ErrorGlyph(14));
        line.Children.Add(Ui.Text(T.ShowSettingsFailed, "HazeCaptionStyle", wrap: false));
        var again = new HyperlinkButton { Content = T.ActionTryAgain, Padding = new Thickness(4, 0, 4, 0), MinHeight = 0 };
        again.Click += (_, _) =>
        {
            Say(null);
            Ask();
        };
        line.Children.Add(again);
        return line;
    }

    /// <summary>The status slot's line, or none: it keeps its height either way.</summary>
    void Say(UIElement? line)
    {
        status.Children.Clear();
        if (line is null)
            return;
        status.Children.Add(line);
        Motion.FadeIn(line, Motion.Quick);
    }

    /// <summary>Opens the sheet for show <paramref name="id"/> and asks the engine for its values.</summary>
    public static async Task Open(Ctx ctx, uint id, string title, XamlRoot root)
    {
        var sheet = new ShowSettingsDialog(ctx, id, title, root);
        ctx.Batch += sheet.Heard;
#if ANISTREAM_TESTING
        // --show=settings-loading or settings-failed: the sheet with no answer, for a screenshot.
        if (ctx.Options.Show is "settings-loading" or "settings-failed")
        {
            if (ctx.Options.Show == "settings-failed")
                sheet.giveUp.Interval = TimeSpan.FromSeconds(1);
            sheet.silence.Start();
            sheet.giveUp.Start();
        }
        else
#endif
        sheet.Ask();
        try
        {
            await Dialogs.Show(sheet.dialog);
        }
        finally
        {
            ctx.Batch -= sheet.Heard;
            sheet.silence.Stop();
            sheet.giveUp.Stop();
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
        var first = view is null;
        view = sheet;
        silence.Stop();
        giveUp.Stop();
        if (first)
            Say(null);
        foreach (var (key, box) in rows)
        {
            if (sheet.Rows.FirstOrDefault(r => r.Key == key) is not { } row)
                continue;
            box.Items.Clear();
            box.Items.Add(T.ValueUseDefaultIs(ShowSettingsCatalog.Shown(key, row.Default)));
            foreach (var choice in ShowSettingsCatalog.Choices(key))
                box.Items.Add(choice.Label);
            box.SelectedIndex = ShowSettingsCatalog.IndexOf(key, row.Own) + 1;
            box.IsEnabled = true;
        }
        subtitles.Text = Subtitles(sheet, ctx.Known.Setting(SettingKey.Subtitles)?.Value ?? "");
        var chosen = sheet.Source is { } s ? Array.IndexOf(sources, s) : -1;
        source.SelectedIndex = chosen + 1;
        source.IsEnabled = sources.Length > 0;
        reset.IsEnabled = true;
        quiet = false;
    }

    void Chosen(ShowSettingKey key, ComboBox box)
    {
        if (quiet || box.SelectedIndex < 0)
            return;
        SettingValue? value = box.SelectedIndex == 0 ? null : ShowSettingsCatalog.Choices(key)[box.SelectedIndex - 1].Value;
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
        ShowSettingKey.Translation => T.SettingsAudio,
        ShowSettingKey.SkipOpening => T.ShowSettingsSkipOpening,
        ShowSettingKey.SkipFiller => T.ShowSettingsSkipFiller,
        ShowSettingKey.SpoilerProtection => T.SettingsSpoiler,
        ShowSettingKey.NotifyNewEpisodes => T.SettingsNewEpisodes,
        ShowSettingKey.Quality => T.SettingsQuality,
        ShowSettingKey.DownloadQuality => T.DownloadsQuality,
        _ => T.SettingsKeepNext,
    };

    static string? NoteOf(ShowSettingKey key) => key switch
    {
        ShowSettingKey.SpoilerProtection => T.ShowSettingsSpoilerDetail,
        ShowSettingKey.KeepNext => T.ShowSettingsKeepNextDetail,
        ShowSettingKey.NotifyNewEpisodes => T.ShowSettingsNotifyDetail,
        _ => null,
    };

    /// <summary>The subtitles remembered for the audio in effect, or following Settings when none are.</summary>
    static string Subtitles(ShowSettingsView sheet, string settings)
    {
        var dub = sheet.Rows.FirstOrDefault(r => r.Key == ShowSettingKey.Translation)?.Value.StartsWith("dub", StringComparison.Ordinal) == true;
        var translation = dub ? Translation.Dub : Translation.Sub;
        var remembered = sheet.Remembered.FirstOrDefault(r => r.Translation == translation)?.Subtitles;
        return remembered switch
        {
            RememberedSubtitles.Off => T.ValueOff,
            RememberedSubtitles.Track t => Wording.TrackName(t.TrackValue.Language, t.TrackValue.Title),
            _ when settings.Length == 0 => T.ValueUseDefault,
            _ => T.ValueUseDefaultIs(Wording.LanguageName(settings.Split(',')[0].Trim())),
        };
    }
}
