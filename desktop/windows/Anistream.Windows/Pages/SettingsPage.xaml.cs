using Anistream.App;
using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
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
        [
            // Laid out by ShowTorrents: the switch, the index, what protects them, the last
            // check and what a failed one does.
            ("Torrents", [K.Torrents, K.VpnMode, K.VpnOnFailure]),
            ("Seeding", [K.KeepSeeding, K.SeedRatio, K.SeedHours, K.UploadLimit]),
        ]),
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
    /// <summary>The Last check card while it shows, kept up to date in place: the guard's word and how long ago.</summary>
    SettingsCard? guardCard;
    /// <summary>Protect with while it shows, and until when it says a proxy was saved.</summary>
    SettingsCard? protectCard;
    StackPanel? savedMark;
    DateTimeOffset? savedUntil;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer savedClock;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer guardClock;

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
        // "· 2 min ago" moves on while the Last check card is on screen.
        guardClock = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread().CreateTimer();
        guardClock.Interval = TimeSpan.FromSeconds(30);
        guardClock.Tick += (_, _) => ShowGuard();
        savedClock = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread().CreateTimer();
        savedClock.Interval = TimeSpan.FromSeconds(8);
        savedClock.IsRepeating = false;
        savedClock.Tick += (_, _) => ShowSaved();
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) =>
        {
            ctx.Batch -= Heard;
            guardClock.Stop();
            savedClock.Stop();
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
        else if (batch.Any(e => e is Event.Vpn))
            ShowGuard();
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
        guardCard = null;
        protectCard = null;
        guardClock.Stop();
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
                foreach (var (heading, keys) in pane.Groups)
                {
                    // Only where the engine was built with a presence to publish.
                    if (keys.Contains(K.Presence) && ctx.Known.Settings is { PresenceAvailable: false })
                        continue;
                    if (heading == "Torrents")
                    {
                        ShowTorrents();
                        continue;
                    }
                    // Seeding only says anything once torrents are on.
                    if (heading == "Seeding" && !SettingsCatalog.IsOn(ctx.Known.Setting(K.Torrents)))
                        continue;
                    var rows = keys.Select(k => ctx.Known.Setting(k)).OfType<SettingRow>().ToList();
                    if (rows.Count == 0)
                        continue;
                    Cards.Children.Add(Heading(heading));
                    if (heading == "Seeding")
                        Cards.Children.Add(Note("Seeding runs only while anistream is open."));
                    foreach (var row in rows)
                        Cards.Children.Add(Card(row));
                    if (pane.Name == "downloads" && heading == "Files")
                        Cards.Children.Add(WhenClosingCard());
                }
                State.Done();
#if ANISTREAM_TESTING
                Demo();
#endif
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

    static TextBlock Note(string text)
    {
        var note = Ui.Text(text, "HazeCaptionStyle");
        note.Margin = new Thickness(0, -2, 0, 6);
        return note;
    }

    /// <summary>
    /// Turning torrents on asks how to protect them first, in the dialog that also takes the
    /// index: the switch goes back off unless it is sent.
    /// </summary>
    async void TurnOnTorrents(ToggleSwitch toggle)
    {
        if (await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.TurnOn) is { } saved)
        {
            if (saved == TorrentProtection.Proxy)
                ProxySaved();
            return;
        }
        quiet = true;
        toggle.IsOn = false;
        quiet = false;
    }

    /// <summary>
    /// Torrents: the switch, saying what protects them, and once they are on the index, Protect
    /// with, the last connection check and what a failed one does. The Mac's Sources pane and
    /// Linux's torrents group.
    /// </summary>
    void ShowTorrents()
    {
        if (ctx.Known.Setting(K.Torrents) is not { } row || ctx.Known.Settings is not { } settings)
            return;
        var on = SettingsCatalog.IsOn(row);
        var view = settings.Torrent;
        Cards.Children.Add(Heading("Torrents"));
        if (Card(row) is SettingsCard toggle)
        {
            toggle.Description = on ? TorrentWords.OnNote(view) : TorrentWords.OffNote;
            Cards.Children.Add(toggle);
        }
        if (!on)
            return;

        // The index, sent when the box is left or Enter is pressed.
        var saved = settings.Sources.Index ?? "";
        var index = new TextBox { Text = saved, MinWidth = 280, PlaceholderText = "https://…/?q={query}" };
        var indexTitle = Words.SettingTitle(K.TorrentIndex);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(index, indexTitle);
        void Commit()
        {
            var typed = index.Text.Trim();
            if (!quiet && typed.Length > 0 && typed != saved)
                ctx.Send(new Command.SetSetting(K.TorrentIndex, new SettingValue.Str(typed)));
        }
        index.LostFocus += (_, _) => Commit();
        index.KeyDown += (_, e) =>
        {
            if (e.Key == global::Windows.System.VirtualKey.Enter)
                Commit();
        };
        Cards.Children.Add(CardFrame(indexTitle, TorrentWords.IndexNote, index));

        // Protect with: the device's VPN at once, a proxy through the dialog that asks for it,
        // no protection once confirmed. A choice not made puts the box back.
        var protect = new ComboBox
        {
            MinWidth = 200,
            ItemsSource = TorrentWords.Order.Select(TorrentWords.Label).ToList(),
            SelectedIndex = Array.IndexOf(TorrentWords.Order, view.Protection),
        };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(protect, "Protect with");
        protect.SelectionChanged += async (_, _) =>
        {
            var current = ctx.Known.Settings?.Torrent ?? view;
            var was = Array.IndexOf(TorrentWords.Order, current.Protection);
            if (quiet || protect.SelectedIndex < 0 || protect.SelectedIndex == was)
                return;
            var choice = TorrentWords.Order[protect.SelectedIndex];
            var sent = choice switch
            {
                TorrentProtection.Device => Sent(Torrents.Protect(current, choice)),
                TorrentProtection.Proxy => await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.Protect, start: TorrentProtection.Proxy) is { } chosen
                    && (chosen != TorrentProtection.Proxy || ProxySaved()),
                _ => await Ui.Confirm(XamlRoot, TorrentWords.NoneTitle, TorrentWords.NoneMessage, TorrentWords.NoneAction(turningOn: false))
                    && Sent(Torrents.Protect(current, choice, acknowledged: true)),
            };
            if (!sent)
                PutBack(protect, was);
        };
        // Edit proxy… beside the choice once a proxy is saved: its form, filled in but for the
        // password, saved only once the new details pass the check.
        var protectRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        savedMark = null;
        if (view is { Protection: TorrentProtection.Proxy, Proxy: not null })
        {
            // Proxy saved, in the row and faded in place: its room is kept, so nothing moves.
            savedMark = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center, Margin = new Thickness(0, 0, 8, 0), Opacity = 0 };
            savedMark.Children.Add(Ui.Dot(Colours.Water));
            savedMark.Children.Add(new TextBlock { Text = TorrentWords.ProxySaved, VerticalAlignment = VerticalAlignment.Center });
            Motion.QuickOpacity(savedMark);
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(savedMark, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
            protectRow.Children.Add(savedMark);
            var edit = new Button { Content = "Edit proxy…", VerticalAlignment = VerticalAlignment.Center };
            edit.Click += async (_, _) =>
            {
                if (await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.EditProxy) is not null)
                    ProxySaved();
            };
            protectRow.Children.Add(edit);
        }
        protectRow.Children.Add(protect);
        protectCard = CardFrame("Protect with", null, protectRow);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetLiveSetting(protectCard, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        Cards.Children.Add(protectCard);
        ShowSaved();

        // The last check: where traffic left through and when, and how it went.
        guardCard = CardFrame("Last check", null, null);
        Cards.Children.Add(guardCard);
        ShowGuard();
        guardClock.Start();

        // If a check fails: pause or stop at once, Warn only once confirmed.
        var failed = new ComboBox
        {
            MinWidth = 200,
            ItemsSource = TorrentWords.FailedChecks.Select(TorrentWords.Label).ToList(),
            SelectedIndex = Array.IndexOf(TorrentWords.FailedChecks, view.OnFailure),
        };
        var failedTitle = Words.SettingTitle(K.VpnOnFailure);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(failed, failedTitle);
        failed.SelectionChanged += async (_, _) =>
        {
            var current = ctx.Known.Settings?.Torrent ?? view;
            var was = Array.IndexOf(TorrentWords.FailedChecks, current.OnFailure);
            if (quiet || failed.SelectedIndex < 0 || failed.SelectedIndex == was)
                return;
            var check = TorrentWords.FailedChecks[failed.SelectedIndex];
            if (check == FailedCheck.Warn
                && !await Ui.Confirm(XamlRoot, TorrentWords.WarnTitle, TorrentWords.WarnMessage, "Warn only"))
            {
                PutBack(failed, was);
                return;
            }
            if (Torrents.FailedCheckCommand(current, check) is { } command)
                ctx.Send(command);
        };
        Cards.Children.Add(CardFrame(failedTitle, TorrentWords.FailedNote, failed));
    }

    /// <summary>
    /// A proxy saved once its check passed: said beside Protect with for a few seconds, and kept
    /// through the redraw the saved settings bring.
    /// </summary>
    bool ProxySaved()
    {
        savedUntil = DateTimeOffset.UtcNow.AddSeconds(8);
        savedClock.Stop();
        savedClock.Start();
        ShowSaved();
        AnnounceSaved();
        return true;
    }

    /// <summary>Narrator says it on the card the saved settings' redraw leaves, once it has.</summary>
    async void AnnounceSaved()
    {
        await Task.Delay(700);
        if (protectCard is { } card && savedUntil is not null)
            Microsoft.UI.Xaml.Automation.Peers.FrameworkElementAutomationPeer.CreatePeerForElement(card)
                ?.RaiseAutomationEvent(Microsoft.UI.Xaml.Automation.Peers.AutomationEvents.LiveRegionChanged);
    }

    /// <summary>Protect with's Proxy saved: shown while it stands, faded out otherwise, in the same place.</summary>
    void ShowSaved()
    {
        if (protectCard is null)
            return;
        var showing = savedUntil is { } until && DateTimeOffset.UtcNow < until;
        if (!showing)
            savedUntil = null;
        if (savedMark is not null)
            savedMark.Opacity = showing ? 1 : 0;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(protectCard, showing ? $"Protect with, {TorrentWords.ProxySaved}" : "Protect with");
    }

    /// <summary>A drop-down back on what is set, for a choice not made.</summary>
    void PutBack(ComboBox box, int at)
    {
        quiet = true;
        box.SelectedIndex = at;
        quiet = false;
    }

