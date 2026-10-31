using System.Text.RegularExpressions;
using Anistream.App;
using Anistream.Engine.Model;
using Anistream.Windows.Pages;
using Microsoft.UI;
using Microsoft.UI.Text;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Automation.Provider;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.Foundation;
using Windows.System;

namespace Anistream.Windows.Discussion;

/// <summary>
/// One comment, drawn after the board: the avatar, the name and age, the text, the like and Reply
/// row, and under a top-level comment its replies. A row is made once and updated as the engine
/// says the discussion again, so focus, the opened spoilers and the scroll place stay where they are.
/// </summary>
sealed partial class CommentRow
{
    readonly DiscussionPane pane;
    readonly Ctx ctx;
    public uint Id { get; }

    public Grid Root { get; } = new();
    readonly Grid card = new() { Background = new SolidColorBrush(Colors.Transparent) };
    readonly Border face = new() { VerticalAlignment = VerticalAlignment.Top, Margin = new Thickness(0, 2, 0, 0) };
    readonly TextBlock initial = new() { FontSize = 11, FontWeight = FontWeights.SemiBold, HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center };
    readonly Image photo = new() { Stretch = Stretch.UniformToFill, Opacity = 0 };
    readonly TextBlock author = new() { FontWeight = FontWeights.SemiBold, FontSize = 12, TextTrimming = TextTrimming.CharacterEllipsis };
    readonly TextBlock age = Ui.Text("", "HazeCaptionStyle", wrap: false);
    readonly TextBlock answering = Ui.Text("", "HazeCaptionStyle", wrap: false);
    readonly ContentControl body = new() { HorizontalContentAlignment = HorizontalAlignment.Stretch, IsTabStop = false };
    readonly Button more = new();
    readonly LikeButton like = new();
    readonly FontIcon heart = new() { FontSize = 13 };
    readonly TextBlock count = new() { FontSize = 12 };
    readonly Button reply = new();
    readonly Button menu = new();
    readonly StackPanel failed = new() { Orientation = Orientation.Horizontal, Spacing = 4, Visibility = Visibility.Collapsed };
    readonly Button again = new();
    readonly Button fold = new();
    public StackPanel Replies { get; } = new();
    readonly MenuFlyout flyout = new();

    public CommentView Comment { get; private set; } = null!;
    readonly uint? parent;
    readonly int depth;
    string bodyKey = "", avatarSeen = "";
    bool loadedImages, folded = true, over;
    readonly List<SpoilerGroup> spoilers = [];

    public CommentRow(DiscussionPane pane, Ctx ctx, CommentView first, uint? parent, int depth, string? to)
    {
        this.pane = pane;
        this.ctx = ctx;
        this.parent = parent;
        this.depth = depth;
        Id = first.Id;
        To = to;
        Build();
        Update(first);
    }

    /// <summary>Who a reply to a reply answers, named after its author.</summary>
    public string? To { get; set; }

