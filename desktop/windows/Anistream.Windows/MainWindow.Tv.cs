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
#if ANISTREAM_TESTING
        // A fixture run never looks on the real network: the fake TVs beside the engine, or none.
        if (options.Fixture)
            browser = FixtureTvs(engine);
#endif
        tv.HandedOff = () =>
        {
            if (Player.Showing)
                Player.Close();
        };
        tv.Changed += TvChanged;
        tv.Attach(tvSender, browser);
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
#endif
    }

    void TvChanged()
    {
        ShowTvBar();
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

    /// <summary>
    /// The way back to the card while a TV is driven, at the title bar's right: the TV's name,
    /// and what it does while it plays something.
    /// </summary>
    void ShowTvBar()
    {
        var card = tv.Connected is null ? null : tv.Card;
        if (card is null)
        {
            if (tvBar is not null)
                tvBar.Visibility = Visibility.Collapsed;
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
            line.Children.Add(new FontIcon { Glyph = "", FontSize = 14 });
            line.Children.Add(new TextBlock { VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis, MaxWidth = 220 });
            tvBar.Content = line;
            tvBar.Click += (_, _) => tv.OpenRemote();
            AppTitleBar.RightHeader = tvBar;
        }
        var text = tv.Now is null ? card.TvName : $"Playing on {card.TvName}";
        ((TextBlock)((StackPanel)tvBar.Content).Children[1]).Text = text;
        var tip = $"Show the remote for {card.TvName}";
        ToolTipService.SetToolTip(tvBar, tip);
        AutomationProperties.SetName(tvBar, text);
        AutomationProperties.SetHelpText(tvBar, tip);
        tvBar.Visibility = Visibility.Visible;
    }

#if ANISTREAM_TESTING
    readonly List<FfiFakeTv> fakeTvs = [];
    FixtureTvBrowser? fixtureTvs;

    /// <summary>
    /// The fixture run's TVs: two made-up ones on the loopback interface, each a real remote
    /// server with a scripted player (<c>StartFakeTv</c>), found as DNS-SD would find them. For
    /// tv-slow, a third found at an address nothing answers on; for tv-empty and tv-looking, a
    /// network where no TV answers.
    /// </summary>
    FfiTvBrowser FixtureTvs(FfiEngine engine)
    {
        try
        {
            fakeTvs.Add(engine.StartFakeTv("Living room"));
            fakeTvs.Add(engine.StartFakeTv("Bedroom"));
            FfiFakeTv? slow = options.Show == "tv-slow" ? engine.StartFakeTv("Kitchen") : null;
            if (slow is not null)
                fakeTvs.Add(slow);
            fixtureTvs = new FixtureTvBrowser(fakeTvs.Take(2).ToArray(), slow, silent: options.Show is "tv-empty" or "tv-looking");
        }
        catch (FfiException e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "tv", $"the fake TVs did not start: {e.Message}");
            fixtureTvs = new FixtureTvBrowser([], null, silent: true);
        }
        return fixtureTvs;
    }

    sealed class FixtureTvBrowser(FfiFakeTv[] tvs, FfiFakeTv? slow, bool silent) : FfiTvBrowser
    {
        public FfiFakeTv[] Tvs => tvs;

        public void Start(FfiDiscoverySink sink)
        {
            if (silent)
                return;
            foreach (var fake in tvs)
            {
                var advert = fake.Advert();
                sink.Found(advert.Instance, [fake.Host()], fake.Port(), advert.Txt);
            }
            if (slow is not null)
            {
                var advert = slow.Advert();
                sink.Found(advert.Instance, ["10.255.255.1"], slow.Port(), advert.Txt);
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
                        if (scene == "tv-wrong" || tv.Pairing?.Problem?.Contains("ran out", StringComparison.Ordinal) != false)
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
