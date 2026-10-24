using System.Globalization;
using System.Runtime.InteropServices;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;

namespace Anistream.Windows.Player;

/// <summary>
/// The player, over the page it came from. mpv draws through d3d11 composition into a swapchain
/// of its own; this view hands that swapchain to the SwapChainPanel (polled, since mpv says
/// nothing when it makes a new one) and tells mpv the panel's size in pixels. The engine drives
/// mpv through anistream-mpv-host (FfiPlayer); the controls here only send the engine's player
/// commands and draw what the engine and the player say.
/// </summary>
public sealed partial class PlayerView : UserControl
{
    static readonly TimeSpan IdleHide = TimeSpan.FromSeconds(2);

    Ctx? ctx;
    FfiPlayer? player;
    readonly DispatcherQueueTimer tick;
    readonly DispatcherQueueTimer idle;
    nint bound;
    bool paused, fromPlayer, overBar, menuOpen, keepControls;
    double position, duration, volume = 100;
    Coming? upNext;
    PlayerUi.Menus? menus;
    readonly StillWatching still = new();
    bool pictureSeen, pointerHidden, mini, miniTipShown;
    SeekPreview? preview;
    PreviewSource? stream, thumbs, previewSource;
    string? thumbnails;
    readonly object loads = new();
    PreviewSource? lastLoad;

    public PlayerView()
    {
        InitializeComponent();
        tick = DispatcherQueue.GetForCurrentThread().CreateTimer();
        tick.Interval = TimeSpan.FromMilliseconds(250);
        tick.Tick += (_, _) => Tick();
        idle = DispatcherQueue.GetForCurrentThread().CreateTimer();
        idle.Interval = IdleHide;
        idle.IsRepeating = false;
        idle.Tick += (_, _) => HideControlsIfIdle();
        // A slider takes its pointer events: the preview listens even to the handled ones.
        Timeline.AddHandler(PointerMovedEvent, new PointerEventHandler(OnTimelinePointerMoved), true);
    }

    /// <summary>True while the player is on screen.</summary>
    public bool Showing => Visibility == Visibility.Visible;

    /// <summary>Fullscreen asked for: by F, the button or the engine.</summary>
    internal event Action<bool?>? FullscreenRequested;

    /// <summary>The player shown or hidden, for the window to give its title bar to it.</summary>
    internal event Action<bool>? ShowingChanged;

    /// <summary>The controls shown or hidden, for the window's caption buttons to follow.</summary>
    internal event Action<bool>? ControlsChanged;

    internal void Attach(Ctx ctx, FfiPlayer? player)
    {
        this.ctx = ctx;
        this.player = player;
        keepControls = ctx.Options.KeepControls;
    }

    /// <summary>A player command that comes from the viewer: it also says somebody is there.</summary>
    void Send(PlayerCommand command)
    {
        still.Input();
        ctx?.Send(new Command.Player(command));
    }

    /// <summary>A player command the player sends itself, such as playing on when the countdown ends.</summary>
    void SendQuietly(PlayerCommand command) => ctx?.Send(new Command.Player(command));

    // The window's side: show, hide, and what the player and the engine say.

    internal void Open()
    {
        if (Showing)
            return;
        Visibility = Visibility.Visible;
        still.Reset();
        ShowControls();
        tick.Start();
        Focus(FocusState.Programmatic);
        ShowingChanged?.Invoke(true);
    }

    internal void Hide()
    {
        if (!Showing)
            return;
        tick.Stop();
        idle.Stop();
        ShowPointer();
        ClosePreview();
        if (mini)
            MiniRequested?.Invoke(false);
        SetMini(false);
        still.Reset();
        StillLayer.Visibility = Visibility.Collapsed;
        Visibility = Visibility.Collapsed;
        Starting.Visibility = Visibility.Collapsed;
        SkipCard.Visibility = UpNextCard.Visibility = StallCard.Visibility = AlertCard.Visibility = Visibility.Collapsed;
        upNext = null;
        thumbnails = null;
        thumbs = null;
        ShowingChanged?.Invoke(false);
    }