    void Build()
    {
        var size = depth == 0 ? 28 : 20;
        face.Width = face.Height = size;
        face.CornerRadius = new CornerRadius(size / 2.0);
        initial.FontSize = depth == 0 ? 11 : 9;
        var circle = new Grid();
        circle.Children.Add(initial);
        circle.Children.Add(photo);
        face.Child = circle;
        AutomationProperties.SetAccessibilityView(face, AccessibilityView.Raw);

        var nameLine = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        nameLine.Children.Add(author);
        nameLine.Children.Add(age);
        nameLine.Children.Add(answering);

        var content = new StackPanel { Spacing = 0 };
        content.Children.Add(nameLine);
        var text = new StackPanel { Margin = new Thickness(0, 3, 0, 0) };
        text.Children.Add(body);
        Style(more, T.ActionMore, true);
        more.Visibility = Visibility.Collapsed;
        more.Click += (_, _) =>
        {
            pane.Expanded.Add(Id);
            Refold();
        };
        text.Children.Add(more);
        content.Children.Add(text);

        var actions = new Grid { MinHeight = 38, ColumnSpacing = 4 };
        actions.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        actions.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        actions.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var likeBox = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        likeBox.Children.Add(heart);
        likeBox.Children.Add(count);
        like.Content = likeBox;
        Quiet(like, new Thickness(8, 0, 8, 0));
        like.Margin = new Thickness(-8, 3, 0, 3);
        AutomationProperties.SetName(like, T.DiscussionLike);
        like.Toggle = () => ctx.Send(new Command.LikeComment(Id));
        like.Click += (_, _) => ctx.Send(new Command.LikeComment(Id));
        actions.Children.Add(like);

        reply.Content = new TextBlock { Text = T.DiscussionReply, FontSize = 12, FontWeight = FontWeights.SemiBold };
        Quiet(reply, new Thickness(8, 0, 8, 0));
        reply.Margin = new Thickness(0, 3, 0, 3);
        reply.Click += (_, _) => pane.Compose(new ComposeMode.Reply(Id, Comment.Author));
        Grid.SetColumn(reply, 1);
        actions.Children.Add(reply);

        menu.Content = new FontIcon { Glyph = "", FontSize = 12 };
        Quiet(menu, new Thickness(0));
        menu.Width = menu.Height = 32;
        menu.HorizontalAlignment = HorizontalAlignment.Right;
        menu.Margin = new Thickness(0, 3, 0, 3);
        menu.Opacity = 0;
        menu.Flyout = flyout;
        ToolTipService.SetToolTip(menu, T.DiscussionCommentMenu);
        AutomationProperties.SetName(menu, T.DiscussionCommentMenu);
        menu.GotFocus += (_, _) => ShowMenuButton(true);
        menu.LostFocus += (_, _) => ShowMenuButton(over);
        Grid.SetColumn(menu, 2);
        actions.Children.Add(menu);
        content.Children.Add(actions);

        failed.Children.Add(States.ErrorGlyph(12));
        again.Content = new TextBlock { Text = T.DiscussionPostFailed, FontSize = 12 };
        Quiet(again, new Thickness(4, 0, 4, 0));
        again.Click += (_, _) => ctx.Send(new Command.PostComment(parent, Comment.Markdown));
        failed.Children.Add(again);
        content.Children.Add(failed);

        Style(fold, "", true);
        fold.Margin = new Thickness(-8, 0, 0, 0);
        fold.HorizontalAlignment = HorizontalAlignment.Left;
        fold.Click += (_, _) =>
        {
            pane.Unfolded.Add(Id);
            pane.Redraw();
        };
        fold.Visibility = Visibility.Collapsed;
        content.Children.Add(fold);
        content.Children.Add(Replies);

        card.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        card.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        card.ColumnSpacing = depth == 0 ? 10 : 8;
        card.Children.Add(face);
        Grid.SetColumn(content, 1);
        card.Children.Add(content);
        card.Padding = depth == 0 ? new Thickness(20, 10, 16, 0) : new Thickness(0, 10, 0, 0);

        // The hairline between top-level comments, inset to the text column.
        Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        if (depth == 0)
        {
            var rule = new Border { Height = 1, Margin = new Thickness(56, 0, 0, 0), Background = Ui.Res<Brush>("FaintRuleBrush"), Visibility = Visibility.Collapsed };
            Rule = rule;
            Root.Children.Add(rule);
        }
        Grid.SetRow(card, 1);
        Root.Children.Add(card);

        card.PointerEntered += (_, _) => Hover(true);
        card.PointerExited += (_, _) => Hover(false);
        card.RightTapped += (_, e) =>
        {
            e.Handled = true;
            flyout.ShowAt(card, new FlyoutShowOptions { Position = e.GetPosition(card) });
        };
        flyout.Opening += (_, _) => FillMenu();
        menu.Flyout.Opening += (_, _) => FillMenu();
        card.OpacityTransition = new ScalarTransition { Duration = TimeSpan.FromMilliseconds(200) };
        if (Motion.Enabled)
            Motion.QuickOpacity(card);
        AutomationProperties.SetAccessibilityView(Replies, AccessibilityView.Content);
    }

