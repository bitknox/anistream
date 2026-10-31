using System.Diagnostics;
using Anistream.App;
using Anistream.Engine.Model;
using Anistream.Windows.Pages;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;

namespace Anistream.Windows.Discussion;

/// <summary>
/// An episode's discussion: AniList's forum thread in a 420 epx pane on the trailing side, after
/// the board. The engine says the whole discussion on every change; the pane keeps a row per
/// comment and updates them, so focus, the opened spoilers and the scroll place stay put. Nothing
/// shows for 600 ms, then placeholders at their final size, one crossfade to the comments, and
/// "Still loading from AniList" at 4 s.
/// </summary>
sealed class DiscussionPane : Grid
{
    /// <summary>The pane's width; below <see cref="OverlayBelow"/> of window it covers the page instead.</summary>
    public const double PaneWidth = 420;
    public const double OverlayBelow = 1000;

    static readonly TimeSpan Quiet = TimeSpan.FromMilliseconds(600), Slow = TimeSpan.FromSeconds(4);

    readonly Ctx ctx;
    readonly uint media;
    readonly uint episode;
    DiscussionView? view;
    readonly Stopwatch since = Stopwatch.StartNew();
    DateTime? limitedAt;
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer ticking;
    bool closed;

    readonly TextBlock count = Ui.Text("", "HazeCaptionStyle", wrap: false);
    readonly Button open = new();
    readonly Button close = new();

    readonly Grid layers = new();
    readonly Grid wait = new() { Visibility = Visibility.Collapsed };
    readonly TextBlock waitNote = Ui.Text("", "HazeCaptionStyle");
    readonly Grid status = new() { Visibility = Visibility.Collapsed };
    readonly Grid listLayer = new() { Visibility = Visibility.Collapsed };
    readonly ScrollViewer scroll = new() { VerticalScrollBarVisibility = ScrollBarVisibility.Auto, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled };
    readonly TextBlock offlineNote = Ui.Text("", "HazeCaptionStyle");
    readonly StackPanel list = new();
    readonly StackPanel more = new() { Visibility = Visibility.Collapsed, Padding = new Thickness(20, 12, 16, 0) };
    UIElement? shown;

    readonly StackPanel foot = new() { Spacing = 8 };
    readonly Grid modeBar = new() { Visibility = Visibility.Collapsed };
    readonly TextBlock modeText = Ui.Text("", "HazeCaptionStyle", wrap: false);
    readonly Grid field = new() { ColumnSpacing = 8 };
    readonly TextBox box = new() { AcceptsReturn = true, TextWrapping = TextWrapping.Wrap, MinHeight = 32, MaxHeight = 120, VerticalAlignment = VerticalAlignment.Bottom };
    readonly Button post = new() { Style = Ui.Res<Style>("AccentButtonStyle"), MinHeight = 32, MinWidth = 64, VerticalAlignment = VerticalAlignment.Bottom, IsEnabled = false };
    readonly Button signIn = new() { Content = T.DiscussionSignIn, Visibility = Visibility.Collapsed, HorizontalAlignment = HorizontalAlignment.Stretch, MinHeight = 32 };
    readonly TextBlock waitsForNetwork = Ui.Text(T.DiscussionOfflinePost, "HazeCaptionStyle");

    readonly Dictionary<uint, CommentRow> rows = [];
    ComposeMode mode = new ComposeMode.New();
    int asked = -1;
    bool toEnd, focusedEmpty;
    string? settled;

    /// <summary>Spoilers the viewer opened, by comment and place in it.</summary>
    public HashSet<(uint, int)> Revealed { get; } = [];
    /// <summary>Replies the viewer unfolded, by the reply they answer.</summary>
    public HashSet<uint> Unfolded { get; } = [];
    /// <summary>Long comments the viewer opened with More.</summary>
    public HashSet<uint> Expanded { get; } = [];

    public bool CanPost => view?.CanPost == true;
    public bool SignedIn => view?.SignedIn == true;
    public bool LoadImages => view?.LoadImages == true;

    /// <summary>The pane was closed: by its cross, by Escape, or by the host.</summary>
    public event Action? Closed;