    internal void Heard(FfiPlayerEvent e)
    {
        switch (e)
        {
            case FfiPlayerEvent.Opened o:
                Open();
                if (o.FullscreenValue)
                    FullscreenRequested?.Invoke(true);
                break;
            case FfiPlayerEvent.Closed:
                Hide();
                break;
            case FfiPlayerEvent.Fullscreen f:
                FullscreenRequested?.Invoke(f.On);
                break;
            case FfiPlayerEvent.State s:
                ShowState(s.StateValue);
                break;
        }
    }

    internal void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.PlayerUi ui:
                    Heard(ui.Ui);
                    break;
                case Event.PlaybackVolume v:
                    volume = v.V1;
                    fromPlayer = true;
                    VolumeSlider.Value = Math.Clamp(volume, 0, 100);
                    fromPlayer = false;
                    VolumeIcon.Glyph = volume <= 0 ? "" : "";
                    break;
            }
        }
    }

    void Heard(PlayerUi ui)
    {
        switch (ui)
        {
            case PlayerUi.Starting s:
                Open();
                still.Started();
                pictureSeen = false;
                ClosePreview();
                StillLayer.Visibility = Visibility.Collapsed;
                still.Answered();
                ShowTitle.Text = s.Title;
                EpisodeTitle.Text = s.EpisodeTitle is { Length: > 0 } t ? $"Episode {s.EpisodeValue} · {t}" : $"Episode {s.EpisodeValue}";
                StartingShow.Text = s.Title;
                StartingLine.Text = $"Starting episode {s.EpisodeValue}";
                StartingArt.Source = null;
                ctx?.Art.Bind(s.BannerUrl ?? s.CoverUrl, image => StartingArt.Source = image);
                Starting.Visibility = Visibility.Visible;
                break;
            case PlayerUi.Episode ep:
                ShowTitle.Text = ep.Show;
                EpisodeTitle.Text = ep.Title is { Length: > 0 } title ? $"{ep.Label} · {title}" : ep.Label;
                break;
            case PlayerUi.Menus m:
                menus = m;
                BuildMenus();
                break;
            case PlayerUi.Skip skip:
                SkipCard.Content = skip.Kind switch
                {
                    SkipKind.Opening => "Skip opening",
                    SkipKind.Ending => "Skip ending",
                    _ => "Skip",
                };
                SkipCard.Visibility = Visibility.Visible;
                break;
            case PlayerUi.SkipCleared:
                SkipCard.Visibility = Visibility.Collapsed;
                break;
            case PlayerUi.AutoSkipped auto:
                Alert($"Skipped the {Words.Sentence(auto.Kind.ToString()).ToLowerInvariant()}");
                break;
            case PlayerUi.UpNext next:
                UpNextTitle.Text = next.Next.Title is { Length: > 0 } nt ? $"Episode {next.Next.Number} · {nt}" : $"Episode {next.Next.Number}";
                upNext = new Coming(next.Auto, next.EndingAt, next.Countdown);
                still.After = (int)next.StillWatchingAfter;
                ShowUpNext();
                break;
            case PlayerUi.UpNextCleared:
                upNext = null;
                ShowUpNext();
                break;
            case PlayerUi.Stall stall:
                StallLine.Text = $"This stream has stalled for {stall.Seconds} s.";
                StallCard.Visibility = Visibility.Visible;
                break;
            case PlayerUi.StallCleared:
                StallCard.Visibility = Visibility.Collapsed;
                break;
            case PlayerUi.Alert alert:
                Alert(alert.Text);
                break;
            case PlayerUi.Thumbnails thumb:
                if (thumb.Url != thumbnails)
                    ClosePreview();
                thumbnails = thumb.Url;
                thumbs = null;
                break;
        }
    }

    async void Alert(string text)
    {
        AlertText.Text = text;
        AlertCard.Visibility = Visibility.Visible;
        await Task.Delay(TimeSpan.FromSeconds(4));
        if (AlertText.Text == text)
            AlertCard.Visibility = Visibility.Collapsed;
    }

    void ShowState(FfiPlayerState state)
    {
        if (state.HasPicture && Starting.Visibility == Visibility.Visible)
            Starting.Visibility = Visibility.Collapsed;
        if (state.HasPicture && !pictureSeen)
        {
            pictureSeen = true;
            FirstPicture();
        }
        paused = state.Paused;
        PlayPauseIcon.Glyph = paused ? "" : "";
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(PlayPause, paused ? "Play" : "Pause");
        if (paused)
            ShowControls();
        else if (Chrome.Visibility == Visibility.Visible && !idle.IsRunning)
            idle.Start();
        if (state.Position is { } p)
            position = p;
        if (state.Duration is { } d && d > 0)
            duration = d;
        Elapsed.Text = AnistreamFfiMethods.PlayClock(position);
        Remaining.Text = "-" + AnistreamFfiMethods.PlayClock(Math.Max(0, duration - position));
        ShowUpNext();
        fromPlayer = true;
        Timeline.Maximum = Math.Max(1, duration);
        Timeline.Value = Math.Min(position, Timeline.Maximum);
        fromPlayer = false;
    }

    // The swapchain and the panel's size.

    void Tick()
    {
        if (player is null)
            return;
        var swapchain = (nint)player.Swapchain();
        if (swapchain != 0 && swapchain != bound)
        {
            try
            {
                Dxgi.SetSwapChain(VideoPanel, swapchain);
                var scale = VideoPanel.CompositionScaleX;
                if (Math.Abs(scale - 1) > 0.001)
                    Dxgi.SetInverseScale(swapchain, VideoPanel.CompositionScaleX, VideoPanel.CompositionScaleY);
                bound = swapchain;
            }
            catch (COMException e)
            {
                AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"binding mpv's swapchain failed: {e.Message}");
            }
        }
    }

    void OnPanelSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (player is null || e.NewSize.Width < 1 || e.NewSize.Height < 1)
            return;
        var scale = XamlRoot?.RasterizationScale ?? 1.0;
        var w = (uint)Math.Max(1, Math.Round(e.NewSize.Width * scale));
        var h = (uint)Math.Max(1, Math.Round(e.NewSize.Height * scale));
        player.CompositionSize(w, h);
        player.Display(w, h, scale, null);
    }

    // Showing and hiding the controls.

    void ShowControls()
    {
        ShowPointer();
        if (still.Asking)
            return;
        if (Chrome.Visibility != Visibility.Visible)
        {
            Chrome.Visibility = Visibility.Visible;
            ControlsChanged?.Invoke(true);
        }
        idle.Stop();
        if (!paused)
            idle.Start();
    }

    void HideControlsIfIdle()
    {
        if (keepControls || paused || overBar || menuOpen)
            return;
        Chrome.Visibility = Visibility.Collapsed;
        PreviewBox.Visibility = Visibility.Collapsed;
        ControlsChanged?.Invoke(false);
        HidePointer();
    }

    [LibraryImport("user32.dll")]
    private static partial int ShowCursor([MarshalAs(UnmanagedType.Bool)] bool show);

    /// <summary>The pointer goes with the controls when nobody is using it.</summary>
    void HidePointer()
    {
        if (pointerHidden || !Showing)
            return;
        pointerHidden = true;
        while (ShowCursor(false) >= 0)
        {
            // The count falls below zero and the pointer is gone.
        }
    }

    void ShowPointer()
    {
        if (!pointerHidden)
            return;
        pointerHidden = false;
        while (ShowCursor(true) < 0)
        {
            // Back to zero or above: the pointer is shown again.
        }
    }

    void OnPointerMoved(object sender, PointerRoutedEventArgs e) => ShowControls();
    void OnBarEntered(object sender, PointerRoutedEventArgs e) => overBar = true;
    void OnBarExited(object sender, PointerRoutedEventArgs e) => overBar = false;
    void OnMenuOpened(object? sender, object e) => menuOpen = true;

    void OnMenuClosed(object? sender, object e)
    {
        menuOpen = false;
        ShowControls();
    }

    void OnVideoTapped(object sender, TappedRoutedEventArgs e)
    {
        if (e.OriginalSource != VideoPanel && e.OriginalSource != Chrome && e.OriginalSource != Starting)
            return;
        if (e.PointerDeviceType == Microsoft.UI.Input.PointerDeviceType.Touch)
        {
            // Touch: a tap toggles the controls rather than pausing.
            if (Chrome.Visibility == Visibility.Visible)
            {
                Chrome.Visibility = Visibility.Collapsed;
                ControlsChanged?.Invoke(false);
            }
            else
                ShowControls();
            return;
        }
        Send(new PlayerCommand.TogglePause());
    }

    void OnVideoDoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
    {
        if (e.OriginalSource == VideoPanel || e.OriginalSource == Chrome)
            FullscreenRequested?.Invoke(null);
    }

    void OnWheel(object sender, PointerRoutedEventArgs e)
    {
        var point = e.GetCurrentPoint(this);
        var delta = point.Properties.MouseWheelDelta;
        if (point.Properties.IsHorizontalMouseWheel)
            Send(new PlayerCommand.Seek(delta > 0 ? 5 : -5));
        else
            Send(new PlayerCommand.Volume(delta > 0 ? 5 : -5));
        e.Handled = true;
    }

    // The controls.

    void OnClose(object sender, RoutedEventArgs e) => Close();

    internal void Close()
    {
        player?.ViewerClosed();
        Hide();
    }

    void OnPlayPause(object sender, RoutedEventArgs e) => Send(new PlayerCommand.TogglePause());
    void OnBack10(object sender, RoutedEventArgs e) => Send(new PlayerCommand.Seek(-10));
    void OnForward10(object sender, RoutedEventArgs e) => Send(new PlayerCommand.Seek(10));
    void OnFullscreen(object sender, RoutedEventArgs e) => FullscreenRequested?.Invoke(null);
    void OnSkip(object sender, RoutedEventArgs e) => Send(new PlayerCommand.TakeSkip());
    void OnPlayNext(object sender, RoutedEventArgs e) => PlayNext(auto: false);
    void OnCancelNext(object sender, RoutedEventArgs e) => CancelNext();

    /// <summary>
    /// Up Next as the engine hands it over: the card shows from where the ending starts (or near
    /// the end of the file when that is not known), and its countdown runs on the playhead from
    /// there; at zero this sends PlayNext, once.
    /// </summary>
    sealed record Coming(bool Auto, double? EndingAt, uint Countdown)
    {
        public bool Cancelled { get; set; }
        public bool Played { get; set; }

        public double? ShowsAt(double duration)
        {
            if (EndingAt is { } at)
                return at;
            if (duration <= 0)
                return null;
            return duration > 120 ? duration - 30 : Math.Max(0, duration - (Countdown > 0 ? Countdown : 10));
        }

        public double? Remaining(double position, double duration) =>
            !Auto || Cancelled || Countdown == 0 || ShowsAt(duration) is not { } start || position < start
                ? null
                : Math.Max(0, Countdown - (position - start));
    }

    void ShowUpNext()
    {
        var card = upNext;
        var shown = card is { Cancelled: false } && card.ShowsAt(duration) is { } at && position >= at;
        UpNextCard.Visibility = shown ? Visibility.Visible : Visibility.Collapsed;
        if (!shown || card is null)
            return;
        var left = card.Remaining(position, duration);
        UpNextBar.Visibility = left is null ? Visibility.Collapsed : Visibility.Visible;
        UpNextLine.Visibility = UpNextBar.Visibility;
        if (left is { } l)
        {
            UpNextBar.Value = l / card.Countdown;
            UpNextLine.Text = $"Plays in {Math.Ceiling(l)} s";
            if (l <= 0)
                PlayNext(auto: true);
        }
    }

    void PlayNext(bool auto)
    {
        if (upNext is { Played: true })
            return;
        if (upNext is not null)
            upNext.Played = true;
        if (auto)
        {
            still.RollingOn();
            SendQuietly(new PlayerCommand.PlayNext());
        }
        else
            Send(new PlayerCommand.PlayNext());
    }

    void CancelNext()
    {
        if (upNext is not null)
            upNext.Cancelled = true;
        ShowUpNext();
        Send(new PlayerCommand.CancelNext());
    }
    void OnStallWait(object sender, RoutedEventArgs e) => Send(new PlayerCommand.StallWait());
    void OnStallSwitch(object sender, RoutedEventArgs e) => Send(new PlayerCommand.StallSwitch());

    internal void ShowFullscreen(bool on) => FullscreenIcon.Glyph = on ? "" : "";

    void OnTimelineChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        if (!fromPlayer && Math.Abs(e.NewValue - position) > 1)
            Send(new PlayerCommand.SeekTo(e.NewValue));
    }

    void OnVolumeChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        if (fromPlayer)
            return;
        var delta = e.NewValue - volume;
        if (Math.Abs(delta) >= 1)
        {
            volume = e.NewValue;
            Send(new PlayerCommand.Volume(delta));
        }
    }

    void BuildMenus()
    {
        if (menus is null)
            return;
        EpisodesMenu.Items.Clear();
        foreach (var ep in menus.Episodes)
        {
            var item = new ToggleMenuFlyoutItem
            {
                Text = ep.Title is { Length: > 0 } t ? $"{ep.Label} · {t}" : ep.Label,
                IsChecked = ep.Current,
            };
            var number = ep.Number;
            item.Click += (_, _) => Send(new PlayerCommand.Choose(new PlayerChoice.Episode(number)));
            EpisodesMenu.Items.Add(item);
        }
        EpisodesButton.IsEnabled = menus.Episodes.Length > 0;

        TracksMenu.Items.Clear();
        foreach (var (translation, label) in new[] { (Engine.Core.Translation.Sub, "Subtitled"), (Engine.Core.Translation.Dub, "Dubbed") })
        {
            var item = new RadioMenuFlyoutItem { Text = label, GroupName = "translation", IsChecked = menus.Translation == translation };
            item.Click += (_, _) => Send(new PlayerCommand.Choose(new PlayerChoice.Translation(translation)));
            TracksMenu.Items.Add(item);
        }

        SettingsMenu.Items.Clear();
        if (menus.Sources.Length > 0)
        {
            var sources = new MenuFlyoutSubItem { Text = "Source" };
            foreach (var source in menus.Sources)
            {
                var item = new RadioMenuFlyoutItem { Text = source.Detail is { Length: > 0 } d ? $"{source.Label} · {d}" : source.Label, GroupName = "source", IsChecked = source.Current };
                var id = source.Id;
                item.Click += (_, _) => Send(new PlayerCommand.Choose(new PlayerChoice.Source(id)));
                sources.Items.Add(item);
            }
            SettingsMenu.Items.Add(sources);
        }
        var upscaling = new MenuFlyoutSubItem { Text = "Upscaling" };
        foreach (var choice in menus.Upscaling.Options ?? [])
        {
            var item = new RadioMenuFlyoutItem { Text = choice.Label, GroupName = "upscaling", IsChecked = choice.Id == menus.Upscaling.Current };
            var id = choice.Id;
            item.Click += (_, _) => Send(new PlayerCommand.Upscaling(id));
            upscaling.Items.Add(item);
        }
        SettingsMenu.Items.Add(upscaling);
        var speed = new MenuFlyoutSubItem { Text = "Speed" };
        foreach (var s in new[] { 0.75, 1.0, 1.25, 1.5, 2.0 })
        {
            var item = new MenuFlyoutItem { Text = s == 1 ? "Normal" : $"{s.ToString(CultureInfo.InvariantCulture)}×" };
            item.Click += (_, _) => Send(new PlayerCommand.SpeedTo(s));
            speed.Items.Add(item);
        }
        SettingsMenu.Items.Add(speed);
        var night = new ToggleMenuFlyoutItem { Text = "Night mode", IsChecked = menus.NightMode };
        night.Click += (_, _) => Send(new PlayerCommand.NightMode(night.IsChecked));
        SettingsMenu.Items.Add(night);
    }

    // Keys: arrows and S act without showing the controls; every other key shows them.

    void Key(KeyboardAcceleratorInvokedEventArgs e, Action act, bool show = true)
    {
        if (!Showing)
            return;
        e.Handled = true;
        if (show)
            ShowControls();
        act();
    }

    void OnKeyPause(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.TogglePause()));
    void OnKeyBack10(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Seek(-10)));
    void OnKeyForward10(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Seek(10)));
    void OnKeyBack5(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Seek(-5)), show: false);
    void OnKeyForward5(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Seek(5)), show: false);
    void OnKeyBack30(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Seek(-30)), show: false);
    void OnKeyForward30(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Seek(30)), show: false);
    void OnKeyVolumeUp(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Volume(5)), show: false);
    void OnKeyVolumeDown(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Volume(-5)), show: false);
    void OnKeyFullscreen(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => FullscreenRequested?.Invoke(null));
    void OnKeySkip(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.TakeSkip()), show: false);
    void OnKeyNext(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Step(1)));
    void OnKeyPrevious(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Step(-1)));
    void OnKeyMini(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => ToggleMini());
    void OnKeyClose(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, Close, show: false);

    void OnKeyEnter(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (still.Asking)
            ContinueWatching();
        else if (StallCard.Visibility == Visibility.Visible)
            Send(new PlayerCommand.StallSwitch());
        else if (UpNextCard.Visibility == Visibility.Visible)
            PlayNext(auto: false);
        else if (SkipCard.Visibility == Visibility.Visible)
            Send(new PlayerCommand.TakeSkip());
    });

    /// <summary>Esc: cancel Up Next or the stall prompt, then leave fullscreen, then close.</summary>
    void OnKeyEscape(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (still.Asking)
            StillStopNow();
        else if (mini)
            ToggleMini();
        else if (UpNextCard.Visibility == Visibility.Visible)
            CancelNext();
        else if (StallCard.Visibility == Visibility.Visible)
            Send(new PlayerCommand.StallWait());
        else if (FullscreenOn?.Invoke() == true)
            FullscreenRequested?.Invoke(false);
        else
            Close();
    }, show: false);

    // Still watching?

    /// <summary>The new episode has its first picture: pause and ask when nobody has been here for a run of them.</summary>
    void FirstPicture()
    {
        if (still.FirstPicture())
            AskStillWatching();
#if ANISTREAM_TESTING
        else if (ctx?.Options.Show is { } show)
            Demo(show);
#endif
    }

    void AskStillWatching()
    {
        StillLine.Text = $"Paused after {still.After} episodes in a row.";
        StillLayer.Visibility = Visibility.Visible;
        Chrome.Visibility = Visibility.Collapsed;
        ControlsChanged?.Invoke(false);
        ShowPointer();
        if (!paused)
            SendQuietly(new PlayerCommand.TogglePause());
        StillContinue.Focus(FocusState.Programmatic);
    }

    void ContinueWatching()
    {
        still.Input();
        still.Answered();
        StillLayer.Visibility = Visibility.Collapsed;
        if (paused)
            SendQuietly(new PlayerCommand.TogglePause());
        ShowControls();
    }

    void StillStopNow()
    {
        still.Answered();
        StillLayer.Visibility = Visibility.Collapsed;
        Close();
    }

    void OnStillContinue(object sender, RoutedEventArgs e) => ContinueWatching();
    void OnStillStop(object sender, RoutedEventArgs e) => StillStopNow();