    public Border? Rule { get; private set; }


    void Hover(bool on)
    {
        over = on;
        card.Background = new SolidColorBrush(on && depth == 0 ? ((SolidColorBrush)Ui.Res<Brush>("RaisedBrush")).Color : Colors.Transparent);
        ShowMenuButton(on || menu.FocusState != FocusState.Unfocused);
    }

    void ShowMenuButton(bool on) => menu.Opacity = on ? 1 : 0;

    static void Quiet(Button button, Thickness padding)
    {
        button.Style = Ui.Res<Style>("SubtleButtonStyle");
        button.Padding = padding;
        button.MinHeight = 32;
        button.MinWidth = 0;
        button.Foreground = Ui.Res<Brush>("HazeBrush");
        button.Resources["ButtonForegroundPointerOver"] = Ui.Res<Brush>("SilkBrush");
        button.Resources["ButtonForegroundPressed"] = Ui.Res<Brush>("SilkBrush");
    }

    static void Style(Button button, string text, bool haze)
    {
        Quiet(button, new Thickness(8, 0, 8, 0));
        button.Content = new TextBlock { Text = text, FontSize = 12, FontWeight = FontWeights.SemiBold };
        _ = haze;
    }

    /// <summary>The comment as the engine last said it.</summary>
    public void Update(CommentView c)
    {
        Comment = c;
        author.Text = c.Author;
        age.Text = Discussions.Age(c);
        answering.Text = To is { Length: > 0 } name ? "· " + T.DiscussionAnswering(name) : "";
        answering.Visibility = answering.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
        initial.Text = c.Author.Length > 0 ? char.ToUpperInvariant(c.Author[0]).ToString() : "";
        face.Background = new SolidColorBrush(Tone(c.Author));
        if (c.Avatar is { Length: > 0 } url && url != avatarSeen)
        {
            avatarSeen = url;
            ctx.Art.Bind(url, image =>
            {
                photo.Source = image;
                if (Motion.Enabled)
                    Motion.FadeIn(photo);
                else
                    photo.Opacity = 1;
            });
        }

        heart.Glyph = c.Liked ? "" : "";
        var tint = c.Liked ? Ui.Res<Brush>("SilkBrush") : Ui.Res<Brush>("HazeBrush");
        heart.Foreground = count.Foreground = tint;
        like.Foreground = tint;
        like.Resources["ButtonForeground"] = tint;
        like.Resources["ButtonForegroundPointerOver"] = Ui.Res<Brush>("SilkBrush");
        count.Text = c.LikeCount.ToString(System.Globalization.CultureInfo.CurrentCulture);
        count.Visibility = c.LikeCount > 0 ? Visibility.Visible : Visibility.Collapsed;
        like.Liked = c.Liked;
        AutomationProperties.SetFullDescription(like, c.LikeCount > 0 ? c.LikeCount.ToString(System.Globalization.CultureInfo.CurrentCulture) : "");
        // Signed out, the count stands alone.
        like.IsHitTestVisible = like.IsTabStop = pane.SignedIn;

        var posted = c.Pending == PostState.Posted;
        reply.Visibility = pane.CanPost && posted ? Visibility.Visible : Visibility.Collapsed;
        card.Opacity = posted ? 1 : 0.5;
        failed.Visibility = c.Pending == PostState.Failed ? Visibility.Visible : Visibility.Collapsed;
        menu.Visibility = posted ? Visibility.Visible : Visibility.Collapsed;

        fold.Visibility = c.HiddenReplies > 0 && depth == 1 && !pane.Unfolded.Contains(c.Id) ? Visibility.Visible : Visibility.Collapsed;
        if (fold.Content is TextBlock folds)
            folds.Text = T.DiscussionMoreReplies(c.HiddenReplies);

        var key = BodyKey(c) + pane.LoadImages;
        if (key != bodyKey)
        {
            bodyKey = key;
            loadedImages = pane.LoadImages;
            BuildBody();
        }
        // The spoilers the viewer opened stay open.
        foreach (var group in spoilers)
            group.Set(pane.Revealed.Contains((Id, group.At)), false);
    }

