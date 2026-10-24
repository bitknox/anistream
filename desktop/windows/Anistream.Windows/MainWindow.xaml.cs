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
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;
using Windows.Graphics;

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
    bool selecting, played, closing, askedClose;

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
        Size();

        var host = new AppHost(ui, options.Fixture) { Notified = (title, body) => ShowToast(InfoBarSeverity.Informational, $"{title}: {body}") };
        (var engine, dirs) = StartEngine(host);
        session = new EngineSession(engine, act => ui.TryEnqueue(() => act()));
        var art = new Art(session.Send);
        ctx = new Ctx { Session = session, Art = art, Window = this, Options = options };
        Ctx.Current = ctx;
        session.Batch += batch =>
        {
            art.Take(batch);
            Player.Heard(batch);
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
        Closed += (_, _) => Shutdown();
        Root.Loaded += (_, _) => FirstPage();
    }

    void Size()
    {
        var area = DisplayArea.GetFromWindowId(AppWindow.Id, DisplayAreaFallback.Primary).WorkArea;
        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        var width = options.Width is { } w ? (int)(w * scale) : Math.Min((int)(1280 * scale), area.Width);
        var height = options.Height is { } h ? (int)(h * scale) : Math.Min((int)(800 * scale), area.Height);
        AppWindow.MoveAndResize(new RectInt32(
            area.X + Math.Max(0, (area.Width - width) / 2), area.Y + Math.Max(0, (area.Height - height) / 2), width, height));
        if (AppWindow.Presenter is OverlappedPresenter overlapped)
        {
            overlapped.PreferredMinimumWidth = (int)(500 * scale);
            overlapped.PreferredMinimumHeight = (int)(400 * scale);
        }
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
        public void Event(FfiPlayerEvent e) => window.ui.TryEnqueue(() => window.Player.Heard(e));
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
                    ShowToast(t.V1.Kind switch
                    {
                        NoticeKind.Alert => InfoBarSeverity.Warning,
                        _ => InfoBarSeverity.Informational,
                    }, t.V1.Text);
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

    void ShowAccount()
    {
        var anilist = ctx.Known.Anilist;
        AccountText.Text = anilist is null ? "Not signed in" : anilist.User ?? "AniList";
        AccountDot.Visibility = anilist is { Outbox: > 0 } || anilist is { NeedsReauth: true } ? Visibility.Visible : Visibility.Collapsed;
        ToolTipService.SetToolTip(AccountItem, anilist is null ? "Connect a tracker" : $"Signed in to AniList as {anilist.User}");
    }

    async void ShowToast(InfoBarSeverity severity, string text)
    {
        Toast.Severity = severity;
        Toast.Message = text;
        Toast.IsOpen = true;
        await Task.Delay(TimeSpan.FromSeconds(5));
        if (Toast.Message == text)
            Toast.IsOpen = false;
    }

    // Moving between pages.

    void Navigate(string tag, object? parameter = null)
    {
        if (!Destinations.TryGetValue(tag, out var page))
            return;
        if (ContentFrame.CurrentSourcePageType == page && parameter is null && tag != "search")
            return;
        ContentFrame.Navigate(page, parameter, new SuppressNavigationTransitionInfo());
    }

    internal void OpenTitle(uint id)
    {
        if (Player.Showing)
            Player.Close();
        ContentFrame.Navigate(typeof(TitlePage), id, new DrillInNavigationTransitionInfo());
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
        if (downloading + seeding > 0 && !askedClose && !DontAsk)
        {
            args.Cancel = true;
            AskBeforeClosing();
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
            DontAsk = true;
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

    /// <summary>Keep going: the window goes, the engine stays until nothing is left running.</summary>
    void KeepGoingHidden()
    {
        AppWindow.Hide();
        var timer = ui.CreateTimer();
        timer.Interval = TimeSpan.FromSeconds(5);
        timer.Tick += (_, _) =>
        {
            var (d, s) = ctx.Known.Running();
            if (d + s == 0)
            {
                timer.Stop();
                CloseNow();
            }
        };
        timer.Start();
    }

    void CloseNow()
    {
        closing = true;
        Close();
    }

    /// <summary>"Don't ask again", kept beside the engine's own config.</summary>
    bool DontAsk
    {
        get => File.Exists(DontAskPath);
        set
        {
            Directory.CreateDirectory(dirs.Config);
            if (value)
                File.WriteAllText(DontAskPath, "");
            else
                File.Delete(DontAskPath);
        }
    }

    string DontAskPath => Path.Combine(dirs.Config, "windows-close-without-asking");

    void Shutdown()
    {
        closing = true;
        player?.Shutdown();
        session.Dispose();
        player?.Dispose();
    }
}
