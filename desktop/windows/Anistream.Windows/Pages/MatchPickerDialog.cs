using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Anistream.Windows.Pages;

/// <summary>
/// The match picker: which of a source's titles a show is, when the source could not tell or got
/// it wrong. The titles the source offered, a search of the source by hand, the match there is
/// now and Match automatically to forget a choice. It opens from the title page's Choose the
/// match, from Fix this match in the More menu and from the source picker's last row, and closes
/// itself when a pick settles. Return in the field searches new words, or picks the selected row
/// when the words are the search's; Up and Down move through the rows; Esc closes.
/// </summary>
sealed class MatchPickerDialog
{
    static readonly TimeSpan Silence = TimeSpan.FromMilliseconds(600);

    readonly Ctx ctx;
    readonly uint id;
    readonly int settledBefore;
    readonly ContentDialog dialog;
    readonly TextBox field = new() { MinWidth = 440 };
    readonly StackPanel body = new() { Spacing = 8, MinHeight = 300 };
    readonly DispatcherQueueTimer searchTimer, pickTimer;
    string? selected;
    bool searchSlow, pickSlow;
    readonly List<(string Key, ListViewItem Item)> rows = [];

    MatchPickerDialog(Ctx ctx, uint id, string show, XamlRoot root)
    {
        this.ctx = ctx;
        this.id = id;
        settledBefore = ctx.Known.Matches.Settled is { } s && s.Id == id ? s.Serial : 0;

        var name = Source()?.Name ?? "";
        field.PlaceholderText = T.MatchSearchField(name);
        AutomationProperties.SetName(field, T.MatchSearchField(name));
        field.KeyDown += OnKey;

        var top = new StackPanel { Spacing = 6 };
        top.Children.Add(Ui.Text(show, "ShowTitleStyle", wrap: false));
        if (ctx.Known.Entry(id) is { } entry && MatchWords.AniList(entry) is { Length: > 0 } facts)
            top.Children.Add(Ui.Haze(facts));
        top.Children.Add(field);
        top.Children.Add(new ScrollViewer { MaxHeight = 380, Content = body, VerticalScrollBarVisibility = ScrollBarVisibility.Auto });

        var queue = DispatcherQueue.GetForCurrentThread();
        searchTimer = queue.CreateTimer();
        searchTimer.Interval = Silence;
        searchTimer.IsRepeating = false;
        searchTimer.Tick += (_, _) => { searchSlow = true; Fill(); };
        pickTimer = queue.CreateTimer();
        pickTimer.Interval = Silence;
        pickTimer.IsRepeating = false;
        pickTimer.Tick += (_, _) => { pickSlow = true; Fill(); };

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.MatchChoose,
            Content = top,
            CloseButtonText = T.ActionCancel,
            SecondaryButtonText = "",
            DefaultButton = ContentDialogButton.None,
        };
        dialog.SecondaryButtonClick += (_, _) => MatchAutomatically();
        dialog.Opened += (_, _) => field.Focus(FocusState.Keyboard);
    }

    /// <summary>
    /// Opens the picker for title <paramref name="id"/>. <paramref name="fromMenu"/> is Fix this
    /// match, which fills the field with the title and searches at once; the page's question shows
    /// what the source offered instead. <paramref name="query"/> fills the field with other words.
    /// </summary>
    public static async Task Open(Ctx ctx, uint id, string show, XamlRoot root, bool fromMenu, string? query = null)
    {
        var picker = new MatchPickerDialog(ctx, id, show, root);
        ctx.Batch += picker.Heard;
        if (query is not null)
            picker.field.Text = query;
        else if (fromMenu && show.Length > 0)
            picker.field.Text = show;
        picker.FirstPick();
        if (query is not null || fromMenu)
            picker.Search();
        picker.Fill();
        try
        {
            await Dialogs.Show(picker.dialog);
        }
        finally
        {
            ctx.Batch -= picker.Heard;
            picker.searchTimer.Stop();
            picker.pickTimer.Stop();
            ctx.Known.Matches.Closed(id);
        }
    }

    /// <summary>The source the picker searches: the one that asked or is matched, else the first one in use.</summary>
    (string ProviderId, string Name)? Source() =>
        ctx.Known.Matches.SourceFor(id)
        ?? ctx.Known.OrderedProviders.Where(p => p.Usable).Select(p => ((string, string)?)(p.Id, p.Name)).FirstOrDefault();

    void Heard(IReadOnlyList<Event> batch)
    {
        if (!batch.Any(e => e is Event.MatchChoices { } m && m.Id == id
                || e is Event.MatchSearchFailed f && f.Id == id
                || e is Event.MatchFixFailed x && x.Id == id
                || e is Event.TitleMatch t && t.Id == id))
            return;
        if (ctx.Known.Matches.Settled is { } settled && settled.Id == id && settled.Serial > settledBefore)
        {
            dialog.Hide();
            return;
        }
        FirstPick();
        Fill();
    }

    MatchCandidate[]? Candidates(out string providerId, out string source)
    {
        var book = ctx.Known.Matches;
        providerId = source = "";
        switch (book.Search(id))
        {
            case MatchSearch.Found f:
                (providerId, source) = (f.ProviderId, f.Source);
                return f.Candidates;
            case null when book.Asked(id) is { } asked:
                (providerId, source) = (asked.ProviderId, asked.Source);
                return asked.Candidates;
            default:
                return null;
        }
    }

    /// <summary>The best row is selected until the person moves: the first one not ruled out.</summary>
    void FirstPick()
    {
        if (Candidates(out _, out _) is not { } candidates)
        {
            selected = null;
            return;
        }
        if (selected is null || !candidates.Any(c => c.Key == selected))
            selected = (candidates.FirstOrDefault(c => c.Rejected is null) ?? candidates.FirstOrDefault())?.Key;
    }

    void Search()
    {
        var query = field.Text.Trim();
        if (query.Length == 0 || Source() is not { } source)
            return;
        ctx.Known.Matches.Searching(id, query);
        selected = null;
        searchSlow = false;
        searchTimer.Stop();
        searchTimer.Start();
        ctx.Send(new Command.ManualSearch(id, query, source.ProviderId));
        Fill();
    }

    void OnKey(object sender, KeyRoutedEventArgs e)
    {
        switch (e.Key)
        {
            case global::Windows.System.VirtualKey.Enter:
                e.Handled = true;
                Submitted();
                break;
            case global::Windows.System.VirtualKey.Down:
                e.Handled = true;
                Step(1);
                break;
            case global::Windows.System.VirtualKey.Up:
                e.Handled = true;
                Step(-1);
                break;
        }
    }

    /// <summary>Return in the field: new words search, the same ones pick the selected row.</summary>
    void Submitted()
    {
        var typed = field.Text.Trim();
        if (typed.Length > 0 && ctx.Known.Matches.Search(id)?.Query != typed)
            Search();
        else if (Candidates(out var providerId, out var source) is { } candidates
            && candidates.FirstOrDefault(c => c.Key == selected) is { } candidate)
            Pick(candidate, providerId, source);
    }

    void Step(int step)
    {
        if (rows.Count == 0)
            return;
        var at = rows.FindIndex(r => r.Key == selected);
        var next = at < 0 ? (step > 0 ? 0 : rows.Count - 1) : Math.Clamp(at + step, 0, rows.Count - 1);
        selected = rows[next].Key;
        Fill();
    }

    void Pick(MatchCandidate candidate, string providerId, string source)
    {
        if (ctx.Known.Matches.Picking(id) is not null)
            return;
        selected = candidate.Key;
        ctx.Known.Matches.Pick(id, candidate, source);
        pickSlow = false;
        pickTimer.Stop();
        pickTimer.Start();
        ctx.Send(new Command.FixMatch(id, providerId, candidate.Key, candidate.Title));
        Fill();
    }

    void MatchAutomatically()
    {
        ctx.Known.Matches.Cleared(id);
        ctx.Window.Say(T.MatchCleared);
        ctx.Send(new Command.ClearMatch(id));
    }

    /// <summary>The list area from what the book holds now.</summary>
    void Fill()
    {
        body.Children.Clear();
        rows.Clear();
        var book = ctx.Known.Matches;
        var now = book.Current(id);
        var picking = book.Picking(id);
        // The button that forgets a choice shows only while the match is the person's.
        dialog.SecondaryButtonText = now is { ChosenByYou: true } ? T.MatchAutomatic : "";

        if (now is not null && !book.Asks(id))
        {
            body.Children.Add(Heading(T.MatchNow));
            var line = new Grid { Padding = new Thickness(12, 10, 12, 10), Background = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("RaisedBrush"), CornerRadius = new CornerRadius(6) };
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            line.Children.Add(Ui.Text(now.Title ?? now.Source, "BodyTextBlockStyle", wrap: false));
            var how = Ui.Haze(now.ChosenByYou ? T.MatchChosenByYou : T.MatchChosenAutomatically, wrap: false);
            Grid.SetColumn(how, 1);
            line.Children.Add(how);
            body.Children.Add(line);
        }

        switch (book.Search(id))
        {
            case MatchSearch.Searching s:
                body.Children.Add(Heading(T.MatchSectionSource(Source()?.Name ?? "")));
                var wait = new StackPanel { Spacing = 10, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 60, 0, 0), Opacity = searchSlow ? 1 : 0 };
                wait.Children.Add(new ProgressRing { IsActive = searchSlow, Width = 24, Height = 24 });
                var said = Ui.Haze(T.MatchSearching(Source()?.Name ?? "", s.Query));
                said.TextAlignment = TextAlignment.Center;
                wait.Children.Add(said);
                body.Children.Add(wait);
                break;
            case MatchSearch.Empty e:
                body.Children.Add(Said(false, T.MatchSearchEmpty(e.Source, e.Query), T.MatchSearchEmptyHint, retry: false));
                break;
            case MatchSearch.Failed f:
                body.Children.Add(Said(true, T.MatchSearchFailed(Source()?.Name ?? ""), f.Reason, retry: true));
                break;
            default:
                if (Candidates(out var providerId, out var source) is { } candidates)
                {
                    body.Children.Add(Heading(T.MatchSectionSource(source)));
                    var list = new ListView { SelectionMode = ListViewSelectionMode.Single, IsItemClickEnabled = true };
                    AutomationProperties.SetName(list, T.MatchSectionSource(source));
                    list.ItemClick += (_, e) =>
                    {
                        if (e.ClickedItem is ListViewItem { Tag: ValueTuple<MatchCandidate, string, string> t })
                            Pick(t.Item1, t.Item2, t.Item3);
                    };
                    foreach (var candidate in candidates)
                        list.Items.Add(Row(candidate, providerId, source, picking));
                    list.SelectedItem = rows.FirstOrDefault(r => r.Key == selected).Item;
                    body.Children.Add(list);
                    if (book.PickFailed(id) is { } why)
                        body.Children.Add(Said(true, T.MatchFixFailed, why, retry: false));
                }
                break;
        }
    }

    static TextBlock Heading(string text)
    {
        var heading = Ui.Text(text, "BodyStrongTextBlockStyle", wrap: false);
        heading.Margin = new Thickness(6, 8, 0, 0);
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
        return heading;
    }

    /// <summary>A state said in the list's place: no results, or a failure with its reason and a way to try again.</summary>
    StackPanel Said(bool dot, string text, string detail, bool retry)
    {
        var column = new StackPanel { Spacing = 6, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 60, 0, 0) };
        var head = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center };
        if (dot)
            head.Children.Add(Ui.Dot(Ui.Vermilion));
        head.Children.Add(Ui.Text(text, "BodyStrongTextBlockStyle"));
        column.Children.Add(head);
        var line = Ui.Haze(detail);
        line.TextAlignment = TextAlignment.Center;
        column.Children.Add(line);
        if (retry)
        {
            var again = Ui.Label(T.ActionTryAgain, (_, _) => Search());
            again.HorizontalAlignment = HorizontalAlignment.Center;
            again.Margin = new Thickness(0, 6, 0, 0);
            column.Children.Add(again);
        }
        return column;
    }

    /// <summary>
    /// One candidate: its name and facts, and how close it is or why it was ruled out. While a
    /// pick runs the others dim, and the one picked says Matching after 600 ms.
    /// </summary>
    ListViewItem Row(MatchCandidate candidate, string providerId, string source, string? picking)
    {
        var facts = MatchWords.Facts(candidate);
        var closeness = MatchWords.Closeness(candidate);
        var picked = picking == candidate.Key;
        var item = new ListViewItem
        {
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
            MinHeight = 52,
            IsEnabled = picking is null || picked,
            Opacity = picking is not null && pickSlow && !picked ? 0.5 : candidate.Rejected is not null ? 0.7 : 1,
        };
        AutomationProperties.SetName(item, T.MatchRowLabel(candidate.Title, facts, closeness));
        var grid = new Grid { ColumnSpacing = 12 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel { VerticalAlignment = VerticalAlignment.Center, Padding = new Thickness(0, 6, 0, 6) };
        words.Children.Add(Ui.Text(candidate.Title, "BodyTextBlockStyle", wrap: false));
        if (facts.Length > 0)
            words.Children.Add(Ui.Text(facts, "HazeCaptionStyle", wrap: false));
        grid.Children.Add(words);
        var end = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        if (picked && pickSlow)
            end.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
        end.Children.Add(Ui.Text(picked && pickSlow ? T.MatchPicking : closeness, "HazeCaptionStyle", wrap: false));
        Grid.SetColumn(end, 1);
        grid.Children.Add(end);
        item.Content = grid;
        item.Tag = (candidate, providerId, source);
        rows.Add((candidate.Key, item));
        return item;
    }
}
