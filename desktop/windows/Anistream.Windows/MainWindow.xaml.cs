using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Anistream.Windows.Pages;
using Anistream.Windows.Player;
using Microsoft.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Input;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;
using Windows.Graphics;
using VirtualKey = Windows.System.VirtualKey;
using VirtualKeyModifiers = Windows.System.VirtualKeyModifiers;

namespace Anistream.Windows;

/// <summary>
/// The one window: the title bar with search, the navigation pane, the page, and the player over
/// them. It starts the engine (or, in the testing build, one of its fixture scenes on a scratch
/// root), the player's core, and the loop that brings the engine's events to the pages.
/// </summary>
public sealed partial class MainWindow : Window
{
    static readonly Dictionary<string, Type> Destinations = new()
    {
        ["continue"] = typeof(ContinuePage),
        ["calendar"] = typeof(CalendarPage),
        ["seasonal"] = typeof(SeasonalPage),
        ["library"] = typeof(LibraryPage),
        ["downloads"] = typeof(DownloadsPage),
        ["providers"] = typeof(ProvidersPage),
        ["stores"] = typeof(StoresPage),
        ["search"] = typeof(SearchPage),
        ["settings"] = typeof(SettingsPage),
    };

    readonly Options options;
    readonly DispatcherQueue ui;
    readonly EngineSession session;
    readonly FfiPlayer? player;
    readonly FfiDirs dirs;
    readonly Ctx ctx;
    readonly NetworkWatch network;
    readonly Taskbar? taskbar;
    readonly MediaControls? media;
    Tray? tray;
    bool selecting, played, closing, askedClose, firstRun;

    public MainWindow(Options options)
    {
        this.options = options;
        InitializeComponent();
        ui = DispatcherQueue.GetForCurrentThread();

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        AppWindow.TitleBar.ButtonBackgroundColor = Colors.Transparent;
        AppWindow.TitleBar.ButtonInactiveBackgroundColor = Colors.Transparent;
        AppWindow.TitleBar.ButtonForegroundColor = Colours.Silk;
        AppWindow.TitleBar.ButtonHoverBackgroundColor = global::Windows.UI.Color.FromArgb(0x18, 0xFF, 0xFF, 0xFF);
        AppWindow.TitleBar.ButtonPressedBackgroundColor = global::Windows.UI.Color.FromArgb(0x10, 0xFF, 0xFF, 0xFF);
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "anistream.ico"));
        SystemBackdrop = new TintedMica();

        // The engine's own notices (the launch digest): in the window while it is in front,
        // else as a Windows notification.
        var host = new AppHost(ui, options.Fixture) { Notified = (title, body) => Notified(title, body) };
        (var engine, dirs) = StartEngine(host);
        Size();
        session = new EngineSession(engine, act => ui.TryEnqueue(() => act()));
        var art = new Art(session.Send);
        ctx = new Ctx { Session = session, Art = art, Window = this, Options = options };
        Ctx.Current = ctx;
        session.Batch += batch =>
        {
            art.Take(batch);
            Player.Heard(batch);
            media?.Heard(batch);
            Heard(batch);
            ctx.Heard(batch);
        };
        session.Stopped += () => { if (!closing) Close(); };

        player = StartPlayer(engine);
        Player.Attach(ctx, player);
        Player.FullscreenRequested += Fullscreen;
        Player.FullscreenOn = () => AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen;
        Player.ShowingChanged += PlayerShowing;
        Player.ControlsChanged += shown =>
        {
            if (Player.Showing)
                AppWindow.TitleBar.PreferredHeightOption = shown ? TitleBarHeightOption.Tall : TitleBarHeightOption.Collapsed;
        };
        Player.MiniRequested += Mini;
        session.OffThread = e =>
        {
#if ANISTREAM_TESTING
            if (options.Showcase && Showcase.Holds(e))
                return true;
#endif
            if (e is not Event.PlayerOps ops)
                return false;
            Player.NoteLoad(ops.Ops);
            player?.Apply(ops.PlaybackValue, ops.Ops);
            return true;
        };
        session.Start();
        // The network path, said to the engine now and on every change; it answers with one
        // online or offline state (Event.Connectivity) the pages show.
        network = new NetworkWatch(c => ctx.Send(c), ui, options.Offline);
        taskbar = Taskbar.For(WinRT.Interop.WindowNative.GetWindowHandle(this));
        // The media overlay, the media keys and the thumbnail's buttons, while an episode plays.
        media = MediaControls.Start(WinRT.Interop.WindowNative.GetWindowHandle(this), taskbar, art, ui, c => ctx.Send(new Command.Player(c)));
        StartNotifications(art);
        Keys();