    static string BodyKey(CommentView c) =>
        string.Join('\u001f', (c.Body ?? []).Select(b => (b.Quote ? "q" : "p") + string.Join('\u001e', (b.Spans ?? []).Select(s =>
            $"{s.Text}|{s.Bold}{s.Italic}{s.Strike}{s.Spoiler}|{s.Link}|{s.Media}"))));

    static readonly global::Windows.UI.Color[] Tones =
    [
        global::Windows.UI.Color.FromArgb(255, 0x4F, 0x63, 0x82), global::Windows.UI.Color.FromArgb(255, 0x6E, 0x54, 0x70),
        global::Windows.UI.Color.FromArgb(255, 0x5A, 0x68, 0x78), global::Windows.UI.Color.FromArgb(255, 0x55, 0x6B, 0x5C),
        global::Windows.UI.Color.FromArgb(255, 0x7A, 0x6A, 0x45),
    ];

    static global::Windows.UI.Color Tone(string name)
    {
        var sum = 0;
        foreach (var ch in name)
            sum += ch;
        return Tones[sum % Tones.Length];
    }

    // The text.

    const double LineHeight = 20;

    void BuildBody()
    {
        spoilers.Clear();
        var column = new StackPanel { Spacing = 6 };
        var spoilerAt = 0;
        foreach (var block in Comment.Body ?? [])
        {
            var parts = Discussions.Parts(block, pane.LoadImages, ref spoilerAt);
            var paragraph = new Paragraph();
            RichTextBlock? lines = null;
            UIElement Flush()
            {
                var done = lines!;
                lines = null;
                if (!block.Quote)
                    return done;
                done.Foreground = Ui.Res<Brush>("HazeBrush");
                return new Border
                {
                    BorderBrush = Ui.Res<Brush>("RuleBrush"),
                    BorderThickness = new Thickness(1, 0, 0, 0),
                    Padding = new Thickness(10, 0, 0, 0),
                    Child = done,
                };
            }
            RichTextBlock Lines() => lines ??= NewText(paragraph = new Paragraph());
            foreach (var part in parts)
            {
                switch (part)
                {
                    case CommentPart.Text t:
                        foreach (var span in t.Spans)
                            AddSpan(Lines(), paragraph, span);
                        break;
                    case CommentPart.Spoiler s:
                        var group = new SpoilerGroup(s.At, pane.Revealed.Contains((Id, s.At)), Toggle);
                        spoilers.Add(group);
                        foreach (var chunk in group.Chunks(s.Spans))
                            paragraph.Inlines.Add(chunk);
                        Lines();
                        break;
                    case CommentPart.Picture picture:
                        if (lines is not null)
                            column.Children.Add(Flush());
                        column.Children.Add(Picture(picture.Url));
                        break;
                    case CommentPart.Media media:
                        if (lines is not null)
                            column.Children.Add(Flush());
                        column.Children.Add(MediaLine(media));
                        break;
                }
            }
            if (lines is not null)
                column.Children.Add(Flush());
        }
        body.Content = Folding(column);
    }

    RichTextBlock NewText(Paragraph paragraph)
    {
        var text = new RichTextBlock
        {
            FontSize = 14,
            LineHeight = LineHeight,
            LineStackingStrategy = LineStackingStrategy.BlockLineHeight,
            TextWrapping = TextWrapping.Wrap,
            IsTextSelectionEnabled = false,
        };
        text.Blocks.Add(paragraph);
        return text;
    }

