using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// What several pages read, kept as the engine last said it: every title seen, the lists, the
/// settings, the trackers, the downloads. <see cref="Take"/> folds one event in; the pages then
/// read from here rather than each keeping its own copy. Only the UI thread touches it.
/// </summary>
public sealed class Known
{
    /// <summary>Every title seen in a list or on its page, by AniList id, as it last stood.</summary>
    public Dictionary<uint, Entry> Entries { get; } = [];

    /// <summary>Each list's titles as the engine last sent them, by the list.</summary>
    public Dictionary<ContentFor, ContentList> Lists { get; } = [];

    /// <summary>A title's episodes, by AniList id.</summary>
    public Dictionary<uint, EpisodeRow[]> Episodes { get; } = [];

    public SettingsView? Settings { get; private set; }
    public Dictionary<string, SyncState> Sync { get; } = [];
    public DownloadRow[] Downloads => Queue.Rows;
    public SeedRow[] Seeding { get; private set; } = [];

    /// <summary>The download queue, by title, with each running download's speed.</summary>
    public DownloadQueue Queue { get; } = new();

    /// <summary>The sources, as the engine last said them, in the order it lists them.</summary>
    public ProviderRow[] Providers { get; private set; } = [];
    public bool ProvidersLoaded { get; private set; }

    /// <summary>Why no sources are available, when none are.</summary>
    public string? ProviderNote { get; private set; }

    /// <summary>The sources are being rebuilt, from <c>SourcesChanged</c> until the rebuilt list.</summary>
    public bool Rebuilding { get; private set; }

    /// <summary>The providers in the order failover tries them, as the config now has it.</summary>
    public ProviderRow[] OrderedProviders => SourceOrder.Sorted(Providers, Settings?.Sources.Order ?? []);

    /// <summary>Every joined plugin store.</summary>
    public StoreRow[] Stores { get; private set; } = [];
    public bool StoresLoaded { get; private set; }

    /// <summary>What a pasted invite link leads to, once checked.</summary>
    public InvitePreview? Preview { get; set; }

    /// <summary>The store just joined, until the page has shown it.</summary>
    public string? Joined { get; set; }

    /// <summary>An invite link made, to show and send.</summary>
    public InviteLinkView? Invite { get; set; }

    /// <summary>A store's members and unused invites, for its owner.</summary>
    public Dictionary<string, (StoreMemberRow[] Members, StoreInviteRow[] Invites)> Members { get; } = [];

    /// <summary>The calendar's broadcasts count their <c>AiringIn</c> from this Unix time.</summary>
    public long? CalendarAnchor { get; private set; }

    /// <summary>Titles hidden from Continue in this run, until the engine's next list.</summary>
    public HashSet<uint> HiddenFromContinue { get; } = [];

    /// <summary>The AniList account, when signed in.</summary>
    public SyncState? Anilist => Sync.TryGetValue("anilist", out var s) && s.Connected ? s : null;

    /// <summary>Episodes downloading or waiting to, and those seeding: what closing the window stops.</summary>
    public (int Downloading, int Seeding) Running() =>
        (Downloads.Count(d => d.State is DownloadState.Active or DownloadState.Queued), Seeding.Length);

    /// <summary>The list's titles, each as it now stands, or none when it has not loaded.</summary>
    public Entry[] ListEntries(ContentFor view) =>
        Lists.TryGetValue(view, out var list) && list is ContentList.Entries e
            ? [.. e.V1.Select(x => Entries.GetValueOrDefault(x.Id, x))]
            : [];

    /// <summary>
    /// The list's rows as the engine sent them. The calendar's rows are broadcasts, one title
    /// several times over, each with its own episode and air time, which the titles as they now
    /// stand would lose.
    /// </summary>
    public Entry[] ListRows(ContentFor view) =>
        Lists.TryGetValue(view, out var list) && list is ContentList.Entries e ? e.V1 : [];

    public Entry? Entry(uint id) => Entries.GetValueOrDefault(id);

    public void Take(Event e)
    {
        switch (e)
        {
            case Event.Content c:
                Lists[c.View] = c.ContentValue;
                if (c.ContentValue is ContentList.Entries list)
                {
                    foreach (var entry in list.V1)
                        Entries[entry.Id] = Merge(entry);
                    if (c.View is ContentFor.Continue)
                        HiddenFromContinue.Clear();
                }
                break;
            case Event.Detail d:
                Entries[d.V1.Id] = d.V1;
                break;
            case Event.Episodes ep:
                Episodes[ep.Id] = ep.Rows;
                break;
            case Event.Progress p:
                if (Entries.TryGetValue(p.Id, out var known))
                    Entries[p.Id] = known with { Progress = p.ProgressValue, Resume = p.Resume };
                break;
            case Event.LastAired aired:
                foreach (var title in aired.V1)
                    if (Entries.TryGetValue(title.Id, out var was))
                        Entries[title.Id] = was with { LastAired = title.Aired };
                break;
            case Event.CalendarWindow w:
                CalendarAnchor = w.Anchor;
                break;
            case Event.Settings s:
                Settings = s.V1;
                break;
            case Event.Sync s:
                Sync[s.V1.Tracker] = s.V1;
                break;
            case Event.Downloads d:
                Queue.Apply(d.V1, DateTimeOffset.UtcNow);
                break;
            case Event.Providers p:
                Providers = p.V1;
                ProvidersLoaded = true;
                Rebuilding = false;
                if (p.V1.Length > 0)
                    ProviderNote = null;
                break;
            case Event.ProviderNote n:
                ProviderNote = n.V1;
                break;
            case Event.SourcesChanged:
                Rebuilding = true;
                break;
            case Event.Stores st:
                Stores = st.V1;
                StoresLoaded = true;
                break;
            case Event.StorePreview preview:
                Preview = preview.V1;
                break;
            case Event.StoreJoined joined:
                Joined = joined.Store;
                Preview = null;
                break;
            case Event.InviteLink link:
                Invite = link.V1;
                break;
            case Event.StoreMembers members:
                Members[members.Store] = (members.Members, members.Invites);
                break;
            case Event.Seeding s:
                Seeding = s.V1;
                break;
        }
    }

    /// <summary>
    /// A list's row for a title, keeping what the title's own page knew that a list row leaves
    /// out (its synopsis, relations and banner when the row has none).
    /// </summary>
    Entry Merge(Entry row)
    {
        if (!Entries.TryGetValue(row.Id, out var page))
            return row;
        return row with
        {
            Synopsis = row.Synopsis.Length > 0 ? row.Synopsis : page.Synopsis,
            BannerUrl = row.BannerUrl ?? page.BannerUrl,
            Related = row.Related is { Length: > 0 } ? row.Related : page.Related,
            Recommended = row.Recommended is { Length: > 0 } ? row.Recommended : page.Recommended,
        };
    }

    /// <summary>A setting's row as it stands.</summary>
    public SettingRow? Setting(SettingKey key) =>
        Settings is { } view ? view.Rows.Concat(view.NativeRows).FirstOrDefault(r => r.Key == key) : null;
}
