using System.Runtime.InteropServices;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Windows.Pages;
using Anistream.Windows.Tv;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Windows.Networking.Connectivity;

namespace Anistream.Windows;

/// <summary>
/// Play on TV, the window's side: the engine's sender, browsing with Windows' DNS-SD, the dialog
/// that comes up whenever the remote has a sheet to show, the title bar's way back to the card
/// while a TV is driven, and trying the TV again as the network or the window comes back.
/// </summary>
public sealed partial class MainWindow
{
    readonly TvRemote tv = new();
    FfiTvSender? tvSender;
    Button? tvBar;
    bool openingTv;

    /// <summary>The sender, when the engine was built with the remote; without one Play on TV stays out of sight.</summary>
    void StartTv(FfiEngine engine)
    {
        try
        {
            tvSender = engine.TvSender(TvWords.Device, $"anistream-windows/{Updater.Current}");
        }
        catch (FfiException e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Info, "tv", $"no Play on TV: {e.Message}");
            return;
        }
        FfiTvBrowser browser = new DnssdBrowser();
        FfiCastBrowser casts = new CastBrowser();
#if ANISTREAM_TESTING
        // A fixture run never looks on the real network: the fake TVs beside the engine, or none.
        if (options.Fixture)
        {
            browser = FixtureTvs(engine);
            casts = FixtureCasts(engine);
        }
#endif
        tv.HandedOff = () =>
        {
            if (Player.Showing)
                Player.Close();
        };
        // Windows' firewall asks the first time the cast's relay listens; once a cast has played,
        // the picker stops saying so.
        var served = Path.Combine(dirs.Config, "windows-cast-served");
        tv.CastServed = File.Exists(served);
#if ANISTREAM_TESTING
        if (options.Show is not null && options.Show.StartsWith("cast-", StringComparison.Ordinal))
            tv.CastServed = options.Show == "cast-picker-allowed";
#endif
        tv.Served = () =>
        {
            try
            {
                Directory.CreateDirectory(dirs.Config);
                File.WriteAllText(served, "");
            }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException)
            {
                AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"the cast mark was not kept: {e.Message}");
            }
        };
        // Someone else took the TV: the card has closed, and the notice offers to carry on here.
        tv.TakenOver = (play, notice) => Say(notice.Text(), T.TvCastPlayHere, () => ctx.Play(play.Id, play.Episode));
        tv.Changed += TvChanged;
        tv.Attach(tvSender, browser, casts);
        // The link waits between tries while the TV is away; a network back, or the window
        // brought to the front, tries now.
        NetworkInformation.NetworkStatusChanged += TvNetworkChanged;
        Activated += (_, e) =>
        {
            if (e.WindowActivationState != WindowActivationState.Deactivated)
                tv.RetryNow();
        };
    }

    void TvNetworkChanged(object sender) => ui.TryEnqueue(tv.RetryNow);

    void StopTv()
    {
        NetworkInformation.NetworkStatusChanged -= TvNetworkChanged;
        if (tvSender is null)
            return;
        tv.Changed -= TvChanged;
        tv.Disconnect();
#if ANISTREAM_TESTING
        foreach (var fake in fakeTvs)
            fake.Stop();
        foreach (var fake in fakeCasts)
            fake.Stop();
        StopSilentTv();
#endif
    }

    void TvChanged()
    {
        ShowTvBar();
        // While a TV plays, the media keys and Windows' media controls drive it: the cast's TV
        // first, as a cast is what was started last.
        if (tv.Casting && tv.Stage is CastStage.Starting or CastStage.Playing && tv.CastNow is { } cast)
            media?.Tv(cast, tv.CastPlayhead() ?? cast.Position, tv.Stage == CastStage.Playing, Picture(cast), tv.SendCast);
        else
        {
            var now = tv.Connected is null ? null : tv.Now;
            media?.Tv(now, now is null ? 0 : tv.Playhead() ?? now.Position, tv.Live, Picture(now), c => _ = tv.Send(c));
        }
        if (tv.Sheet is not null && TvDialog.Showing is null && !openingTv)
            OpenTvDialog();
    }

    async void OpenTvDialog()
    {
        openingTv = true;
        try
        {
            if (CloseDialogs())
                await Task.Delay(250);
            if (tv.Sheet is not null)
                await TvDialog.Show(ctx, Root.XamlRoot);
        }
        catch (COMException)
        {
            // Another dialog came up first.
            tv.SheetClosed();
        }
        finally
        {
            openingTv = false;
        }
    }

    /// <summary>The show's banner or cover for the media overlay, when the catalogue knows the show.</summary>
    string? Picture(FfiNowCard? now) =>
        now is null || ctx.Known.Entry(now.Id) is not { } entry ? null : entry.BannerUrl ?? entry.CoverUrl;

    /// <summary>
    /// The way back to the card while a TV is driven, at the title bar's right: the TV's name and
    /// what it plays while the link is up, and how the link stands, by its dot, while it is not.
    /// </summary>
    void ShowTvBar()
    {
        var card = tv.Connected is null ? null : tv.Card;
        if (card is null && !tv.Casting)
        {
            if (tvBar is not null)
                tvBar.Visibility = Visibility.Collapsed;
            KeepSearchInPlace();
            return;
        }
        if (tvBar is null)
        {
            tvBar = new Button
            {
                Height = 32,
                Padding = new Thickness(10, 0, 12, 0),
                Margin = new Thickness(0, 0, 8, 0),
                VerticalAlignment = VerticalAlignment.Center,
                Background = new Microsoft.UI.Xaml.Media.SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
            };
            var line = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            var mark = new Grid { Width = 14, VerticalAlignment = VerticalAlignment.Center };
            mark.Children.Add(new FontIcon { Glyph = "", FontSize = 14 });
            mark.Children.Add(Ui.Dot(Colours.Haze));
            line.Children.Add(mark);
            line.Children.Add(new TextBlock { VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis, MaxWidth = 220 });
            tvBar.Content = line;
            tvBar.Click += (_, _) => tv.OpenRemote();
            tvBar.SizeChanged += (_, _) => KeepSearchInPlace();
            AppTitleBar.SizeChanged += (_, _) => KeepSearchInPlace();
            AppTitleBar.RightHeader = tvBar;
        }
        var parts = ((StackPanel)tvBar.Content).Children;
        // The TV's glyph while the link is up or the cast plays; the dot, as the card's line draws it, while not.
        var (connected, dot, text, name) = tv.Casting
            ? (tv.Stage == CastStage.Playing, TvWords.CastDot(tv.Stage),
                TvWords.CastStatus(tv.Stage, tv.CastCard, tv.CastName, tv.CastWaited()), tv.CastName)
            : (card!.Phase == FfiLinkPhase.Connected, TvWords.DotFor(card.Phase), TvWords.BarLine(card, tv.Now is not null), card.TvName);
        var glyphAndDot = ((Grid)parts[0]).Children;
        glyphAndDot[0].Visibility = connected ? Visibility.Visible : Visibility.Collapsed;
        glyphAndDot[1].Visibility = connected ? Visibility.Collapsed : Visibility.Visible;
        ((Microsoft.UI.Xaml.Shapes.Ellipse)glyphAndDot[1]).Fill = new Microsoft.UI.Xaml.Media.SolidColorBrush(
            dot == TvWords.Dot.Vermilion ? Ui.Vermilion : Colours.Haze);
        ((TextBlock)parts[1]).Text = text;
        var tip = T.TvShowRemote(name);
        ToolTipService.SetToolTip(tvBar, tip);
        AutomationProperties.SetName(tvBar, text);
        AutomationProperties.SetHelpText(tvBar, tip);
        tvBar.Visibility = Visibility.Visible;
        KeepSearchInPlace();
    }

    /// <summary>
    /// The title bar centres the search box in what is left between its headers, so the TV's
    /// button would push it left by half its width. A left margin of that width puts it back
    /// where it sat, while the bar has room for both.
    /// </summary>
    void KeepSearchInPlace()
    {
        var width = tvBar is { Visibility: Visibility.Visible } bar ? bar.ActualWidth + bar.Margin.Left + bar.Margin.Right : 0;
        var left = AppTitleBar.ActualWidth >= 1000 ? width : 0;
        if (SearchBox.Margin.Left != left)
            SearchBox.Margin = new Thickness(left, 0, 0, 0);
    }