    void AddSpan(RichTextBlock _, Paragraph paragraph, CommentSpan span)
    {
        var lines = span.Text.Split('\n');
        for (var i = 0; i < lines.Length; i++)
        {
            if (i > 0)
                paragraph.Inlines.Add(new LineBreak());
            if (lines[i].Length == 0)
                continue;
            var run = new Run { Text = lines[i] };
            if (span.Bold)
                run.FontWeight = FontWeights.SemiBold;
            if (span.Italic)
                run.FontStyle = global::Windows.UI.Text.FontStyle.Italic;
            if (span.Strike)
                run.TextDecorations = global::Windows.UI.Text.TextDecorations.Strikethrough;
            if (Discussions.Web(span.Link) is { } url)
            {
                var link = new Hyperlink { Foreground = Ui.Res<Brush>("SilkBrush") };
                link.Inlines.Add(run);
                link.Click += (_, _) => ctx.Send(new Command.OpenExternal(url));
                paragraph.Inlines.Add(link);
            }
            else
                paragraph.Inlines.Add(run);
        }
    }

    RichTextBlock MediaLine(CommentPart.Media media)
    {
        var paragraph = new Paragraph();
        var text = NewText(paragraph);
        text.FontSize = 12;
        text.LineHeight = 16;
        text.Foreground = Ui.Res<Brush>("HazeBrush");
        var words = media.Kind == MediaKind.Image ? T.DiscussionImage : T.DiscussionVideo;
        var host = Discussions.Host(media.Url);
        var line = host.Length > 0 ? words + " · " + host : words;
        if (media.Url is { } url)
        {
            var link = new Hyperlink { Foreground = Ui.Res<Brush>("HazeBrush") };
            link.Inlines.Add(new Run { Text = line });
            link.Click += (_, _) => ctx.Send(new Command.OpenExternal(url));
            paragraph.Inlines.Add(link);
        }
        else
            paragraph.Inlines.Add(new Run { Text = line });
        return text;
    }

    UIElement Picture(string url)
    {
        var image = new Image { Stretch = Stretch.Uniform, MaxHeight = 240, HorizontalAlignment = HorizontalAlignment.Left, Opacity = 0 };
        ctx.Art.Bind(url, source =>
        {
            image.Source = source;
            Motion.FadeIn(image);
        });
        return image;
    }

    void Toggle(SpoilerGroup group)
    {
        var key = (Id, group.At);
        var open = !pane.Revealed.Remove(key);
        if (open)
            pane.Revealed.Add(key);
        group.Set(open, true);
    }

    // A long comment stops at ten lines, with More.

    FrameworkElement? foldContent;
    Grid? foldBox;

    UIElement Folding(StackPanel column)
    {
        foldContent = column;
        foldBox = new Grid { VerticalAlignment = VerticalAlignment.Top };
        foldBox.Children.Add(column);
        folded = !pane.Expanded.Contains(Id);
        column.SizeChanged += (_, _) => Refold();
        foldBox.SizeChanged += (_, e) => foldBox.Clip = new RectangleGeometry { Rect = new Rect(0, 0, e.NewSize.Width, e.NewSize.Height) };
        return foldBox;
    }

    void Refold()
    {
        if (foldContent is null || foldBox is null)
            return;
        folded = !pane.Expanded.Contains(Id);
        var limit = Discussions.FoldLines * LineHeight;
        var long_ = foldContent.ActualHeight > limit + LineHeight / 2;
        // The cut falls between two lines, in whichever paragraph the tenth line is.
        var cut = limit;
        if (long_ && foldContent is StackPanel column)
        {
            var top = 0.0;
            foreach (var child in column.Children.OfType<FrameworkElement>())
            {
                if (top + child.ActualHeight > limit)
                {
                    cut = top + Math.Max(1, Math.Floor((limit - top) / LineHeight)) * LineHeight;
                    break;
                }
                top += child.ActualHeight + column.Spacing;
            }
        }
        foldBox.MaxHeight = folded && long_ ? cut : double.PositiveInfinity;
        more.Visibility = folded && long_ ? Visibility.Visible : Visibility.Collapsed;
    }

