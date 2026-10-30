using Microsoft.UI.Dispatching;

namespace Anistream.Windows;

/// <summary>
/// Dispatcher timers that do not keep their owner alive. A timer's Tick handler is held by the
/// native timer, and a handler that captures its page holds the page from outside the managed
/// heap: the page holds the timer, the timer holds the page, and the collector never sees the
/// pair as garbage. These timers reach their owner through a weak reference, so a page left
/// behind is collected and its timers with it.
/// </summary>
static class Timers
{
    /// <summary>
    /// A timer on the current thread's queue that calls <paramref name="tick"/> with its owner.
    /// <paramref name="tick"/> must not capture the owner itself; take it from the argument.
    /// </summary>
    public static DispatcherQueueTimer Weak<T>(T owner, TimeSpan interval, bool repeating, Action<T> tick) where T : class
    {
        var timer = DispatcherQueue.GetForCurrentThread().CreateTimer();
        timer.Interval = interval;
        timer.IsRepeating = repeating;
        var weak = new WeakReference<T>(owner);
        timer.Tick += (sender, _) =>
        {
            if (weak.TryGetTarget(out var target))
                tick(target);
            else
                sender.Stop();
        };
        return timer;
    }
}