#if ANISTREAM_TESTING
    /// <summary>A state to bring up for a screenshot, on the testing build: <c>--show=still|mini|preview</c>.</summary>
    async void Demo(string what)
    {
        await Task.Delay(1200);
        switch (what)
        {
            case "still":
                still.After = 3;
                AskStillWatching();
                break;
            case "mini":
                ToggleMini();
                break;
            case "preview":
                ShowControls();
                PreviewAtFraction(0.4);
                break;
        }
    }
#endif

    // The mini player: the video in a small window that stays on top.

    /// <summary>Asked to go into the mini window (true), or back (false).</summary>
    internal event Action<bool>? MiniRequested;

    void OnMini(object sender, RoutedEventArgs e) => ToggleMini();

    void ToggleMini()
    {
        var on = !mini;
        SetMini(on);
        MiniRequested?.Invoke(on);
        if (on && !miniTipShown)
        {
            miniTipShown = true;
            MiniTip.IsOpen = true;
            _ = Task.Delay(TimeSpan.FromSeconds(6)).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => MiniTip.IsOpen = false));
        }
        ShowControls();
    }

    /// <summary>The controls of the small window: play, skip, the timeline, and the way back.</summary>
    void SetMini(bool on)
    {
        mini = on;
        var big = on ? Visibility.Collapsed : Visibility.Visible;
        BackButton.Visibility = TracksButton.Visibility = EpisodesButton.Visibility = FullscreenButton.Visibility = big;
        Elapsed.Visibility = Remaining.Visibility = VolumeButton.Visibility = SettingsButton.Visibility = big;
        Corner.Visibility = big;
        ShowTitle.FontSize = on ? 14 : 18;
        ShowTitle.LineHeight = on ? 20 : 24;
        EpisodeTitle.Visibility = big;
        TopBar.Padding = on ? new Thickness(12, 6, 140, 6) : new Thickness(8, 8, 148, 8);
        MiniIcon.Glyph = on ? "\uE73F" : "\uE8A7";
        ToolTipService.SetToolTip(MiniButton, on ? "Back to the window (P)" : "Mini player (P)");
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(MiniButton, on ? "Back to the window" : "Mini player");
        if (on)
            ClosePreview();
    }

    // Previews on the timeline.

    internal void NoteLoad(PlayerOp[] ops)
    {
        foreach (var op in ops)
        {
            if (op is PlayerOp.Load load)
            {
                lock (loads)
                    lastLoad = new PreviewSource(load.Url, PreviewSource.Reach(load.Options));
            }
        }
    }

    PreviewSource? Source()
    {
        PreviewSource? now;
        lock (loads)
            now = lastLoad;
        if (thumbnails is { Length: > 0 } url)
            return thumbs ??= new PreviewSource(url, now?.Options ?? []);
        return stream is not null && now is not null && stream.Url == now.Url ? stream : stream = now;
    }

    void OnTimelinePointerMoved(object sender, PointerRoutedEventArgs e)
    {
        if (mini || duration <= 0)
            return;
        var point = e.GetCurrentPoint(Timeline).Position;
        PreviewAtFraction(Math.Clamp((point.X - 10) / Math.Max(1, Timeline.ActualWidth - 20), 0, 1));
    }

    void PreviewAtFraction(double fraction)
    {
        if (duration <= 0 || ctx is null || Source() is not { } from)
            return;
        var seconds = PreviewSource.TimeAt(fraction * (Timeline.ActualWidth - 20), Timeline.ActualWidth - 20, duration);
        preview ??= new SeekPreview(ctx.Session.Engine, PreviewPanel, DispatcherQueue);
        preview.Show(seconds, from, XamlRoot?.RasterizationScale ?? 1.0);
        PreviewTime.Text = AnistreamFfiMethods.PlayClock(seconds);
        var left = Timeline.TransformToVisual(Chrome).TransformPoint(new global::Windows.Foundation.Point(10 + fraction * (Timeline.ActualWidth - 20), 0)).X;
        PreviewBox.Margin = new Thickness(Math.Clamp(left - 84, 8, Math.Max(8, Chrome.ActualWidth - 176)), 0, 0, 8);
        PreviewBox.Visibility = Visibility.Visible;
    }

    void OnTimelineExited(object sender, PointerRoutedEventArgs e) => PreviewBox.Visibility = Visibility.Collapsed;

    void ClosePreview()
    {
        PreviewBox.Visibility = Visibility.Collapsed;
        preview?.Close();
    }

    /// <summary>The top bar's title: what drags the window while the player is up.</summary>
    internal UIElement DragArea => TitleArea;

    /// <summary>Whether the window is full screen now.</summary>
    internal Func<bool>? FullscreenOn { get; set; }
}
