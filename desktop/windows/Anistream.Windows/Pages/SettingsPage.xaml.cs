using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using K = Anistream.Engine.Model.SettingKey;

namespace Anistream.Windows.Pages;

/// <summary>
/// Settings, styled as Windows 11 Settings: the panes as a list with icons, the pane's rows as
/// setting cards grouped under their headings. Rows follow the engine's settings view: a switch
/// for a setting that is on or off, previous and next for one stepped through a fixed set, a box
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
            ("Watching", [K.Translation, K.Quality, K.Subtitles, K.Titles, K.CommitThreshold]),
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
            ("Watch together", [K.Syncplay, K.SyncplayServer, K.SyncplayRoom]),
            ("Discord", [K.Presence, K.PresenceTitle]),
            ("Updates", [K.UpdateCheck]),
        ]),
        new("advanced", "Advanced", "", [("mpv", [K.MpvOptions])]),
        new("about", "About", "", []),
    ];

    readonly Ctx ctx = Ctx.Current;
    Pane pane = PaneList[0];
    bool quiet;

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
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
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

    static T Res<T>(string key) => (T)Application.Current.Resources[key];

    void Show()
    {
        quiet = true;
        Cards.Children.Clear();
        switch (pane.Name)
        {
            case "accounts":
                ShowAccounts();
                break;
            case "about":
                ShowAbout();
                break;
            default:
                foreach (var (heading, keys) in pane.Groups)
                {
                    var rows = keys.Select(k => ctx.Known.Setting(k)).OfType<SettingRow>().ToList();
                    if (rows.Count == 0)
                        continue;
                    Cards.Children.Add(Heading(heading));
                    foreach (var row in rows)
                        Cards.Children.Add(Card(row));
                }
                if (Cards.Children.Count == 0)
                    Cards.Children.Add(new TextBlock { Text = "Loading settings", Style = Res<Style>("HazeBodyStyle") });
                break;
        }
        quiet = false;
    }

    static TextBlock Heading(string text) => new()
    {
        Text = text,
        Style = Res<Style>("BodyStrongTextBlockStyle"),
        Margin = new Thickness(0, 16, 0, 4),
    };

    static Grid CardFrame(string title, string? description, FrameworkElement? control, string? glyph = null)
    {
        var card = new Grid { Style = Res<Style>("SettingCardStyle") };
        if (description is not null)
            card.MinHeight = 68;
        card.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        card.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        card.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        if (glyph is not null)
            card.Children.Add(new FontIcon { Glyph = glyph, FontSize = 16, VerticalAlignment = VerticalAlignment.Center });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center, Spacing = 2 };
        text.Children.Add(new TextBlock { Text = title, TextWrapping = TextWrapping.WrapWholeWords });
        if (description is not null)
            text.Children.Add(new TextBlock { Text = description, Style = Res<Style>("HazeCaptionStyle"), TextWrapping = TextWrapping.WrapWholeWords });
        Grid.SetColumn(text, 1);
        card.Children.Add(text);
        if (control is not null)
        {
            control.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(control, 2);
            card.Children.Add(control);
        }
        return card;
    }

    static bool IsOn(string value) => value is "on" or "yes" or "true";
    static bool IsOnOff(string value) => value is "on" or "off" or "yes" or "no" or "true" or "false";

    FrameworkElement Card(SettingRow row)
    {
        var title = Words.SettingTitle(row.Key);
        var overridden = ctx.Known.Settings?.Overridden?.FirstOrDefault(o => o.Key == row.Key);
        string? description = overridden is { Shows: > 0 } o ? $"Overridden for {AnistreamFfiMethods.Plural(o.Shows, "show")}" : null;
        FrameworkElement control;
        switch (row.Change)
        {
            case SettingChange.Steps steps when IsOnOff(row.Value):
            {
                var toggle = new ToggleSwitch { IsOn = IsOn(row.Value), OnContent = "", OffContent = "", MinWidth = 0 };
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(toggle, title);
                toggle.Toggled += (_, _) =>
                {
                    if (!quiet)
                        ctx.Send(new Command.SetSetting(row.Key, new SettingValue.Bool(toggle.IsOn)));
                };
                control = toggle;
                break;
            }
            case SettingChange.Steps steps:
            {
                var stepper = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 4 };
                var back = new Button { Content = new FontIcon { Glyph = "", FontSize = 12 }, IsEnabled = steps.Back, Height = 32 };
                var value = new TextBlock { Text = Shown(row.Value), VerticalAlignment = VerticalAlignment.Center, MinWidth = 96, TextAlignment = TextAlignment.Center };
                var forward = new Button { Content = new FontIcon { Glyph = "", FontSize = 12 }, IsEnabled = steps.Forward, Height = 32 };
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(back, $"{title}: previous");
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(forward, $"{title}: next");
                back.Click += (_, _) => ctx.Send(new Command.StepSetting(row.Key, -1));
                forward.Click += (_, _) => ctx.Send(new Command.StepSetting(row.Key, 1));
                stepper.Children.Add(back);
                stepper.Children.Add(value);
                stepper.Children.Add(forward);
                control = stepper;
                break;
            }
            case SettingChange.Typed typed:
            {
                var box = new TextBox { Text = typed.Text, MinWidth = 220, PlaceholderText = "Not set" };
                Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, title);
                void Commit()
                {
                    if (!quiet && box.Text.Trim() != typed.Text)
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
        return CardFrame(title, description, control);
    }

    /// <summary>A value as a row shows it, in sentence case.</summary>
    static string Shown(string value) => value.Length == 0 ? "None" : char.ToUpperInvariant(value[0]) + value[1..];

    void ShowAccounts()
    {
        Cards.Children.Add(Heading("Trackers"));
        foreach (var tracker in new[] { "anilist", "mal", "kitsu" })
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
                description = "Not connected";
                action = new Button { Content = "Connect" };
                action.Click += (_, _) => ctx.Send(new Command.Connect(tracker));
            }
            Cards.Children.Add(CardFrame(name, description, action, ""));
        }
        var sync = new Button { Content = "Sync now" };
        sync.Click += (_, _) => ctx.Send(new Command.SyncNow());
        Cards.Children.Add(Heading("Sync"));
        Cards.Children.Add(CardFrame("Sync with the trackers", "Sends what changed here and fetches what changed elsewhere.", sync, ""));
    }

    void ShowAbout()
    {
        var top = new Grid { ColumnSpacing = 24, Margin = new Thickness(0, 8, 0, 8) };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        top.Children.Add(new Nagare(4));
        var text = new StackPanel { Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = "anistream", Style = Res<Style>("SubtitleTextBlockStyle") });
        var version = typeof(SettingsPage).Assembly.GetName().Version;
        text.Children.Add(new TextBlock { Text = $"Version {version?.ToString(3) ?? "unknown"}", Style = Res<Style>("HazeBodyStyle") });
        text.Children.Add(new TextBlock { Text = "anistream hosts no content.", Style = Res<Style>("HazeBodyStyle") });
        var check = new Button { Content = "Check for updates", Margin = new Thickness(0, 6, 0, 0) };
        check.Click += (_, _) => ctx.Send(new Command.CheckForUpdates());
        text.Children.Add(check);
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
