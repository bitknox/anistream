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
    /// <summary>A pane: its name (what navigation asks for), its icon, and its groups by id, each with its settings.</summary>
    sealed record Pane(string Name, string Glyph, (string Group, K[] Keys)[] Groups)
    {
        /// <summary>The pane's name as the list shows it.</summary>
        public string Title => Name switch
        {
            "playback" => T.SettingsGroupPlayback,
            "downloads" => T.NavDownloads,
            "notifications" => T.SettingsGroupNotifications,
            "sources" => T.SourcesTitle,
            "accounts" => T.NavAccounts,
            "integrations" => T.SettingsGroupIntegrations,
            "advanced" => T.SettingsGroupAdvanced,
            _ => T.AboutTitle,
        };
    }

    static readonly Pane[] PaneList =
    [
        new("playback", "",
        [
            // The app's language and how titles read everywhere, not only while watching, so they
            // head the pane on their own.
            ("language", [K.Titles, K.Trailers]),
            ("watching", [K.Translation, K.Quality, K.Subtitles, K.CommitThreshold]),
            ("player", [K.AutoNext, K.UpNextCountdown, K.SkipOpening, K.SkipFiller, K.MarkChapters, K.SkipButton, K.Fullscreen]),
            ("picture", [K.Upscaling, K.NightMode, K.SpoilerProtection, K.Motion]),
        ]),
        new("downloads", "",
        [
            ("files", [K.DownloadDir, K.DownloadQuality, K.MergeSubtitles, K.DownloadHook]),
            ("ahead", [K.KeepNextEpisodes]),
        ]),
        new("notifications", "", [("new-episodes", [K.EpisodeNotifications])]),
        new("sources", "",
        [
            // Laid out by ShowTorrents: the switch, the index, what protects them, the last
            // check and what a failed one does.
            ("torrents", [K.Torrents, K.VpnMode, K.VpnOnFailure]),
            ("seeding", [K.KeepSeeding, K.SeedRatio, K.SeedHours, K.UploadLimit]),
        ]),
        new("accounts", "", []),
        new("integrations", "",
        [
            ("discord", [K.Presence, K.PresenceTitle, K.PresenceRich]),
            ("discussions", [K.EpisodeDiscussions, K.CommentImages]),
            ("updates", [K.UpdateCheck]),
        ]),
        new("advanced", "", [("mpv", [K.MpvOptions])]),
        new("about", "", []),
    ];

    /// <summary>A group's heading, from its id.</summary>
    static string GroupHeading(string group) => group switch
    {
        "language" => T.SettingsLanguage,
        "watching" => T.SettingsGroupWatching,
        "player" => T.SettingsGroupPlayer,
        "picture" => T.SettingsGroupPicture,
        "files" => T.SettingsGroupFiles,
        "ahead" => T.SettingsGroupAhead,
        "new-episodes" => T.SettingsNewEpisodes,
        "torrents" => T.TorrentsTitle,
        "seeding" => T.DownloadsSeedingTitle,
        "discord" => T.SettingsDiscord,
        "discussions" => T.DiscussionSettingsGroup,
        "updates" => T.UpdatesTitle,
        _ => T.SettingsGroupMpv,
    };

    /// <summary>A setting's row title, from the catalogue.</summary>
    static string Title(K key) => key switch
    {
        K.Theme => T.SettingsTheme,
        K.Motion => T.SettingsMotion,
        K.Titles => T.SettingsTitleLanguage,
        K.Trailers => T.SettingsLabelTrailers,
        K.Translation => T.SettingsAudio,
        K.Quality => T.SettingsQuality,
        K.Subtitles => T.PlayerSubtitles,
        K.CommitThreshold => T.SettingsCountsWatched,
        K.AutoNext => T.SettingsAutoPlay,
        K.SkipOpening => T.SettingsSkipOpening,
        K.SkipFiller => T.SettingsSkipFiller,
        K.MarkChapters => T.SettingsMarkOpening,
        K.SkipButton => T.SettingsSkipButton,
        K.Fullscreen => T.SettingsStartFullscreen,
        K.Upscaling => T.PlayerUpscaling,
        K.PlayerUi => T.SettingsPlayerInterface,
        K.NightMode => T.PlayerNightMode,
        K.DownloadDir => T.DownloadsFolder,
        K.MergeSubtitles => T.SettingsMergeSubtitles,
        K.KeepSeeding => T.SettingsKeepSeeding,
        K.DownloadHook => T.SettingsAfterDownload,
        K.Presence => T.SettingsDiscordPresence,
        K.PresenceTitle => T.SettingsPresenceTitle,
        K.PresenceRich => T.SettingsPresenceRich,
        K.Torrents => T.SettingsTorrentSource,
        K.TorrentIndex => T.TorrentsIndex,
        K.VpnMode => T.SettingsVpnMode,
        K.SeedRatio => T.SettingsSeedAfter,
        K.SeedHours => T.SettingsSeedFor,
        K.UploadLimit => T.SettingsUploadLimit,
        K.TokenStorage => T.SettingsTokenStorage,
        K.Syncplay => T.SettingsWatchParties,
        K.SyncplayServer => T.PartyServer,
        K.SyncplayRoom => T.PartyRoom,
        K.UpdateCheck => T.SettingsCheckUpdates,
        K.SourceOrder => T.SettingsSourceOrder,
        K.UpNextCountdown => T.SettingsUpNextCountdown,
        K.SpoilerProtection => T.SettingsSpoiler,
        K.DownloadQuality => T.DownloadsQuality,
        K.KeepNextEpisodes => T.SettingsKeepNext,
        K.DownloadsOnCellular => T.SettingsDownloadsCellular,
        K.TorrentsOnCellular => T.SettingsTorrentsCellular,
        K.VpnOnFailure => T.TorrentsIfCheckFails,
        K.AdultTitles => T.SettingsAdult,
        K.EpisodeNotifications => T.SettingsEpisodeNotifications,
        K.EpisodeDiscussions => T.SettingsEpisodeDiscussions,
        K.CommentImages => T.SettingsCommentImages,
        K.CastTvs => T.SettingsLabelCastTvs,
        K.TorrentsWhileCasting => T.SettingsLabelTorrentsWhileCasting,
        _ => T.SettingsMpvOptions,
    };

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
        State.ActivityText = T.SettingsLoading;
        // "· 2 min ago" moves on while the Last check card is on screen.
        guardClock = Timers.Weak(this, TimeSpan.FromSeconds(30), true, static page => page.ShowGuard());
        savedClock = Timers.Weak(this, TimeSpan.FromSeconds(8), false, static page => page.ShowSaved());
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Tv.Changed += TvChanged;
        ctx.Hear(this, Heard, () =>
        {
            ctx.Tv.Changed -= TvChanged;
            guardClock.Stop();
            savedClock.Stop();
        });
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
        Crumbs.ItemsSource = picking ? new[] { T.SettingsTitle } : new[] { T.SettingsTitle, pane.Title };
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
                tvGroup = null;
                foreach (var (group, keys) in pane.Groups)
                {
                    // Play on TV's paired TVs, between Discord and Updates.
                    if (pane.Name == "integrations" && group == "updates" && ctx.Tv.Available)
                    {
                        Cards.Children.Add(Heading(T.TvPlayOn));
                        tvGroup = new StackPanel { Spacing = 4 };
                        ShowTvs(tvGroup);
                        Cards.Children.Add(tvGroup);
                        if (ctx.Known.Setting(K.CastTvs) is { } castTvs)
                            Cards.Children.Add(castCard = Card(castTvs));
                    }
                    // Only where the engine was built with a presence to publish.
                    if (keys.Contains(K.Presence) && ctx.Known.Settings is { PresenceAvailable: false })
                        continue;
                    if (group == "torrents")
                    {
                        ShowTorrents();
                        continue;
                    }
                    // Seeding only says anything once torrents are on.
                    if (group == "seeding" && !SettingsCatalog.IsOn(ctx.Known.Setting(K.Torrents)))
                        continue;
                    var rows = keys.Select(k => ctx.Known.Setting(k)).OfType<SettingRow>().ToList();
                    if (rows.Count == 0)
                        continue;
                    Cards.Children.Add(Heading(GroupHeading(group)));
                    if (group == "seeding")
                        Cards.Children.Add(Note(T.SettingsSeedingOpenOnly));
                    if (group == "language")
                        Cards.Children.Add(LanguageCard());
                    foreach (var row in rows)
                        Cards.Children.Add(Card(row));
                    if (pane.Name == "downloads" && group == "files")
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
            var forget = new Button { Content = T.ActionForget };
            AutomationProperties.SetName(forget, T.TvForgetLabel(paired.Name));
            forget.Click += async (_, _) =>
            {
                if (XamlRoot is { } root && await Ui.Confirm(root, T.TvForgetTitle(paired.Name), T.TvForgetDetail, T.ActionForget))
                    await tv.Forget(paired.Fingerprint);
            };
            group.Children.Add(CardFrame(paired.Name, note, forget, "\uE7F4"));
        }
        var pair = new Button { Content = T.TvPairMenu };
        pair.Click += (_, _) => tv.OpenPicker();
        group.Children.Add(CardFrame(tv.Paired.Length == 0 ? T.TvNonePaired : T.SettingsAnotherTv, TvWords.PairNote, pair, tv.Paired.Length == 0 ? "\uE7F4" : "\uE710"));
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
        var choices = new[] { (CloseChoice.Ask, T.DownloadsBackgroundAsk), (CloseChoice.KeepGoing, T.DownloadsBackgroundKeep), (CloseChoice.Stop, T.DownloadsBackgroundStop) };
        var box = new ComboBox { MinWidth = 200, ItemsSource = choices.Select(c => c.Item2).ToList() };
        box.SelectedIndex = Array.FindIndex(choices, c => c.Item1 == ctx.Window.WhenClosing);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(box, T.DownloadsBackgroundTitle);
        box.SelectionChanged += (_, _) =>
        {
            if (!quiet && box.SelectedIndex >= 0)
                ctx.Window.WhenClosing = choices[box.SelectedIndex].Item1;
        };
        return CardFrame(T.DownloadsBackgroundTitle, T.SettingsClosingDetail, box);
    }

    /// <summary>
    /// The app's own language: the system's by default, or one this build ships, a draft one
    /// labelled. Kept in the app's language override and read at start, so a change says that a
    /// restart applies it.
    /// </summary>
    SettingsCard LanguageCard()
    {
        static string Name((string Tag, string Name, bool Draft) l) => l.Draft ? $"{l.Name} · {T.LanguageDraftLabel}" : l.Name;
        var languages = T.Languages.ToList();
#if ANISTREAM_TESTING
        languages.Add(("qps-ploc", "Pseudo", true));
#endif
        var names = new List<string> { T.LanguageFollowSystem };
        names.AddRange(languages.Select(Name));
        var box = new ComboBox { MinWidth = 200, ItemsSource = names };
        var at = languages.FindIndex(l => l.Tag == StringResources.Chosen);
        box.SelectedIndex = at + 1;
        AutomationProperties.SetName(box, T.SettingsLanguageApp);
        string? Description()
        {
            var parts = new List<string>();
            if (languages.FirstOrDefault(l => l.Tag == StringResources.Chosen) is { Draft: true })
                parts.Add(T.LanguageDraftNote);
            if (StringResources.Chosen != StringResources.AtStart)
                parts.Add(T.LanguageRestartNote);
            return parts.Count == 0 ? null : string.Join(" ", parts);
        }
        var card = CardFrame(T.SettingsLanguageApp, Description(), box, "\uF2B7");
        box.SelectionChanged += (_, _) =>
        {
            if (quiet || box.SelectedIndex < 0)
                return;
            StringResources.Choose(box.SelectedIndex == 0 ? null : languages[box.SelectedIndex - 1].Tag);
            card.Description = Description();
        };
        return card;
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
        Cards.Children.Add(Heading(T.TorrentsTitle));
        if (Card(row) is SettingsCard toggle)
        {
            toggle.Description = on ? TorrentWords.OnNote(view) : TorrentWords.OffNote;
            Cards.Children.Add(toggle);
        }
        if (!on)
            return;

        // The index, sent when the box is left or Enter is pressed.
        var saved = settings.Sources.Index ?? "";
        var index = new TextBox { Text = saved, MinWidth = 280, PlaceholderText = T.TorrentsIndexHint };
        var indexTitle = Title(K.TorrentIndex);
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
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(protect, T.TorrentsProtectWith);
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
            var edit = new Button { Content = T.SettingsEditProxy, VerticalAlignment = VerticalAlignment.Center };
            edit.Click += async (_, _) =>
            {
                if (await TorrentDialog.Ask(ctx, XamlRoot, TorrentDialog.Purpose.EditProxy) is not null)
                    ProxySaved();
            };
            protectRow.Children.Add(edit);
        }
        protectRow.Children.Add(protect);
        protectCard = CardFrame(T.TorrentsProtectWith, null, protectRow);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetLiveSetting(protectCard, Microsoft.UI.Xaml.Automation.Peers.AutomationLiveSetting.Polite);
        Cards.Children.Add(protectCard);
        ShowSaved();

        // The last check: where traffic left through and when, and how it went.
        guardCard = CardFrame(T.TorrentsLastCheck, null, null);
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
        var failedTitle = Title(K.VpnOnFailure);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(failed, failedTitle);
        failed.SelectionChanged += async (_, _) =>
        {
            var current = ctx.Known.Settings?.Torrent ?? view;
            var was = Array.IndexOf(TorrentWords.FailedChecks, current.OnFailure);
            if (quiet || failed.SelectedIndex < 0 || failed.SelectedIndex == was)
                return;
            var check = TorrentWords.FailedChecks[failed.SelectedIndex];
            if (check == FailedCheck.Warn
                && !await Ui.Confirm(XamlRoot, TorrentWords.WarnTitle, TorrentWords.WarnMessage, T.TorrentsFailWarn))
            {
                PutBack(failed, was);
                return;
            }
            if (Torrents.FailedCheckCommand(current, check) is { } command)
                ctx.Send(command);
        };
        Cards.Children.Add(CardFrame(failedTitle, TorrentWords.FailedNote, failed));

        // Whether a cast plays torrents, which this PC then serves to the TV.
        if (ctx.Known.Setting(K.TorrentsWhileCasting) is { } casting)
            Cards.Children.Add(Card(casting));
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
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(protectCard, showing ? T.SettingsCardState(T.TorrentsProtectWith, TorrentWords.ProxySaved) : T.TorrentsProtectWith);
    }

    /// <summary>A drop-down back on what is set, for a choice not made.</summary>
    void PutBack(ComboBox box, int at)
    {
        quiet = true;
        box.SelectedIndex = at;
        quiet = false;
    }

    // The Chromecast switch under Play on TV, for the testing build to scroll to.
    FrameworkElement? castCard;

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
        // --show=settings-cast-tvs: Integrations scrolled to Play on TV and its Chromecast switch.
        if (!demoed && pane.Name == "integrations" && ctx.Options.Show == "settings-cast-tvs" && castCard is { } cast)
        {
            demoed = true;
            await Task.Delay(800);
            cast.StartBringIntoView(new BringIntoViewOptions { VerticalAlignmentRatio = 0.6, AnimationDesired = false });
            return;
        }
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
                        await Ui.Confirm(XamlRoot, TorrentWords.WarnTitle, TorrentWords.WarnMessage, T.TorrentsFailWarn);
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
            var again = new Button { Content = T.ActionTryAgain, Margin = new Thickness(8, 0, 0, 0) };
            again.Click += (_, _) => ctx.Send(new Command.CheckConnection());
            verdict.Children.Add(again);
        }
        guardCard.Content = verdict;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(guardCard, T.SettingsCardCheck(T.TorrentsLastCheck, word, note));
    }

    FrameworkElement Card(SettingRow row)
    {
        var title = Title(row.Key);
        var overridden = ctx.Known.Settings?.Overridden?.FirstOrDefault(o => o.Key == row.Key);
        string? description = overridden is { Shows: > 0 } o ? T.SettingsOverridden((long)o.Shows) : null;
        if (row.Key == K.Trailers)
            description = T.SettingsNoteTrailersWindows;
        if (row.Key == K.PresenceRich)
            description = T.SettingsPresenceRichDetail(AnistreamFfiMethods.TrackerName("anilist"));
        if (row.Key == K.CastTvs)
            description = T.SettingsNoteCastTvsPc;
        if (row.Key == K.TorrentsWhileCasting)
            description = T.SettingsNoteTorrentsWhileCastingPc;
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
                var box = new ComboBox { MinWidth = 200, ItemsSource = options.Select(o => o.Shown).ToList() };
                var at = SettingsCatalog.Current(row) is { } current ? options.ToList().IndexOf(current) : -1;
                if (at < 0)
                    box.PlaceholderText = SettingsCatalog.Shown(row.Key, row.Value);
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
                var box = new TextBox { Text = text, MinWidth = 220, PlaceholderText = T.SettingsNotSet };
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
                control = new TextBlock { Text = SettingsCatalog.Shown(row.Key, row.Value), Style = Res<Style>("HazeBodyStyle") };
                break;
        }
        var card = CardFrame(title, description, control);
        // A row whose switch is off waits, greyed, as Windows Settings shows a dependent row.
        card.IsEnabled = SettingsCatalog.Needs(row.Key).All(k => SettingsCatalog.IsOn(ctx.Known.Setting(k)));
        return card;
    }

    void ShowAccounts()
    {
        Cards.Children.Add(Heading(T.SettingsTrackers));
        foreach (var tracker in new[] { "anilist", "mal", "kitsu", "simkl", "trakt" })
        {
            var name = AnistreamFfiMethods.TrackerName(tracker);
            var state = ctx.Known.Sync.GetValueOrDefault(tracker);
            Button action;
            string description;
            if (state is { Connected: true })
            {
                var waiting = (long)state.Outbox;
                description = (state.User, waiting) switch
                {
                    ({ } user, > 0) => T.AccountsStateSignedInWaiting(name, user, waiting),
                    ({ } user, _) => T.AccountsStateSignedIn(name, user),
                    (null, > 0) => T.SettingsSignedInWaiting(waiting),
                    _ => T.SettingsSignedIn,
                };
                action = new Button { Content = T.AccountsSignOut };
                action.Click += (_, _) => ctx.Send(new Command.Disconnect(tracker));
            }
            else
            {
                description = tracker is "simkl" or "trakt" ? T.SettingsSignedOutCode : T.AccountsStateSignedOut;
                action = new Button { Content = T.AccountsSignIn };
                // Kitsu takes a user name and password here; the others sign in in the browser or with a code.
                action.Click += (_, _) =>
                {
                    if (tracker == "kitsu")
                        KitsuDialog.Show(ctx, XamlRoot);
                    else
                        ctx.Send(new Command.Connect(tracker));
                };
            }
            Cards.Children.Add(CardFrame(name, description, action));
        }
        var sync = new Button { Content = T.AccountsSyncNow };
        sync.Click += (_, _) => ctx.Send(new Command.SyncNow());
        Cards.Children.Add(Heading(T.SettingsSync));
        Cards.Children.Add(CardFrame(T.AccountsSyncAll, T.SettingsSyncDetail, sync));
    }

    void ShowAbout()
    {
        var top = new Grid { ColumnSpacing = 24, Margin = new Thickness(0, 8, 0, 8) };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        top.Children.Add(new Nagare(6));
        var text = new StackPanel { Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = T.AppName, Style = Res<Style>("SubtitleTextBlockStyle") });
        text.Children.Add(new TextBlock { Text = T.AboutVersion(Updater.Current), Style = Res<Style>("HazeBodyStyle") });
        text.Children.Add(new TextBlock { Text = T.LegalHostsNoContent, Style = Res<Style>("HazeBodyStyle") });
        text.Children.Add(new UpdatePanel(ctx, ctx.Updater) { Margin = new Thickness(0, 6, 0, 0) });
        Grid.SetColumn(text, 1);
        top.Children.Add(text);
        Cards.Children.Add(top);
        Cards.Children.Add(Heading(T.SettingsNotices));
        // The typeface's and the library's names are their own, and stay as they are.
        foreach (var (name, line) in new[]
        {
            (T.AppName, T.AboutLicence),
            ("Zen Kaku Gothic New", T.AboutFontLicence),
            ("libmpv", T.SettingsLibmpvLicence),
            (T.SettingsArtwork, T.SettingsArtworkDetail),
        })
            Cards.Children.Add(CardFrame(name, line, null));
    }
}
