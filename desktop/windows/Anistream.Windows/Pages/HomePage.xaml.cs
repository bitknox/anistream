using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace Anistream.Windows.Pages;

/// <summary>
/// Home: the spotlight, then the engine's sections in the engine's order, as one page. The
/// engine decides which title the spotlight holds, which sections there are, what each holds and
/// what every line says; this draws it, and sends back what is done (play, Add to Planning, Not
/// interested, Mark as watched). A section that is still on its way gets placeholders from
/// 600 ms in its own place, so nothing above it moves when it lands.
/// </summary>
public sealed partial class HomePage : Page, IPageKeys
{
    readonly Ctx ctx = Ctx.Current;
    readonly Dictionary<SectionKind, FrameworkElement> shown = [];
    readonly DateTimeOffset opened = DateTimeOffset.Now;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer silence, activity;
    HomeView? drawn;
    HomeView? complained;
    uint? probed;
    bool placeholders;
    DateTimeOffset placeholdersSince;
    StackPanel? waitingPlates;

    /// <summary>Titles put on Planning from here, until the page goes: their button says so at once.</summary>
    internal HashSet<uint> PlannedHere { get; } = [];

    /// <summary>Titles whose Add to Planning is taking longer than 600 ms.</summary>
    internal HashSet<uint> AddingHere { get; } = [];

    public HomePage()
    {
        InitializeComponent();
        Spot.Use(this);
        State.Placeholders = () => States.Rows(5, 36, 54);
        State.ActivityText = T.HomeLoading;
        silence = Timers.Weak(this, TimeSpan.FromMilliseconds(HomeWording.PlaceholderAfterMs), false, static page => page.Show());
        activity = Timers.Weak(this, TimeSpan.FromMilliseconds(HomeWording.ActivityAfterMs), false, static page => page.ShowActivity());
        // Heard from the start: what arrives before the page is loaded is not missed. The shelves
        // let go of their cards once the frame leaves, so the page can go.
        ctx.Hear(this, Heard, () =>
        {
            silence.Stop();
            activity.Stop();
            Shelves.Children.Clear();
            shown.Clear();
        });
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Send(new Command.LoadHome(false));
        silence.Start();
        activity.Start();
        Show();
    }

    void OnRefresh(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        args.Handled = true;
        ctx.Send(new Command.LoadHome(true));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        var again = false;
        foreach (var e in batch)
            switch (e)
            {
                case Event.Home:
                    again = true;
                    // The engine has answered: whatever is waiting on it is done.
                    AddingHere.Clear();
                    Spot.Refresh();
                    break;
                case Event.Sync:
                case Event.Connectivity:
                    again = true;
                    break;
            }
        if (again)
            Show();
    }

    // Drawing

    void Show()
    {
        ShowSyncLine();
        var home = ctx.Known.Home;
        if (home is null)
        {
            State.Wait();
            return;
        }
        ShowOffline(home);
        var empty = home.Spotlight is null && home.Sections.Length == 0;
        if (empty && home.Offline && home.OfflineSince is null)
        {
            State.Empty(States.Empty(T.HomeOfflineEmptyTitle, T.HomeOfflineEmptyBody, T.HomeOpenDownloads, () => ctx.Window.Go("downloads")));
            return;
        }
        if (empty && home.Pending.Length > 0 && !placeholders && !SilenceOver)
        {
            State.Wait();
            return;
        }
        State.Done();

        var before = drawn;
        drawn = home;
        var moved = before is null || home.Spotlight is null || before.Spotlight is null
            ? before?.Spotlight?.Entry.Id != home.Spotlight?.Entry.Id
            : before.Spotlight.Entry.Id != home.Spotlight.Entry.Id || before.Spotlight.Case != home.Spotlight.Case;
        var fresh = Spot.Id is null;
        if (before is null || moved || Spot.Id != home.Spotlight?.Entry.Id)
            Spot.Show(home.Spotlight, crossfade: !fresh);
        else
            Spot.Refresh();
        if (home.Spotlight is { } spotlight && probed != spotlight.Entry.Id)
        {
            probed = spotlight.Entry.Id;
            ctx.Send(new Command.ProbeTitle(spotlight.Entry.Id));
        }

        Reconcile(home);
        ShowPlaceholders(home);
        ShowEnd(home);
        if (home.Failed && !ReferenceEquals(complained, home))
        {
            complained = home;
            ctx.Window.Say(T.HomeFailed, T.HomeTryAgain, () => ctx.Send(new Command.LoadHome(true)));
        }
        if (home.Pending.Length == 0)
        {
            activity.Stop();
            Motion.Hide(Activity);
        }
    }

