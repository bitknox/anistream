using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Anistream.Windows.Pages;

/// <summary>
/// Library: the tracker's lists, each a poster grid with its count, ordered by last updated,
/// title or score. Signed out, Nagare and Connect AniList.
/// </summary>
public sealed partial class LibraryPage : Page
{
    static readonly (LibrarySegment Segment, string Name)[] Segments =
    [
        (LibrarySegment.Watching, T.LibraryStatusWatching),
        (LibrarySegment.Planning, T.LibraryStatusPlanning),
        (LibrarySegment.Completed, T.LibraryStatusCompleted),
        (LibrarySegment.Paused, T.LibraryStatusPaused),
        (LibrarySegment.Dropped, T.LibraryStatusDropped),
    ];

    readonly Ctx ctx = Ctx.Current;
    LibrarySegment segment = LibrarySegment.Watching;
    string order = "updated";

    public LibraryPage()
    {
        InitializeComponent();
        SignedOut.Child = States.Empty(T.LibrarySignedOutTitle, T.LibrarySignedOutDetail, T.LibrarySignedOutAction, () => ctx.Send(new Command.Connect("anilist")));
        if (SignedOut.Child is StackPanel panel && panel.Children[^1] is Button signIn)
            signIn.Style = Ui.Res<Style>("AccentButtonStyle");
        State.Placeholders = () => States.Covers();
        State.ActivityText = T.LibraryLoading;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(OrderButton, T.LibrarySortLabel(T.LibrarySortUpdated));
        foreach (var (s, name) in Segments)
            Lists.Items.Add(new SelectorBarItem { Text = name, Tag = s, IsSelected = s == segment });
        // Heard from the start: what arrives before the page is loaded is not missed.
        ctx.Batch += Heard;
        Unloaded += (_, _) => ctx.Batch -= Heard;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        foreach (var (s, _) in Segments)
            ctx.Send(new Command.LoadLibrary(s));
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Content { View: ContentFor.Library } or Event.Sync or Event.Progress))
            Show();
    }

    void Show()
    {
        var signedIn = ctx.Known.Anilist is not null;
        SignedOut.Visibility = signedIn ? Visibility.Collapsed : Visibility.Visible;
        Bar.Visibility = Scroll.Visibility = signedIn ? Visibility.Visible : Visibility.Collapsed;
        if (!signedIn)
            return;
        foreach (var item in Lists.Items)
        {
            var s = (LibrarySegment)item.Tag;
            var name = Segments.First(x => x.Segment == s).Name;
            var count = ctx.Known.Lists.TryGetValue(new ContentFor.Library(s), out var list) && list is ContentList.Entries e ? e.V1.Length : (int?)null;
            item.Text = count is { } n ? T.LibraryListCount(name, n) : name;
        }
        IEnumerable<Entry> entries = ctx.Known.ListEntries(new ContentFor.Library(segment));
        entries = order switch
        {
            "title" => entries.OrderBy(e => e.Title, StringComparer.CurrentCultureIgnoreCase),
            "score" => entries.OrderByDescending(e => e.Score ?? 0),
            _ => entries,
        };
        var covers = entries.Select(e => new CoverItem(e).Load(ctx.Art)).ToList();
        Grid.Covers = covers;
        ctx.Known.Lists.TryGetValue(new ContentFor.Library(segment), out var shown);
        if (covers.Count > 0)
            State.Done();
        else if (shown is ContentList.Failed f)
            State.Failed(f.V1, () => ctx.Send(new Command.LoadLibrary(segment)));
        else if (shown is ContentList.Entries or ContentList.Empty)
            State.Empty(States.Unavailable(T.LibraryListEmpty(Segments.First(x => x.Segment == segment).Name.ToLower(System.Globalization.CultureInfo.CurrentCulture)), T.LibraryListEmptyDetail));
        else
            State.Wait();
    }

    void OnListChanged(SelectorBar sender, SelectorBarSelectionChangedEventArgs e)
    {
        if (sender.SelectedItem?.Tag is LibrarySegment s)
        {
            segment = s;
            Show();
        }
    }

    void OnOrder(object sender, RoutedEventArgs e)
    {
        if (sender is RadioMenuFlyoutItem item)
        {
            order = (string)item.Tag;
            OrderButton.Content = item.Text;
            Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(OrderButton, T.LibrarySortLabel(item.Text));
            Show();
        }
    }

}