#if ANISTREAM_TESTING
    bool demoed;

    /// <summary>
    /// A torrent state to bring up on Sources for a screenshot, on the testing build:
    /// <c>--show=torrent</c> (the turn-on dialog), <c>torrent-proxy</c> (it with a proxy typed),
    /// <c>torrent-none</c> (confirming no protection), <c>torrents-on</c> (turned on through the
    /// device's VPN, with the fixture's made-up exit), <c>torrents-warn</c> (then confirming
    /// Warn only), <c>torrents-checking</c> and <c>torrents-failed</c> (Last check while it checks,
    /// and failed with Try again), <c>torrent-protect</c> (the proxy dialog once torrents are on),
    /// and on a saved proxy <c>torrent-edit</c> (Edit proxy filled in), <c>torrent-edit-checking</c>
    /// and <c>torrent-edit-failed</c> (a new proxy's check under way, and failed with nothing
    /// saved), <c>torrent-edit-saved</c> (passed, and Proxy saved beside Protect with) and
    /// <c>torrent-edit-protect</c> (the proxy dialog with a proxy saved).
    /// </summary>
    async void Demo()
    {
        if (demoed || pane.Name != "sources" || ctx.Options.Show is not { } show || !show.StartsWith("torrent", StringComparison.Ordinal))
            return;
        demoed = true;
        await Task.Delay(800);
        const string index = "https://torrents.example/rss?q={query}";
        var draft = TorrentDraft.From(ctx.Known.Settings, turningOn: true) with { Index = index };
        switch (show)
        {
            case "torrent":
                await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.TurnOn, from: draft with { Index = "" });
                break;
            case "torrent-proxy":
                await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.TurnOn,
                    from: draft with { Protection = TorrentProtection.Proxy, Host = "proxy.example", Port = "1080", User = "kai" });
                break;
            case "torrent-none":
                await Ui.Confirm(XamlRoot, TorrentWords.NoneTitle, TorrentWords.NoneMessage, TorrentWords.NoneAction(turningOn: true));
                break;
            case "torrents-on" or "torrents-warn" or "torrents-checking" or "torrents-failed" or "torrent-protect"
                or "torrent-edit" or "torrent-edit-checking" or "torrent-edit-failed" or "torrent-edit-saved" or "torrent-edit-protect":
            {
                // On a saved proxy for Edit proxy, the device's VPN otherwise.
                var on = show.StartsWith("torrent-edit", StringComparison.Ordinal)
                    ? draft with { Protection = TorrentProtection.Proxy, Host = "proxy.example", Port = "1080", User = "kai" }
                    : draft with { Protection = TorrentProtection.Device };
                foreach (var command in Torrents.Apply(on, ctx.Known.Settings, turningOn: true))
                    ctx.Send(command);
                await Task.Delay(2500);
                switch (show)
                {
                    case "torrents-warn":
                        await Ui.Confirm(XamlRoot, TorrentWords.WarnTitle, TorrentWords.WarnMessage, "Warn only");
                        break;
                    case "torrent-protect":
                        await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.Protect, start: TorrentProtection.Proxy);
                        break;
                    case "torrent-edit":
                        await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.EditProxy);
                        break;
                    case "torrent-edit-protect":
                        await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.Protect, start: TorrentProtection.Proxy);
                        break;
                    case "torrent-edit-checking" or "torrent-edit-failed" or "torrent-edit-saved":
                    {
                        // New details typed, then Save: held, answered as failed, or the fixture's pass.
                        var typed = TorrentDraft.From(ctx.Known.Settings, turningOn: false, TorrentProtection.Proxy)
                            with { Host = "relay.example", Port = "1081" };
                        if (await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.EditProxy, from: typed, shown: d => d.PressSave()) is not null)
                            ProxySaved();
                        break;
                    }
                    case "torrents-checking" or "torrents-failed":
                    {
                        // The check's word as the engine would say it, after the fixture's own pass.
                        await Task.Delay(1500);
                        Event said = new Event.Vpn(show == "torrents-checking" ? new GuardView.Checking() : new GuardView.Unreachable());
                        ctx.Known.Take(said);
                        ctx.Heard([said]);
                        break;
                    }
                }
                break;
            }
        }
    }