#if ANISTREAM_TESTING
        if (options.Showcase)
        {
            // After the first batch, so the pages that read the engine's entries have them.
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1500);
                var events = Showcase.Events();
                foreach (var e in events)
                    session.Known.Take(e);
                ctx.Heard(events);
            });
        }
#endif

        AppWindow.Closing += OnClosing;
        AppWindow.Changed += (_, e) =>
        {
            if (e.DidPositionChange || e.DidSizeChange || e.DidPresenterChange)
                RememberPlace();
        };
        Closed += (_, _) => Shutdown();
        Root.Loaded += (_, _) => FirstPage();
    }

    /// <summary>
    /// The window where it was last left (size, position and maximised), when that still lies on
    /// a display; otherwise 1280 by 800, centred. The minimum is 500 by 420.
    /// </summary>
    void Size()
    {
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        if (AppWindow.Presenter is OverlappedPresenter overlapped)
        {
            overlapped.PreferredMinimumWidth = (int)(500 * scale);
            overlapped.PreferredMinimumHeight = (int)(420 * scale);
        }
        if (options.Width is null && Place.Read(PlacePath) is { } place && Place.OnADisplay(place.Frame))
        {
            AppWindow.MoveAndResize(place.Frame);
            if (place.Maximised && AppWindow.Presenter is OverlappedPresenter p)
                p.Maximize();
            placed = true;
            return;
        }
        var width = options.Width is { } w ? (int)(w * scale) : Math.Min((int)(1280 * scale), area.Width);
        var height = options.Height is { } h ? (int)(h * scale) : Math.Min((int)(800 * scale), area.Height);
        AppWindow.MoveAndResize(new RectInt32(
            area.X + Math.Max(0, (area.Width - width) / 2), area.Y + Math.Max(0, (area.Height - height) / 2), width, height));
        placed = true;
    }

    bool placed;

    string PlacePath => Path.Combine(dirs.Config, "windows-window.json");

    /// <summary>Keeps the window's frame for the next launch: the normal frame, and whether it was maximised.</summary>
    void RememberPlace()
    {
        if (!placed || options.Width is not null || AppWindow.Presenter is not OverlappedPresenter overlapped || !AppWindow.IsVisible)
            return;
        var maximised = overlapped.State == OverlappedPresenterState.Maximized;
        if (overlapped.State == OverlappedPresenterState.Minimized)
            return;
        var frame = maximised && Place.Read(PlacePath) is { } was
            ? was.Frame
            : new RectInt32(AppWindow.Position.X, AppWindow.Position.Y, AppWindow.Size.Width, AppWindow.Size.Height);
        Place.Write(PlacePath, new Place(frame, maximised));
    }

    [System.Runtime.InteropServices.LibraryImport("user32.dll")]
    private static partial uint GetDpiForWindow(nint hwnd);

    (FfiEngine, FfiDirs) StartEngine(FfiHost host)
    {
        var start = EngineSession.AppStart();
#if ANISTREAM_TESTING
        if (options.Scene is { } name)
        {
            var root = AppDirs.NewScratchRoot("app");
            var scratch = AppDirs.Under(root);
            AnistreamFfiMethods.StartLog(scratch.Cache);
            if (options.Downloads && options.Media is { } clip)
                AnistreamFfiMethods.SeedFixtureDownloads(scratch, clip);
            FfiScene scene = name switch
            {
                "fresh" => new FfiScene.Fresh(),
                "sourceless" => new FfiScene.Sourceless(),
                "playing" => new FfiScene.Playing(options.Media ?? @"C:\setup\sdr-testsrc.mp4"),
                _ => new FfiScene.Watching(),
            };
            return (FfiEngine.StartScene(scratch, host, scene, start), scratch);
        }
#endif
        var user = AppDirs.User();
        // A first launch: no config yet. Existing installs never see the welcome.
        firstRun = !Directory.Exists(user.Config) || !Directory.EnumerateFileSystemEntries(user.Config).Any();
        AnistreamFfiMethods.StartLog(user.Cache);
        return (EngineSession.StartUser(host, start), user);
    }

    FfiPlayer? StartPlayer(FfiEngine engine)
    {
        try
        {
            var opts = new FfiPlayerOptions(
                Video: new FfiVideo.Composition(1280, 720),
                CacheDir: Path.Combine(dirs.Cache, "player"));
            return new FfiPlayer(opts, engine, new Sink(this));
        }
        catch (Exception e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Error, "windows", $"the player did not start: {e.Message}");
            return null;
        }
    }

    /// <summary>The player's events, from mpv's threads to the UI's.</summary>
    sealed class Sink(MainWindow window) : FfiPlayerSink
    {
        public void Event(FfiPlayerEvent e) => window.ui.TryEnqueue(() =>
        {
            window.Player.Heard(e);
            window.media?.Heard(e);
        });
    }

    // The first page, and what the command line asks for once the engine has a list to show.

    void FirstPage()
    {
        var page = options.Page ?? "continue";
        if (options.Query is not null)
            ShowSearch(options.Query);
        else
            Navigate(page, options.Pane);
        if (options.Title is { } id)
            OpenTitle(id);
        if (options.Join)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1800);
                ShowJoinStore("https://plugins.hotaru.example/join/fixture#key");
            });
        if (options.AskClose)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(2500);
                AskBeforeClosing();
            });
