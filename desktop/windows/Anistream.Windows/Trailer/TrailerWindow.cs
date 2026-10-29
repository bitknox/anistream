using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows;

/// <summary>
/// Play trailer: the trailer in a window of its own, with YouTube's controls and sound on. When
/// the embed refuses the video (embedding disabled by its owner, a region lock), or the web view
/// is not there, the window closes and the watch page opens in the browser, as it always did.
/// </summary>
static class TrailerWindow
{
    static Window? current;

    public static void Play(Ctx ctx, Entry entry)
    {
        if (entry.TrailerUrl is not { } url)
            return;
        void Browser() => ctx.Send(new Command.PlayTrailer(url, entry.Title));
        if (!TrailerHub.Available || !Trailers.Embeddable(entry))
        {
            Browser();
            return;
        }
        // Whatever plays by itself gives way to a trailer the viewer asked for.
        TrailerHub.StopAll();
        current?.Close();
        var surface = new Grid { Background = new SolidColorBrush(Colors.Black) };
        var window = new Window { Title = entry.Title, Content = surface };
        var stage = new TrailerStage(surface, EmbedStyle.Full);
        var refused = false;
        stage.Refused += code =>
        {
            if (refused)
                return;
            refused = true;
            window.Close();
            Browser();
        };
        window.Closed += (_, _) =>
        {
            stage.Stop();
            if (current == window)
                current = null;
        };
        window.AppWindow.Resize(new global::Windows.Graphics.SizeInt32(1088, 656));
        current = window;
        window.Activate();
        stage.Size(1040, 585);
        // The view fills the window; the player letterboxes inside it.
        surface.SizeChanged += (_, e) => stage.Size(e.NewSize.Width, e.NewSize.Height);
        stage.Start(entry);
    }
}