#endif

    bool Sent(Command command)
    {
        ctx.Send(command);
        return true;
    }

    /// <summary>
    /// The Last check card as the guard last said: the line under it, a dot and its word, and
    /// Try again after a failed check. Water for passed, vermilion for failed, haze otherwise.
    /// </summary>
    void ShowGuard()
    {
        if (guardCard is null)
            return;
        var guard = ctx.Known.Guard;
        var ago = guard is { } g ? (long)(DateTimeOffset.UtcNow - g.At).TotalSeconds : (long?)null;
        var (note, word, isFailed) = TorrentWords.Guard(guard?.View, ago);
        guardCard.Description = note;
        var verdict = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        var colour = isFailed ? Ui.Vermilion : guard?.View is GuardView.Protected ? Colours.Water : Colours.Haze;
        verdict.Children.Add(Ui.Dot(colour));
        verdict.Children.Add(new TextBlock { Text = word, VerticalAlignment = VerticalAlignment.Center });
        if (isFailed)
        {
            var again = new Button { Content = "Try again", Margin = new Thickness(8, 0, 0, 0) };
            again.Click += (_, _) => ctx.Send(new Command.CheckConnection());
            verdict.Children.Add(again);
        }
        guardCard.Content = verdict;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(guardCard, $"Last check, {word}, {note}");
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
                        TurnOnTorrents(toggle);
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
