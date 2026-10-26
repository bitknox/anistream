using Anistream.App;
using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using CommunityToolkit.WinUI.Controls;
using Microsoft.UI.Xaml.Navigation;
using K = Anistream.Engine.Model.SettingKey;

namespace Anistream.Windows.Pages;

/// <summary>
/// Settings, styled as Windows 11 Settings: the panes as a list with icons, the pane's rows as
/// setting cards grouped under their headings. Rows follow the engine's settings view: a switch
/// for a setting that is on or off, a drop-down of the catalog's values for a stepped one, a box
/// for a typed one. Tracker tokens live in Credential Manager, so token storage is not a row, and
/// the theme is dark only in this version.
/// </summary>
public sealed partial class SettingsPage : Page
{
    sealed record Pane(string Name, string Title, string Glyph, (string Heading, K[] Keys)[] Groups);

    static readonly Pane[] PaneList =
    [
        new("playback", "Playback", "",
        [
            // How titles read everywhere, not only while watching, so it heads the pane on its own.
            ("General", [K.Titles]),
            ("Watching", [K.Translation, K.Quality, K.Subtitles, K.CommitThreshold]),
            ("Player", [K.AutoNext, K.UpNextCountdown, K.SkipOpening, K.SkipFiller, K.MarkChapters, K.SkipButton, K.Fullscreen]),
            ("Picture", [K.Upscaling, K.NightMode, K.SpoilerProtection, K.Motion]),
        ]),
        new("downloads", "Downloads", "",
        [
            ("Files", [K.DownloadDir, K.DownloadQuality, K.MergeSubtitles, K.DownloadHook]),
            ("Ahead", [K.KeepNextEpisodes]),
        ]),
        new("notifications", "Notifications", "", [("New episodes", [K.EpisodeNotifications])]),
        new("sources", "Sources", "",
            [("Torrents", [K.Torrents, K.VpnMode, K.VpnOnFailure, K.KeepSeeding, K.SeedRatio, K.SeedHours, K.UploadLimit])]),
        new("accounts", "Accounts", "", []),
        new("integrations", "Integrations", "",
        [
            ("Discord", [K.Presence, K.PresenceTitle, K.PresenceRich]),
            ("Updates", [K.UpdateCheck]),
        ]),
        new("advanced", "Advanced", "", [("mpv", [K.MpvOptions])]),
        new("about", "About", "", []),
    ];

    readonly Ctx ctx = Ctx.Current;
    Pane pane = PaneList[0];
    bool quiet, narrow, picking;

