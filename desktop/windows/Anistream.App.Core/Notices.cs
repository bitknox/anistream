using Anistream.Engine.Core;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// A system notification the app posts: the show as its title, one line of what happened, and
/// what Play starts. <see cref="Tag"/> names it, so the same news posted twice replaces itself.
/// </summary>
public sealed record SystemNotice(string Tag, string Title, string Body, uint Show, string? Play, string? Art);

/// <summary>
/// What the Windows notifications say, from the engine's news: a new episode out, as the Apple
/// apps word it, or a download finished.
/// </summary>
public static class Notices
{
    /// <summary>
    /// One notification per episode the engine's check said: "Episode 9 is out." when caught up,
    /// "Episode 10 is out. Next for you: episode 8." when behind, with Play starting the one next
    /// for the viewer.
    /// </summary>
    public static SystemNotice[] Aired(IEnumerable<NewEpisode> said) =>
        [.. said
            .DistinctBy(e => (e.Id, e.Episode, e.Translation))
            .Select(e =>
            {
                var (body, play) = Words(e.Episode, e.NextForYou ?? e.Episode, e.Translation == Translation.Dub);
                var dub = e.Translation == Translation.Dub ? "-dub" : "";
                return new SystemNotice($"aired-{e.Id}-{e.Episode}{dub}", e.Title, body, e.Id, play.ToString(), e.BannerUrl ?? e.CoverUrl);
            })];

    public static (string Body, uint Play) Words(uint episode, uint next, bool dubbed)
    {
        var out_ = dubbed ? T.NotifyEpisodeOutDubbed(episode) : T.NotifyEpisodeOut(episode);
        return next < episode ? ($"{out_} {T.NotifyNextForYou(next)}", next) : (out_, episode);
    }

    /// <summary>
    /// The downloads finished between two lists of the queue, a notification per show: "Episode 3
    /// downloaded." or "4 episodes downloaded.". None for the first list, which only says where
    /// things stand.
    /// </summary>
    public static SystemNotice[] Downloaded(DownloadRow[]? before, DownloadRow[] now)
    {
        if (before is null)
            return [];
        var was = before.ToDictionary(r => r.Id, r => r.State);
        return [.. now
            .Where(r => r.State == DownloadState.Done && was.TryGetValue(r.Id, out var s) && s != DownloadState.Done)
            .GroupBy(r => r.AnilistId)
            .Select(g =>
            {
                var rows = g.ToArray();
                var body = rows.Length == 1 ? T.NotifyDownloadedOne(rows[0].Episode) : T.NotifyDownloaded(rows.Length);
                var first = rows.OrderBy(r => double.TryParse(r.Episode, out var n) ? n : double.MaxValue).First();
                return new SystemNotice($"downloaded-{g.Key}-{string.Join('.', rows.Select(r => r.Id))}", rows[0].Title, body, g.Key, first.Episode, null);
            })];
    }

    /// <summary>What a notification's button or body asks, read back from its arguments.</summary>
    public static (string Action, uint Show, string? Episode)? Read(IDictionary<string, string> arguments) =>
        arguments.TryGetValue("action", out var action) && arguments.TryGetValue("show", out var show) && uint.TryParse(show, out var id)
            ? (action, id, arguments.TryGetValue("episode", out var e) ? e : null)
            : null;
}