    public DiscussionPane(Ctx ctx, uint media, uint episode)
    {
        this.ctx = ctx;
        this.media = media;
        this.episode = episode;
        Width = PaneWidth;
        Background = Ui.Res<Brush>("SidebarBrush");
        BorderBrush = Ui.Res<Brush>("FaintRuleBrush");
        BorderThickness = new Thickness(1, 0, 0, 0);
        HorizontalAlignment = HorizontalAlignment.Right;
        RequestedTheme = ElementTheme.Dark;
        AutomationProperties.SetName(this, T.DiscussionPaneLabel(episode));
        AutomationProperties.SetLandmarkType(this, AutomationLandmarkType.Custom);
        TabFocusNavigation = KeyboardNavigationMode.Cycle;
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        Head();
        Body();
        Foot();

        // The pane's clicks and wheel are its own: under the player they would pause it or change its volume.
        Tapped += (_, e) => e.Handled = true;
        DoubleTapped += (_, e) => e.Handled = true;
        PointerWheelChanged += (_, e) => e.Handled = true;
        KeyDown += (_, e) =>
        {
            if (e.Key == VirtualKey.Escape && !e.Handled)
            {
                e.Handled = true;
                Close();
            }
        };
        ticking = Microsoft.UI.Dispatching.DispatcherQueue.GetForCurrentThread().CreateTimer();
        ticking.Interval = TimeSpan.FromMilliseconds(250);
        ticking.Tick += (_, _) => Waiting();
        ctx.Batch += Heard;
        Loaded += (_, _) =>
        {
            ticking.Start();
            Waiting();
            if (!(field.Visibility == Visibility.Visible && view?.Comments is not { Length: > 0 }))
                close.Focus(FocusState.Programmatic);
        };
    }

    /// <summary>Asks the engine for the discussion.</summary>
    public void Start()
    {
        ctx.Send(new Command.OpenDiscussion(media, episode));
        Count();
    }

    /// <summary>Closes the pane: the engine is told, and nothing more is heard.</summary>
    public void Close()
    {
        if (closed)
            return;
        closed = true;
        ticking.Stop();
        ctx.Batch -= Heard;
        ctx.Send(new Command.CloseDiscussion());
        Closed?.Invoke();
    }

    /// <summary>The thread, or the show's forum page.</summary>
    public string CommentUrl(uint id) => view is { Url.Length: > 0 } v ? v.Url + "/comment/" + id : $"https://anilist.co/anime/{media}";

    // The header: the title with the count under it, Open on AniList, and the cross.

    void Head()
    {
        var head = new Grid { Padding = new Thickness(20, 14, 12, 8), ColumnSpacing = 8 };
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        head.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var words = new StackPanel();
        var title = new TextBlock { Text = T.DiscussionTitleEpisode(episode), FontSize = 20, LineHeight = 28, FontWeight = FontWeights.SemiBold, TextTrimming = TextTrimming.CharacterEllipsis };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        words.Children.Add(title);
        words.Children.Add(count);
        head.Children.Add(words);
        open.Content = T.TitleOpenAnilist;
        open.MinHeight = 32;
        open.VerticalAlignment = VerticalAlignment.Top;
        open.Click += (_, _) => ctx.Send(new Command.OpenExternal(view is { Url.Length: > 0 } v ? v.Url : $"https://anilist.co/anime/{media}"));
        Grid.SetColumn(open, 1);
        head.Children.Add(open);
        close.Content = new FontIcon { Glyph = "", FontSize = 11 };
        close.Style = Ui.Res<Style>("SubtleButtonStyle");
        close.Width = close.Height = 32;
        close.Padding = new Thickness(0);
        close.VerticalAlignment = VerticalAlignment.Top;
        ToolTipService.SetToolTip(close, T.ActionClose);
        AutomationProperties.SetName(close, T.ActionClose);
        close.Click += (_, _) => Close();
        Grid.SetColumn(close, 2);
        head.Children.Add(close);
        Children.Add(head);
    }

    // The body: nothing, placeholders, a state, or the comments.

