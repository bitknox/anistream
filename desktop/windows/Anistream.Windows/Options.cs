namespace Anistream.Windows;

/// <summary>
/// The command line. The user's own run takes none. The testing build adds the fixture scenes,
/// for screenshots and for trying a screen on the fake AniList without touching the profile:
/// <c>anistream.exe --scene=watching --page=calendar</c>, or
/// <c>anistream.exe --scene=playing --media=C:\clip.mp4 --play</c>.
/// </summary>
/// <param name="Scene">fresh, watching, sourceless or playing: the testing engine's scenes.</param>
/// <param name="Media">The file every episode streams in the playing scene.</param>
/// <param name="Page">The destination to open: continue, downloads, providers, stores, search, seasonal, calendar, library or settings.</param>
/// <param name="Title">An AniList id whose title page opens over the page.</param>
/// <param name="Query">What the search page searches for.</param>
/// <param name="Play">Play the first title in Continue once it is there.</param>
/// <param name="Pane">The settings pane to open.</param>
/// <param name="Width">The window's width in effective pixels, or as wide as the work area holds.</param>
/// <param name="KeepControls">The player's controls stay up, for a screenshot.</param>
/// <param name="Downloads">Seed the fixture's downloads before the scene starts.</param>
/// <param name="AskClose">Close the window once it is up, to show what closing asks.</param>
/// <param name="Showcase">Testing build: invented providers, stores, downloads and seeding on screen.</param>
/// <param name="Join">Testing build: open the join-store dialog with an invite already pasted.</param>
/// <param name="Select">The row to select on Providers or Plugin stores, by id.</param>
/// <param name="Show">A player state to bring up for a screenshot: still, mini, preview or idle; with --welcome, the page: anilist or sources.</param>
/// <param name="Welcome">Show the first-launch pages, which a fixture run otherwise skips.</param>
/// <param name="Offline">Tell the engine there is no network, to show the offline states.</param>
/// <param name="Shortcuts">Open the keyboard shortcuts dialog once the window is up.</param>
/// <param name="UpdateFrom">Testing build: an installer to offer as a newer release, <c>&lt;url&gt;,&lt;sha256&gt;,&lt;size&gt;,&lt;version&gt;</c>, fetched and installed for real.</param>
/// <param name="Language">Testing build: the language to show this run, such as <c>qps-ploc</c> for the pseudo-locale; never saved.</param>
/// <param name="Hide">Testing build: titles to hide from Continue at start, <c>id,id</c>, so a short queue leaves room for Up next.</param>
/// <param name="UpNext">Testing build: show made-up suggestions under Continue, <c>signed-in</c> or <c>signed-out</c>, because the fixture catalogue has too few titles for the engine to suggest any.</param>
/// <param name="Opens">The <c>anistream://</c> link the app was opened with, as the installer's <c>anistream.exe "%1"</c> hands it over.</param>
public sealed record Options(
    string? Scene, string? Media, string? Page, uint? Title, string? Query, bool Play,
    string? Pane, int? Width, int? Height, bool KeepControls, bool Downloads, bool AskClose,
    bool Showcase = false, bool Join = false, string? Select = null, string? Show = null,
    bool Welcome = false, bool Offline = false, bool Shortcuts = false, string? Opens = null, string? UpdateFrom = null, string? Language = null, string? Hide = null, string? UpNext = null)
{
    public bool Fixture => Scene is not null;

    public static Options Parse(IEnumerable<string> args)
    {
        string? scene = null, media = null, page = null, query = null, pane = null, select = null, show = null, updateFrom = null, language = null, hide = null, upNext = null;
        uint? title = null;
        int? width = null, height = null;
        bool play = false, keep = false, downloads = false, askClose = false, showcase = false, join = false;
        bool welcome = false, offline = false, shortcuts = false;
        var opens = Anistream.App.Link.InArguments(args);
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
                case "--showcase": showcase = true; break;
                case "--join": join = true; break;
                case "--select": select = value; break;
                case "--show": show = value; break;
                case "--welcome": welcome = true; break;
                case "--offline": offline = true; break;
                case "--shortcuts": shortcuts = true; break;
                case "--update-from": updateFrom = value; break;
#if ANISTREAM_TESTING
                case "--language": language = value; break;
                case "--hide": hide = value; break;
                case "--up-next": upNext = value; break;
#endif
                case "--size":
                    if (value.Split('x') is [var w, var h] && int.TryParse(w, out var wi) && int.TryParse(h, out var hi))
                        (width, height) = (wi, hi);
                    break;
            }
        }
        return new(scene, media, page, title, query, play, pane, width, height, keep, downloads, askClose, showcase, join, select, show, welcome, offline, shortcuts, opens, updateFrom, language, hide, upNext);
    }
}