    public SettingsPage()
    {
        InitializeComponent();
        foreach (var p in PaneList)
        {
            var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12, Height = 36 };
            line.Children.Add(new FontIcon { Glyph = p.Glyph, FontSize = 16, VerticalAlignment = VerticalAlignment.Center });
            line.Children.Add(new TextBlock { Text = p.Title, VerticalAlignment = VerticalAlignment.Center });
            Panes.Items.Add(new ListViewItem { Content = line, Tag = p });
        }
        State.Placeholders = () =>
        {
            var plates = new StackPanel { Spacing = 4 };
            plates.Children.Add(States.Plate(120, 14, 3));
            for (var i = 0; i < 5; i++)
                plates.Children.Add(new Border { Height = 48, CornerRadius = new CornerRadius(4), Background = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("RaisedBrush") });
            return plates;
        };
        State.ActivityText = "Loading settings";
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        ctx.Tv.Changed += TvChanged;
        Unloaded += (_, _) =>
        {
            ctx.Batch -= Heard;
            ctx.Tv.Changed -= TvChanged;
        };
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        var name = e.Parameter as string ?? ctx.Options.Pane ?? "playback";
        var at = Array.FindIndex(PaneList, p => p.Name == name);
        Panes.SelectedIndex = at < 0 ? 0 : at;
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Settings || e is Event.Sync && pane.Name == "accounts"))
            Show();
    }

    void OnPaneChanged(object sender, SelectionChangedEventArgs e)
    {
        if ((Panes.SelectedItem as ListViewItem)?.Tag is Pane p)
        {
            pane = p;
            Show();
        }
    }

    /// <summary>Narrow, a click on a pane opens it in the panes' place.</summary>
    void OnPaneClicked(object sender, ItemClickEventArgs e)
    {
        if (!narrow)
            return;
        picking = false;
        Layout();
    }

    void OnCrumb(BreadcrumbBar sender, BreadcrumbBarItemClickedEventArgs e)
    {
        if (e.Index == 0)
        {
            picking = true;
            Layout();
        }
    }

    void OnColumnsSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var now = e.NewSize.Width < 760;
        if (now == narrow)
            return;
        narrow = now;
        picking = false;
        Layout();
    }

    void Layout()
    {
        PaneColumn.Width = narrow ? (picking ? new GridLength(1, GridUnitType.Star) : new GridLength(0)) : new GridLength(200);
        Panes.Visibility = !narrow || picking ? Visibility.Visible : Visibility.Collapsed;
        CardScroll.Visibility = !narrow || !picking ? Visibility.Visible : Visibility.Collapsed;
        Grid.SetColumn(Panes, 0);
        Crumbs.Visibility = narrow ? Visibility.Visible : Visibility.Collapsed;
        Crumbs.ItemsSource = picking ? new[] { "Settings" } : new[] { "Settings", pane.Title };
    }

    static T Res<T>(string key) => (T)Application.Current.Resources[key];

    void Show()
    {
        quiet = true;
        Cards.Children.Clear();
        if (narrow)
            Layout();
        switch (pane.Name)
        {
            case "accounts":
                ShowAccounts();
                break;
            case "about":
                ShowAbout();
                break;
            default:
                if (ctx.Known.Settings is null)
                {
                    State.Wait();
                    break;
                }
                tvGroup = null;
                foreach (var (heading, keys) in pane.Groups)
                {
                    // Play on TV's paired TVs, between Discord and Updates.
                    if (pane.Name == "integrations" && heading == "Updates" && ctx.Tv.Available)
                    {
                        Cards.Children.Add(Heading("Play on TV"));
                        tvGroup = new StackPanel { Spacing = 4 };
                        ShowTvs(tvGroup);
                        Cards.Children.Add(tvGroup);
                    }
                    // Only where the engine was built with a presence to publish.
                    if (keys.Contains(K.Presence) && ctx.Known.Settings is { PresenceAvailable: false })
                        continue;
                    var rows = keys.Select(k => ctx.Known.Setting(k)).OfType<SettingRow>().ToList();
                    if (rows.Count == 0)
                        continue;
                    Cards.Children.Add(Heading(heading));
                    foreach (var row in rows)
                        Cards.Children.Add(Card(row));
                    if (pane.Name == "downloads" && heading == "Files")
                        Cards.Children.Add(WhenClosingCard());
                }
                State.Done();
                break;
        }
        quiet = false;
    }

    StackPanel? tvGroup;
    string? tvsDrawn;

    void TvChanged()
    {
        if (tvGroup is { } group)
            ShowTvs(group);
    }

    /// <summary>
    /// The TVs this PC is paired with, each with what is known of it and Forget, then the way to
    /// pair another, and the code a TV lists this PC with.
    /// </summary>
    void ShowTvs(StackPanel group)
    {
        var tv = ctx.Tv;
        var now = DateTimeOffset.UtcNow;
        var notes = tv.Paired.Select(p => (p, Note: TvWords.Note(p, tv.Connected == p.Fingerprint, now))).ToList();
        // Drawn again only when what it says changed, so a click in it is not lost.
        var key = string.Join("|", notes.Select(n => $"{n.p.Fingerprint},{n.p.Name},{n.Note}")) + tv.ShortFingerprint;
        if (key == tvsDrawn && group.Children.Count > 0)
            return;
        tvsDrawn = key;
        group.Children.Clear();
        foreach (var (paired, note) in notes)
        {
            var forget = new Button { Content = "Forget" };
            AutomationProperties.SetName(forget, $"Forget {paired.Name}");
            forget.Click += async (_, _) =>
            {
                if (XamlRoot is { } root && await Ui.Confirm(root, $"Forget {paired.Name}?", "To play on it again, pair with it again.", "Forget"))
                    await tv.Forget(paired.Fingerprint);
            };
            group.Children.Add(CardFrame(paired.Name, note, forget, "\uE7F4"));
        }
        var pair = new Button { Content = "Pair a TV…" };
        pair.Click += (_, _) => tv.OpenPicker();
        group.Children.Add(CardFrame(tv.Paired.Length == 0 ? "No TVs paired yet" : "Another TV", TvWords.PairNote, pair, tv.Paired.Length == 0 ? "\uE7F4" : "\uE710"));
        if (tv.ShortFingerprint.Length > 0)
        {
            var foot = Ui.Text(TvWords.Footnote(tv.ShortFingerprint), "HazeCaptionStyle");
            foot.Margin = new Thickness(2, 4, 0, 0);
            group.Children.Add(foot);
        }
    }

    static TextBlock Heading(string text) => new()
    {
        Text = text,
        Style = Res<Style>("BodyStrongTextBlockStyle"),
        Margin = new Thickness(0, 16, 0, 4),
    };

    /// <summary>A row as Windows 11 Settings draws one: the Toolkit's SettingsCard.</summary>
    static SettingsCard CardFrame(string title, string? description, FrameworkElement? control, string? glyph = null)
    {
        var card = new SettingsCard { Header = title, Description = description, Content = control };
        if (glyph is not null)
            card.HeaderIcon = new FontIcon { Glyph = glyph };
        return card;
    }

    /// <summary>What closing the window does while episodes download: ask, keep going, or stop.</summary>
    SettingsCard WhenClosingCard()
    {
        var choices = new[] { (CloseChoice.Ask, "Ask"), (CloseChoice.KeepGoing, "Keep going"), (CloseChoice.Stop, "Stop") };
        var box = new ComboBox { MinWidth = 200, ItemsSource = choices.Select(c => c.Item2).ToList() };
        box.SelectedIndex = Array.FindIndex(choices, c => c.Item1 == ctx.Window.WhenClosing);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, "When the window closes with downloads running");
        box.SelectionChanged += (_, _) =>
        {
            if (!quiet && box.SelectedIndex >= 0)
                ctx.Window.WhenClosing = choices[box.SelectedIndex].Item1;
        };
        return CardFrame("When the window closes with downloads running", "Keep going leaves anistream in the notification area until they finish.", box);
    }

    /// <summary>
    /// Turning torrents on says what it means first, and without protection asks plainly: the
    /// switch goes back off unless the answer is Turn on.
    /// </summary>
    async void ConfirmTorrents(ToggleSwitch toggle, SettingRow row)
    {
        var unprotected = ctx.Known.Setting(K.VpnMode)?.Value == "none";
        var body = new StackPanel { Spacing = 10 };
        body.Children.Add(Ui.Text("Torrents download from other people and share what you download with them while you watch. They can see your IP address.", "BodyTextBlockStyle"));
        body.Children.Add(Ui.Haze(unprotected
            ? "Torrent protection is off, so nothing hides your IP address."
            : "anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you."));
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = unprotected ? "Use torrents without protection?" : "Turn on torrents?",
            Content = body,
            PrimaryButtonText = "Turn on",
            CloseButtonText = "Cancel",
            DefaultButton = unprotected ? ContentDialogButton.Close : ContentDialogButton.Primary,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
            ctx.Send(new Command.SetSetting(row.Key, new SettingValue.Bool(true)));
        else
        {
            quiet = true;
            toggle.IsOn = false;
            quiet = false;
        }
    }

    FrameworkElement Card(SettingRow row)
    {
        var title = Words.SettingTitle(row.Key);
        var overridden = ctx.Known.Settings?.Overridden?.FirstOrDefault(o => o.Key == row.Key);
        string? description = overridden is { Shows: > 0 } o ? $"Overridden for {AnistreamFfiMethods.Plural(o.Shows, "show")}" : null;
        if (row.Key == K.PresenceRich)
            description = "The cover, a progress bar and a link to the show on AniList. Needs the title shown.";
        FrameworkElement control;
        switch (SettingsCatalog.ControlFor(row))
        {
            case SettingControl.Switch:
            {
                var toggle = new ToggleSwitch { IsOn = SettingsCatalog.IsOn(row), OnContent = "", OffContent = "", MinWidth = 0 };
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(toggle, title);
                toggle.Toggled += (_, _) =>
                {
                    if (quiet)
                        return;
                    if (row.Key == K.Torrents && toggle.IsOn)
                        ConfirmTorrents(toggle, row);
                    else
                        ctx.Send(new Command.SetSetting(row.Key, new SettingValue.Bool(toggle.IsOn)));
                };
                control = toggle;
                break;
            }
            case SettingControl.Choice:
            {
                var options = SettingsCatalog.Options(row.Key);
                var box = new ComboBox { MinWidth = 200, ItemsSource = options.Select(o => Shown(o.Label)).ToList() };
                var at = SettingsCatalog.Current(row) is { } current ? options.ToList().IndexOf(current) : -1;
                if (at < 0)
                    box.PlaceholderText = Shown(row.Value);
                else
                    box.SelectedIndex = at;
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, title);
                box.SelectionChanged += (_, _) =>
                {
                    if (!quiet && box.SelectedIndex >= 0 && box.SelectedIndex != at)
                        ctx.Send(new Command.SetSetting(row.Key, options[box.SelectedIndex].Value));
                };
                control = box;
                break;
            }
            case SettingControl.Text:
            {
                var text = SettingsCatalog.TypedText(row) ?? "";
                var box = new TextBox { Text = text, MinWidth = 220, PlaceholderText = "Not set" };
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, title);
                void Commit()
                {
                    if (!quiet && box.Text.Trim() != text)
                        ctx.Send(new Command.SetSetting(row.Key, new SettingValue.Str(box.Text.Trim())));
                }
                box.LostFocus += (_, _) => Commit();
                box.KeyDown += (_, e) =>
                {
                    if (e.Key == global::Windows.System.VirtualKey.Enter)
                        Commit();
                };
                control = box;
                break;
            }
            default:
                control = new TextBlock { Text = Shown(row.Value), Style = Res<Style>("HazeBodyStyle") };
                break;
        }
        var card = CardFrame(title, description, control);
        // A row whose switch is off waits, greyed, as Windows Settings shows a dependent row.
        card.IsEnabled = SettingsCatalog.Needs(row.Key).All(k => SettingsCatalog.IsOn(ctx.Known.Setting(k)));
        return card;
    }

    /// <summary>A value as a row shows it, in sentence case.</summary>
    static string Shown(string value) => value.Length == 0 ? "None" : char.ToUpperInvariant(value[0]) + value[1..];

    void ShowAccounts()
    {
        Cards.Children.Add(Heading("Trackers"));
        foreach (var tracker in new[] { "anilist", "mal", "kitsu", "simkl", "trakt" })
        {
            var name = AnistreamFfiMethods.TrackerName(tracker);
            var state = ctx.Known.Sync.GetValueOrDefault(tracker);
            Button action;
            string description;
            if (state is { Connected: true })
            {
                description = state.User is { } user ? $"Signed in as {user}" : "Signed in";
                if (state.Outbox > 0)
                    description += $" · {AnistreamFfiMethods.Plural(state.Outbox, "change")} waiting to sync";
                action = new Button { Content = "Sign out" };
                action.Click += (_, _) => ctx.Send(new Command.Disconnect(tracker));
            }
            else
            {
                description = tracker is "simkl" or "trakt" ? "Not signed in · You enter a code on its site" : "Not signed in";
                action = new Button { Content = "Sign in" };
                action.Click += (_, _) => ctx.Send(new Command.Connect(tracker));
            }
            Cards.Children.Add(CardFrame(name, description, action));
        }
        var sync = new Button { Content = "Sync now" };
        sync.Click += (_, _) => ctx.Send(new Command.SyncNow());
        Cards.Children.Add(Heading("Sync"));
        Cards.Children.Add(CardFrame("Sync with the trackers", "Sends what changed here and fetches what changed elsewhere.", sync));
    }

    void ShowAbout()
    {
        var top = new Grid { ColumnSpacing = 24, Margin = new Thickness(0, 8, 0, 8) };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        top.Children.Add(new Nagare(6));
        var text = new StackPanel { Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = "anistream", Style = Res<Style>("SubtitleTextBlockStyle") });
        text.Children.Add(new TextBlock { Text = $"Version {Updater.Current}", Style = Res<Style>("HazeBodyStyle") });
        text.Children.Add(new TextBlock { Text = "anistream hosts no content.", Style = Res<Style>("HazeBodyStyle") });
        text.Children.Add(new UpdatePanel(ctx, ctx.Updater) { Margin = new Thickness(0, 6, 0, 0) });
        Grid.SetColumn(text, 1);
        top.Children.Add(text);
        Cards.Children.Add(top);
        Cards.Children.Add(Heading("Notices"));
        foreach (var (name, line) in new[]
        {
            ("anistream", "MIT licence"),
            ("Zen Kaku Gothic New", "SIL Open Font Licence 1.1"),
            ("libmpv", "LGPL 2.1 or later"),
            ("Artwork under CC BY-NC", "Attribution in the app's notices"),
        })
            Cards.Children.Add(CardFrame(name, line, null));
    }
}