    bool SilenceOver => DateTimeOffset.Now - opened >= TimeSpan.FromMilliseconds(HomeWording.PlaceholderAfterMs);

    void ShowSyncLine()
    {
        var waiting = ctx.Known.Anilist?.Outbox ?? 0;
        SyncLine.Text = waiting > 0 ? T.ContinueSyncWaiting(waiting) : "";
        SyncLine.Visibility = waiting > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>"Offline. Showing Home from 14:20." over a kept Home; the plain offline line over what is only local.</summary>
    void ShowOffline(HomeView home)
    {
        var on = home.Offline || ctx.Known.Offline;
        OfflineText.Text = home.OfflineSince is { } since ? HomeWording.OfflineLine(since, TimeZoneInfo.Local) : T.StateOfflineLine;
        if (on && OfflineLine.Visibility != Visibility.Visible)
            Motion.Show(OfflineLine);
        else if (!on && OfflineLine.Visibility == Visibility.Visible)
            Motion.Hide(OfflineLine);
    }

    /// <summary>
    /// Takes the engine's sections in its order. A section that is already up is updated in place;
    /// one that left fades out and closes the gap; one that arrived fades in where it belongs.
    /// </summary>
    void Reconcile(HomeView home)
    {
        var kinds = home.Sections.Select(s => s.Kind).ToHashSet();
        foreach (var kind in shown.Keys.Where(k => !kinds.Contains(k)).ToList())
            Remove(kind);
        var index = 0;
        foreach (var section in home.Sections)
        {
            if (shown.TryGetValue(section.Kind, out var element))
            {
                Update(element, section, home);
                if (Shelves.Children.IndexOf(shown[section.Kind]) != index)
                {
                    Shelves.Children.Remove(shown[section.Kind]);
                    Shelves.Children.Insert(Math.Min(index, Shelves.Children.Count), shown[section.Kind]);
                }
            }
            else
            {
                var made = Make(section, home);
                shown[section.Kind] = made;
                Shelves.Children.Insert(Math.Min(index, Shelves.Children.Count), made);
                if (drawn is not null && Spot.Id is not null)
                    Motion.FadeIn(made, Motion.Standard);
            }
            index++;
        }
        FirstMargin();
    }

    /// <summary>The first shelf sits 40 under the spotlight; with no spotlight it has nothing to clear.</summary>
    void FirstMargin()
    {
        if (Shelves.Children.FirstOrDefault() is FrameworkElement first && Spot.Visibility != Visibility.Visible)
            first.Margin = new Thickness(0);
    }

    FrameworkElement Make(HomeSection section, HomeView home) =>
        HomeWording.Shape(section.Kind) == HomeShape.Week
            ? new HomeWeek(this, section)
            : new HomeShelf(this, section, home.Season);

    void Update(FrameworkElement element, HomeSection section, HomeView home)
    {
        if (element is not HomeShelf shelf)
        {
            // This week is drawn again in its place.
            var at = Shelves.Children.IndexOf(element);
            var again = Make(section, home);
            Shelves.Children[at] = again;
            shown[section.Kind] = again;
            return;
        }
        var now = section.Items.Select(i => i.Entry.Id).ToHashSet();
        var leaving = shelf.Cards.Select(c => c.Id).Where(id => !now.Contains(id)).ToList();
        if (leaving.Count == 0)
        {
            shelf.Update(section, home.Season, []);
            return;
        }
        // Cards that left fade out, then the ones after them close the gap.
        var left = leaving.Count;
        foreach (var id in leaving)
            shelf.Leave(id, () =>
            {
                if (--left == 0 && shown.TryGetValue(section.Kind, out var current) && current == shelf)
                    shelf.Update(ctx.Known.Home?.Section(section.Kind) ?? section, ctx.Known.Home?.Season, []);
            });
    }

    void Remove(SectionKind kind)
    {
        var element = shown[kind];
        shown.Remove(kind);
        if (!Motion.Enabled)
        {
            Shelves.Children.Remove(element);
            return;
        }
        // Fades out, then the shelves below close the gap on the standard token.
        var height = element.ActualHeight;
        Motion.Fade(element, 0);
        _ = Task.Delay(Motion.FadeOutTime).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
        {
            Motion.Ease(element, height, 0, v => element.Height = Math.Max(0, v));
            _ = Task.Delay(Motion.Standard).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => Shelves.Children.Remove(element)));
        }));
    }

    // Placeholders and the activity line

    void ShowPlaceholders(HomeView home)
    {
        var want = home.Pending.Length > 0 && SilenceOver;
        if (want && !placeholders)
        {
            placeholders = true;
            placeholdersSince = DateTimeOffset.Now;
            waitingPlates = new StackPanel();
            foreach (var _ in home.Pending.Where(p => p != HomePart.Week).DefaultIfEmpty(HomePart.Week))
                waitingPlates.Children.Add(Plates(true));
            if (home.Pending.Contains(HomePart.Week))
                waitingPlates.Children.Add(Plates(false));
            Waiting.Children.Add(waitingPlates);
            Motion.FadeIn(waitingPlates);
        }
        else if (!want && placeholders && waitingPlates is { } plates)
        {
            placeholders = false;
            waitingPlates = null;
            var left = TimeSpan.FromMilliseconds(HomeWording.PlaceholderLeastMs) - (DateTimeOffset.Now - placeholdersSince);
            // One crossfade, held at least 0.3 s, and the plates are the real shelf's size.
            _ = Task.Delay(left > TimeSpan.Zero ? left : TimeSpan.Zero).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
            {
                Motion.Hide(plates);
                _ = Task.Delay(Motion.FadeOutTime).ContinueWith(_ => DispatcherQueue.TryEnqueue(() => Waiting.Children.Remove(plates)));
            }));
        }
    }

    /// <summary>A heading bar and a row of plates at the final size of a poster shelf, or of the week.</summary>
    static StackPanel Plates(bool posters)
    {
        var panel = new StackPanel { Margin = new Thickness(0, HomeWording.ShelfGap, 0, 0), Spacing = 12 };
        panel.Children.Add(States.Plate(160, 20, 4));
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = HomeWording.Gap };
        for (var i = 0; i < 7; i++)
            row.Children.Add(States.Plate(posters ? HomeWording.PosterWidth : 150, posters ? HomeWording.PosterHeight : 66, 4));
        panel.Children.Add(row);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(panel, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return panel;
    }

    void ShowActivity()
    {
        if (ctx.Known.Home is { Pending.Length: > 0 })
        {
            Activity.Text = T.HomeLoading;
            Motion.Show(Activity);
        }
    }

    // The closing line

    void ShowEnd(HomeView home)
    {
        var key = home.End?.ToString();
        if ((EndLine.Tag as string) == key)
            return;
        EndLine.Tag = key;
        EndLine.Children.Clear();
        if (home.End is not { } end)
            return;
        var line = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 16,
            Margin = new Thickness(0, 32, 0, 0),
            BorderBrush = Ui.Res<Brush>("FaintRuleBrush"),
            BorderThickness = new Thickness(0, 1, 0, 0),
            Padding = new Thickness(0, 16, 0, 0),
        };
        switch (end)
        {
            case HomeEnd.SignIn:
                line.Children.Add(Ui.Haze(T.HomeEndSignedOut, wrap: false));
                var signIn = new Button { Content = T.HomeSignIn, MinHeight = 32 };
                signIn.Click += (_, _) => ctx.Send(new Command.Connect("anilist"));
                line.Children.Add(signIn);
                break;
            case HomeEnd.NoSources:
                line.Children.Add(Linked(T.HomeEndNoSources, (T.NavPluginStores, () => ctx.Window.Go("stores"))));
                break;
            default:
                line.Children.Add(Linked(T.HomeEnd, (T.NavSeasonal, () => ctx.Window.Go("seasonal")), (T.NavCalendar, () => ctx.Window.Go("calendar"))));
                break;
        }
        EndLine.Children.Add(line);
        Motion.FadeIn(line);
    }

    /// <summary>A haze sentence with some of its words as links.</summary>
    static TextBlock Linked(string sentence, params (string Words, Action Go)[] links)
    {
        var block = new TextBlock { Style = Ui.Res<Style>("HazeBodyStyle"), TextWrapping = TextWrapping.Wrap };
        var rest = sentence;
        while (rest.Length > 0)
        {
            var next = links.Select(l => (l, At: rest.IndexOf(l.Words, StringComparison.CurrentCulture))).Where(x => x.At >= 0).OrderBy(x => x.At).Cast<((string Words, Action Go) L, int At)?>().FirstOrDefault();
            if (next is not { } hit)
            {
                block.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = rest });
                break;
            }
            if (hit.At > 0)
                block.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = rest[..hit.At] });
            var link = new Microsoft.UI.Xaml.Documents.Hyperlink();
            link.Inlines.Add(new Microsoft.UI.Xaml.Documents.Run { Text = hit.L.Words });
            var go = hit.L.Go;
            link.Click += (_, _) => go();
            block.Inlines.Add(link);
            rest = rest[(hit.At + hit.L.Words.Length)..];
        }
        return block;
    }

    // What a card or the spotlight does

    internal void OpenTitle(uint id) => ctx.OpenTitle(id);

    /// <summary>A click: a Continue card plays what is next; a poster opens its title.</summary>
    internal void OpenCard(HomeCard card, HomeShape shape)
    {
        if (shape == HomeShape.Card && card.Item.Action is HomeAction.Resume or HomeAction.Play && card.Item.Episode is { } episode)
            ctx.Play(card.Id, episode.ToString());
        else
            ctx.OpenTitle(card.Id);
    }

    internal void OpenViewAll(SectionKind kind, HomeLink? link)
    {
        switch (link)
        {
            case HomeLink.Watching:
                ctx.Window.Go("library");
                break;
            case HomeLink.SeasonalView:
                ctx.Window.Go("seasonal");
                break;
            case HomeLink.Browse browse:
                ctx.Window.ShowSearch(browse.Query);
                break;
            default:
                ctx.Window.Go(kind == SectionKind.ThisWeek || link is HomeLink.CalendarView ? "calendar" : "seasonal");
                break;
        }
    }

    internal void MainAction(Spotlight spot, HomeAction action)
    {
        switch (action)
        {
            case HomeAction.Resume or HomeAction.Play:
                ctx.Play(spot.Entry.Id, spot.Episode.ToString());
                break;
            case HomeAction.AddToPlanning:
                AddToPlanning(spot.Entry.Id);
                break;
        }
    }

    internal void SecondAction(Spotlight spot, SpotSecond what)
    {
        if (what == SpotSecond.Add)
            AddToPlanning(spot.Entry.Id);
        else if (what == SpotSecond.Details)
            ctx.OpenTitle(spot.Entry.Id);
    }

    /// <summary>Add to Planning: "Adding" only if saving takes 600 ms, then "On your Planning list" with a check.</summary>
    internal void AddToPlanning(uint id)
    {
        ctx.Send(new Command.SetStatus(id, LibrarySegment.Planning));
        ctx.Known.PlannedFromUpNext(id);
        PlannedHere.Add(id);
        AddingHere.Add(id);
        Spot.Refresh();
        _ = Task.Delay(HomeWording.PlaceholderAfterMs).ContinueWith(_ => DispatcherQueue.TryEnqueue(() =>
        {
            AddingHere.Remove(id);
            Spot.Refresh();
        }));
    }

    internal void MarkWatched(uint id, uint episode)
    {
        ctx.Send(new Command.SetWatched(id, [episode.ToString()], true));
        ctx.Window.Say(T.HomeMarkedWatched(episode));
    }

    internal void RemoveFromContinue(uint id)
    {
        ctx.Known.HiddenFromContinue.Add(id);
        ctx.Send(new Command.HideFromContinue(id));
    }

    /// <summary>
    /// Not interested: the title leaves every shelf at once, and a notice says so with Undo, for
    /// 8 s. The engine sends the sections that held it again from what it kept.
    /// </summary>
    internal void NotInterested(Entry entry, bool untilAirs)
    {
        var id = entry.Id;
        ctx.Known.DismissedHere.Add(id);
        ctx.Send(new Command.DismissUpNext(id));
        foreach (var shelf in shown.Values.OfType<HomeShelf>())
            if (shelf.Cards.Any(c => c.Id == id))
                shelf.Leave(id, () => { });
        ctx.Window.Say(untilAirs ? T.UpNextHiddenUntilAirs(entry.Title) : T.UpNextHidden(entry.Title), T.ActionUndo, () =>
        {
            ctx.Known.DismissedHere.Remove(id);
            ctx.Send(new Command.RestoreUpNext(id));
        });
    }

    internal MenuFlyout MenuFor(HomeCard card, HomeShape shape)
    {
        var menu = new MenuFlyout();
        var item = card.Item;
        MenuFlyoutItem Add(string text, string glyph, Action act, string? keys = null)
        {
            var entry = new MenuFlyoutItem { Text = text, Icon = new FontIcon { Glyph = glyph } };
            if (keys is not null)
                entry.KeyboardAcceleratorTextOverride = keys;
            entry.Click += (_, _) => act();
            menu.Items.Add(entry);
            return entry;
        }
        var episode = item.Episode ?? 1;
        if (shape == HomeShape.Card)
        {
            var resume = item.Resume is not null;
            if (item.Action is HomeAction.Resume or HomeAction.Play)
            {
                Add(resume ? T.ContinueMenuResume : T.ActionPlay, "", () => ctx.Play(card.Id, episode.ToString()));
                if (resume)
                    Add(T.EpisodePlayFromBeginning, "", () => ctx.Play(card.Id, episode.ToString(), fromStart: true));
            }
            Add(T.HomeMarkWatched(episode), "", () => MarkWatched(card.Id, episode), "Ctrl+Shift+M");
            Add(T.HomeRemoveContinue, "", () => RemoveFromContinue(card.Id), "Delete");
        }
        else
        {
            if (item.Action == HomeAction.Play && item.Playable != Playable.NotFound)
                Add(T.HomePlayEpisode(episode), "", () => ctx.Play(card.Id, episode.ToString()));
            if (item.Action != HomeAction.Nothing && item.Reason != UpNextReason.Planning)
                Add(T.UpNextAddPlanning, "", () => AddToPlanning(card.Id));
            if (shape == HomeShape.Poster)
                Add(T.UpNextNotInterested, "", () => NotInterested(card.Item.Entry, card.Item.Starts is not null), "Delete");
        }
        menu.Items.Add(new MenuFlyoutSeparator());
        Add(T.HomeDetails, "", () => ctx.OpenTitle(card.Id));
        return menu;
    }

    internal void CardKey(HomeCard card, HomeShape shape, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Delete)
            return;
        e.Handled = true;
        if (shape == HomeShape.Card)
            RemoveFromContinue(card.Id);
        else
            NotInterested(card.Item.Entry, card.Item.Starts is not null);
    }

    // Ctrl+Shift+M and Delete, from the window, on the focused card.

    (HomeCard Card, HomeShape Shape)? Focused()
    {
        if (FocusManager.GetFocusedElement(XamlRoot) is FrameworkElement { Tag: HomeCard card })
            return (card, card.Item.Progress is not null || card.Item.Resume is not null || Shelves.Children.OfType<HomeShelf>().Any(s => s.Kind == SectionKind.Continue && s.Cards.Contains(card)) ? HomeShape.Card : HomeShape.Poster);
        return null;
    }

    bool IPageKeys.MarkWatched()
    {
        if (Focused() is { Shape: HomeShape.Card } focused && focused.Card.Item.Episode is { } episode)
        {
            MarkWatched(focused.Card.Id, episode);
            return true;
        }
        return false;
    }

    bool IPageKeys.Delete() => false;
}
