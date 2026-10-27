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

    /// <summary>
    /// Whether a title has a dub, by AniList id, once the engine has said. A title its source
    /// could not say for counts as without one.
    /// </summary>
    public Dictionary<uint, bool> Dubbed { get; } = [];

    public SettingsView? Settings { get; private set; }

    /// <summary>What the search filters choose from, once the engine has said.</summary>
    public SearchFacets? Facets { get; private set; }
    public Dictionary<string, SyncState> Sync { get; } = [];
    public DownloadRow[] Downloads => Queue.Rows;
    public SeedRow[] Seeding { get; private set; } = [];

    /// <summary>The download queue, by title, with each running download's speed.</summary>
    public DownloadQueue Queue { get; } = new();

    /// <summary>The source picker's listings per title, and which pins have been said to be unavailable.</summary>
    public SourceChoices SourceChoices { get; } = new();

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

    /// <summary>The search asked last, which a change of title language asks again.</summary>
    ContentFor.Search? lastSearch;

    /// <summary>The calendar's broadcasts count their <c>AiringIn</c> from this Unix time.</summary>
    public long? CalendarAnchor { get; private set; }

    /// <summary>Titles hidden from Continue in this run, until the engine's next list.</summary>
    public HashSet<uint> HiddenFromContinue { get; } = [];

    /// <summary>The engine has no network path: from <c>Connectivity</c>, fed by the window's network watch.</summary>
    public bool Offline { get; private set; }

    /// <summary>
    /// The torrent connection check's last verdict and when it came, or null while torrents are
    /// off or before the first check.
    /// </summary>
    public (GuardView View, DateTimeOffset At)? Guard { get; private set; }

    /// <summary>What the update check last said, or null before it has said anything.</summary>
    public UpdateStatus? Update { get; private set; }

    /// <summary>The watch party this device is in, or null when it is in none.</summary>
    public PartyView? Party { get; private set; }

    /// <summary>The party's members, this device's first, once the room has listed them.</summary>
    public PartyMember[]? PartyMembers { get; private set; }

    /// <summary>What the form that starts a party opens with, once the engine has said.</summary>
    public PartyForm? PartyForm { get; set; }

    /// <summary>The AniList account, when signed in.</summary>
    public SyncState? Anilist => Sync.TryGetValue("anilist", out var s) && s.Connected ? s : null;

    /// <summary>Episodes downloading or waiting to, and those seeding: what closing the window stops.</summary>
    public (int Downloading, int Seeding) Running() =>
        (Downloads.Count(d => d.State is DownloadState.Active or DownloadState.Queued), Seeding.Length);

    /// <summary>The list's titles, each as it now stands, or none when it has not loaded.</summary>
    public Entry[] ListEntries(ContentFor view) =>
        List(view) is ContentList.Entries e
            ? [.. e.V1.Select(x => Entries.GetValueOrDefault(x.Id, x))]
            : [];

    /// <summary>
    /// The list's rows as the engine sent them. The calendar's rows are broadcasts, one title
    /// several times over, each with its own episode and air time, which the titles as they now
    /// stand would lose.
    /// </summary>
    public Entry[] ListRows(ContentFor view) =>
        List(view) is ContentList.Entries e ? e.V1 : [];

    /// <summary>
    /// The list as the engine last sent it, or null. A search is found by what it asks: the query
    /// the engine sends back carries arrays of its own, which a record compares by reference.
    /// </summary>
    public ContentList? List(ContentFor view) =>
        Lists.TryGetValue(view, out var list) ? list
        : view is ContentFor.Search ? Lists.FirstOrDefault(p => Searches.Same(p.Key, view)).Value
        : null;

    public Entry? Entry(uint id) => Entries.GetValueOrDefault(id);

    public void Take(Event e)
    {
        switch (e)
        {
            case Event.Content c:
                // One key per search, whichever instance of its query asked it.
                var key = c.View is ContentFor.Search && !Lists.ContainsKey(c.View)
                    ? Lists.Keys.FirstOrDefault(k => Searches.Same(k, c.View)) ?? c.View
                    : c.View;
                Lists[key] = c.ContentValue;
                if (key is ContentFor.Search search)
                    lastSearch = search;
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
            case Event.SourceOptions:
                SourceChoices.Take(e);
                break;
            case Event.TitleDub td:
                Dubbed[td.Id] = td.Dub is not null;
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
            case Event.Facets f:
                Facets = f.V1;
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
            case Event.Vpn v:
                Guard = v.V1 is { } view ? (view, DateTimeOffset.UtcNow) : null;
                break;
            case Event.Connectivity c:
                Offline = c.Offline;
                break;
            case Event.PartyForm f:
                PartyForm = f.V1;
                break;
            case Event.PartyJoined j:
                // A room joined afresh lists its members afresh.
                if (Party is null || Party.Server != j.V1.Server || Party.Room != j.V1.Room)
                    PartyMembers = null;
                Party = j.V1;
                break;
            case Event.PartyState p:
                Party = p.V1;
                break;
            case Event.PartyMembers m:
                PartyMembers = m.V1;
                break;
            case Event.PartyLeft:
                Party = null;
                PartyMembers = null;
                break;
            case Event.Update u:
                // A check that found nothing new leaves a release found before on offer only
                // while it is still the newest; the engine says UpToDate when it is not.
                Update = u.V1;
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

    /// <summary>
    /// What to ask for again once every title is named another way (<c>TitlesChanged</c>): each
    /// list that has been shown and the search asked last, so what is on screen names its titles
    /// the new way. A title page asks for its own title again.
    /// </summary>
    public IEnumerable<Command> AskAgain()
    {
        foreach (var view in Lists.Keys)
        {
            switch (view)
            {
                case ContentFor.Continue:
                    yield return new Command.LoadContinue();
                    break;
                case ContentFor.Seasonal:
                    yield return new Command.LoadSeasonal();
                    break;
                case ContentFor.Calendar:
                    yield return new Command.LoadCalendar();
                    break;
                case ContentFor.Library library:
                    yield return new Command.LoadLibrary(library.V1);
                    break;
            }
        }
        if (lastSearch is { } search)
            yield return new Command.Search(search.Query);
    }

    /// <summary>A setting's row as it stands.</summary>
    public SettingRow? Setting(SettingKey key) =>
        Settings is { } view ? view.Rows.Concat(view.NativeRows).FirstOrDefault(r => r.Key == key) : null;
}
