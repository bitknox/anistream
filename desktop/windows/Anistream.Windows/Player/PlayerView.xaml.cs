using System.Diagnostics;
using System.Globalization;
using System.Runtime.InteropServices;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Anistream.Windows.Discussion;
using Anistream.Windows.Pages;
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
    /// <summary>The file's audio and subtitle tracks, as mpv last listed them.</summary>
    FfiTrack[] tracks = [];
    string tracksSeen = "";
#if ANISTREAM_TESTING
    bool tracksHeld;
#endif
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
    string startingEpisode = "", playingEpisode = "";
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
        Timeline.Spoken = () => T.PlayerPositionSpoken(SpokenClock(position), SpokenClock(duration));
        VolumeSlider.Spoken = () => T.PlayerVolumeSpoken((long)Math.Round(VolumeSlider.Value));
        ToolTips();
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
        // Where focus was, for the way back when the player closes.
        returnFocus = XamlRoot is null ? null : FocusManager.GetFocusedElement(XamlRoot) as Control;
        Visibility = Visibility.Visible;
        still.Reset();
        ShowControls();
        LiftSubtitles(animated: false);
        tick.Start();
        ShowStats();
        Focus(FocusState.Programmatic);
        ShowingChanged?.Invoke(true);
    }

    Control? returnFocus;

    /// <summary>After the player closed: focus goes back to the control that opened it, when it is still there.</summary>
    internal bool RestoreFocus()
    {
        var back = returnFocus;
        returnFocus = null;
        return back is { XamlRoot: not null, IsLoaded: true } && back.Focus(FocusState.Programmatic);
    }

    internal void Hide()
    {
        if (!Showing)
            return;
        discussion?.Close();
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
        foreach (var layer in new UIElement[] { StillLayer, Starting, SkipSlot, UndoCard, UpNextCard, StallCard, AlertCard, Flash, StatsLayer })
            Motion.Set(layer, false);
        // Kept up for the next playback, which waits for its own figures.
        stats.Restart();
        statsUp = false;
        startingShown = skipShown = stallShown = upNextShown = false;
        cornerCard = null;
        undo = null;
        upNext = null;
        thumbnails = null;
        thumbs = null;
        timelineMarks = [];
        buffered = [];
        DrawTimeline();
        DropSubtitleLift();
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
                case Event.PartyJoined or Event.PartyLeft:
                    ShowPartyButton();
                    break;
                case Event.TitleDub td when td.Id == playingId:
                    BuildTracks();
                    break;
                case Event.PlayerTimeline t:
                    timelineMarks = t.Marks ?? [];
                    DrawTimeline();
                    break;
                case Event.PlayerBuffered b:
                    buffered = b.Ranges ?? [];
                    DrawTimeline();
                    break;
                case Event.PlayerStats ps:
                    TookStats(ps.Stats);
                    break;
                case Event.Clipboard clip:
                    TookClipboard(clip.Text);
                    break;
                case Event.PlaybackVolume v:
                    volume = v.V1;
                    fromPlayer = true;
                    VolumeSlider.Value = Math.Clamp(volume, 0, 100);
                    fromPlayer = false;
                    VolumeIcon.Glyph = volume <= 0 ? "" : "";
                    AutomationProperties.SetName(VolumeButton, volume <= 0 ? T.PlayerVolumeMuted : T.PlayerVolume);
                    if (volumeFlash)
                    {
                        volumeFlash = false;
                        ShowFlash(volume <= 0 ? T.PlayerMuted : T.PlayerVolumeLevel(Wording.Percent(Math.Round(volume))));
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
                // Whether the show has a dub decides whether Tracks offers Sub and Dub; asked once
                // a session for a show whose page has not already asked.
                playingId = s.Id;
                if (ctx is { } c && !c.Known.Dubbed.ContainsKey(s.Id))
                    c.Send(new Command.LoadDub(s.Id));
                ShowTitle.Text = s.Title;
                EpisodeTitle.Text = Lines.EpisodeNamed(s.EpisodeValue, s.EpisodeTitle);
                StartingShow.Text = s.Title;
                startingEpisode = playingEpisode = s.EpisodeValue;
                ShowTvButton();
                StartingArt.Source = null;
                ctx?.Art.Bind(s.BannerUrl ?? s.CoverUrl, image => StartingArt.Source = image);
                ShowStarting();
                undo = null;
                UpdateCorner();
                break;
            case PlayerUi.Episode ep:
                playingEpisode = ep.Number;
                ShowTitle.Text = ep.Show;
                EpisodeTitle.Text = Lines.Episode(ep.Label, ep.Title, ep.Translation);
                break;
            case PlayerUi.Menus m:
                menus = m;
                BuildMenus();
                break;
            case PlayerUi.Skip skip:
                SkipCard.Content = skip.Kind == SkipKind.Opening ? T.PlayerSkipOpening : T.PlayerSkipEnding;
                skipDone = Skipped(skip.Kind);
                skipShown = true;
                UpdateCorner();
                break;
            case PlayerUi.SkipCleared:
                skipShown = false;
                UpdateCorner();
                break;
            case PlayerUi.AutoSkipped auto:
                undo = new Undoable(auto.UndoTo, Skipped(auto.Kind));
                UndoLine.Text = undo.Line;
                UpdateCorner();
                break;
            case PlayerUi.UpNext when HoldUpNext:
                break;
            case PlayerUi.UpNext next:
                UpNextTitle.Text = Lines.EpisodeNamed(next.Next.Number, next.Next.Title);
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
                StallLine.Text = T.PlayerStallTitle((long)stall.Seconds);
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

    /// <summary>
    /// A screenshot run's: no Up next card, for a fixture clip short enough to be near its end
    /// from the start.
    /// </summary>
    internal bool HoldUpNext { get; set; }

    /// <summary>A line for the viewer over the video, where the window's toasts can't be seen.</summary>
    internal void Say(string text) => Alert(text);

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
    /// The corner's one card, by rank: the stall prompt, the Undo note, Up next, the Skip button.
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
        if (want == UpNextCard && DeviceWatch.ScreenReaderRunning())
            DispatcherQueue.TryEnqueue(() => UpNextCancel.Focus(FocusState.Programmatic));
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
        StartingLine.Text = Lines.Starting(startingEpisode);
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
            StartingLine.Text = Lines.StartFailed(startingEpisode);
            StartingRing.Visibility = Visibility.Collapsed;
            // The source menu, once the player has one; before that, the next stream, as the stall card does.
            var listed = menus is { Sources.Length: > 0 };
            StartingChoose.Content = listed ? T.PlayerChooseSource : T.PlayerSwitchSource;
            StartingChoose.Flyout = listed ? StartingSources : null;
            StartingChoices.Visibility = Visibility.Visible;
            Announce(StartingLine);
            ShowPointer();
        }
        else if (waited >= 3)
            StartingLine.Text = T.PlayerFindingSource((long)waited);
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
            StallLine.Text = T.PlayerStallTitle((long)(stallBase + (ulong)stallSince.Elapsed.TotalSeconds));
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

    // The seek bar's track: silk at a quarter for the length, at under half for what is
    // buffered, amber for what has played, and water on top for the opening and ending, so the
    // marks still read once played past (native-ux.md, the timeline).

    TimelineMark[] timelineMarks = [];
    uint playingId;
    TimeRange[] buffered = [];

    void OnTimelineSized(object sender, SizeChangedEventArgs e) => DrawTimeline();

    void DrawTimeline()
    {
        TimelineUnder.Children.Clear();
        if (Find(Timeline, "HorizontalTrackRect") is not FrameworkElement track || track.ActualWidth <= 0)
            return;
        var thumb = Find(Timeline, "HorizontalThumb") as FrameworkElement;
        var at = track.TransformToVisual(TimelineUnder).TransformPoint(default);
        var width = track.ActualWidth;
        var height = Math.Max(2, track.ActualHeight);
        // The slider's value runs between the thumb's centre at either end.
        var half = (thumb?.ActualWidth ?? 0) / 2;
        var usable = Math.Max(1, width - 2 * half);
        double X(double seconds) => at.X + half + Math.Clamp(duration > 0 ? seconds / duration : 0, 0, 1) * usable;
        var silk = (Brush)Application.Current.Resources["SilkBrush"];
        var water = (Brush)Application.Current.Resources["WaterBrush"];
        void Bar(double from, double to, Brush brush, double opacity)
        {
            if (to - from < 1)
                return;
            var bar = new Microsoft.UI.Xaml.Shapes.Rectangle
            {
                Width = to - from, Height = height, Fill = brush, Opacity = opacity,
                RadiusX = height / 2, RadiusY = height / 2,
            };
            Canvas.SetLeft(bar, from);
            Canvas.SetTop(bar, at.Y);
            TimelineUnder.Children.Add(bar);
        }
        Bar(at.X, at.X + width, silk, 0.25);
        playedBar = null;
        if (duration <= 0)
            return;
        foreach (var range in buffered)
            Bar(X(range.Start), X(range.End), silk, 0.45);
        // What has played, from the track's start to the thumb, kept to move with the value.
        playedBar = new Microsoft.UI.Xaml.Shapes.Rectangle
        {
            Height = height, Fill = (Brush)Application.Current.Resources["AmberBrush"],
            RadiusX = height / 2, RadiusY = height / 2,
        };
        Canvas.SetLeft(playedBar, at.X);
        Canvas.SetTop(playedBar, at.Y);
        TimelineUnder.Children.Add(playedBar);
        playedFrom = at.X;
        playedTo = X;
        ShowPlayed();
        // The opening and ending go on top, so they read wherever the viewer is.
        foreach (var mark in timelineMarks)
            Bar(X(mark.Start), X(mark.End), water, 1);
    }

    Microsoft.UI.Xaml.Shapes.Rectangle? playedBar;
    double playedFrom;
    Func<double, double> playedTo = _ => 0;

    void ShowPlayed()
    {
        if (playedBar is not null)
            playedBar.Width = Math.Max(0, playedTo(Timeline.Value) - playedFrom);
    }

    static DependencyObject? Find(DependencyObject parent, string name)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is FrameworkElement { Name: var n } && n == name)
                return child;
            if (Find(child, name) is { } found)
                return found;
        }
        return null;
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
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(PlayPause, paused ? T.ActionPlay : T.ActionPause);
        ToolTipService.SetToolTip(PlayPause, T.TooltipWithKey(paused ? T.ActionPlay : T.ActionPause, "Space"));
        if (paused)
            ShowControls();
        else if (chromeShown && !idle.IsRunning)
            idle.Start();
        if (state.Position is { } p)
            position = p;
        if (state.Duration is { } d && d > 0 && d != duration)
        {
            duration = d;
            DrawTimeline();
        }
        Elapsed.Text = AnistreamFfiMethods.PlayClock(position);
        Remaining.Text = "-" + AnistreamFfiMethods.PlayClock(Math.Max(0, duration - position));
        var total = AnistreamFfiMethods.PlayClock(duration);
        var status = paused ? T.PlayerTimelinePaused(Elapsed.Text, total) : T.PlayerTimelinePlaying(Elapsed.Text, total);
        if (status != timelineStatus)
        {
            timelineStatus = status;
            AutomationProperties.SetItemStatus(Timeline, status);
        }
        ShowTracks(state.Tracks);
        ShowUpNext();
        fromPlayer = true;
        Timeline.Maximum = Math.Max(1, duration);
        // While the keyboard is on the slider the thumb is left to the person, so the screen
        // reader does not hear it change under them. A jump of more than a few seconds still lands.
        if (Timeline.FocusState != FocusState.Keyboard || Math.Abs(Timeline.Value - position) > 5)
            Timeline.Value = Math.Min(position, Timeline.Maximum);
        fromPlayer = false;
    }

    // The swapchain and the panel's size.

    void Tick()
    {
        StartingClock();
        CornerClock();
        // The panel's timed states: the dashes, the waiting line, "Copied" going.
        if (stats.Shown)
            ShowStats();
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

    (uint W, uint H, double Scale)? panelAsked, panelSent;
    Microsoft.UI.Dispatching.DispatcherQueueTimer? panelPace;

    /// <summary>
    /// A drag of the window's edge sends a size for every step of it, and each one makes mpv
    /// rebuild its swapchain and draw again. The first size goes at once, then at most one
    /// every 50 ms, and the last always; between them the panel stretches the last frame.
    /// </summary>
    void OnPanelSizeChanged(object sender, SizeChangedEventArgs e)
    {
        if (player is null || e.NewSize.Width < 1 || e.NewSize.Height < 1)
            return;
        var scale = XamlRoot?.RasterizationScale ?? 1.0;
        panelAsked = ((uint)Math.Max(1, Math.Round(e.NewSize.Width * scale)), (uint)Math.Max(1, Math.Round(e.NewSize.Height * scale)), scale);
        if (panelPace is null)
        {
            panelPace = DispatcherQueue.CreateTimer();
            panelPace.Interval = TimeSpan.FromMilliseconds(50);
            panelPace.IsRepeating = false;
            panelPace.Tick += (_, _) => SendPanelSize();
        }
        if (panelPace.IsRunning)
            return;
        SendPanelSize();
    }

    void SendPanelSize()
    {
        if (player is null || panelAsked is not { } size || size == panelSent)
            return;
        panelSent = size;
        player.CompositionSize(size.W, size.H);
        player.Display(size.W, size.H, size.Scale, null);
        LiftSubtitles(animated: false);
        panelPace?.Start();
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
            LiftSubtitles();
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
        LiftSubtitles();
        PreviewBox.Visibility = Visibility.Collapsed;
        ControlsChanged?.Invoke(false);
    }

    void HideControlsIfIdle()
    {
        if (keepControls || paused || overBar || menuOpen)
            return;
        if (ControlsInUse())
        {
            // A screen reader is running, or a control has focus by any means: hold them.
            idle.Start();
            return;
        }
        HideChrome();
        HidePointer();
    }

    /// <summary>
    /// Whether the controls must stay: a screen reader runs (its cursor moves without focus, and
    /// controls that collapse leave the tree), or a control has focus in any state.
    /// </summary>
    bool ControlsInUse()
    {
        if (DeviceWatch.ScreenReaderRunning())
            return true;
        if (XamlRoot is null || FocusManager.GetFocusedElement(XamlRoot) is not Control { FocusState: not FocusState.Unfocused } focused)
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

    // Watch party.

    PartyPanel? partyPanel;

    void OnParty(object sender, RoutedEventArgs e) => ShowParty();

    /// <summary>The party's panel when in one; the form that starts one when not.</summary>
    internal void ShowParty()
    {
        if (ctx is null)
            return;
        if (ctx.Known.Party is null)
        {
            ctx.Window.ShowParty(new PartyDialog.For.Playing());
            return;
        }
        if (partyPanel is null)
        {
            partyPanel = new PartyPanel(ctx);
            partyPanel.Flyout.Opened += OnMenuOpened;
            partyPanel.Flyout.Closed += OnMenuClosed;
        }
        ShowControls();
        partyPanel.Flyout.ShowAt(PartyButton);
    }

    /// <summary>Play on TV, in the big player only, once the engine gives a sender.</summary>
    void ShowTvButton() =>
        TvButton.Visibility = ctx?.Tv.Available == true && !mini ? Visibility.Visible : Visibility.Collapsed;

    /// <summary>
    /// Play on TV from here: paused here, then the TV to choose, or straight to the one driven
    /// already. Once the TV plays it the window closes the player.
    /// </summary>
    void OnPlayOnTv(object sender, RoutedEventArgs e)
    {
        if (ctx is not { } c || playingId == 0 || playingEpisode.Length == 0)
            return;
        if (!paused)
            Send(new PlayerCommand.TogglePause());
        c.Tv.Offer(new TvPlay(playingId, ShowTitle.Text, playingEpisode, position > 5 ? position : null, FromPlayer: true));
    }

    static string Skipped(SkipKind kind) => kind == SkipKind.Opening ? T.PlayerSkippedOpening : T.PlayerSkippedEnding;

    /// <summary>The buttons' tooltips, each with its key: "Episodes (E)".</summary>
    void ToolTips()
    {
        ToolTipService.SetToolTip(BackButton, T.TooltipWithKey(T.ActionBack, "Alt+Left"));
        ToolTipService.SetToolTip(TracksButton, T.TooltipWithKey(T.PlayerSubtitlesAndAudio, "T"));
        ToolTipService.SetToolTip(EpisodesButton, T.TooltipWithKey(T.PlayerEpisodes, "E"));
        ToolTipService.SetToolTip(MiniButton, T.TooltipWithKey(T.PlayerMiniPlayer, "P"));
        ToolTipService.SetToolTip(FullscreenButton, T.TooltipWithKey(T.PlayerFullscreen, "F"));
        ToolTipService.SetToolTip(PlayPause, T.TooltipWithKey(T.ActionPause, "Space"));
        ToolTipService.SetToolTip(Back10Button, T.TooltipWithKey(T.PlayerBack10, "J"));
        ToolTipService.SetToolTip(Forward10Button, T.TooltipWithKey(T.PlayerForward10, "L"));
        ToolTipService.SetToolTip(StatsClose, T.TooltipWithKey(T.ActionClose, "I"));
        ToolTipService.SetToolTip(UndoButton, T.TooltipWithKey(T.ActionUndo, "U"));
        MiniTip.Subtitle = T.PlayerMiniTip("P");
    }

    void ShowPartyButton()
    {
        var inParty = ctx?.Known.Party is not null;
        PartyDot.Opacity = inParty ? 1 : 0;
        ToolTipService.SetToolTip(PartyButton, T.PartyTitle);
        AutomationProperties.SetName(PartyButton, T.PartyTitle);
    }

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
    /// Up next as the engine hands it over: the card shows from where the ending starts (or near
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
        // With a screen reader running the card waits for a choice instead of counting down.
        var left = DeviceWatch.ScreenReaderRunning() ? null : card.Remaining(position, duration);
        UpNextBar.Visibility = left is null ? Visibility.Collapsed : Visibility.Visible;
        UpNextLine.Visibility = UpNextBar.Visibility;
        if (left is { } l)
        {
            UpNextBar.Value = l / card.Countdown;
            UpNextLine.Text = T.PlayerPlaysIn((long)Math.Ceiling(l));
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

    internal void ShowFullscreen(bool on)
    {
        FullscreenIcon.Glyph = on ? "" : "";
        var name = on ? T.PlayerExitFullscreen : T.PlayerFullscreen;
        AutomationProperties.SetName(FullscreenButton, name);
        ToolTipService.SetToolTip(FullscreenButton, T.TooltipWithKey(name, "F"));
    }

    /// <summary>A time said in words for a screen reader: "12 minutes 4 seconds".</summary>
    static string SpokenClock(double seconds)
    {
        var total = (long)Math.Max(0, Math.Floor(seconds));
        var parts = new List<string>(3);
        if (total / 3600 > 0)
            parts.Add(T.PlayerSpokenHours(total / 3600));
        if (total % 3600 / 60 > 0)
            parts.Add(T.PlayerSpokenMinutes(total % 3600 / 60));
        if (total % 60 > 0 || parts.Count == 0)
            parts.Add(T.PlayerSpokenSeconds(total % 60));
        return string.Join(" ", parts);
    }

    void OnTimelineChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        ShowPlayed();
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

    /// <summary>mpv listed the file's tracks again: rebuild the menu when what it offers changed.</summary>
    void ShowTracks(FfiTrack[] now)
    {
#if ANISTREAM_TESTING
        if (tracksHeld)
            return;
#endif
        var seen = string.Join(";", now.Select(t => $"{t.Kind}{t.Id}{(t.Selected ? "*" : "")}{t.Language}{t.Title}"));
        if (seen == tracksSeen)
            return;
        tracksSeen = seen;
        tracks = now;
        BuildTracks();
    }

    /// <summary>
    /// The Tracks menu: Sub or Dub at the top when the show has both, then the file's own
    /// subtitles (Off first) and audio, each by its language and title. A pick goes to the
    /// engine, which applies it and keeps it for the series.
    /// </summary>
    void BuildTracks()
    {
        TracksMenu.Items.Clear();
        // Both, when the source says the show has a dub, or the dub is what plays.
        if (menus is { } m && (m.Translation == Engine.Core.Translation.Dub || ctx?.Known.Dubbed.GetValueOrDefault(playingId) == true))
            foreach (var (translation, label) in new[] { (Engine.Core.Translation.Sub, T.AudioSubbed), (Engine.Core.Translation.Dub, T.AudioDubbed) })
            {
                var item = new RadioMenuFlyoutItem { Text = label, GroupName = "translation", IsChecked = m.Translation == translation };
                item.Click += (_, _) => Send(new PlayerCommand.Choose(new PlayerChoice.Translation(translation)));
                TracksMenu.Items.Add(item);
            }
        var subtitles = tracks.Where(t => t.Kind == TrackKind.Subtitle).ToArray();
        var audio = tracks.Where(t => t.Kind == TrackKind.Audio).ToArray();
        if (subtitles.Length == 0 && audio.Length == 0)
            return;
        if (TracksMenu.Items.Count > 0)
            TracksMenu.Items.Add(new MenuFlyoutSeparator());

        var subs = new MenuFlyoutSubItem { Text = T.PlayerSubtitles };
        var off = new RadioMenuFlyoutItem { Text = T.ValueOff, GroupName = "subtitles", IsChecked = !subtitles.Any(t => t.Selected) };
        off.Click += (_, _) => Send(new PlayerCommand.ChooseTrack(TrackKind.Subtitle, null));
        subs.Items.Add(off);
        foreach (var track in subtitles)
            subs.Items.Add(TrackItem(track, "subtitles"));
        subs.Items.Add(new MenuFlyoutSeparator());
        subs.Items.Add(LoadItem(TrackKind.Subtitle));
        TracksMenu.Items.Add(subs);

        var sound = new MenuFlyoutSubItem { Text = T.PlayerAudio };
        foreach (var track in audio)
            sound.Items.Add(TrackItem(track, "audio"));
        if (audio.Length > 0)
            sound.Items.Add(new MenuFlyoutSeparator());
        sound.Items.Add(LoadItem(TrackKind.Audio));
        TracksMenu.Items.Add(sound);
    }

    MenuFlyoutItem LoadItem(TrackKind kind)
    {
        var item = new MenuFlyoutItem { Text = kind == TrackKind.Subtitle ? T.PlayerLoadSubtitleFile : T.PlayerLoadAudioFile };
        item.Click += (_, _) => LoadTrack(kind);
        return item;
    }

    static readonly string[] SubtitleFiles = [".srt", ".ass", ".ssa", ".vtt", ".sub", ".sup"];
    static readonly string[] AudioFiles = [".mka", ".m4a", ".aac", ".ac3", ".eac3", ".flac", ".mp3", ".ogg", ".opus", ".wav"];

    /// <summary>
    /// Load subtitle file… or Load audio file…: the system's file picker, then mpv adds the file
    /// as a track and selects it. mpv's alone, so the next episode does not carry it. Cancelling
    /// the picker says nothing.
    /// </summary>
    async void LoadTrack(TrackKind kind)
    {
        if (ctx is null || player is not { } p)
            return;
        var subtitle = kind == TrackKind.Subtitle;
        string? path;
        try
        {
            var picker = new Microsoft.Windows.Storage.Pickers.FileOpenPicker(ctx.Window.AppWindow.Id)
            {
                CommitButtonText = subtitle ? T.PlayerLoadSubtitlesCommit : T.PlayerLoadAudioCommit,
                SuggestedStartLocation = Microsoft.Windows.Storage.Pickers.PickerLocationId.Downloads,
            };
            foreach (var extension in subtitle ? SubtitleFiles : AudioFiles)
                picker.FileTypeFilter.Add(extension);
            path = (await picker.PickSingleFileAsync())?.Path;
        }
        catch (Exception e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"the file picker failed: {e.Message}");
            Alert(T.PlayerFilePickerFailed);
            return;
        }
        if (path is not { Length: > 0 })
            return;
        try
        {
            // mpv reads the file before it answers: off the UI thread.
            await Task.Run(() => p.LoadTrack(kind, path));
        }
        catch (FfiException e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "windows", $"mpv did not load the track: {e.Message}");
            Alert(subtitle ? T.PlayerSubtitleFileFailed : T.PlayerAudioFileFailed);
        }
        ShowControls();
    }

    RadioMenuFlyoutItem TrackItem(FfiTrack track, string group)
    {
        var item = new RadioMenuFlyoutItem
        {
            Text = TrackLabel(track),
            GroupName = group,
            IsChecked = track.Selected,
        };
        var (kind, id) = (track.Kind, track.Id);
        item.Click += (_, _) => Send(new PlayerCommand.ChooseTrack(kind, id));
        return item;
    }

    /// <summary>"English · Signs &amp; Songs": the language, then the track's own words when it has any.</summary>
    static string TrackLabel(FfiTrack track)
    {
        var name = Wording.TrackName(track.Language, track.Title ?? "");
        return track.External ? T.PlayerTrackAdded(name) : name;
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
                Text = Lines.Episode(ep.Label, ep.Title, null),
                IsChecked = ep.Current,
            };
            var number = ep.Number;
            item.Click += (_, _) => Send(new PlayerCommand.Choose(new PlayerChoice.Episode(number)));
            EpisodesMenu.Items.Add(item);
        }
        EpisodesButton.IsEnabled = menus.Episodes.Length > 0;

        BuildTracks();

        SettingsMenu.Items.Clear();
        if (menus.Sources.Length > 0)
        {
            var sources = new MenuFlyoutSubItem { Text = T.PlayerSources };
            foreach (var source in menus.Sources)
            {
                var item = new RadioMenuFlyoutItem { Text = source.Detail is { Length: > 0 } d ? $"{source.Label} · {d}" : source.Label, GroupName = "source", IsChecked = source.Current };
                var id = source.Id;
                item.Click += (_, _) => Send(new PlayerCommand.Choose(new PlayerChoice.Source(id)));
                sources.Items.Add(item);
            }
            SettingsMenu.Items.Add(sources);
        }
        var upscaling = new MenuFlyoutSubItem { Text = T.PlayerUpscaling };
        foreach (var choice in menus.Upscaling.Options ?? [])
        {
            var item = new RadioMenuFlyoutItem { Text = choice.Label, GroupName = "upscaling", IsChecked = choice.Id == menus.Upscaling.Current };
            var id = choice.Id;
            item.Click += (_, _) => Send(new PlayerCommand.Upscaling(id));
            upscaling.Items.Add(item);
        }
        SettingsMenu.Items.Add(upscaling);
        var speed = new MenuFlyoutSubItem { Text = T.PlayerSpeed };
        foreach (var s in new[] { 0.75, 1.0, 1.25, 1.5, 2.0 })
        {
            var item = new MenuFlyoutItem { Text = s == 1 ? T.PlayerSpeedNormal : T.PlayerSpeedValue(Wording.Number(s)) };
            item.Click += (_, _) =>
            {
                this.speed = s;
                Send(new PlayerCommand.SpeedTo(s));
            };
            speed.Items.Add(item);
        }
        SettingsMenu.Items.Add(speed);
        var night = new ToggleMenuFlyoutItem { Text = T.PlayerNightMode, IsChecked = menus.NightMode };
        night.Click += (_, _) => Send(new PlayerCommand.NightMode(night.IsChecked));
        SettingsMenu.Items.Add(night);
        SettingsMenu.Items.Add(new MenuFlyoutSeparator());
        SettingsMenu.Items.Add(StatsItem());
    }

    // The episode's discussion: Discussion in the More menu from 85% of the episode, once the engine found a thread.

    DiscussionPane? discussion;

    static bool Within(DependencyObject element, DependencyObject ancestor)
    {
        for (var at = element; at is not null; at = VisualTreeHelper.GetParent(at))
            if (at == ancestor)
                return true;
        return false;
    }

    void OnSettingsMenuOpening(object? sender, object e)
    {
        if (SettingsMenu.Items.FirstOrDefault(i => i.Tag as string == "discussion") is { } old)
            SettingsMenu.Items.Remove(old);
        if (ctx is not { } c || !uint.TryParse(playingEpisode, out var episode) || !Discussions.Counted(position, duration)
            || !c.Known.Discussions.ContainsKey((playingId, episode)))
            return;
        var item = new MenuFlyoutItem { Text = T.DiscussionOpen, Tag = "discussion" };
        item.Click += (_, _) => OpenDiscussion(episode);
        SettingsMenu.Items.Insert(0, item);
        if (SettingsMenu.Items.Count > 1 && SettingsMenu.Items[1] is not MenuFlyoutSeparator)
            SettingsMenu.Items.Insert(1, new MenuFlyoutSeparator { Tag = "discussion" });
    }

    /// <summary>Opens the discussion beside the video. Playback pauses and stays paused: the viewer chooses.</summary>
    internal void OpenDiscussion(uint episode)
    {
        if (ctx is not { } c || playingId == 0)
            return;
        discussion?.Close();
        if (!paused)
            Send(new PlayerCommand.TogglePause());
        // The window's caption buttons float over the player; the pane starts below them.
        var pane = new DiscussionPane(c, playingId, episode) { Padding = new Thickness(0, 40, 0, 0) };
        discussion = pane;
        pane.Closed += () =>
        {
            if (discussion != pane)
                return;
            discussion = null;
            Root.Children.Remove(pane);
            if (discussionScrim is not null)
                Root.Children.Remove(discussionScrim);
            discussionScrim = null;
            Root.ColumnDefinitions[1].Width = GridLength.Auto;
            Focus(FocusState.Programmatic);
        };
        Root.Children.Add(pane);
        PlaceDiscussion();
        pane.Start();
    }

    Microsoft.UI.Xaml.Shapes.Rectangle? discussionScrim;

    /// <summary>Beside the video from 1000 epx of window; over it, with a light dismiss, below.</summary>
    void PlaceDiscussion()
    {
        if (discussion is not { } pane)
            return;
        var window = XamlRoot?.Size.Width ?? Root.ActualWidth;
        var over = window > 0 && window < DiscussionPane.OverlayBelow;
        if (over)
        {
            Grid.SetColumn(pane, 0);
            Grid.SetColumnSpan(pane, 2);
            if (discussionScrim is null)
            {
                discussionScrim = new Microsoft.UI.Xaml.Shapes.Rectangle { Fill = (Brush)Application.Current.Resources["ScrimBrush"] };
                Grid.SetColumnSpan(discussionScrim, 2);
                discussionScrim.Tapped += (_, e) =>
                {
                    e.Handled = true;
                    discussion?.Close();
                };
                Root.Children.Insert(Root.Children.IndexOf(pane), discussionScrim);
            }
        }
        else
        {
            if (discussionScrim is not null)
                Root.Children.Remove(discussionScrim);
            discussionScrim = null;
            Grid.SetColumn(pane, 1);
            Grid.SetColumnSpan(pane, 1);
        }
    }

    // Keys: arrows and S act without showing the controls; every other key shows them.

    void Key(KeyboardAcceleratorInvokedEventArgs e, Action act, bool show = true)
    {
        if (!Showing)
            return;
        // Typing in the discussion's field, or pressing a key on one of its buttons, is not a player key.
        if (discussion is { } pane && XamlRoot is { } root && FocusManager.GetFocusedElement(root) is DependencyObject focus && Within(focus, pane))
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
        // Named for the playback on screen, so the engine lets a seek go once the next episode
        // has taken over instead of carrying it there.
        ulong now;
        lock (loads)
            now = playback;
        if (now == 0)
            Send(new PlayerCommand.Seek(seconds));
        else
        {
            still.Input();
            ctx?.Send(new Command.Control(now, new PlayerCommand.Seek(seconds)));
        }
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
        ShowFlash(to == 1 ? T.PlayerSpeedNormal : T.PlayerSpeedValue(Wording.Number(to)));
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

    /// <summary>Esc: cancel Up next or the stall prompt, then leave fullscreen, then close.</summary>
    void OnKeyEscape(KeyboardAccelerator s, KeyboardAcceleratorInvokedEventArgs e) => Key(e, () =>
    {
        if (still.Asking)
            StillStopNow();
        else if (discussion is not null)
            discussion.Close();
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
        StillLine.Text = T.PlayerStillWatchingDetail(still.After);
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
    /// <c>--show=still|mini|preview|undo|stall|upnext|skip|starting|flash|tracks|timeline|settings|subtitles|subtitles-mini</c>, and
    /// Stats for nerds as <c>stats</c>, <c>stats-waiting</c>, <c>stats-buffering</c>, <c>stats-hdr</c>,
    /// <c>stats-copied</c> or <c>stats-mini</c>; a <c>.srt</c> beside the clip is loaded for those and for
    /// <c>subtitles</c> and <c>subtitles-mini</c>.
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
            case "discussion-player":
                if (ctx is { } talk && uint.TryParse(playingEpisode, out var talked))
                {
                    talk.Known.Take(new Event.DiscussionFound(playingId, talked, 214));
                    ShowControls();
                    OpenDiscussion(talked);
                }
                break;
            case "preview":
                ShowControls();
                PreviewAtFraction(0.4);
                break;
            case "undo":
                undo = new Undoable(0, T.PlayerSkippedOpening) { Played = -600, Waited = -600 };
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
                UpNextTitle.Text = T.EpisodeNumber(2);
                UpNextStillFrame.Visibility = Visibility.Visible;
                upNext = new Coming(false, position, 0);
                ShowUpNext();
                break;
            case "skip":
                SkipCard.Content = T.PlayerSkipOpening;
                skipShown = true;
                UpdateCorner();
                break;
            case "starting":
                startingEpisode = "1";
                startingHeld = true;
                ShowStarting();
                startingFrom -= 21 * Stopwatch.Frequency;
                break;
            case "tracks":
            case "tracks-sub":
                tracksHeld = true;
                // The fixture's source has a dub of the show; tracks-sub answers as one without.
                if (what == "tracks-sub" && ctx is { } known)
                    known.Known.Dubbed[playingId] = false;
                tracks =
                [
                    new FfiTrack(TrackKind.Audio, 1, "jpn", null, true, false),
                    new FfiTrack(TrackKind.Audio, 2, "eng", null, false, false),
                    new FfiTrack(TrackKind.Subtitle, 1, "eng", "Signs & Songs", false, false),
                    new FfiTrack(TrackKind.Subtitle, 2, "eng", "Full", true, false),
                    new FfiTrack(TrackKind.Subtitle, 3, "spa", null, false, false),
                ];
                BuildTracks();
                ShowControls();
                TracksMenu.ShowAt(TracksButton);
                await Task.Delay(400);
                // Open Subtitles, as a viewer's arrow key would.
                if (TracksMenu.Items.OfType<MenuFlyoutSubItem>().FirstOrDefault() is { } sub
                    && FrameworkElementAutomationPeer.CreatePeerForElement(sub)?.GetPattern(PatternInterface.ExpandCollapse)
                        is Microsoft.UI.Xaml.Automation.Provider.IExpandCollapseProvider peer)
                    peer.Expand();
                break;
            case "timeline":
                // An opening, an ending and what is buffered, as fractions of the fixture clip, the
                // opening early so the played fill runs under it.
                var d = Math.Max(duration, 1);
                timelineMarks =
                [
                    new TimelineMark(SkipKind.Opening, d * 0.03, d * 0.2),
                    new TimelineMark(SkipKind.Ending, d * 0.85, d * 0.97),
                ];
                buffered = [new TimeRange(0, d * 0.7)];
                DrawTimeline();
                ShowControls();
                break;
            case "flash":
                flash.Interval = TimeSpan.FromSeconds(600);
                ShowFlash("12:04");
                break;
            case "settings":
                // The player's Settings menu open, with Stats for nerds at its foot.
                ShowControls();
                SettingsMenu.ShowAt(SettingsButton);
                break;
            case "stats":
            case "stats-waiting":
            case "stats-buffering":
            case "stats-hdr":
            case "stats-copied":
            case "stats-mini":
            case "subtitles":
            case "subtitles-mini":
                // The short clip is near its end from the start: no Up next card over the shot.
                HoldUpNext = true;
                upNext = null;
                ShowUpNext();
                // Subtitles from a file beside the clip, so the shot shows the panel clear of them.
                if (player is { } p && Path.ChangeExtension(ctx?.Options.Media ?? @"C:\setup\sdr-testsrc.mp4", ".srt") is { } srt
                    && File.Exists(srt))
                    await Task.Run(() => p.LoadTrack(TrackKind.Subtitle, srt));
                // The live panel reads mpv itself; the others hold the engine's made-up states.
                switch (what)
                {
                    case "stats-waiting":
                        // Opened before the file has: over the starting screen, a while on.
                        startingHeld = true;
                        ShowStarting();
                        StageStats(StatsFixture.Waiting, TimeSpan.FromSeconds(5), copied: false);
                        break;
                    case "stats-buffering":
                        StageStats(StatsFixture.Buffering, null, copied: false);
                        break;
                    case "stats-hdr":
                        StageStats(StatsFixture.Hdr, null, copied: false);
                        break;
                    case "stats-copied":
                        StageStats(StatsFixture.Playing, null, copied: true);
                        break;
                    case "stats-mini":
                        StageStats(null, null, copied: false);
                        await Task.Delay(2500);
                        ToggleMini();
                        break;
                    case "subtitles":
                        ShowControls();
                        break;
                    case "subtitles-mini":
                        // In the mini window, its controls up: the subtitles clear of them.
                        ToggleMini();
                        await Task.Delay(800);
                        ShowControls();
                        break;
                    default:
                        StageStats(null, null, copied: false);
                        break;
                }
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
        ShowTvButton();
        Elapsed.Visibility = Remaining.Visibility = VolumeButton.Visibility = SettingsButton.Visibility = big;
        Corner.Visibility = big;
        ShowTitle.FontSize = on ? 14 : 18;
        ShowTitle.LineHeight = on ? 20 : 24;
        EpisodeTitle.Visibility = big;
        TopBar.Padding = on ? new Thickness(12, 6, 140, 6) : new Thickness(8, 8, 148, 8);
        MiniIcon.Glyph = on ? "\uE73F" : "\uE8A7";
        ToolTipService.SetToolTip(MiniButton, T.TooltipWithKey(on ? T.PlayerMiniBack : T.PlayerMiniPlayer, "P"));
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(MiniButton, on ? T.PlayerMiniBack : T.PlayerMiniPlayer);
        if (on)
            ClosePreview();
        stats.Mini = on;
        ShowStats();
    }

    // Previews on the timeline.

    internal void NoteLoad(ulong playback, PlayerOp[] ops)
    {
        lock (loads)
            this.playback = playback;
        // A new file opening: Stats for nerds, if up, asks the engine's new player again.
        if (ops.Any(op => op is PlayerOp.Open))
            DispatcherQueue.TryEnqueue(StatsReopen);
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
