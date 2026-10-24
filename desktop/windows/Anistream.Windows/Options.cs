namespace Anistream.Windows;

/// <summary>
/// The command line. The user's own run takes none. The testing build adds the fixture scenes,
/// for screenshots and for trying a screen on the fake AniList without touching the profile:
/// <c>anistream.exe --scene=watching --page=calendar</c>, or
/// <c>anistream.exe --scene=playing --media=C:\clip.mp4 --play</c>.
/// </summary>
/// <param name="Scene">fresh, watching, sourceless or playing: the testing engine's scenes.</param>
/// <param name="Media">The file every episode streams in the playing scene.</param>
/// <param name="Page">The destination to open: continue, search, seasonal, calendar, library or settings.</param>
/// <param name="Title">An AniList id whose title page opens over the page.</param>
/// <param name="Query">What the search page searches for.</param>
/// <param name="Play">Play the first title in Continue once it is there.</param>
/// <param name="Pane">The settings pane to open.</param>
/// <param name="Width">The window's width in effective pixels, or as wide as the work area holds.</param>
/// <param name="KeepControls">The player's controls stay up, for a screenshot.</param>
/// <param name="Downloads">Seed the fixture's downloads before the scene starts.</param>
/// <param name="AskClose">Close the window once it is up, to show what closing asks.</param>
public sealed record Options(
    string? Scene, string? Media, string? Page, uint? Title, string? Query, bool Play,
    string? Pane, int? Width, int? Height, bool KeepControls, bool Downloads, bool AskClose)
{
    public bool Fixture => Scene is not null;

    public static Options Parse(IEnumerable<string> args)
    {
        string? scene = null, media = null, page = null, query = null, pane = null;
        uint? title = null;
        int? width = null, height = null;
        bool play = false, keep = false, downloads = false, askClose = false;
        foreach (var arg in args)
        {
            var (name, value) = arg.Split('=', 2) is [var n, var v] ? (n, v) : (arg, "");
            switch (name)
            {
                case "--scene": scene = value; break;
                case "--media": media = value; break;
                case "--page": page = value; break;
                case "--title": title = uint.TryParse(value, out var t) ? t : null; break;
                case "--query": query = value; break;
                case "--play": play = true; break;
                case "--pane": pane = value; break;
                case "--keep-controls": keep = true; break;
                case "--downloads": downloads = true; break;
                case "--close": askClose = true; break;
                case "--size":
                    if (value.Split('x') is [var w, var h] && int.TryParse(w, out var wi) && int.TryParse(h, out var hi))
                        (width, height) = (wi, hi);
                    break;
            }
        }
        return new(scene, media, page, title, query, play, pane, width, height, keep, downloads, askClose);
    }
}
