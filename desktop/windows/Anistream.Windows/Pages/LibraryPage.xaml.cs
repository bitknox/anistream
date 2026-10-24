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
        (LibrarySegment.Watching, "Watching"),
        (LibrarySegment.Planning, "Planning"),
        (LibrarySegment.Completed, "Completed"),
        (LibrarySegment.Paused, "Paused"),
        (LibrarySegment.Dropped, "Dropped"),
    ];

    readonly Ctx ctx = Ctx.Current;
    LibrarySegment segment = LibrarySegment.Watching;
    string order = "updated";

    public LibraryPage()
    {
        InitializeComponent();
        NagareSlot.Child = new Nagare(5);
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
            item.Text = count is { } n ? $"{name} {n}" : name;
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
        var loaded = ctx.Known.Lists.ContainsKey(new ContentFor.Library(segment));
        Empty.Text = $"Nothing on {Segments.First(x => x.Segment == segment).Name} yet.";
        Empty.Visibility = loaded && covers.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
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
            Show();
        }
    }

    void OnConnect(object sender, RoutedEventArgs e) => ctx.Send(new Command.Connect("anilist"));
}