#if ANISTREAM_TESTING
        // --show=devicecode: Trakt's sign-in code, for a screenshot.
        if (options.Show == "devicecode")
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1500);
                Pages.DeviceCodeDialog.Heard(Root.XamlRoot, new DeviceCodePrompt("trakt", "7F3K9QX2", "https://tracker.example/activate",
                    DateTimeOffset.UtcNow.ToUnixTimeSeconds() + 15 * 60 - 1));
            });
#endif
        if (options.Shortcuts)
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(1500);
                ShowShortcuts();
            });
        // First launch: the three pages, once. A fixture run shows them only when asked.
        var welcomed = Path.Combine(dirs.Config, "windows-welcomed");
        if (options.Welcome || (!options.Fixture && firstRun && !File.Exists(welcomed)))
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(600);
                try
                {
                    Directory.CreateDirectory(dirs.Config);
                    File.WriteAllText(welcomed, "");
                }
                catch (IOException)
                {
                    // Not kept: it shows again next time, which is harmless.
                }
                var welcome = new WelcomeDialog(ctx, Root.XamlRoot);
                if (options.Welcome && options.Show is { } step)
                    welcome.Step(step);
                await welcome.ShowAsync();
                if (welcome.Invite is { } invite)
                    ShowJoinStore(invite);
            });
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.Sync:
                    ShowAccount();
                    break;
                case Event.Toast t:
                    ShowToast(t.V1);
                    break;
                case Event.DeviceCode code:
                    Pages.DeviceCodeDialog.Heard(Root.XamlRoot, code.V1);
                    break;
                case Event.Downloads d:
                    taskbar?.Downloads(ctx.Known.Downloads);
                    foreach (var notice in Notices.Downloaded(downloadsSeen, d.V1))
                        toasts?.Post(notice);
                    downloadsSeen = d.V1;
                    break;
                case Event.NewEpisodes n:
                    foreach (var notice in Notices.Aired(n.V1))
                        toasts?.Post(notice);
                    break;
                case Event.Content { View: ContentFor.Continue, ContentValue: ContentList.Entries } when options.Play && !played:
                    played = true;
                    var entries = ctx.Known.ListEntries(new ContentFor.Continue());
                    var queue = AnistreamFfiMethods.ContinueQueue(entries, [.. ctx.Known.HiddenFromContinue], Casing.Sentence);
                    if (queue.InProgress.Concat(queue.NewThisWeek).FirstOrDefault(q => q.Playable) is { } first)
                        ctx.Play(first.Entry.Id, first.Episode.ToString());
                    break;
            }
        }
    }

    /// <summary>
    /// The account in the pane's foot. Water is calm state: changes waiting to sync. A sign-in
    /// that needs renewing is a problem, so it says so in words beside the error glyph.
    /// </summary>
    void ShowAccount()
    {
        var anilist = ctx.Known.Anilist;
        var reauth = anilist is { NeedsReauth: true };
        AccountText.Text = anilist is null ? "Not signed in" : reauth ? "Sign in again" : anilist.User ?? "AniList";
        AccountDot.Visibility = anilist is { Outbox: > 0 } && !reauth ? Visibility.Visible : Visibility.Collapsed;
        AccountAlert.Visibility = reauth ? Visibility.Visible : Visibility.Collapsed;
        var tip = anilist is null ? "Sign in to AniList"
            : reauth ? "AniList needs you to sign in again"
            : anilist.Outbox > 0 ? $"Signed in to AniList as {anilist.User} · {AnistreamFfiMethods.Plural(anilist.Outbox, "change")} waiting to sync"
            : $"Signed in to AniList as {anilist.User}";
        ToolTipService.SetToolTip(AccountItem, tip);
        AutomationProperties.SetName(AccountItem, tip);
    }

    // Notices: the engine's words with the first letter capitalised, for as long as they take to
    // read (4 s, 8 s with a button or for an alert, and 1 s for every 10 words past 12), paused
    // under the pointer, with the hint as the one button.

    int toastShown;
    bool toastHeld;
    DateTimeOffset toastUntil;

    void ShowToast(Notice notice)
    {
        var text = Words.Notice(notice.Text);
        var (button, act) = notice.Hint switch
        {
            NoticeHint.StopPlayback => ("Stop", (Action)(() => { if (Player.Showing) Player.Close(); })),
            NoticeHint.OpenStores => ("Open stores", () => Go("stores")),
            NoticeHint.KitsuCredentials => ("Sign in", () => ContentFrame.Navigate(typeof(SettingsPage), "accounts", new SuppressNavigationTransitionInfo())),
            _ => (null, null),
        };
        ShowToast(notice.Kind == NoticeKind.Alert ? InfoBarSeverity.Warning : InfoBarSeverity.Informational, text, button, act);
    }

    void ShowToast(InfoBarSeverity severity, string text, string? button = null, Action? act = null)
    {
        var shown = ++toastShown;
        if (Toast.IsOpen)
            Motion.FadeIn(Toast);
        Toast.Severity = severity;
        Toast.Message = text;
        if (button is not null && act is not null)
        {
            var b = new Button { Content = button };
            b.Click += (_, _) =>
            {
                Toast.IsOpen = false;
                act();
            };
            Toast.ActionButton = b;
        }
        else
            Toast.ActionButton = null;
        Toast.IsOpen = true;
        var words = text.Split(' ', StringSplitOptions.RemoveEmptyEntries).Length;
        var seconds = (button is not null || severity == InfoBarSeverity.Warning ? 8 : 4) + Math.Max(0, words - 12) / 10;
        toastUntil = DateTimeOffset.UtcNow.AddSeconds(seconds);
        _ = CloseToastLater(shown);
    }

    async Task CloseToastLater(int shown)
    {
        while (shown == toastShown)
        {
            await Task.Delay(250);
            if (toastHeld)
            {
                toastUntil = toastUntil.AddMilliseconds(250);
                continue;
            }
            if (DateTimeOffset.UtcNow >= toastUntil)
                break;
        }
        if (shown == toastShown)
            Toast.IsOpen = false;
    }

    void OnToastPointerEntered(object sender, PointerRoutedEventArgs e) => toastHeld = true;

    void OnToastPointerExited(object sender, PointerRoutedEventArgs e) => toastHeld = false;

    // Moving between pages.

    void Navigate(string tag, object? parameter = null)
    {
        if (!Destinations.TryGetValue(tag, out var page))
            return;
        if (ContentFrame.CurrentSourcePageType == page && parameter is null && tag != "search")
            return;
        ContentFrame.Navigate(page, parameter, new SuppressNavigationTransitionInfo());
    }

    /// <summary>Goes to a destination by its tag, as the pane and Ctrl+1 to Ctrl+7 do.</summary>
    internal void Go(string tag)
    {
        if (Player.Showing)
            Player.Close();
        Navigate(tag);
    }

    /// <summary>
    /// A title's page: drill-in, with the cover carried from where it was clicked when one was
    /// (a connected animation). With animation effects off, both become a crossfade.
    /// </summary>
    internal void OpenTitle(uint id)
    {
        if (Player.Showing)
            Player.Close();
        if (Motion.Enabled)
            ContentFrame.Navigate(typeof(TitlePage), id, new DrillInNavigationTransitionInfo());
        else
        {
            ConnectedAnimationService.GetForCurrentView().GetAnimation(TitlePage.CoverAnimation)?.Cancel();
            ContentFrame.Navigate(typeof(TitlePage), id, new SuppressNavigationTransitionInfo());
            Motion.FadeIn(ContentFrame, Motion.Standard);
        }
    }

    internal void ShowSearch(string query)
    {
        SearchBox.Text = query;
        ContentFrame.Navigate(typeof(SearchPage), query, new SuppressNavigationTransitionInfo());
    }

    /// <summary>The join-store dialog, with an invite already pasted when one is given.</summary>
    internal async void ShowJoinStore(string? pasted = null)
    {
        await new JoinStoreDialog(ctx, Root.XamlRoot, pasted).ShowAsync(options.Show == "preview");
        if (ctx.Known.Joined is { } store)
        {
            ctx.Known.Joined = null;
            ContentFrame.Navigate(typeof(StoresPage), store, new SuppressNavigationTransitionInfo());
        }
    }

    void OnNavSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (selecting)
            return;
        if (args.IsSettingsSelected)
            Navigate("settings");
        else if (args.SelectedItem is NavigationViewItem { Tag: string tag })
        {
            if (tag == "account")
                ContentFrame.Navigate(typeof(SettingsPage), "accounts", new SuppressNavigationTransitionInfo());
            else
                Navigate(tag);
        }
    }

    void OnFrameNavigated(object sender, NavigationEventArgs e)
    {
        AppTitleBar.IsBackButtonEnabled = ContentFrame.CanGoBack;
        selecting = true;
        if (e.SourcePageType == typeof(SettingsPage))
            Nav.SelectedItem = e.Parameter as string == "accounts" ? AccountItem : Nav.SettingsItem;
        else if (Destinations.FirstOrDefault(d => d.Value == e.SourcePageType).Key is { } tag && tag != "search")
            Nav.SelectedItem = Nav.MenuItems.OfType<NavigationViewItem>().FirstOrDefault(i => (string?)i.Tag == tag);
        else
            Nav.SelectedItem = null;
        selecting = false;
    }

    void OnBackRequested(TitleBar sender, object args) => GoBack();

    void GoBack()
    {
        if (Player.Showing)
            Player.Close();
        else if (ContentFrame.CanGoBack)
            ContentFrame.GoBack();
    }

    void OnPaneToggleRequested(TitleBar sender, object args) => Nav.IsPaneOpen = !Nav.IsPaneOpen;

    // Search in the title bar: suggestions from the titles the app already knows, Enter searches.

    void OnSearchTextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput)
            return;
        var text = sender.Text.Trim();
        sender.ItemsSource = text.Length < 2
            ? null
            : ctx.Known.Entries.Values
                .Where(e => e.Title.Contains(text, StringComparison.CurrentCultureIgnoreCase)
                    || e.Secondary?.Contains(text, StringComparison.CurrentCultureIgnoreCase) == true)
                .OrderBy(e => e.Title)
                .Take(6)
                .Select(e => new Suggestion(e.Id, e.Title))
                .ToList();
    }

    sealed record Suggestion(uint Id, string Title)
    {
        public override string ToString() => Title;
    }

    void OnSuggestionChosen(AutoSuggestBox sender, AutoSuggestBoxSuggestionChosenEventArgs args) { }

    void OnSearchSubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        if (args.ChosenSuggestion is Suggestion s)
        {
            sender.Text = "";
            OpenTitle(s.Id);
        }
        else
            ShowSearch(args.QueryText.Trim());
    }

    void OnFocusSearch(Microsoft.UI.Xaml.Input.KeyboardAccelerator sender, Microsoft.UI.Xaml.Input.KeyboardAcceleratorInvokedEventArgs args)
    {
        if (Player.Showing)
            return;
        args.Handled = true;
        SearchBox.Focus(FocusState.Keyboard);
    }

    // The player over the window.

    void PlayerShowing(bool showing)
    {
        // The player's top bar is the title bar while it is up: its title drags the window, and
        // the caption buttons follow its controls.
        SetTitleBar(showing ? Player.DragArea : AppTitleBar);
        AppTitleBar.Visibility = showing ? Visibility.Collapsed : Visibility.Visible;
        Nav.Visibility = showing ? Visibility.Collapsed : Visibility.Visible;
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        if (!showing && AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen)
            Fullscreen(false);
    }

    RectInt32? beforeMini;

    /// <summary>
    /// The mini player: the window as a small one that stays on top of the others, the video and
    /// its few controls in it, and the way back to where the window was.
    /// </summary>
    void Mini(bool on)
    {
        var now = AppWindow.Presenter.Kind == AppWindowPresenterKind.CompactOverlay;
        if (on == now)
            return;
        if (on)
        {
            if (AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen)
                Fullscreen(false);
            beforeMini = new RectInt32(AppWindow.Position.X, AppWindow.Position.Y, AppWindow.Size.Width, AppWindow.Size.Height);
            AppWindow.SetPresenter(AppWindowPresenterKind.CompactOverlay);
            var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
            AppWindow.Resize(new SizeInt32((int)(480 * scale), (int)(270 * scale)));
        }
        else
        {
            AppWindow.SetPresenter(AppWindowPresenterKind.Default);
            if (beforeMini is { } was)
                AppWindow.MoveAndResize(was);
            beforeMini = null;
        }
    }

    void Fullscreen(bool? on)
    {
        var now = AppWindow.Presenter.Kind == AppWindowPresenterKind.FullScreen;
        var want = on ?? !now;
        if (want == now)
            return;
        AppWindow.SetPresenter(want ? AppWindowPresenterKind.FullScreen : AppWindowPresenterKind.Default);
        Player.ShowFullscreen(want);
    }

    // Closing: downloads and seeding carry on only if the viewer says so.

    void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        if (closing)
            return;
        var (downloading, seeding) = ctx.Known.Running();
        if (downloading + seeding == 0 || askedClose)
            return;
        switch (WhenClosing)
        {
            case CloseChoice.Ask:
                args.Cancel = true;
                AskBeforeClosing();
                break;
            case CloseChoice.KeepGoing:
                args.Cancel = true;
                KeepGoingHidden();
                break;
        }
    }

    async void AskBeforeClosing()
    {
        var (downloading, seeding) = ctx.Known.Running();
        if (downloading + seeding == 0)
            (downloading, seeding) = options.AskClose ? (2, 1) : (0, 0);
        askedClose = true;
        var dontAsk = new CheckBox { Content = "Don't ask again" };
        var body = new StackPanel { Spacing = 16 };
        body.Children.Add(new TextBlock { Text = CloseLine(downloading, seeding), TextWrapping = TextWrapping.WrapWholeWords });
        body.Children.Add(dontAsk);
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            Title = "Downloads are still running",
            Content = body,
            PrimaryButtonText = "Keep going",
            SecondaryButtonText = "Stop and close",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
        };
        var answer = await dialog.ShowAsync();
        askedClose = false;
        if (answer == ContentDialogResult.None)
            return;
        if (dontAsk.IsChecked == true)
            WhenClosing = answer == ContentDialogResult.Primary ? CloseChoice.KeepGoing : CloseChoice.Stop;
        if (answer == ContentDialogResult.Primary)
            KeepGoingHidden();
        else
            CloseNow();
    }

    static string CloseLine(int downloading, int seeding)
    {
        var parts = new List<string>();
        if (downloading > 0)
            parts.Add(downloading == 1 ? "1 episode is downloading" : $"{downloading} episodes are downloading");
        if (seeding > 0)
            parts.Add(seeding == 1 ? "1 is seeding" : $"{seeding} are seeding");
        return $"{string.Join(" and ", parts)}. Keep them going after the window closes, or stop them?";
    }

    DispatcherQueueTimer? keepGoing;

    /// <summary>
    /// Keep going: the window goes and the notification area icon comes, the first time with a
    /// notification saying where anistream went. The engine stays until nothing is left running,
    /// or until the icon's Quit; the icon's Open brings the window back.
    /// </summary>
    void KeepGoingHidden()
    {
        if (Player.Showing)
            Player.Close();
        AppWindow.Hide();
        tray ??= new Tray(WinRT.Interop.WindowNative.GetWindowHandle(this), Path.Combine(AppContext.BaseDirectory, "Assets", "anistream.ico"))
        {
            Open = () => ui.TryEnqueue(ShowAgain),
            PauseAll = () => ui.TryEnqueue(ctx.PauseAllDownloads),
            Quit = () => ui.TryEnqueue(CloseNow),
            AnyRunning = () => ctx.Known.Downloads.Any(r => r.State is DownloadState.Active or DownloadState.Queued),
        };
        var told = Path.Combine(dirs.Config, "windows-told-tray");
        tray.Show(tell: !File.Exists(told));
        try
        {
            File.WriteAllText(told, "");
        }
        catch (IOException)
        {
            // Told again next time: harmless.
        }
        keepGoing?.Stop();
        keepGoing = ui.CreateTimer();
        keepGoing.Interval = TimeSpan.FromSeconds(5);
        keepGoing.Tick += (_, _) =>
        {
            var (d, s) = ctx.Known.Running();
            if (d + s == 0)
                CloseNow();
        };
        keepGoing.Start();
    }

    // Windows notifications: new episodes from the engine's check, finished downloads, and the
    // engine's own notices while the window is not in front.

    Toasts? toasts;
    DownloadRow[]? downloadsSeen;
    DispatcherQueueTimer? newEpisodes;
    bool inFront = true;

    /// <summary>How often the open app asks the engine what aired; the engine asks AniList at most every five minutes.</summary>
    static readonly TimeSpan CheckEvery = TimeSpan.FromMinutes(15);

    void StartNotifications(Art art)
    {
        Activated += (_, e) => inFront = e.WindowActivationState != WindowActivationState.Deactivated;
        var asked = options.Show == "toast";
        if (options.Fixture && !asked)
            return;
        toasts = Toasts.Start(Path.Combine(dirs.Cache, "notification-art"), art, (action, show, episode) => ui.TryEnqueue(() => FromNotification(action, show, episode)));
#if ANISTREAM_TESTING
        if (asked)
        {
            ui.TryEnqueue(DispatcherQueuePriority.Low, async () =>
            {
                await Task.Delay(2500);
                var banner = ctx.Known.Entry(900001)?.BannerUrl ?? ctx.Known.Entry(900001)?.CoverUrl;
                foreach (var notice in Notices.Aired([new NewEpisode(900001, ctx.Known.Entry(900001)?.Title ?? "Stardust Wayfarer", 8, 0, NextForYou: 7, BannerUrl: banner)]))
                    toasts?.Post(notice);
            });
            return;
        }
#endif
        if (options.Fixture)
            return;
        // Soon after launch, then while the app runs, window or notification area.
        newEpisodes = ui.CreateTimer();
        newEpisodes.Interval = TimeSpan.FromSeconds(20);
        newEpisodes.Tick += (_, _) =>
        {
            newEpisodes.Interval = CheckEvery;
            ctx.Send(new Command.CheckNewEpisodes());
        };
        newEpisodes.Start();
    }

    /// <summary>A notification clicked: the window to the front, then the episode playing or the show open.</summary>
    void FromNotification(string action, uint show, string? episode)
    {
        if (!AppWindow.IsVisible)
            ShowAgain();
        else
            Activate();
        if (show == 0)
            return;
        if (action == "play" && episode is not null)
            ctx.Play(show, episode);
        else
            OpenTitle(show);
    }

    void Notified(string title, string body)
    {
        if ((inFront && AppWindow.IsVisible) || toasts is null)
            ShowToast(InfoBarSeverity.Informational, $"{title}: {body}");
        else
            toasts.Post(new SystemNotice($"engine-{title}", title, body, 0, null, null));
    }

    /// <summary>The window back from the notification area; the icon goes with the window's return.</summary>
    void ShowAgain()
    {
        keepGoing?.Stop();
        tray?.Hide();
        AppWindow.Show(true);
        Activate();
    }

    void CloseNow()
    {
        closing = true;
        Close();
    }

    /// <summary>
    /// What closing the window does while downloads run: Settings › Downloads, "When the window
    /// closes with downloads running", kept beside the engine's own config. "Don't ask again" in
    /// the dialog keeps the answer given. The old flag meant closing without asking, which stops.
    /// </summary>
    internal CloseChoice WhenClosing
    {
        get
        {
            try
            {
                if (File.Exists(WhenClosingPath))
                    return File.ReadAllText(WhenClosingPath).Trim() switch
                    {
                        "keep-going" => CloseChoice.KeepGoing,
                        "stop" => CloseChoice.Stop,
                        _ => CloseChoice.Ask,
                    };
                return File.Exists(Path.Combine(dirs.Config, "windows-close-without-asking")) ? CloseChoice.Stop : CloseChoice.Ask;
            }
            catch (IOException)
            {
                return CloseChoice.Ask;
            }
        }
        set
        {
            Directory.CreateDirectory(dirs.Config);
            File.WriteAllText(WhenClosingPath, value switch
            {
                CloseChoice.KeepGoing => "keep-going",
                CloseChoice.Stop => "stop",
                _ => "ask",
            });
            File.Delete(Path.Combine(dirs.Config, "windows-close-without-asking"));
        }
    }

    string WhenClosingPath => Path.Combine(dirs.Config, "windows-when-closing");

    void Shutdown()
    {
        closing = true;
        keepGoing?.Stop();
        newEpisodes?.Stop();
        toasts?.Stop();
        tray?.Dispose();
        network.Dispose();
        taskbar?.Clear();
        player?.Shutdown();
        session.Dispose();
        player?.Dispose();
    }

    // The window's keys, after the brief's table. The player's own keys are on the player; these
    // stand aside while it is up, and while a text box has the focus for the keys it uses.

    void Keys()
    {
        string[] tags = ["continue", "calendar", "seasonal", "library", "downloads", "providers", "stores"];
        for (var i = 0; i < tags.Length; i++)
        {
            var tag = tags[i];
            Key(VirtualKey.Number1 + i, VirtualKeyModifiers.Control, () => Go(tag));
        }
        Key(VirtualKey.Left, VirtualKeyModifiers.Menu, GoBack, inPlayer: true);
        Key((VirtualKey)0xBC, VirtualKeyModifiers.Control, () => Navigate("settings"));
        Key(VirtualKey.R, VirtualKeyModifiers.Control, SyncNow);
        Key(VirtualKey.F5, VirtualKeyModifiers.None, SyncNow);
        Key(VirtualKey.M, VirtualKeyModifiers.Control | VirtualKeyModifiers.Shift, () => (ContentFrame.Content as IPageKeys)?.MarkWatched(), typing: false);
        Key(VirtualKey.Delete, VirtualKeyModifiers.None, () => (ContentFrame.Content as IPageKeys)?.Delete(), typing: false);
        Key((VirtualKey)0xBF, VirtualKeyModifiers.Control, ShowShortcuts);
        // The mouse's back button, as Alt+Left.
        Root.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, e) =>
        {
            if (e.GetCurrentPoint(Root).Properties.IsXButton1Pressed)
            {
                GoBack();
                e.Handled = true;
            }
        }), true);
    }

    void Key(VirtualKey key, VirtualKeyModifiers modifiers, Action act, bool inPlayer = false, bool typing = true)
    {
        var accelerator = new KeyboardAccelerator { Key = key, Modifiers = modifiers };
        accelerator.Invoked += (_, args) =>
        {
            if (Player.Showing && !inPlayer)
                return;
            if (!typing && FocusManager.GetFocusedElement(Root.XamlRoot) is TextBox or AutoSuggestBox or PasswordBox)
                return;
            args.Handled = true;
            act();
        };
        Root.KeyboardAccelerators.Add(accelerator);
    }

    void SyncNow()
    {
        if (ctx.Known.Anilist is null)
            return;
        ctx.Send(new Command.SyncNow());
        ShowToast(InfoBarSeverity.Informational, "Syncing with AniList");
    }

    /// <summary>Ctrl+/: every key the window and the player answer, in a dialog.</summary>
    internal async void ShowShortcuts()
    {
        var grid = new Grid { ColumnSpacing = 24, RowSpacing = 6 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var row = 0;
        void Section(string name)
        {
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var heading = new TextBlock { Text = name, Style = Ui.Res<Style>("BodyStrongTextBlockStyle"), Margin = new Thickness(0, row == 0 ? 0 : 12, 0, 2) };
            AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
            Grid.SetRow(heading, row++);
            Grid.SetColumnSpan(heading, 2);
            grid.Children.Add(heading);
        }
        void Line(string what, string keys)
        {
            grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var name = new TextBlock { Text = what };
            var key = new TextBlock { Text = keys, Style = Ui.Res<Style>("HazeBodyStyle") };
            Grid.SetRow(name, row);
            Grid.SetRow(key, row++);
            Grid.SetColumn(key, 1);
            grid.Children.Add(name);
            grid.Children.Add(key);
        }
        Section("Window");
        Line("Continue to Plugin stores", "Ctrl+1 to Ctrl+7");
        Line("Search", "Ctrl+F, Ctrl+E");
        Line("Back", "Alt+Left, mouse back");
        Line("Settings", "Ctrl+,");
        Line("Sync now", "Ctrl+R, F5");
        Line("Mark as watched", "Ctrl+Shift+M");
        Line("Hide from Continue", "Delete");
        Line("Context menu", "Shift+F10, Menu key");
        Line("Keyboard shortcuts", "Ctrl+/");
        Section("Player");
        Line("Play or pause", "Space, K");
        Line("Back or forward 5 s, 30 s with Shift", "Left, Right");
        Line("Back or forward 10 s", "J, L");
        Line("Volume", "Up, Down");
        Line("Mute", "M");
        Line("Full screen", "F, F11");
        Line("Skip, undo the skip", "S, U");
        Line("Next or previous episode", "N, Shift+N");
        Line("Episodes", "E");
        Line("Subtitles and audio", "T");
        Line("Slower, faster, normal speed", "[, ], Backspace");
        Line("Mini player", "P");
        Line("Take the corner card's action", "Enter");
        Line("Close the player", "Esc, Alt+Left");
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            Title = "Keyboard shortcuts",
            Content = new ScrollViewer { Content = grid, MaxHeight = 480, VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Padding = new Thickness(0, 0, 16, 0) },
            CloseButtonText = "Close",
            DefaultButton = ContentDialogButton.Close,
        };
        try
        {
            await dialog.ShowAsync();
        }
        catch (Exception)
        {
            // Another dialog is open: one at a time.
        }
    }
}

/// <summary>What closing the window does while downloads run.</summary>
enum CloseChoice
{
    Ask,
    KeepGoing,
    Stop,
}