    // The menu: Reply, Copy text and Open on AniList for everyone; Edit and Delete on one's own.

    void FillMenu()
    {
        flyout.Items.Clear();
        var c = Comment;
        if (pane.CanPost && c.Pending == PostState.Posted)
        {
            var replyItem = new MenuFlyoutItem { Text = T.DiscussionReply };
            replyItem.Click += (_, _) => pane.Compose(new ComposeMode.Reply(c.Id, c.Author));
            flyout.Items.Add(replyItem);
        }
        var copy = new MenuFlyoutItem { Text = T.DiscussionCopyText };
        copy.Click += (_, _) =>
        {
            Pages.Ui.TryCopy(PlainText(c));
        };
        flyout.Items.Add(copy);
        var open = new MenuFlyoutItem { Text = T.TitleOpenAnilist };
        open.Click += (_, _) => ctx.Send(new Command.OpenExternal(pane.CommentUrl(c.Id)));
        flyout.Items.Add(open);
        if (c.Own && c.Pending == PostState.Posted)
        {
            flyout.Items.Add(new MenuFlyoutSeparator());
            var edit = new MenuFlyoutItem { Text = T.ActionEdit };
            edit.Click += (_, _) => pane.Compose(new ComposeMode.Edit(c.Id), c.Markdown);
            flyout.Items.Add(edit);
            var delete = new MenuFlyoutItem { Text = T.ActionDelete, Foreground = new SolidColorBrush(Ui.Vermilion) };
            delete.Click += (_, _) => _ = pane.ConfirmDelete(c.Id);
            flyout.Items.Add(delete);
        }
    }

    static string PlainText(CommentView c) =>
        string.Join("\n\n", (c.Body ?? []).Select(b => string.Concat((b.Spans ?? []).Select(s => s.Text))));

    /// <summary>A shot's menu: this comment's menu open.</summary>
    public void OpenMenu() => flyout.ShowAt(card, new FlyoutShowOptions { Position = new Point(120, 60) });

    public void Focus() => like.Focus(FocusState.Programmatic);

    /// <summary>Spoiler words: a bar of Raised selected as long as the hidden text, opened by a click.</summary>
    sealed class SpoilerGroup(int at, bool open, Action<SpoilerGroup> toggle)
    {
        public int At { get; } = at;
        readonly List<(SpoilerBar Bar, TextBlock Text)> words = [];
        bool shown = open;

        public IEnumerable<Inline> Chunks(CommentSpan[] spans)
        {
            foreach (var span in spans)
            {
                foreach (Match m in Regex.Matches(span.Text, @"\S+\s*|\s+"))
                {
                    var text = new TextBlock { Text = m.Value.Replace('\n', ' '), FontSize = 14, Opacity = shown ? 1 : 0 };
                    if (span.Bold)
                        text.FontWeight = FontWeights.SemiBold;
                    if (span.Italic)
                        text.FontStyle = global::Windows.UI.Text.FontStyle.Italic;
                    if (span.Strike)
                        text.TextDecorations = global::Windows.UI.Text.TextDecorations.Strikethrough;
                    var bar = new SpoilerBar
                    {
                        Background = Ui.Res<Brush>("SpoilerBarBrush"),
                        Opacity = shown ? 0.55 : 1,
                        CornerRadius = new CornerRadius(words.Count == 0 ? 4 : 0, 0, 0, words.Count == 0 ? 4 : 0),
                        Content = text,
                        Padding = new Thickness(0),
                        IsTabStop = words.Count == 0,
                        UseSystemFocusVisuals = true,
                    };
                    var tint = (Border: bar, Text: text);
                    words.Add(tint);
                    // One element for the whole spoiler: the first word is the toggle, the rest are hidden from the tree.
                    bar.Toggles = words.Count == 1;
                    AutomationProperties.SetName(bar, T.DiscussionSpoiler);
                    AutomationProperties.SetAccessibilityView(bar, words.Count == 1 ? AccessibilityView.Content : AccessibilityView.Raw);
                    bar.Toggle = () => toggle(this);
                    bar.Tapped += (_, e) =>
                    {
                        e.Handled = true;
                        toggle(this);
                    };
                    bar.KeyDown += (_, e) =>
                    {
                        if (e.Key is VirtualKey.Enter or VirtualKey.Space)
                        {
                            e.Handled = true;
                            toggle(this);
                        }
                    };
                    var inline = new InlineUIContainer { Child = bar };
                    yield return inline;
                }
            }
            Set(shown, false);
        }

