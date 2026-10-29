using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.UI.ViewManagement;

namespace Anistream.Windows;

/// <summary>
/// The motion tokens on Windows' animation APIs. Springs are critically damped (quick: 200 ms
/// period, standard: 300 ms); fade-in is 200 ms decelerating, fade-out 300 ms accelerating.
/// With "Animation effects" off (<see cref="UISettings.AnimationsEnabled"/>) the spatial
/// motions (drill-in, the connected cover) become crossfades; fades stay, as the brief says.
/// </summary>
static class Motion
{
    static readonly UISettings settings = new();

    public static bool Enabled => settings.AnimationsEnabled;

    public static readonly TimeSpan Quick = TimeSpan.FromMilliseconds(200);
    public static readonly TimeSpan Standard = TimeSpan.FromMilliseconds(300);
    public static readonly TimeSpan FadeInTime = TimeSpan.FromMilliseconds(200);
    public static readonly TimeSpan FadeOutTime = TimeSpan.FromMilliseconds(300);

    static readonly Dictionary<UIElement, Storyboard> running = [];

    /// <summary>Shows a collapsed element by fading it in (fade-in).</summary>
    public static void Show(UIElement element) => Show(element, FadeInTime);

    /// <summary>Shows an element with the standard token, for cards appearing inside a screen.</summary>
    public static void ShowStandard(UIElement element) => Show(element, Standard);

    static void Show(UIElement element, TimeSpan time)
    {
        if (element.Visibility == Visibility.Visible && !running.ContainsKey(element) && element.Opacity >= 1)
            return;
        var from = element.Visibility == Visibility.Visible ? element.Opacity : 0;
        element.Visibility = Visibility.Visible;
        Fade(element, from, 1, time, EasingMode.EaseOut, null);
    }

    /// <summary>Fades an element out (fade-out), then collapses it so the video scans out alone.</summary>
    public static void Hide(UIElement element) => Hide(element, FadeOutTime);

    public static void HideStandard(UIElement element) => Hide(element, Standard);

    public static void HideQuick(UIElement element) => Hide(element, Quick);

    static void Hide(UIElement element, TimeSpan time)
    {
        if (element.Visibility == Visibility.Collapsed)
            return;
        Fade(element, element.Opacity, 0, time, EasingMode.EaseIn, () =>
        {
            element.Visibility = Visibility.Collapsed;
            element.Opacity = 1;
        });
    }

    /// <summary>Fades freshly arrived content in from nothing: art, a page, a crossfade's new half.</summary>
    public static void FadeIn(UIElement element, TimeSpan? time = null)
    {
        element.Visibility = Visibility.Visible;
        Fade(element, 0, 1, time ?? FadeInTime, EasingMode.EaseOut, null);
    }

    /// <summary>
    /// A trailer's video fading in over the banner (fade-in) or away again (fade-out), from where
    /// it stands. <paramref name="done"/> runs when the fade ends, unless another took over.
    /// </summary>
    public static void Video(UIElement element, bool shown, Action? done = null)
    {
        if (!shown && element.Opacity <= 0)
        {
            done?.Invoke();
            return;
        }
        Fade(element, element.Opacity, shown ? 1 : 0, shown ? FadeInTime : FadeOutTime, shown ? EasingMode.EaseOut : EasingMode.EaseIn, done);
    }

    /// <summary>
    /// Eases a value from one number to another on the standard token, calling <paramref name="set"/>
    /// with each step: what a property no animation can reach (a padding) follows. With animation
    /// effects off it takes the end value at once. Starting another for the same <paramref name="owner"/>
    /// replaces the first.
    /// </summary>
    public static void Ease(object owner, double from, double to, Action<double> set)
    {
        if (eases.Remove(owner, out var was))
            was.Stop();
        if (!Enabled || Math.Abs(from - to) < 0.5)
        {
            set(to);
            return;
        }
        var queue = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread();
        var timer = queue.CreateTimer();
        timer.Interval = TimeSpan.FromMilliseconds(16);
        var started = DateTime.UtcNow;
        timer.Tick += (_, _) =>
        {
            var t = Math.Min(1, (DateTime.UtcNow - started) / Standard);
            var eased = 1 - Math.Pow(1 - t, 3);
            set(from + (to - from) * eased);
            if (t >= 1)
            {
                timer.Stop();
                eases.Remove(owner);
            }
        };
        eases[owner] = timer;
        timer.Start();
    }

    static readonly Dictionary<object, Microsoft.UI.Dispatching.DispatcherQueueTimer> eases = [];

    /// <summary>Stops any fade and sets the element where it would have ended.</summary>
    public static void Set(UIElement element, bool shown)
    {
        if (running.Remove(element, out var board))
            board.Stop();
        element.Opacity = 1;
        element.Visibility = shown ? Visibility.Visible : Visibility.Collapsed;
    }

    static void Fade(UIElement element, double from, double to, TimeSpan time, EasingMode mode, Action? done)
    {
        if (running.Remove(element, out var was))
            was.Stop();
        var animation = new DoubleAnimation
        {
            From = from,
            To = to,
            Duration = time,
            EasingFunction = new CubicEase { EasingMode = mode },
        };
        Storyboard.SetTarget(animation, element);
        Storyboard.SetTargetProperty(animation, "Opacity");
        var board = new Storyboard();
        board.Children.Add(animation);
        board.Completed += (_, _) =>
        {
            if (running.TryGetValue(element, out var current) && current == board)
            {
                running.Remove(element);
                element.Opacity = to;
                done?.Invoke();
            }
        };
        running[element] = board;
        board.Begin();
    }

    /// <summary>
    /// An element's height eased from one value to another on the standard token, then left to its
    /// content again: a dialog moving to a taller or shorter step. With animation effects off it
    /// simply takes the new height.
    /// </summary>
    public static void Height(FrameworkElement element, double from, double to)
    {
        if (running.Remove(element, out var was))
            was.Stop();
        element.Height = double.NaN;
        if (!Enabled || Math.Abs(from - to) < 1)
            return;
        var animation = new DoubleAnimation
        {
            From = from,
            To = to,
            Duration = Standard,
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut },
            EnableDependentAnimation = true,
        };
        Storyboard.SetTarget(animation, element);
        Storyboard.SetTargetProperty(animation, "Height");
        var board = new Storyboard();
        board.Children.Add(animation);
        board.Completed += (_, _) =>
        {
            if (running.TryGetValue(element, out var current) && current == board)
            {
                running.Remove(element);
                board.Stop();
                element.Height = double.NaN;
            }
        };
        running[element] = board;
        board.Begin();
    }

    /// <summary>An opacity that eases to each new value on the quick token: the row dim after marking watched.</summary>
    public static void QuickOpacity(UIElement element) =>
        element.OpacityTransition = new ScalarTransition { Duration = Quick };

    /// <summary>
    /// A critically damped spring on a composition property ("Scale.X", "Offset.Y"...) of the
    /// element's visual: quick or standard.
    /// </summary>
    public static void Spring(UIElement element, string property, float to, bool quick = false)
    {
        var visual = ElementCompositionPreview.GetElementVisual(element);
        var spring = visual.Compositor.CreateSpringScalarAnimation();
        spring.DampingRatio = 1f;
        spring.Period = quick ? Quick : Standard;
        spring.FinalValue = to;
        if (!Enabled)
        {
            spring.Period = TimeSpan.FromMilliseconds(1);
        }
        visual.StartAnimation(property, spring);
    }
}