    void Body()
    {
        for (var i = 0; i < 4; i++)
            wait.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        wait.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        for (var i = 0; i < 4; i++)
        {
            var row = Skeleton();
            Grid.SetRow(row, i);
            wait.Children.Add(row);
        }
        waitNote.Margin = new Thickness(20, 16, 16, 0);
        Grid.SetRow(waitNote, 4);
        AutomationProperties.SetLiveSetting(waitNote, AutomationLiveSetting.Polite);
        wait.Children.Add(waitNote);
        AutomationProperties.SetAccessibilityView(wait, AccessibilityView.Raw);

        offlineNote.Margin = new Thickness(20, 0, 16, 4);
        offlineNote.Visibility = Visibility.Collapsed;
        var inner = new StackPanel();
        inner.Children.Add(offlineNote);
        inner.Children.Add(list);
        more.Children.Add(Skeleton(false));
        inner.Children.Add(more);
        scroll.Content = inner;
        scroll.ViewChanged += (_, _) => Paging();
        listLayer.Children.Add(scroll);

        layers.Children.Add(wait);
        layers.Children.Add(status);
        layers.Children.Add(listLayer);
        Grid.SetRow(layers, 1);
        Children.Add(layers);
    }

    /// <summary>A grey plate where a comment will land: an avatar and three lines.</summary>
    static Grid Skeleton(bool pad = true)
    {
        var row = new Grid { ColumnSpacing = 10, Padding = pad ? new Thickness(20, 10, 16, 0) : new Thickness(0) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var face = States.Plate(28, 28, 14);
        face.VerticalAlignment = VerticalAlignment.Top;
        row.Children.Add(face);
        var lines = new StackPanel { Spacing = 6, Margin = new Thickness(0, 2, 0, 12) };
        lines.Children.Add(States.Plate(96, 10, 3));
        lines.Children.Add(States.Plate(250, 12, 3));
        lines.Children.Add(States.Plate(180, 12, 3));
        Grid.SetColumn(lines, 1);
        row.Children.Add(lines);
        AutomationProperties.SetAccessibilityView(row, AccessibilityView.Raw);
        return row;
    }

    // The foot: what is being answered, the field and Post, or why there is no field.

    void Foot()
    {
        foot.Padding = new Thickness(16, 12, 16, 16);
        foot.Background = Ui.Res<Brush>("SidebarBrush");
        var border = new Border { BorderBrush = Ui.Res<Brush>("FaintRuleBrush"), BorderThickness = new Thickness(0, 1, 0, 0), Child = foot };
        Grid.SetRow(border, 2);

        modeBar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        modeBar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        modeText.TextTrimming = TextTrimming.CharacterEllipsis;
        modeText.VerticalAlignment = VerticalAlignment.Center;
        modeBar.Children.Add(modeText);
        var cancel = new Button { Content = new FontIcon { Glyph = "", FontSize = 10 }, Style = Ui.Res<Style>("SubtleButtonStyle"), Width = 28, Height = 28, Padding = new Thickness(0) };
        ToolTipService.SetToolTip(cancel, T.DiscussionCancelReply);
        AutomationProperties.SetName(cancel, T.DiscussionCancelReply);
        cancel.Click += (_, _) => Compose(new ComposeMode.New());
        Grid.SetColumn(cancel, 1);
        modeBar.Children.Add(cancel);
        foot.Children.Add(modeBar);

        field.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        field.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        box.PlaceholderText = T.DiscussionAddComment;
        AutomationProperties.SetName(box, T.DiscussionAddComment);
        box.TextChanged += (_, _) => post.IsEnabled = box.Text.Trim().Length > 0;
        box.KeyDown += (_, e) =>
        {
            // Enter sends; Shift+Enter starts a new line.
            if (e.Key == VirtualKey.Enter && !Shifted() && post.IsEnabled)
            {
                e.Handled = true;
                Send();
            }
        };
        SpoilerMenu(box);
        field.Children.Add(box);
        post.Content = T.DiscussionPost;
        post.Click += (_, _) => Send();
        Grid.SetColumn(post, 1);
        field.Children.Add(post);
        foot.Children.Add(field);

        signIn.Click += (_, _) =>
        {
            Close();
            ctx.Go("accounts");
        };
        foot.Children.Add(signIn);
        waitsForNetwork.Visibility = Visibility.Collapsed;
        foot.Children.Add(waitsForNetwork);
        Children.Add(border);
    }

    static bool Shifted() =>
        Microsoft.UI.Input.InputKeyboardSource.GetKeyStateForCurrentThread(VirtualKey.Shift).HasFlag(global::Windows.UI.Core.CoreVirtualKeyStates.Down);

    /// <summary>The field's edit menu offers Spoiler, which wraps the selection in AniList's spoiler marks.</summary>
    void SpoilerMenu(TextBox target)
    {
        TextCommandBarFlyout Flyout()
        {
            var flyout = new TextCommandBarFlyout();
            var item = new AppBarButton { Label = T.DiscussionSpoiler, Icon = new FontIcon { Glyph = "" } };
            item.Click += (_, _) =>
            {
                var from = target.SelectionStart;
                var picked = target.SelectedText;
                target.SelectedText = "~!" + picked + "!~";
                target.SelectionStart = from + 2;
                target.SelectionLength = picked.Length;
                flyout.Hide();
            };
            flyout.SecondaryCommands.Add(item);
            flyout.Opening += (_, _) => item.IsEnabled = target.SelectionLength > 0;
            return flyout;
        }
        target.ContextFlyout = Flyout();
        target.SelectionFlyout = Flyout();
    }

    /// <summary>What the field is for: a new comment, an answer or an edit. Edit starts from the comment's own text.</summary>
    public void Compose(ComposeMode next, string? text = null)
    {
        mode = next;
        modeBar.Visibility = next is ComposeMode.New ? Visibility.Collapsed : Visibility.Visible;
        modeText.Text = next switch
        {
            ComposeMode.Reply r => T.DiscussionReplyingTo(r.Name),
            ComposeMode.Edit => T.DiscussionEditing,
            _ => "",
        };
        var edit = next is ComposeMode.Edit;
        box.PlaceholderText = edit ? T.DiscussionEditComment : T.DiscussionAddComment;
        post.Content = edit ? T.ActionSave : T.DiscussionPost;
        box.Text = text ?? (next is ComposeMode.New ? "" : box.Text);
        if (next is not ComposeMode.New)
        {
            box.Focus(FocusState.Programmatic);
            box.SelectionStart = box.Text.Length;
        }
    }

    void Send()
    {
        var text = box.Text.Trim();
        if (text.Length == 0)
            return;
        toEnd = mode is ComposeMode.New;
        ctx.Send(Discussions.Send(mode, text));
        Compose(new ComposeMode.New());
    }

    /// <summary>Asks first, as the board does: "Delete this comment? It is removed from AniList too."</summary>
    public async Task ConfirmDelete(uint id)
    {
        if (XamlRoot is null)
            return;
        var dialog = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Title = T.DiscussionDeleteTitle,
            Content = T.DiscussionDeleteBody,
            PrimaryButtonText = T.ActionDelete,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Close,
            RequestedTheme = ElementTheme.Dark,
        };
        if (await Dialogs.Show(dialog) == ContentDialogResult.Primary)
            ctx.Send(new Command.DeleteComment(id));
    }