        public void Set(bool open, bool animate)
        {
            shown = open;
            foreach (var (bar, text) in words)
            {
                if (animate && Motion.Enabled)
                {
                    text.OpacityTransition ??= new ScalarTransition { Duration = TimeSpan.FromMilliseconds(150) };
                    bar.OpacityTransition ??= new ScalarTransition { Duration = TimeSpan.FromMilliseconds(150) };
                }
                text.Opacity = open ? 1 : 0;
                bar.Opacity = open ? 0.55 : 1;
                AutomationProperties.SetAccessibilityView(text, open ? AccessibilityView.Content : AccessibilityView.Raw);
                bar.Shown = open;
            }
        }
    }

    /// <summary>
    /// A spoiler word's bar. The first word of a spoiler reports a toggle (hidden or shown) to a
    /// screen reader; the others are out of the tree.
    /// </summary>
    sealed class SpoilerBar : UserControl
    {
        public Action? Toggle { get; set; }
        public bool Toggles { get; set; }
        bool shown;

        public bool Shown
        {
            get => shown;
            set
            {
                if (shown == value)
                    return;
                shown = value;
                if (Toggles && FrameworkElementAutomationPeer.FromElement(this) is { } peer)
                {
                    peer.RaisePropertyChangedEvent(
                        TogglePatternIdentifiers.ToggleStateProperty,
                        value ? ToggleState.Off : ToggleState.On,
                        value ? ToggleState.On : ToggleState.Off);
                }
            }
        }

        protected override AutomationPeer OnCreateAutomationPeer() => new Peer(this);

        sealed class Peer(SpoilerBar owner) : FrameworkElementAutomationPeer(owner), IToggleProvider
        {
            public ToggleState ToggleState => owner.shown ? ToggleState.On : ToggleState.Off;

            public void Toggle() => owner.Toggle?.Invoke();

            protected override AutomationControlType GetAutomationControlTypeCore() => AutomationControlType.Button;

            protected override object GetPatternCore(PatternInterface pattern) =>
                pattern == PatternInterface.Toggle ? this : base.GetPatternCore(pattern);
        }
    }

    /// <summary>The like button: a plain button to look at, a toggle (liked or not) to a screen reader.</summary>
    sealed class LikeButton : Button
    {
        public Action? Toggle { get; set; }
        bool liked;

        public bool Liked
        {
            get => liked;
            set
            {
                if (liked == value)
                    return;
                liked = value;
                if (FrameworkElementAutomationPeer.FromElement(this) is { } peer)
                {
                    peer.RaisePropertyChangedEvent(
                        TogglePatternIdentifiers.ToggleStateProperty,
                        value ? ToggleState.Off : ToggleState.On,
                        value ? ToggleState.On : ToggleState.Off);
                }
            }
        }

        protected override AutomationPeer OnCreateAutomationPeer() => new Peer(this);

        sealed class Peer(LikeButton owner) : ButtonAutomationPeer(owner), IToggleProvider
        {
            public ToggleState ToggleState => owner.liked ? ToggleState.On : ToggleState.Off;

            public void Toggle() => owner.Toggle?.Invoke();

            protected override object GetPatternCore(PatternInterface pattern) =>
                pattern == PatternInterface.Toggle ? this : base.GetPatternCore(pattern);
        }
    }
}