#if ANISTREAM_TESTING
    readonly List<FfiFakeTv> fakeTvs = [];
    FixtureTvBrowser? fixtureTvs;

    /// <summary>
    /// The fixture run's TVs: two made-up ones on the loopback interface, each a real remote
    /// server with a scripted player (<c>StartFakeTv</c>), found as DNS-SD would find them. For
    /// tv-slow, a third whose address is a loopback listener that takes the connection and never
    /// answers, so nothing leaves the machine; for tv-empty and tv-looking, a network where no TV
    /// answers.
    /// </summary>
    FfiTvBrowser FixtureTvs(FfiEngine engine)
    {
        try
        {
            // A cast scene's Living room has Chromecast built in, so only Bedroom runs the app.
            if (CastScene)
            {
                fakeTvs.Add(engine.StartFakeTv("Bedroom"));
                fixtureTvs = new FixtureTvBrowser(fakeTvs.ToArray(), null, 0, silent: options.Show == "cast-none-found", apart: true);
                return fixtureTvs;
            }
            fakeTvs.Add(engine.StartFakeTv("Living room"));
            fakeTvs.Add(engine.StartFakeTv("Bedroom"));
            FfiFakeTv? slow = options.Show == "tv-slow" ? engine.StartFakeTv("Kitchen") : null;
            if (slow is not null)
                fakeTvs.Add(slow);
            var slowPort = slow is null ? (ushort)0 : StartSilentTv();
            fixtureTvs = new FixtureTvBrowser(fakeTvs.Take(2).ToArray(), slow, slowPort, silent: options.Show is "tv-empty" or "tv-looking");
        }
        catch (FfiException e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "tv", $"the fake TVs did not start: {e.Message}");
            fixtureTvs = new FixtureTvBrowser([], null, 0, silent: true);
        }
        return fixtureTvs;
    }

    bool CastScene => options.Show?.StartsWith("cast-", StringComparison.Ordinal) == true;

    readonly List<FfiFakeCast> fakeCasts = [];
    FixtureCastBrowser? fixtureCasts;

    /// <summary>
    /// The fixture run's TVs with Chromecast built in: receivers on the loopback interface
    /// (<c>StartFakeCast</c>), found as a <c>_googlecast._tcp</c> browse would find them. Living
    /// room has fixed volume and Kitchen display a level; each scene sets how Living room behaves.
    /// Only a cast scene has any, so the other scenes' pickers stay as they were.
    /// </summary>
    FfiCastBrowser FixtureCasts(FfiEngine engine)
    {
        var scene = options.Show ?? "";
        fixtureCasts = new FixtureCastBrowser(fakeCasts);
        if (!CastScene || scene == "cast-none-found")
            return fixtureCasts;
        try
        {
            var living = new FfiFakeCastOptions(FixedVolume: true, Fetches: true, Refuses: false, LaunchMs: 0);
            living = scene switch
            {
                "cast-starting" or "cast-counting" => living with { LaunchMs = 7000 },
                "cast-refused" => living with { Refuses = true },
                "cast-never-reads" => living with { Fetches = false },
                _ => living,
            };
            fakeCasts.Add(engine.StartFakeCast("Living room", living));
            fakeCasts.Add(engine.StartFakeCast("Kitchen display", new FfiFakeCastOptions(FixedVolume: false, Fetches: true, Refuses: false, LaunchMs: 0)));
        }
        catch (FfiException e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"the fake TVs with Chromecast built in did not start: {e.Message}");
        }
        return fixtureCasts;
    }

    sealed class FixtureCastBrowser(List<FfiFakeCast> casts) : FfiCastBrowser
    {
        public void Start(FfiCastSink sink)
        {
            foreach (var fake in casts)
            {
                var advert = fake.Advert();
                sink.Found(advert.Instance, [fake.Host()], advert.Port, advert.Txt);
            }
        }

        public void Stop()
        {
        }
    }

    /// <summary>
    /// --show=cast-*: Play on TV with the fake TVs with Chromecast built in, in the state a
    /// screenshot is for. Bedroom runs the app; Living room has fixed volume; Kitchen display a level.
    ///
    ///     cast-picker          an episode to play: Bedroom, then the Chromecast group, with the firewall line
    ///     cast-picker-allowed  the same once a cast has played from this PC: no firewall line
    ///     cast-none-found      no TV of either kind answers, past the wait
    ///     cast-starting        Living room picked, while it opens the receiver (under 4 s)
    ///     cast-counting        the same past 4 s, with the counter
    ///     cast-playing-fixed   Living room playing: fixed volume, so Mute only
    ///     cast-playing-level   Kitchen display playing: a level, so a slider
    ///     cast-refused         Living room refuses every stream
    ///     cast-never-reads     Living room never reads the stream, after 15 s
    ///     cast-lost            Living room drops off mid-episode
    ///     cast-taken-over      someone else takes Living room: the card closes, the notice offers Play here
    ///     cast-bar             Living room playing with the card closed: the title bar's button
    ///     cast-stopped         Stop casting from the card
    /// </summary>
    async void RehearseCast(string scene)
    {
        await Task.Delay(1200);
        await Until(() => ctx.Known.Entry(900001) is not null, 5);
        var play = new TvPlay(900001, ctx.Known.Entry(900001)?.Title ?? "", "7", 0);
        tv.Offer(play);
        if (scene is "cast-picker" or "cast-picker-allowed" or "cast-none-found")
            return;
        var name = scene == "cast-playing-level" ? "Kitchen display" : "Living room";
        if (!await Until(() => tv.CastRows.Any(r => r.Name == name)))
            return;
        await Task.Delay(600);
        tv.Cast(tv.CastRows.First(r => r.Name == name));
        if (scene is "cast-starting" or "cast-counting" or "cast-refused" or "cast-never-reads")
            return;
        if (!await Until(() => tv.Stage == CastStage.Playing, 20))
            return;
        var fake = fakeCasts.FirstOrDefault(f => f.Advert().Txt.Any(t => t.Key == "fn" && t.Value == name));
        switch (scene)
        {
            case "cast-lost":
                await Task.Delay(1500);
                fake?.GoSilent();
                break;
            case "cast-taken-over":
                await Task.Delay(1500);
                fake?.TakeOver();
                break;
            case "cast-bar":
                await Task.Delay(500);
                TvDialog.Showing?.Close();
                break;
            case "cast-stopped":
                await Task.Delay(1500);
                tv.StopCasting();
                break;
        }
    }

    System.Net.Sockets.TcpListener? silentTv;
    readonly List<System.Net.Sockets.Socket> heldOpen = [];

    /// <summary>tv-slow's TV: a port on the loopback interface that takes connections and never says a word.</summary>
    ushort StartSilentTv()
    {
        var listener = silentTv = new System.Net.Sockets.TcpListener(System.Net.IPAddress.Loopback, 0);
        listener.Start();
        _ = Task.Run(async () =>
        {
            try
            {
                while (true)
                {
                    var socket = await listener.AcceptSocketAsync();
                    lock (heldOpen)
                        heldOpen.Add(socket);
                }
            }
            catch (Exception)
            {
                // Stopped.
            }
        });
        return (ushort)((System.Net.IPEndPoint)listener.LocalEndpoint).Port;
    }

    void StopSilentTv()
    {
        silentTv?.Stop();
        silentTv = null;
        lock (heldOpen)
        {
            foreach (var socket in heldOpen)
                socket.Dispose();
            heldOpen.Clear();
        }
    }

    /// <summary>
    /// The fake TVs running the app, found as DNS-SD would find them. <paramref name="apart"/>
    /// finds them at a documentation address rather than loopback, where the TVs with Chromecast
    /// built in are, as the picker leaves out a Chromecast at the address of a TV running the app.
    /// </summary>
    sealed class FixtureTvBrowser(FfiFakeTv[] tvs, FfiFakeTv? slow, ushort slowPort, bool silent, bool apart = false) : FfiTvBrowser
    {
        public FfiFakeTv[] Tvs => tvs;

        public void Start(FfiDiscoverySink sink)
        {
            if (silent)
                return;
            foreach (var fake in tvs)
            {
                var advert = fake.Advert();
                sink.Found(advert.Instance, [apart ? "192.0.2.10" : fake.Host()], fake.Port(), advert.Txt);
            }
            if (slow is not null)
            {
                var advert = slow.Advert();
                sink.Found(advert.Instance, ["127.0.0.1"], slowPort, advert.Txt);
            }
        }

        public void Stop()
        {
        }
    }

    static async Task<bool> Until(Func<bool> there, double seconds = 10)
    {
        var deadline = DateTimeOffset.UtcNow.AddSeconds(seconds);
        while (DateTimeOffset.UtcNow < deadline)
        {
            if (there())
                return true;
            await Task.Delay(50);
        }
        return false;
    }

    /// <summary>
    /// --show=tv-*: Play on TV with the fake TVs, in the state a screenshot is for.
    ///
    ///     tv-paired    Bedroom paired, Living room paired and driven, nothing open (Settings shows them)
    ///     tv-picker    Play on TV asked for an episode: the picker, Bedroom paired and Living room not
    ///     tv-pair      the picker opened to pair, with nothing to play
    ///     tv-code      pairing with Living room, its code to type
    ///     tv-wrong     a wrong code typed
    ///     tv-expired   wrong codes until the code ran out
    ///     tv-slow      pairing with a TV that does not answer yet
    ///     tv-remote    Living room playing the episode, its card open
    ///     tv-lost      the remote scene, then Living room gone from the network mid-episode
    ///     tv-unpaired  the remote scene, then Living room letting this PC go
    ///     tv-bar       the remote scene with its card closed: the title bar's button
    ///     tv-empty     the picker on a network where no TV answers, past its wait
    ///     tv-looking   the same, while it still looks
    /// </summary>
    async void RehearseTv(string scene)
    {
        await Task.Delay(1200);
        if (scene is "tv-empty" or "tv-looking")
        {
            // Nothing paired and nothing found.
            tv.OpenPicker();
            return;
        }
        if (tvSender is not { } sender || fixtureTvs is not { Tvs: [var living, var bedroom] })
            return;
        await Until(() => ctx.Known.Entry(900001) is not null, 5);
        var play = new TvPlay(900001, ctx.Known.Entry(900001)?.Title ?? "", "3", 312);
        try
        {
            var bedroomPairing = await sender.Pair(bedroom.PairLink(bedroom.OpenCode()));
            await bedroomPairing.Pair(null);
            tv.Refresh();
            switch (scene)
            {
                case "tv-picker":
                    tv.Offer(play);
                    return;
                case "tv-pair":
                    tv.OpenPicker();
                    return;
                case "tv-code" or "tv-wrong" or "tv-expired":
                {
                    tv.Offer(play);
                    if (!await Until(() => tv.Rows.Any(r => r.Name == "Living room" && r.Kind == FfiTvRowKind.New)))
                        return;
                    tv.Choose(tv.Rows.First(r => r.Name == "Living room" && r.Kind == FfiTvRowKind.New));
                    if (scene == "tv-code" || !await Until(() => tv.Pairing is { Typing: true, Working: false }))
                        return;
                    await Task.Delay(700);
                    for (var i = 0; i < 6; i++)
                    {
                        var wrong = living.ShownCode() == "111111" ? "222222" : "111111";
                        await tv.Submit(wrong);
                        await Until(() => tv.Pairing is { Working: false });
                        if (scene == "tv-wrong" || tv.Pairing is not { Expired: false })
                            return;
                        await Task.Delay(300);
                    }
                    return;
                }
                case "tv-slow":
                {
                    tv.Offer(play);
                    if (await Until(() => tv.Rows.Any(r => r.Name == "Kitchen")))
                        tv.Choose(tv.Rows.First(r => r.Name == "Kitchen"));
                    return;
                }
            }
            var livingPairing = await sender.Pair(living.PairLink(living.OpenCode()));
            var paired = await livingPairing.Pair(null);
            tv.Refresh();
            if (scene == "tv-paired")
            {
                tv.Connect(paired.Fingerprint);
                return;
            }
            tv.Offer(play);
            tv.Use(paired.Fingerprint);
            await Until(() => tv.Now is { Loading: false });
            switch (scene)
            {
                case "tv-bar":
                    TvDialog.Showing?.Close();
                    break;
                case "tv-lost":
                    await Task.Delay(1500);
                    living.Stop();
                    break;
                case "tv-unpaired":
                    await Task.Delay(1500);
                    living.RevokeAll();
                    break;
            }
        }
        catch (Exception e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "tv", $"the TV scene did not set up: {e.Message}");
        }
    }
#endif
}