    // What the engine says.

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            if (e is Event.Discussion d && d.V1.Media == media && d.V1.Episode == episode)
                Take(d.V1);
            else if (e is Event.DiscussionFound f && f.Media == media && f.Episode == episode)
                Count();
        }
    }

    public void Take(DiscussionView next)
    {
        if (closed)
            return;
        view = next;
        if (next.State is DiscussionState.RateLimited)
            limitedAt ??= DateTime.UtcNow;
        else
            limitedAt = null;
        Render();
    }

    public void Redraw()
    {
        if (view is not null)
            Render();
    }

    /// <summary>The count under the title: the thread's, or what the engine found before it was opened.</summary>
    void Count()
    {
        var n = view is { CommentCount: > 0 } v ? v.CommentCount : ctx.Known.Discussions.GetValueOrDefault((media, episode));
        count.Text = n > 0 ? T.DiscussionCount(n) : "";
        count.Visibility = n > 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    /// <summary>Placeholders and the lines that come with waiting, by how long it has been.</summary>
    void Waiting()
    {
        if (closed)
            return;
        if (view is null || view.State is DiscussionState.Loading or DiscussionState.RateLimited)
            ShowWait();
    }

    void ShowWait()
    {
        if (limitedAt is { } at && view?.State is DiscussionState.RateLimited limited)
        {
            var left = Math.Max(0, limited.Seconds - (long)(DateTime.UtcNow - at).TotalSeconds);
            waitNote.Text = T.DiscussionRateLimited(Wording.Span(new Span(0, 0, (byte)Math.Min(59, left / 60), (byte)(left % 60))));
            waitNote.Visibility = Visibility.Visible;
            Layer(wait);
        }
        else if (since.Elapsed < Quiet)
            Layer(null);
        else
        {
            waitNote.Text = T.DiscussionStillLoading;
            waitNote.Visibility = since.Elapsed >= Slow ? Visibility.Visible : Visibility.Collapsed;
            Layer(wait);
        }
    }

    /// <summary>One layer shows; the comments and the states arrive by the standard crossfade.</summary>
    void Layer(UIElement? next)
    {
        if (next == shown)
            return;
        var arriving = next is not null && next != wait;
        wait.Visibility = status.Visibility = listLayer.Visibility = Visibility.Collapsed;
        shown = next;
        if (next is null)
            return;
        if (arriving && Motion.Enabled)
            Motion.FadeIn(next, Motion.Standard);
        else
            Motion.Set(next, true);
    }

    void Render()
    {
        var v = view!;
        Count();
        open.Visibility = v.State is DiscussionState.NoThread ? Visibility.Collapsed : Visibility.Visible;
        Footing(v);

        switch (v.State)
        {
            case DiscussionState.Loading or DiscussionState.RateLimited:
                offlineNote.Visibility = Visibility.Collapsed;
                ShowWait();
                return;
            case DiscussionState.NoThread:
                Status(States.Unavailable(T.DiscussionNoThread, T.DiscussionNoThreadDetail, T.TitleOpenAnilist,
                    () => ctx.Send(new Command.OpenExternal(v.Url.Length > 0 ? v.Url : $"https://anilist.co/anime/{media}"))));
                return;
            case DiscussionState.Failed:
                Status(States.Unavailable(T.DiscussionFailed, T.TitleNoAnswer, T.ActionTryAgain, Retry, error: true));
                return;
            case DiscussionState.Offline when v.Comments is not { Length: > 0 }:
                Status(States.Unavailable(T.StateOfflineTitle, null, T.ActionTryAgain, Retry));
                return;
        }
        if (v.Comments is not { Length: > 0 })
        {
            Status(States.Unavailable(T.DiscussionNoComments, null));
            if (field.Visibility == Visibility.Visible && !focusedEmpty)
            {
                focusedEmpty = true;
                box.Focus(FocusState.Programmatic);
            }
            return;
        }
        if (v.State is DiscussionState.Offline offline)
        {
            offlineNote.Text = T.DiscussionOfflineCached(Wording.Ago(offline.AgeMinutes * 60L));
            offlineNote.Visibility = Visibility.Visible;
        }
        else
            offlineNote.Visibility = Visibility.Collapsed;

        Sync(list, [.. v.Comments.Select(c => (c, 0, (uint?)null, (string?)null))]);
        more.Visibility = v.State is DiscussionState.Ready { More: true } ? Visibility.Visible : Visibility.Collapsed;
        Layer(listLayer);
        if (toEnd)
        {
            toEnd = false;
            DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, () => scroll.ChangeView(null, scroll.ScrollableHeight, null, true));
        }
        // A page that fits without scrolling would never reach its end.
        DispatcherQueue.TryEnqueue(Microsoft.UI.Dispatching.DispatcherQueuePriority.Low, Paging);
    }

    void Retry() => ctx.Send(new Command.OpenDiscussion(media, episode));

    void Status(UIElement what)
    {
        status.Children.Clear();
        status.Children.Add(what);
        Layer(status);
    }

    /// <summary>The next page, once, when the viewer reaches the end.</summary>
    void Paging()
    {
        if (view is not { State: DiscussionState.Ready { More: true } } v || shown != listLayer)
            return;
        if (scroll.ScrollableHeight - scroll.VerticalOffset > 240 && scroll.ScrollableHeight > 0)
            return;
        var have = v.Comments?.Length ?? 0;
        if (asked == have)
            return;
        asked = have;
        ctx.Send(new Command.MoreComments());
    }

    /// <summary>The field, or the line that takes its place.</summary>
    void Footing(DiscussionView v)
    {
        var noThread = v.State is DiscussionState.NoThread;
        var fieldShown = v.CanPost && !noThread;
        field.Visibility = fieldShown ? Visibility.Visible : Visibility.Collapsed;
        if (!fieldShown)
            modeBar.Visibility = Visibility.Collapsed;
        signIn.Visibility = !v.SignedIn && !noThread ? Visibility.Visible : Visibility.Collapsed;
        var settledState = v.State is DiscussionState.Ready or DiscussionState.Offline;
        waitsForNetwork.Visibility = v.SignedIn && !v.CanPost && settledState ? Visibility.Visible : Visibility.Collapsed;
        foot.Visibility = noThread || (!fieldShown && signIn.Visibility == Visibility.Collapsed && waitsForNetwork.Visibility == Visibility.Collapsed)
            ? Visibility.Collapsed : Visibility.Visible;
        if (!fieldShown && modeBar.Visibility == Visibility.Collapsed)
            Compose(new ComposeMode.New());
    }

    /// <summary>
    /// A panel's rows made to match the comments, in order: rows are kept by comment, new ones
    /// made, gone ones taken away. A top-level comment's replies, and the replies of a reply the
    /// viewer unfolded, sit in its panel at one step.
    /// </summary>
    void Sync(Panel panel, List<(CommentView C, int Depth, uint? Parent, string? To)> wanted)
    {
        var elements = new List<UIElement>();
        foreach (var (c, depth, parent, to) in wanted)
        {
            live.Add(c.Id);
            if (!rows.TryGetValue(c.Id, out var row))
            {
                row = new CommentRow(this, ctx, c, parent, depth, to);
                rows[c.Id] = row;
                if (seenFirst && Motion.Enabled)
                    Motion.FadeIn(row.Root, Motion.Standard);
            }
            else
            {
                row.To = to;
                row.Update(c);
            }
            if (row.Rule is { } rule)
                rule.Visibility = elements.Count == 0 && panel == list ? Visibility.Collapsed : Visibility.Visible;
            elements.Add(row.Root);
            if (depth == 0)
            {
                var replies = new List<(CommentView, int, uint?, string?)>();
                foreach (var reply in c.Replies ?? [])
                {
                    replies.Add((reply, 1, c.Id, null));
                    if (reply.HiddenReplies > 0 && Unfolded.Contains(reply.Id))
                        foreach (var deep in reply.Replies ?? [])
                            replies.Add((deep, 2, reply.Id, reply.Author));
                }
                Sync(row.Replies, replies);
            }
        }
        var keep = new HashSet<UIElement>(elements);
        for (var i = panel.Children.Count - 1; i >= 0; i--)
            if (!keep.Contains(panel.Children[i]))
                panel.Children.RemoveAt(i);
        for (var i = 0; i < elements.Count; i++)
        {
            var at = panel.Children.IndexOf(elements[i]);
            if (at < 0)
                panel.Children.Insert(i, elements[i]);
            else if (at != i)
                panel.Children.Move((uint)at, (uint)i);
        }
        if (panel == list)
        {
            seenFirst = true;
            foreach (var gone in rows.Keys.Where(id => !live.Contains(id)).ToList())
                rows.Remove(gone);
            live.Clear();
        }
    }

    readonly HashSet<uint> live = [];

    bool seenFirst;

    // Testing: a shot of a state the viewer would have to click to.

    internal void Reveal(string what)
    {
        foreach (var c in view?.Comments ?? [])
            for (var i = 0; i < 4; i++)
                Revealed.Add((c.Id, i));
        Redraw();
        _ = what;
    }

    internal CommentRow? Row(uint id) => rows.GetValueOrDefault(id);
}
