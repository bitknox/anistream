using System.Diagnostics;
using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;

namespace Anistream.Windows.Player;

// The subtitles above the bottom bar while the controls show, as on the Mac and Linux.
public sealed partial class PlayerView
{
    static readonly Stopwatch liftClock = Stopwatch.StartNew();
    readonly SubtitleLift lift = new(() => liftClock.Elapsed);
    DispatcherQueueTimer? lifting;
    /// <summary>The playback the engine's ops are for now, which a <c>sub-pos</c> of ours joins.</summary>
    ulong playback;

    /// <summary>
    /// The subtitles up above the bottom bar while the controls show, down to mpv's own place
    /// when they go: with the controls' fade, or at once for a new size or a new playback.
    /// </summary>
    void LiftSubtitles(bool animated = true)
    {
        var up = Showing && chromeShown;
        var target = up && BottomBar.ActualHeight > 0
            ? SubtitleLift.Position(BottomBar.ActualHeight, VideoPanel.ActualHeight)
            : SubtitleLift.Bottom;
        // A move across the picture, so with animation effects off it is at once; the fade stays.
        var time = !animated || !Motion.Enabled ? TimeSpan.Zero : up ? Motion.FadeInTime : Motion.FadeOutTime;
        lift.Head(target, time);
        SetSubtitlePosition();
        if (!lift.Moving)
            return;
        if (lifting is null)
        {
            lifting = DispatcherQueue.GetForCurrentThread().CreateTimer();
            lifting.Interval = TimeSpan.FromMilliseconds(30);
            lifting.Tick += (_, _) =>
            {
                SetSubtitlePosition();
                if (!lift.Moving)
                {
                    SetSubtitlePosition();
                    lifting.Stop();
                }
            };
        }
        lifting.Start();
    }

    /// <summary>Tells mpv where the lift has the subtitles now, when that has changed.</summary>
    void SetSubtitlePosition()
    {
        if (player is null || !Showing)
            return;
        ulong now;
        lock (loads)
            now = playback;
        if (now == 0 || lift.Next() is not { } position)
            return;
        player.Apply(now, [new PlayerOp.Set("sub-pos", new MpvValue.Double(position))]);
    }

    /// <summary>The player closed: the next one starts from mpv's own place, told again.</summary>
    void DropSubtitleLift()
    {
        lifting?.Stop();
        lift.Forget();
    }

    void OnBottomBarSized(object sender, SizeChangedEventArgs e) => LiftSubtitles(animated: false);
}
