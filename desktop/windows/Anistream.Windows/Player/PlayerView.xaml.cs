using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using VirtualKey = global::Windows.System.VirtualKey;

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

    // Motion and the corner: what is shown, so a fade in progress does not read as a state.
    readonly DispatcherQueueTimer click;
    readonly DispatcherQueueTimer flash;
    bool chromeShown = true, startingShown;
    UIElement? cornerCard;
    bool skipShown, stallShown, upNextShown, skipHover;
    string skipDone = "";
    readonly Stopwatch skipSince = new(), stallSince = new();
    long startingFrom;
    bool startingHeld;
    ulong stallBase;
    Undoable? undo;
    string startingEpisode = "";
    double speed = 1, mutedFrom, wheelSeek;
    bool volumeFlash;
    string timelineStatus = "";

    static readonly double[] Speeds = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

    /// <summary>An automatic skip that can be undone: back to <c>To</c>, for 6 s of playback or 15 s paused.</summary>
    sealed class Undoable(double to, string line)
    {
        public double To { get; } = to;
        public string Line { get; } = line;
        public double Played { get; set; }
        public double Waited { get; set; }
    }

    public PlayerView()
    {
        InitializeComponent();
        click = DispatcherQueue.GetForCurrentThread().CreateTimer();
        click.Interval = TimeSpan.FromMilliseconds(GetDoubleClickTime());
        click.IsRepeating = false;
        click.Tick += (_, _) => Send(new PlayerCommand.TogglePause());
        flash = DispatcherQueue.GetForCurrentThread().CreateTimer();
        flash.Interval = TimeSpan.FromMilliseconds(1200);
        flash.IsRepeating = false;
        flash.Tick += (_, _) => Motion.Hide(Flash);
        Motion.QuickOpacity(SkipCard);
        AutomationProperties.SetLiveSetting(SkipCard, AutomationLiveSetting.Polite);
        // ] and [ have no VirtualKey names.
        foreach (var (key, step) in new[] { ((VirtualKey)221, 1), ((VirtualKey)219, -1) })
        {
            var accelerator = new KeyboardAccelerator { Key = key };
            accelerator.Invoked += (_, e) => Key(e, () => SpeedStep(step));
            KeyboardAccelerators.Add(accelerator);
        }
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
        click.Stop();
        Visibility = Visibility.Collapsed;
        foreach (var layer in new UIElement[] { StillLayer, Starting, SkipSlot, UndoCard, UpNextCard, StallCard, AlertCard, Flash })
            Motion.Set(layer, false);
        startingShown = skipShown = stallShown = upNextShown = false;
        cornerCard = null;
        undo = null;
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
                    if (volumeFlash)
                    {
                        volumeFlash = false;
                        ShowFlash(volume <= 0 ? "Muted" : $"Volume {Math.Round(volume)}%");
                    }
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
                Motion.Set(StillLayer, false);
                still.Answered();
                ShowTitle.Text = s.Title;
                EpisodeTitle.Text = s.EpisodeTitle is { Length: > 0 } t ? $"Episode {s.EpisodeValue} · {t}" : $"Episode {s.EpisodeValue}";
                StartingShow.Text = s.Title;
                startingEpisode = s.EpisodeValue;
                StartingArt.Source = null;
                ctx?.Art.Bind(s.BannerUrl ?? s.CoverUrl, image => StartingArt.Source = image);
                ShowStarting();
                undo = null;
                UpdateCorner();
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
                var part = Words.Sentence(skip.Kind.ToString()).ToLowerInvariant();
                SkipCard.Content = $"Skip {part}";
                skipDone = $"Skipped {part}";
                skipShown = true;
                UpdateCorner();
                break;
            case PlayerUi.SkipCleared:
                skipShown = false;
                UpdateCorner();
                break;
            case PlayerUi.AutoSkipped auto:
                undo = new Undoable(auto.UndoTo, $"Skipped {Words.Sentence(auto.Kind.ToString()).ToLowerInvariant()}");
                UndoLine.Text = undo.Line;
                UpdateCorner();
                break;
            case PlayerUi.UpNext next:
                UpNextTitle.Text = next.Next.Title is { Length: > 0 } nt ? $"Episode {next.Next.Number} · {nt}" : $"Episode {next.Next.Number}";
                upNext = new Coming(next.Auto, next.EndingAt, next.Countdown);
                UpNextStill.Source = null;
                UpNextStillFrame.Visibility = next.Next.Thumbnail is null ? Visibility.Collapsed : Visibility.Visible;
                ctx?.Art.Bind(next.Next.Thumbnail, image => UpNextStill.Source = image);
                still.After = (int)next.StillWatchingAfter;
                ShowUpNext();
                break;
            case PlayerUi.UpNextCleared:
                upNext = null;
                ShowUpNext();
                break;
            case PlayerUi.Stall stall:
                stallBase = stall.Seconds;
                stallSince.Restart();
                StallLine.Text = $"Source is buffering · {stall.Seconds} s";
                stallShown = true;
                UpdateCorner();
                break;
            case PlayerUi.StallCleared:
                stallShown = false;
                UpdateCorner();
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
        Motion.Show(AlertCard);
        Announce(AlertText);
        await Task.Delay(TimeSpan.FromSeconds(4));
        if (AlertText.Text == text)
            Motion.Hide(AlertCard);
    }

    /// <summary>
    /// The corner's one card, by rank: the stall prompt, the Undo note, Up Next, the Skip button.
    /// A change fades the old card out and the new one in (standard) and announces the new one once.
    /// </summary>
    void UpdateCorner()
    {
        UIElement? want = stallShown ? StallCard : undo is not null ? UndoCard : upNextShown ? UpNextCard : skipShown ? SkipSlot : null;
        if (want == cornerCard)
            return;
        if (cornerCard is not null)
            Motion.HideStandard(cornerCard);
        cornerCard = want;
        if (want is null)
            return;
        if (want == SkipSlot)
        {
            skipSince.Restart();
            SkipCard.Opacity = 1;
        }
        Motion.ShowStandard(want);
        Announce(want == StallCard ? StallLine : want == UndoCard ? UndoLine : want == UpNextCard ? UpNextTitle : SkipCard);
    }

    /// <summary>Has a screen reader say what an element now says, once.</summary>
    static void Announce(UIElement element) =>
        FrameworkElementAutomationPeer.CreatePeerForElement(element)?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);

    /// <summary>What a key did, while the controls stay hidden: 1.2 s in the middle of the video.</summary>
    void ShowFlash(string text)
    {
        FlashText.Text = text;
        Motion.Show(Flash);
        Announce(FlashText);
        flash.Stop();
        flash.Start();
    }

    // The starting screen: the episode, then after 3 s the wait, and after 20 s a way out.

    void ShowStarting()
    {
        startingShown = true;
        startingFrom = Stopwatch.GetTimestamp();
        StartingLine.Text = $"Starting episode {startingEpisode}";
        StartingRing.Visibility = Visibility.Visible;
        StartingChoices.Visibility = Visibility.Collapsed;
        Motion.Set(Starting, true);
    }

    void StartingClock()
    {
        if (!startingShown || StartingChoices.Visibility == Visibility.Visible)
            return;
        var waited = Stopwatch.GetElapsedTime(startingFrom).TotalSeconds;
        if (waited >= 20)
        {
            StartingLine.Text = $"Couldn't start episode {startingEpisode}";
            StartingRing.Visibility = Visibility.Collapsed;
            // The source menu, once the player has one; before that, the next stream, as the stall card does.
            var listed = menus is { Sources.Length: > 0 };
            StartingChoose.Content = listed ? "Choose source" : "Switch source";
            StartingChoose.Flyout = listed ? StartingSources : null;
            StartingChoices.Visibility = Visibility.Visible;
            Announce(StartingLine);
            ShowPointer();
        }
        else if (waited >= 3)
            StartingLine.Text = $"Finding a source · {(int)waited} s";
    }

    void OnStartingStop(object sender, RoutedEventArgs e) => Close();

    void OnStartingChoose(object sender, RoutedEventArgs e)
    {
        if (StartingChoose.Flyout is not null)
            return;
        Send(new PlayerCommand.StallSwitch());
        ShowStarting();
    }

    void OnStartingSourcesOpening(object? sender, object e)
    {
        StartingSources.Items.Clear();
        foreach (var source in menus?.Sources ?? [])
        {
            var item = new MenuFlyoutItem { Text = source.Detail is { Length: > 0 } d ? $"{source.Label} · {d}" : source.Label };
            var id = source.Id;
            item.Click += (_, _) =>
            {
                Send(new PlayerCommand.Choose(new PlayerChoice.Source(id)));
                ShowStarting();
            };
            StartingSources.Items.Add(item);
        }
    }

    /// <summary>The corner's clocks, four times a second: the stall count, the Undo window, Skip's dim.</summary>
    void CornerClock()
    {
        if (stallShown)
            StallLine.Text = $"Source is buffering · {stallBase + (ulong)stallSince.Elapsed.TotalSeconds} s";
        if (undo is { } u)
        {
            if (paused)
                u.Waited += 0.25;
            else
                u.Played += 0.25;
            if (u.Played >= 6 || u.Waited >= 15)
            {
                undo = null;
                UpdateCorner();
            }
        }
        if (cornerCard == SkipSlot)
            SkipCard.Opacity = skipHover || skipSince.Elapsed < TimeSpan.FromSeconds(8) ? 1 : 0.7;
    }

    void OnSkipEntered(object sender, PointerRoutedEventArgs e) => skipHover = true;

    void OnSkipExited(object sender, PointerRoutedEventArgs e)
    {
        skipHover = false;
        skipSince.Restart();
    }

    void OnUndo(object sender, RoutedEventArgs e) => Undo();

    void Undo()
    {
        if (undo is not { } u)
            return;
        undo = null;
        Send(new PlayerCommand.UndoSkip(u.To));
        UpdateCorner();
    }

    void ShowState(FfiPlayerState state)
    {
        if (state.HasPicture && startingShown && !startingHeld)
        {
            // The first frame fades in over the starting screen.
            startingShown = false;
            Motion.Hide(Starting);
        }
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
        else if (chromeShown && !idle.IsRunning)
            idle.Start();
        if (state.Position is { } p)
            position = p;
        if (state.Duration is { } d && d > 0)
            duration = d;
        Elapsed.Text = AnistreamFfiMethods.PlayClock(position);
        Remaining.Text = "-" + AnistreamFfiMethods.PlayClock(Math.Max(0, duration - position));
        var status = $"{(paused ? "Paused" : "Playing")}, {Elapsed.Text} of {AnistreamFfiMethods.PlayClock(duration)}";
        if (status != timelineStatus)
        {
            timelineStatus = status;
            AutomationProperties.SetItemStatus(Timeline, status);
        }
        ShowUpNext();
        fromPlayer = true;
        Timeline.Maximum = Math.Max(1, duration);
        Timeline.Value = Math.Min(position, Timeline.Maximum);
        fromPlayer = false;
    }

    // The swapchain and the panel's size.

    void Tick()
    {
        StartingClock();
        CornerClock();
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
        if (!chromeShown)
        {
            chromeShown = true;
            Motion.Show(Chrome);
            ControlsChanged?.Invoke(true);
        }
        idle.Stop();
        if (!paused)
            idle.Start();
    }

    /// <summary>The controls fade out (fade-out), then collapse so the video scans out alone.</summary>
    void HideChrome()
    {
        if (!chromeShown)
            return;
        chromeShown = false;
        Motion.Hide(Chrome);
        PreviewBox.Visibility = Visibility.Collapsed;
        ControlsChanged?.Invoke(false);
    }

    void HideControlsIfIdle()
    {
        if (keepControls || paused || overBar || menuOpen)
            return;
        if (KeyboardInChrome())
        {
            // Someone is on the controls with the keyboard or a screen reader: hold them.
            idle.Start();
            return;
        }
        HideChrome();
        HidePointer();
    }

    /// <summary>Whether keyboard focus is on one of the controls.</summary>
    bool KeyboardInChrome()
    {
        if (XamlRoot is null || FocusManager.GetFocusedElement(XamlRoot) is not Control { FocusState: FocusState.Keyboard } focused)
            return false;
        for (DependencyObject? at = focused; at is not null; at = VisualTreeHelper.GetParent(at))
        {
            if (at == Chrome)
                return true;
        }
        return false;
    }

    [LibraryImport("user32.dll")]
    private static partial uint GetDoubleClickTime();

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
            if (chromeShown)
                HideChrome();
            else
                ShowControls();
            return;
        }
        // A click pauses after the double-click interval, so a double-click is fullscreen only.
        click.Stop();
        click.Start();
    }

    void OnVideoDoubleTapped(object sender, DoubleTappedRoutedEventArgs e)
    {
        click.Stop();
        if (e.OriginalSource == VideoPanel || e.OriginalSource == Chrome)
            FullscreenRequested?.Invoke(null);
    }

    void OnWheel(object sender, PointerRoutedEventArgs e)
    {
        // Volume 2% a notch; a horizontal scroll seeks 0.25 s a step, gathered into whole steps.
        var point = e.GetCurrentPoint(this);
        var notches = point.Properties.MouseWheelDelta / 120.0;
        if (point.Properties.IsHorizontalMouseWheel)
        {
            wheelSeek += notches * 2.5;
            var by = Math.Truncate(wheelSeek / 0.25) * 0.25;
            if (by != 0)
            {
                wheelSeek -= by;
                SeekBy(by);
            }
        }
        else
        {
            volumeFlash = true;
            Send(new PlayerCommand.Volume(notches * 2));
        }
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
        upNextShown = shown;
        UpdateCorner();
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
            item.Click += (_, _) =>
            {
                this.speed = s;
                Send(new PlayerCommand.SpeedTo(s));
            };
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
    void OnKeyBack5(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => SeekBy(-5), show: false);
    void OnKeyForward5(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => SeekBy(5), show: false);
    void OnKeyBack30(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => SeekBy(-30), show: false);
    void OnKeyForward30(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => SeekBy(30), show: false);
    void OnKeyVolumeUp(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => StepVolume(5), show: false);
    void OnKeyVolumeDown(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => StepVolume(-5), show: false);
    void OnKeyMute(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, Mute, show: false);
    void OnKeyUndo(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, Undo, show: false);
    void OnKeySpeedNormal(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => SpeedTo(1));
    void OnKeyEpisodes(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => OpenMenu(EpisodesButton));
    void OnKeyTracks(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => OpenMenu(TracksButton));

    /// <summary>A seek by key or scroll, with the time it lands on in the flash.</summary>
    void SeekBy(double seconds)
    {
        Send(new PlayerCommand.Seek(seconds));
        ShowFlash(AnistreamFfiMethods.PlayClock(Math.Clamp(position + seconds, 0, Math.Max(0, duration))));
    }

    void StepVolume(double by)
    {
        volumeFlash = true;
        Send(new PlayerCommand.Volume(by));
    }

    /// <summary>M: to silence and back to where it was.</summary>
    void Mute()
    {
        volumeFlash = true;
        if (volume > 0)
        {
            mutedFrom = volume;
            Send(new PlayerCommand.Volume(-volume));
        }
        else
            Send(new PlayerCommand.Volume(mutedFrom > 0 ? mutedFrom : 100));
    }

    /// <summary>] and [: a step along the speeds the Speed menu lists.</summary>
    void SpeedStep(int step)
    {
        var at = Array.FindIndex(Speeds, x => x >= speed - 0.01);
        at = at < 0 ? Speeds.Length - 1 : at;
        if (step < 0 && Speeds[at] > speed + 0.01)
            at--;
        SpeedTo(Speeds[Math.Clamp(at + step, 0, Speeds.Length - 1)]);
    }

    void SpeedTo(double to)
    {
        speed = to;
        Send(new PlayerCommand.SpeedTo(to));
        ShowFlash(to == 1 ? "Normal speed" : $"{to.ToString(CultureInfo.CurrentCulture)}×");
    }

    void OpenMenu(Button button)
    {
        if (button.Visibility == Visibility.Visible && button.IsEnabled)
            button.Flyout?.ShowAt(button);
    }
    void OnKeyFullscreen(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => FullscreenRequested?.Invoke(null));
    void OnKeySkip(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (skipShown)
            ShowFlash(skipDone);
        Send(new PlayerCommand.TakeSkip());
    }, show: false);
    void OnKeyNext(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Step(1)));
    void OnKeyPrevious(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => Send(new PlayerCommand.Step(-1)));
    void OnKeyMini(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () => ToggleMini());
    void OnKeyClose(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, Close, show: false);

    void OnKeyEnter(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (still.Asking)
            ContinueWatching();
        else if (cornerCard == StallCard)
            Send(new PlayerCommand.StallSwitch());
        else if (cornerCard == UndoCard)
            Undo();
        else if (cornerCard == UpNextCard)
            PlayNext(auto: false);
        else if (cornerCard == SkipSlot)
            Send(new PlayerCommand.TakeSkip());
    });

    /// <summary>Esc: cancel Up Next or the stall prompt, then leave fullscreen, then close.</summary>
    void OnKeyEscape(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (still.Asking)
            StillStopNow();
        else if (mini)
            ToggleMini();
        else if (cornerCard == UpNextCard)
            CancelNext();
        else if (cornerCard == StallCard)
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
        Motion.Show(StillLayer);
        HideChrome();
        ShowPointer();
        if (!paused)
            SendQuietly(new PlayerCommand.TogglePause());
        StillContinue.Focus(FocusState.Programmatic);
    }

    void ContinueWatching()
    {
        still.Input();
        still.Answered();
        Motion.Hide(StillLayer);
        if (paused)
            SendQuietly(new PlayerCommand.TogglePause());
        ShowControls();
    }

    void StillStopNow()
    {
        still.Answered();
        Motion.Set(StillLayer, false);
        Close();
    }

    void OnStillContinue(object sender, RoutedEventArgs e) => ContinueWatching();
    void OnStillStop(object sender, RoutedEventArgs e) => StillStopNow();

#if ANISTREAM_TESTING
    /// <summary>
    /// A state to bring up for a screenshot, on the testing build:
    /// <c>--show=still|mini|preview|undo|stall|upnext|skip|starting|flash</c>.
    /// </summary>
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
            case "undo":
                undo = new Undoable(0, "Skipped opening") { Played = -600, Waited = -600 };
                UndoLine.Text = undo.Line;
                UpdateCorner();
                break;
            case "stall":
                stallBase = 9;
                stallSince.Restart();
                stallShown = true;
                UpdateCorner();
                break;
            case "upnext":
                UpNextTitle.Text = "Episode 2";
                UpNextStillFrame.Visibility = Visibility.Visible;
                upNext = new Coming(false, position, 0);
                ShowUpNext();
                break;
            case "skip":
                SkipCard.Content = "Skip opening";
                skipShown = true;
                UpdateCorner();
                break;
            case "starting":
                startingEpisode = "1";
                startingHeld = true;
                ShowStarting();
                startingFrom -= 21 * Stopwatch.Frequency;
                break;
            case "flash":
                flash.Interval = TimeSpan.FromSeconds(600);
                ShowFlash("12:04");
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
