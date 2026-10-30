using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;

namespace Anistream.Windows.Pages;

/// <summary>
/// Play on TV's dialog: the TVs to choose from, pairing with one by the six digits it shows, and
/// the card that drives the TV once it plays. The engine's sender says what the rows, the
/// pairing's failures and the card read (<see cref="TvRemote"/>); this lays them out as the
/// Mac's sheet does, in a ContentDialog with its buttons at the foot. One dialog for all three,
/// its content swapped in place, so moving from the picker to pairing to the card never shows
/// the dialog closing.
/// </summary>
sealed class TvDialog
{
    readonly Ctx ctx;
    readonly TvRemote tv;
    readonly ContentDialog dialog;
    readonly Grid body = new() { Width = 420 };
    readonly XamlRoot root;
    readonly DispatcherQueueTimer clock;
    Action? primary, secondary;
    string? buttonsSet;
    View? shown;
    UIElement? keysAt;
    bool closing;

    public static TvDialog? Showing { get; private set; }

    TvDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        this.root = root;
        tv = ctx.Tv;
        dialog = new ContentDialog { XamlRoot = root, Content = body };
        // As wide as the Mac's sheet, and narrower in a narrow window so nothing clips.
        Fit();
        root.Changed += Resized;
        // Space and K play or pause the TV from the remote, as they do the player. The dialog's
        // content sits in a popup of its own, so the keys are heard at the top of that, which
        // the dialog's buttons are under too.
        body.Loaded += (_, _) =>
        {
            DependencyObject top = body;
            while (VisualTreeHelper.GetParent(top) is { } parent)
                top = parent;
            if (top is UIElement element && keysAt is null)
            {
                keysAt = element;
                element.PreviewKeyDown += Keys;
            }
        };
        dialog.PrimaryButtonClick += (_, e) =>
        {
            e.Cancel = true;
            primary?.Invoke();
        };
        dialog.SecondaryButtonClick += (_, e) =>
        {
            e.Cancel = true;
            secondary?.Invoke();
        };
        // Pairing's Back, and Escape there, go back to the picker rather than closing.
        dialog.Closing += (_, e) =>
        {
            if (!closing && e.Result == ContentDialogResult.None && tv.Pairing is not null)
            {
                e.Cancel = true;
                tv.CancelPairing();
            }
        };
        clock = dialog.DispatcherQueue.CreateTimer();
        clock.Interval = TimeSpan.FromSeconds(1);
        clock.Tick += (_, _) => shown?.Tick();
    }

    /// <summary>Shows the dialog until the viewer closes it, or the remote says the sheet is down.</summary>
    public static async Task Show(Ctx ctx, XamlRoot root)
    {
        if (Showing is not null)
            return;
        var it = new TvDialog(ctx, root);
        Showing = it;
        it.tv.Changed += it.Update;
        // The browse runs while the dialog is up, not only the picker: a row's key is looked up
        // among the TVs found as pairing starts.
        it.tv.StartBrowsing();
        it.Update();
        it.clock.Start();
        try
        {
            await it.dialog.ShowAsync();
        }
        finally
        {
            it.clock.Stop();
            it.root.Changed -= it.Resized;
            if (it.keysAt is not null)
                it.keysAt.PreviewKeyDown -= it.Keys;
            it.tv.Changed -= it.Update;
            it.shown?.Leave();
            it.tv.StopBrowsing();
            Showing = null;
            it.tv.SheetClosed();
        }
    }

    void Keys(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key is not (global::Windows.System.VirtualKey.Space or global::Windows.System.VirtualKey.K))
            return;
        if (shown is RemoteView remote)
            e.Handled = remote.Key(e.Key, FocusManager.GetFocusedElement(root));
        else if (shown is CastView casting)
            e.Handled = casting.Key(e.Key, FocusManager.GetFocusedElement(root));
    }

    void Resized(XamlRoot sender, XamlRootChangedEventArgs e)
    {
        var was = body.Width;
        Fit();
        if (body.Width != was)
            shown?.Update();
    }

    /// <summary>Whether the dialog is at its full width, with room for the longer button labels.</summary>
    bool Roomy => body.Width >= 420;

    /// <summary>The body's width: 420, less in a window too narrow for the dialog around it.</summary>
    void Fit() => body.Width = Math.Clamp(root.Size.Width - 112, 260, 420);

    /// <summary>Closes the dialog from outside: the sheet went down.</summary>
    public void Close()
    {
        closing = true;
        dialog.Hide();
    }

    /// <summary>The view for what the remote shows now, made once per kind and then kept up to date.</summary>
    void Update()
    {
        if (tv.Sheet is null)
        {
            Close();
            return;
        }
        View next = tv.Pairing is { } step ? shown as PairingView ?? new PairingView(this, step)
            : tv.Sheet == TvSheet.Remote ? shown as RemoteView ?? new RemoteView(this)
            : tv.Sheet == TvSheet.Cast && tv.Casting ? shown as CastView ?? new CastView(this)
            : shown as PickerView ?? new PickerView(this);
        if (next != shown)
        {
            // The dialog grows or shrinks to the new view on the standard token, rather than jumping.
            var from = shown is null ? 0 : body.ActualHeight;
            shown?.Leave();
            shown = next;
            body.Children.Clear();
            body.Children.Add(next.Root);
            Motion.FadeIn(next.Root);
            next.Update();
            // The row picked has gone with the picker: the focus goes to Done, not the next button along.
            if (next is CastView)
                dialog.DispatcherQueue.TryEnqueue(() => (Named(dialog, "CloseButton") as Control)?.Focus(FocusState.Programmatic));
            if (from > 0)
            {
                next.Root.Measure(new global::Windows.Foundation.Size(body.Width, double.PositiveInfinity));
                Motion.Height(body, from, next.Root.DesiredSize.Height);
            }
            return;
        }
        next.Update();
    }

    /// <summary>The element named <paramref name="name"/> in <paramref name="at"/>'s visual tree, such as a dialog's own button.</summary>
    static DependencyObject? Named(DependencyObject at, string name)
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(at); i++)
        {
            var child = VisualTreeHelper.GetChild(at, i);
            if (child is FrameworkElement element && element.Name == name)
                return child;
            if (Named(child, name) is { } found)
                return found;
        }
        return null;
    }

    /// <summary>The dialog's title and its three buttons, for the view up.</summary>
    void Buttons(string? title, (string Text, Action Act, bool Enabled, bool Accent)? first, (string Text, Action Act, bool Enabled)? second, string close, bool closeAccent = false)
    {
        primary = first?.Act;
        secondary = second?.Act;
        // Set again only when they change: a button's style set again takes the focus off it.
        var key = $"{title}\u0001{first?.Text}\u0001{first?.Enabled}\u0001{first?.Accent}\u0001{second?.Text}\u0001{second?.Enabled}\u0001{close}\u0001{closeAccent}";
        if (key == buttonsSet)
            return;
        buttonsSet = key;
        dialog.Title = title;
        dialog.PrimaryButtonText = first?.Text ?? "";
        dialog.IsPrimaryButtonEnabled = first?.Enabled ?? false;
        dialog.PrimaryButtonStyle = first is { Accent: true } ? Ui.Res<Style>("AccentButtonStyle") : Ui.Res<Style>("DefaultButtonStyle");
        // The casting card keeps Done in amber, as the board draws it, wherever the focus is.
        dialog.CloseButtonStyle = closeAccent ? Ui.Res<Style>("AccentButtonStyle") : Ui.Res<Style>("DefaultButtonStyle");
        dialog.DefaultButton = first is { Accent: true } ? ContentDialogButton.Primary : closeAccent ? ContentDialogButton.Close : ContentDialogButton.None;
        dialog.SecondaryButtonText = second?.Text ?? "";
        dialog.IsSecondaryButtonEnabled = second?.Enabled ?? false;
        dialog.CloseButtonText = close;
    }

    /// <summary>Has Narrator say what an element now says, once.</summary>
    static void Announce(UIElement element) =>
        FrameworkElementAutomationPeer.CreatePeerForElement(element)?.RaiseAutomationEvent(AutomationEvents.LiveRegionChanged);

    /// <summary>
    /// What went wrong, as the app draws an error: the vermilion dot, then the engine's words in
    /// plain text. Said by Narrator as it changes.
    /// </summary>
    sealed class ProblemLine
    {
        readonly TextBlock text = Ui.Text("", "CaptionTextBlockStyle");
        string? said;

        public ProblemLine()
        {
            var dot = Ui.Dot(Ui.Vermilion);
            dot.Margin = new Thickness(0, 5, 0, 0);
            dot.VerticalAlignment = VerticalAlignment.Top;
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            Root.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            Root.Children.Add(dot);
            Grid.SetColumn(text, 1);
            Root.Children.Add(text);
            AutomationProperties.SetAccessibilityView(dot, AccessibilityView.Raw);
            AutomationProperties.SetLiveSetting(text, AutomationLiveSetting.Assertive);
            Root.Visibility = Visibility.Collapsed;
        }

        public Grid Root { get; } = new() { ColumnSpacing = 8 };

        public void Say(string? problem)
        {
            if (problem == said)
                return;
            said = problem;
            text.Text = problem ?? "";
            Root.Visibility = problem is null ? Visibility.Collapsed : Visibility.Visible;
            if (problem is not null)
                Announce(text);
        }
    }

    /// <summary>Rows on a card with hairlines between them, as Settings draws its groups.</summary>
    static Border Rows(StackPanel rows) => new()
    {
        Child = rows,
        Background = Ui.Res<Brush>("CardBackgroundFillColorDefaultBrush"),
        BorderBrush = Ui.Res<Brush>("CardStrokeColorDefaultBrush"),
        BorderThickness = new Thickness(1),
        CornerRadius = new CornerRadius(8),
    };

    static Border Hairline() => new()
    {
        Height = 1,
        Margin = new Thickness(14, 0, 0, 0),
        Background = Ui.Res<Brush>("DividerStrokeColorDefaultBrush"),
    };

    abstract class View
    {
        public abstract FrameworkElement Root { get; }
        public abstract void Update();
        public virtual void Tick() { }
        public virtual void Leave() { }
    }

    // The picker.

    /// <summary>
    /// The TVs found and paired, what the engine says under each, and an address typed in for a
    /// TV the browse does not find. While none is listed, nothing for the first 600 ms, as most
    /// TVs answer by then; then the search, said; and after 10 s, what to check. The browse goes
    /// on throughout, and a TV found replaces the row. With an episode to play, TVs with
    /// Chromecast built in follow in a group of their own, which comes with its first TV, and
    /// until a cast from this PC has played, a line saying Windows' firewall will ask.
    /// </summary>
    sealed class PickerView : View
    {
        readonly TvDialog owner;
        readonly StackPanel root = new() { Spacing = 0 };
        readonly TextBlock line = Ui.Haze("");
        readonly StackPanel rows = new();
        readonly Border rowsCard;
        readonly StackPanel castRows = new();
        readonly StackPanel castGroup = new() { Spacing = 8, Margin = new Thickness(0, 20, 0, 0), Visibility = Visibility.Collapsed };
        readonly TextBlock firewall = Ui.Text(T.TvCastFirewallAhead, "HazeCaptionStyle");
        readonly StackPanel typed = new() { Spacing = 6, Margin = new Thickness(0, 14, 0, 0), Visibility = Visibility.Collapsed };
        readonly TextBox address = new() { PlaceholderText = "192.168.1.20:47811", Header = T.TvAddress };
        readonly DateTimeOffset opened = DateTimeOffset.UtcNow;
        string? drawn;
        bool typing;

        public PickerView(TvDialog owner)
        {
            this.owner = owner;
            root.Children.Add(line);
            rowsCard = Rows(rows);
            rowsCard.Margin = new Thickness(0, 16, 0, 0);
            root.Children.Add(rowsCard);
            var heading = Ui.Text(T.TvCastGroup, "BodyStrongTextBlockStyle", wrap: false);
            AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
            castGroup.Children.Add(heading);
            castGroup.Children.Add(Rows(castRows));
            firewall.Margin = new Thickness(0, 4, 0, 0);
            castGroup.Children.Add(firewall);
            root.Children.Add(castGroup);
            address.InputScope = new InputScope { Names = { new(InputScopeNameValue.Url) } };
            address.TextChanged += (_, _) => owner.dialog.IsPrimaryButtonEnabled = address.Text.Trim().Length > 0;
            address.KeyDown += (_, e) =>
            {
                if (e.Key == global::Windows.System.VirtualKey.Enter)
                {
                    e.Handled = true;
                    Connect();
                }
            };
            typed.Children.Add(address);
            typed.Children.Add(Ui.Text(TvWords.Address, "HazeCaptionStyle"));
            root.Children.Add(typed);
        }

        public override FrameworkElement Root => root;

        void Connect()
        {
            if (address.Text.Trim() is { Length: > 0 } text)
                _ = owner.tv.Begin(text);
        }

        public override void Update()
        {
            var tv = owner.tv;
            line.Text = TvWords.PickerLine(tv.Request);
            owner.Buttons(TvWords.PickerTitle(tv.Request),
                typing ? (T.TvConnect, Connect, address.Text.Trim().Length > 0, true) : null,
                typing ? null : (T.TvTypeAddress, Type, true),
                T.ActionDone);
            // TVs with Chromecast built in only with an episode to cast; the group comes with its first TV.
            var casts = tv.Request is null ? [] : tv.CastRows;
            castGroup.Visibility = casts.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            firewall.Visibility = tv.CastServed ? Visibility.Collapsed : Visibility.Visible;
            // The rows again only when they read differently, so focus stays where it was.
            var now = string.Join("|", tv.Rows.Select(r => $"{r.Key},{r.Name},{r.Detail},{r.Kind},{r.Fingerprint == tv.Connected}"))
                + "#" + string.Join("|", casts.Select(r => $"{r.Key},{r.Name}"));
            if (now == drawn && (tv.Rows.Length > 0 || casts.Length > 0))
                return;
            drawn = now;
            castRows.Children.Clear();
            foreach (var (row, i) in casts.Select((r, i) => (r, i)))
            {
                if (i > 0)
                    castRows.Children.Add(Hairline());
                castRows.Children.Add(CastRow(row));
            }
            rows.Children.Clear();
            // One searching row covers both browses; a TV with Chromecast built in found first
            // leaves the anistream TVs' card out until one of those answers.
            rowsCard.Visibility = tv.Rows.Length == 0 && casts.Length > 0 ? Visibility.Collapsed : Visibility.Visible;
            if (tv.Rows.Length == 0)
            {
                if (casts.Length == 0)
                    rows.Children.Add(Searching());
                return;
            }
            foreach (var (row, i) in tv.Rows.Select((r, i) => (r, i)))
            {
                if (i > 0)
                    rows.Children.Add(Hairline());
                rows.Children.Add(Row(row));
            }
        }

        void Type()
        {
            typing = true;
            typed.Visibility = Visibility.Visible;
            Update();
            address.Focus(FocusState.Programmatic);
        }

        public override void Tick()
        {
            if (owner.tv.Rows.Length == 0 && rowsCard.Visibility == Visibility.Visible)
            {
                var stage = Stage();
                if (rows.Children is [Grid { Tag: string was }] && was != stage)
                {
                    rows.Children.Clear();
                    var row = Searching();
                    rows.Children.Add(row);
                    row.Loaded += (_, _) => Announce(row);
                }
            }
        }

        string Stage()
        {
            var waited = DateTimeOffset.UtcNow - opened;
            return waited < TimeSpan.FromMilliseconds(600) ? "quiet" : waited < TimeSpan.FromSeconds(10) ? "looking" : "none";
        }

        /// <summary>The row while no TV is listed: one height in each stage, so nothing below it moves.</summary>
        Grid Searching()
        {
            var stage = Stage();
            var grid = new Grid { MinHeight = 56, Padding = new Thickness(14, 8, 14, 8), Tag = stage };
            switch (stage)
            {
                case "looking":
                {
                    var looking = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 10, VerticalAlignment = VerticalAlignment.Center };
                    looking.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
                    looking.Children.Add(Ui.Haze(TvWords.Looking, wrap: false));
                    grid.Children.Add(looking);
                    break;
                }
                case "none":
                {
                    var none = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
                    none.Children.Add(Ui.Text(TvWords.NoneFound));
                    none.Children.Add(Ui.Text(TvWords.NoneLine(owner.tv.Request), "HazeCaptionStyle"));
                    if (owner.tv.Request is not null)
                        none.Children.Add(Ui.Text(T.TvCastNoneFound, "HazeCaptionStyle"));
                    grid.Children.Add(none);
                    break;
                }
            }
            if (stage != "quiet")
            {
                Motion.FadeIn(grid);
                AutomationProperties.SetLiveSetting(grid, AutomationLiveSetting.Polite);
                AutomationProperties.SetName(grid, stage == "looking" ? TvWords.Looking : T.TvNoneFoundName(TvWords.NoneFound, TvWords.NoneLine(owner.tv.Request)));
            }
            return grid;
        }

        /// <summary>A TV with Chromecast built in: its name, and how the cast reaches it.</summary>
        Button CastRow(FfiCastRow row)
        {
            var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            words.Children.Add(Ui.Text(row.Name, wrap: false));
            if (TvWords.CastRoute(row) is { } route)
                words.Children.Add(Ui.Text(route, "HazeCaptionStyle", wrap: false));
            var button = new Button
            {
                Content = words,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                MinHeight = 56,
                Padding = new Thickness(14, 8, 14, 8),
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
                CornerRadius = new CornerRadius(6),
            };
            AutomationProperties.SetName(button, TvWords.CastRoute(row) is { } spoken ? T.TvRowName(row.Name, spoken) : row.Name);
            button.Click += (_, _) => owner.tv.Cast(row);
            return button;
        }

        /// <summary>A TV: its name, what the engine says under it, and what choosing it does.</summary>
        Button Row(FfiTvRow row)
        {
            var tv = owner.tv;
            var grid = new Grid { ColumnSpacing = 12 };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            words.Children.Add(Ui.Text(row.Name, wrap: false));
            words.Children.Add(Ui.Text(row.Detail, "HazeCaptionStyle", wrap: false));
            grid.Children.Add(words);
            FrameworkElement? end = row.Kind switch
            {
                FfiTvRowKind.New => new TextBlock { Text = T.ActionPair, Style = Ui.Res<Style>("BodyStrongTextBlockStyle") },
                FfiTvRowKind.Paired or FfiTvRowKind.PairedAway when tv.Connected == row.Fingerprint => new FontIcon { Glyph = "", FontSize = 14 },
                FfiTvRowKind.Paired or FfiTvRowKind.PairedAway => new FontIcon { Glyph = "", FontSize = 12, Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush") },
                _ => null,
            };
            if (end is not null)
            {
                end.VerticalAlignment = VerticalAlignment.Center;
                Grid.SetColumn(end, 1);
                grid.Children.Add(end);
            }
            var button = new Button
            {
                Content = grid,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                MinHeight = 56,
                Padding = new Thickness(14, 8, 14, 8),
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
                BorderThickness = new Thickness(0),
                CornerRadius = new CornerRadius(6),
                IsEnabled = row.Kind != FfiTvRowKind.NeedsUpdate,
                Opacity = row.Kind == FfiTvRowKind.NeedsUpdate ? 0.55 : 1,
            };
            var connected = tv.Connected == row.Fingerprint && row.Fingerprint is not null;
            AutomationProperties.SetName(button, connected ? T.TvRowNameConnected(row.Name, row.Detail) : T.TvRowName(row.Name, row.Detail));
            if (row.Kind == FfiTvRowKind.New)
                AutomationProperties.SetHelpText(button, T.TvRowPairHelp);
            button.Click += (_, _) => tv.Choose(row);
            return button;
        }
    }

    // Pairing.

    /// <summary>
    /// Pairing with a TV: while it answers, that it takes a moment; then the six digits it
    /// shows, typed into boxes as the code reads on the TV, sent once there are six. A wrong
    /// code empties the boxes for another try; one that ran out offers a new one.
    /// </summary>
    sealed class PairingView : View
    {
        readonly TvDialog owner;
        readonly StackPanel root = new() { Spacing = 0 };
        readonly TextBlock line = Ui.Haze("");
        readonly Grid code = new() { Margin = new Thickness(0, 18, 0, 0), HorizontalAlignment = HorizontalAlignment.Left };
        readonly TextBox typed = new()
        {
            MaxLength = 6,
            Opacity = 0.01,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
            BorderThickness = new Thickness(0),
        };
        readonly Border[] boxes = new Border[6];
        readonly TextBlock[] digits = new TextBlock[6];
        readonly StackPanel waiting = new() { Orientation = Orientation.Horizontal, Spacing = 10, Margin = new Thickness(0, 18, 0, 0) };
        readonly ProblemLine problem = new();
        readonly ProgressRing working = new() { IsActive = true, Width = 16, Height = 16, HorizontalAlignment = HorizontalAlignment.Left };
        string? lastProblem;
        bool focused;

        public PairingView(TvDialog owner, TvPairingStep step)
        {
            this.owner = owner;
            root.Children.Add(line);
            var boxRow = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, IsHitTestVisible = false };
            AutomationProperties.SetAccessibilityView(boxRow, AccessibilityView.Raw);
            for (var i = 0; i < 6; i++)
            {
                digits[i] = new TextBlock
                {
                    FontSize = 24,
                    FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                    HorizontalAlignment = HorizontalAlignment.Center,
                    VerticalAlignment = VerticalAlignment.Center,
                };
                Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(digits[i], FontNumeralAlignment.Tabular);
                boxes[i] = new Border
                {
                    Width = 42,
                    Height = 50,
                    CornerRadius = new CornerRadius(6),
                    BorderThickness = new Thickness(1),
                    Background = Ui.Res<Brush>("ControlFillColorDefaultBrush"),
                    Child = digits[i],
                };
                boxRow.Children.Add(boxes[i]);
                if (i == 2)
                    boxRow.Children.Add(new Border { Width = 8 });
            }
            code.Children.Add(boxRow);
            code.Children.Add(typed);
            AutomationProperties.SetName(typed, T.TvPairingCode);
            typed.InputScope = new InputScope { Names = { new(InputScopeNameValue.Number) } };
            typed.TextChanged += (_, _) =>
            {
                var kept = new string([.. typed.Text.Where(char.IsAsciiDigit).Take(6)]);
                if (kept != typed.Text)
                {
                    typed.Text = kept;
                    typed.SelectionStart = kept.Length;
                    return;
                }
                Draw();
                Update();
                if (kept.Length == 6)
                    Submit();
            };
            typed.GotFocus += (_, _) => { focused = true; Draw(); };
            typed.LostFocus += (_, _) => { focused = false; Draw(); };
            code.Tapped += (_, _) =>
            {
                if (!typed.IsReadOnly)
                    typed.Focus(FocusState.Pointer);
            };
            Motion.QuickOpacity(code);
            root.Children.Add(code);
            waiting.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
            waiting.Children.Add(Ui.Haze(T.TvWaiting, wrap: false));
            root.Children.Add(waiting);
            problem.Root.Margin = new Thickness(0, 12, 0, 0);
            root.Children.Add(problem.Root);
            working.Margin = new Thickness(0, 12, 0, 0);
            root.Children.Add(working);
            Draw();
        }

        public override FrameworkElement Root => root;

        void Submit()
        {
            if (TvWords.SixDigits(typed.Text) is { } six)
                _ = owner.tv.Submit(six);
        }

        void Again()
        {
            typed.Text = "";
            _ = owner.tv.RequestCodeAgain();
            typed.Focus(FocusState.Programmatic);
        }

        void Draw()
        {
            var text = typed.Text;
            for (var i = 0; i < 6; i++)
            {
                digits[i].Text = i < text.Length ? text[i].ToString() : "";
                var next = focused && !typed.IsReadOnly && i == Math.Min(text.Length, 5);
                boxes[i].BorderBrush = next ? Ui.Res<Brush>("TextFillColorPrimaryBrush") : Ui.Res<Brush>("ControlStrokeColorDefaultBrush");
            }
        }

        public override void Update()
        {
            if (owner.tv.Pairing is not { } step)
                return;
            var typing = step.Typing;
            line.Text = TvWords.PairingLine(step);
            // Once a try failed before the code, its problem says what happened in the line's place.
            line.Visibility = !typing && step.Problem is not null ? Visibility.Collapsed : Visibility.Visible;
            code.Visibility = typing ? Visibility.Visible : Visibility.Collapsed;
            waiting.Visibility = typing ? Visibility.Collapsed : Visibility.Visible;
            ((TextBlock)waiting.Children[1]).Text = step.TvName.Length == 0 ? T.TvWaiting : T.TvWaitingFor(step.TvName);
            // A code that ran out takes no more digits: the boxes lock and dim until a new one.
            typed.IsReadOnly = step.Working || step.Expired;
            code.Opacity = step.Expired ? 0.4 : 1;
            working.Visibility = typing && step.Working ? Visibility.Visible : Visibility.Collapsed;
            if (step.Problem != lastProblem)
            {
                lastProblem = step.Problem;
                problem.Say(step.Problem);
                if (step.Problem is not null)
                {
                    // Another try starts from empty boxes.
                    typed.Text = "";
                    if (typing && !step.Expired)
                        typed.Focus(FocusState.Programmatic);
                }
                // The spinner stopped: the problem in its place.
                if (step.Problem is not null && !typing)
                    waiting.Visibility = Visibility.Collapsed;
            }
            else if (step.Problem is not null && !typing)
                waiting.Visibility = Visibility.Collapsed;
            if (typing && step.Expired)
                owner.Buttons(TvWords.PairingTitle(step), (TvWords.NewCode, Again, !step.Working, true), null, T.ActionBack);
            else
                owner.Buttons(TvWords.PairingTitle(step),
                    typing ? (T.ActionPair, Submit, TvWords.SixDigits(typed.Text) is not null && !step.Working, true) : null,
                    typing ? (TvWords.NewCode, Again, !step.Working) : null,
                    T.ActionBack);
            Draw();
            if (typing && !step.Working && !focused && step.Problem is null)
                typed.Focus(FocusState.Programmatic);
        }
    }

    // The card.

    /// <summary>
    /// The TV this PC drives: whether the link is up, what it plays with its playhead, back and
    /// forward, play or pause and the next episode, its subtitles, audio and volume, and a search
    /// typed on the TV. While the link is lost the controls dim and the playhead stops where it
    /// was; the TV letting this PC go offers Pair again.
    /// </summary>
    sealed class RemoteView : View
    {
        readonly TvDialog owner;
        readonly StackPanel root = new() { Spacing = 0 };
        readonly Ellipse dot = new() { Width = 6, Height = 6, VerticalAlignment = VerticalAlignment.Center };
        readonly TextBlock status = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly StackPanel playing = new() { Spacing = 0 };
        readonly Grid nowRow = new() { ColumnSpacing = 12, Margin = new Thickness(0, 18, 0, 0) };
        Border cover = new() { Width = 36, Height = 54, CornerRadius = new CornerRadius(4) };
        readonly TextBlock title = Ui.Text("", "ShowTitleStyle", wrap: false);
        readonly TextBlock episode = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly Button skip = new() { Height = 32 };
        // Arrow keys step 10 s, as the transport's buttons and the Mac's playhead do.
        readonly Slider seek = new() { Minimum = 0, Maximum = 1, StepFrequency = 1, SmallChange = 10, LargeChange = 60, IsThumbToolTipEnabled = false, Margin = new Thickness(0, 10, 0, 0) };
        readonly TextBlock elapsed = Ui.Text("0:00", "HazeCaptionStyle", wrap: false);
        readonly TextBlock remaining = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly StackPanel transport = new() { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Center, Margin = new Thickness(0, 2, 0, 0) };
        readonly FontIcon playIcon = new() { Glyph = "", FontSize = 22 };
        readonly Button playButton = new();
        readonly StackPanel settings = new();
        readonly DropDownButton subtitles = new() { MinWidth = 120 };
        readonly DropDownButton audio = new() { MinWidth = 120 };
        readonly TextBlock volumeNote = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly StackPanel volume = new() { Orientation = Orientation.Horizontal, Spacing = 6 };
        readonly Border rowsCard;
        readonly TextBlock nothing = Ui.Haze(TvWords.Nothing);
        readonly TextBox search = new() { PlaceholderText = T.TvSearchOnTv, Margin = new Thickness(0, 14, 0, 0) };
        readonly ProblemLine problem = new();
        readonly DispatcherQueueTimer settle;
        string? statusSaid;
        double? held;
        ulong? heldRev;
        DateTimeOffset heldAt;
        bool quiet;
        uint? coverFor;
        string? tracksDrawn;

        public RemoteView(TvDialog owner)
        {
            this.owner = owner;
            settle = owner.dialog.DispatcherQueue.CreateTimer();
            settle.Interval = TimeSpan.FromMilliseconds(350);
            settle.IsRepeating = false;
            settle.Tick += (_, _) =>
            {
                if (held is { } to)
                    _ = owner.tv.Send(new FfiTvCommand.SeekTo(to));
            };
            var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            header.Children.Add(dot);
            header.Children.Add(status);
            AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);
            root.Children.Add(header);

            // What plays: the cover, its title and episode, and the skip on offer.
            nowRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            nowRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            nowRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            nowRow.Children.Add(cover);
            var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            title.FontSize = 15;
            words.Children.Add(title);
            words.Children.Add(episode);
            Grid.SetColumn(words, 1);
            nowRow.Children.Add(words);
            skip.VerticalAlignment = VerticalAlignment.Center;
            skip.Click += (_, _) => _ = owner.tv.Send(new FfiTvCommand.Skip());
            Grid.SetColumn(skip, 2);
            nowRow.Children.Add(skip);
            playing.Children.Add(nowRow);

            // The playhead, dragged or stepped to seek.
            AutomationProperties.SetName(seek, T.PlayerPosition);
            seek.ValueChanged += (_, e) =>
            {
                if (quiet)
                    return;
                held = e.NewValue;
                heldRev = owner.tv.Card?.Rev;
                heldAt = DateTimeOffset.UtcNow;
                ShowTimes(e.NewValue);
                settle.Stop();
                settle.Start();
            };
            playing.Children.Add(seek);
            Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(elapsed, FontNumeralAlignment.Tabular);
            Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(remaining, FontNumeralAlignment.Tabular);
            var times = new Grid { Margin = new Thickness(0, -6, 0, 0) };
            remaining.HorizontalAlignment = HorizontalAlignment.Right;
            times.Children.Add(elapsed);
            times.Children.Add(remaining);
            playing.Children.Add(times);

            // Back and forward 10 seconds, play or pause, and the next episode, as the player's bar draws them.
            transport.Children.Add(Glyph(Ten(back: true), T.PlayerBack10, new FfiTvCommand.Seek(-10)));
            playButton.Content = playIcon;
            Style(playButton, T.ActionPause);
            playButton.Click += (_, _) => _ = owner.tv.Send(new FfiTvCommand.TogglePause());
            transport.Children.Add(playButton);
            transport.Children.Add(Glyph(Ten(back: false), T.PlayerForward10, new FfiTvCommand.Seek(10)));
            transport.Children.Add(Glyph(new FontIcon { Glyph = "", FontSize = 18 }, T.PlayerNextEpisode, new FfiTvCommand.Next()));
            playing.Children.Add(transport);

            // Subtitles, audio and the TV's own volume.
            settings.Children.Add(SettingRow(T.PlayerSubtitles, null, subtitles));
            settings.Children.Add(Hairline());
            settings.Children.Add(SettingRow(T.PlayerAudio, null, audio));
            settings.Children.Add(Hairline());
            volume.Children.Add(Glyph(new FontIcon { Glyph = "", FontSize = 14 }, T.TvVolumeDown, new FfiTvCommand.Volume(-5), small: true));
            volume.Children.Add(Glyph(new FontIcon { Glyph = "", FontSize = 14 }, T.TvVolumeUp, new FfiTvCommand.Volume(5), small: true));
            settings.Children.Add(SettingRow(T.TvVolumeLabel, volumeNote, volume));
            Motion.QuickOpacity(transport);
            rowsCard = Rows(settings);
            Motion.QuickOpacity(rowsCard);
            rowsCard.Margin = new Thickness(0, 12, 0, 0);
            playing.Children.Add(rowsCard);
            root.Children.Add(playing);

            nothing.Margin = new Thickness(0, 14, 0, 0);
            root.Children.Add(nothing);

            // A search typed here lands in the TV's search, as each letter is typed.
            AutomationProperties.SetName(search, T.TvSearchOnTv);
            search.TextChanged += (_, _) =>
            {
                if (!quiet)
                    _ = owner.tv.Send(new FfiTvCommand.Text(search.Text, false));
            };
            search.KeyDown += (_, e) =>
            {
                if (e.Key == global::Windows.System.VirtualKey.Enter)
                {
                    e.Handled = true;
                    _ = owner.tv.Send(new FfiTvCommand.Text(search.Text, true));
                }
            };
            root.Children.Add(search);
            problem.Root.Margin = new Thickness(0, 10, 0, 0);
            root.Children.Add(problem.Root);
        }

        public override FrameworkElement Root => root;

        /// <summary>
        /// Space or K pressed in the remote: play or pause, while the TV can hear it. Not while
        /// typing a search, nor Space on a focused button or menu, which it presses.
        /// </summary>
        public bool Key(global::Windows.System.VirtualKey key, object? focused)
        {
            if (focused is TextBox || (key == global::Windows.System.VirtualKey.Space && focused is ButtonBase))
                return false;
            if (owner.tv.Now is not { Loading: false } || !owner.tv.Live)
                return false;
            _ = owner.tv.Send(new FfiTvCommand.TogglePause());
            return true;
        }

        public static FrameworkElement Ten(bool back)
        {
            var icon = new FontIcon { Glyph = "", FontSize = 20, RenderTransformOrigin = new global::Windows.Foundation.Point(0.5, 0.5) };
            if (back)
                icon.RenderTransform = new ScaleTransform { ScaleX = -1 };
            var grid = new Grid();
            grid.Children.Add(icon);
            grid.Children.Add(new TextBlock { Text = "10", Style = Ui.Res<Style>("SkipDigitsStyle") });
            return grid;
        }

        public static void Style(Button button, string name)
        {
            button.Width = button.Height = 44;
            button.Padding = new Thickness(0);
            button.Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent);
            button.BorderThickness = new Thickness(0);
            ToolTipService.SetToolTip(button, name);
            AutomationProperties.SetName(button, name);
        }

        Button Glyph(FrameworkElement content, string name, FfiTvCommand command, bool small = false)
        {
            var button = new Button { Content = content };
            if (small)
            {
                button.Width = 36;
                button.Height = 32;
                button.Padding = new Thickness(0);
                ToolTipService.SetToolTip(button, name);
                AutomationProperties.SetName(button, name);
            }
            else
                Style(button, name);
            button.Click += (_, _) => _ = owner.tv.Send(command);
            return button;
        }

        public static Grid SettingRow(string name, TextBlock? note, FrameworkElement control)
        {
            var grid = new Grid { MinHeight = 52, Padding = new Thickness(14, 8, 10, 8), ColumnSpacing = 12 };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            words.Children.Add(Ui.Text(name, wrap: false));
            if (note is not null)
                words.Children.Add(note);
            grid.Children.Add(words);
            control.VerticalAlignment = VerticalAlignment.Center;
            Grid.SetColumn(control, 1);
            grid.Children.Add(control);
            return grid;
        }

        public override void Update()
        {
            var tv = owner.tv;
            var card = tv.Card;
            var now = tv.Now;
            var live = tv.Live;
            dot.Fill = new SolidColorBrush(TvWords.DotFor(card?.Phase) switch
            {
                TvWords.Dot.Water => Colours.Water,
                TvWords.Dot.Vermilion => Ui.Vermilion,
                _ => Colours.Haze,
            });
            status.Text = TvWords.Status(card);
            if (status.Text != statusSaid)
            {
                // Said as the link changes, not when the dialog opens on it.
                if (statusSaid is not null)
                    Announce(status);
                statusSaid = status.Text;
            }
            owner.Buttons(card?.TvName ?? T.TvUnnamed,
                TvWords.Lost(card) ? (T.TvPairAgain, () => _ = tv.PairAgain(), true, false) : (owner.Roomy ? T.TvChooseAnother : T.TvAnotherShort, tv.OpenPicker, true, false),
                (T.ActionDisconnect, () =>
                {
                    tv.Disconnect();
                    owner.Close();
                }, true),
                T.ActionDone);

            playing.Visibility = now is null ? Visibility.Collapsed : Visibility.Visible;
            // What to do from here is said only while the TV can hear it; otherwise the status
            // line says what the link is doing.
            nothing.Visibility = now is null && live ? Visibility.Visible : Visibility.Collapsed;
            search.IsEnabled = live;
            problem.Say(tv.Problem);
            if (now is null)
                return;

            if (coverFor != now.Id)
            {
                coverFor = now.Id;
                nowRow.Children.Remove(cover);
                cover = Ui.Cover(owner.ctx, now.Id, 36, 54);
                cover.CornerRadius = new CornerRadius(4);
                nowRow.Children.Insert(0, cover);
            }
            title.Text = now.Title;
            episode.Text = now.Loading ? TvWords.StartingLine : now.EpisodeLine;
            skip.Visibility = now.Skip is null ? Visibility.Collapsed : Visibility.Visible;
            if (now.Skip is { } offered)
                // The card names the part in English; the button says it from the catalogue.
                skip.Content = offered.Label == "Skip ending" ? T.PlayerSkipEnding : T.PlayerSkipOpening;
            skip.IsEnabled = live;

            var controls = live && !now.Loading;
            transport.Opacity = controls ? 1 : 0.4;
            foreach (var child in transport.Children.OfType<Control>())
                child.IsEnabled = controls;
            playIcon.Glyph = now.Paused ? "" : "";
            var name = now.Paused ? T.ActionPlay : T.ActionPause;
            ToolTipService.SetToolTip(playButton, name);
            AutomationProperties.SetName(playButton, name);
            seek.IsEnabled = controls && now.Duration is not null;

            rowsCard.Opacity = live ? 1 : 0.4;
            subtitles.IsEnabled = live && now.Subtitles.Length > 0;
            audio.IsEnabled = live && now.Audio.Length > 0;
            foreach (var child in volume.Children.OfType<Control>())
                child.IsEnabled = live;
            volumeNote.Text = TvWords.Volume(now) ?? "";
            volumeNote.Visibility = now.Volume is null ? Visibility.Collapsed : Visibility.Visible;
            Tracks(now);

            // A seek the TV answered lets the playhead go.
            if (held is not null && card?.Rev != heldRev)
                held = null;
            Tick();
        }

        /// <summary>The subtitle and audio menus, made again only when the tracks change.</summary>
        void Tracks(FfiNowCard now)
        {
            var key = string.Join("|", now.Subtitles.Concat(now.Audio).Select(t => $"{t.Id},{t.Selected},{t.Language},{t.Title}")) + now.SubtitlesOff;
            if (key == tracksDrawn)
                return;
            tracksDrawn = key;
            subtitles.Content = TvWords.Chosen(now.Subtitles, now.SubtitlesOff);
            audio.Content = TvWords.Chosen(now.Audio, off: false);
            AutomationProperties.SetName(subtitles, T.TvSettingValue(T.PlayerSubtitles, (string)subtitles.Content));
            AutomationProperties.SetName(audio, T.TvSettingValue(T.PlayerAudio, (string)audio.Content));
            subtitles.Flyout = Menu(FfiTvTrackKind.Subtitle, now.Subtitles, now.SubtitlesOff);
            audio.Flyout = Menu(FfiTvTrackKind.Audio, now.Audio, off: false);
        }

        MenuFlyout Menu(FfiTvTrackKind kind, FfiTvTrack[] tracks, bool off)
        {
            var menu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
            var group = kind.ToString();
            if (kind == FfiTvTrackKind.Subtitle)
            {
                var none = new RadioMenuFlyoutItem { Text = T.ValueOff, GroupName = group, IsChecked = off };
                none.Click += (_, _) => _ = owner.tv.Send(new FfiTvCommand.Track(kind, null));
                menu.Items.Add(none);
            }
            foreach (var track in tracks)
            {
                var item = new RadioMenuFlyoutItem { Text = TvWords.Track(track), GroupName = group, IsChecked = !off && track.Selected };
                item.Click += (_, _) => _ = owner.tv.Send(new FfiTvCommand.Track(kind, track.Id));
                menu.Items.Add(item);
            }
            return menu;
        }

        /// <summary>The playhead moved on, once a second, on this PC's clock while the TV plays.</summary>
        public override void Tick()
        {
            var now = owner.tv.Now;
            if (now is null)
                return;
            // A seek the TV never answered lets go after a while.
            if (held is not null && DateTimeOffset.UtcNow - heldAt > TimeSpan.FromSeconds(3))
                held = null;
            var duration = Math.Max(now.Duration ?? 0, 1);
            var position = Math.Min(held ?? owner.tv.Playhead() ?? now.Position, duration);
            quiet = true;
            seek.Maximum = duration;
            if (held is null)
                seek.Value = position;
            quiet = false;
            ShowTimes(position);
        }

        void ShowTimes(double position)
        {
            var now = owner.tv.Now;
            elapsed.Text = AnistreamFfiMethods.PlayClock(position);
            remaining.Text = now?.Duration is { } d ? "-" + AnistreamFfiMethods.PlayClock(Math.Max(0, d - position)) : "";
            AutomationProperties.SetItemStatus(seek, AnistreamFfiMethods.PlayClock(position));
        }
    }

    // The casting card.

    /// <summary>
    /// The TV with Chromecast built in this PC casts to: the cast's line with its dot, what plays
    /// with its playhead, back and forward, play or pause and the next episode, then subtitles,
    /// audio when there is a choice, and the TV's volume: a slider sending steps for a TV with a
    /// level, Mute for one whose volume is set on the TV. Play here, Stop casting and Done at the
    /// foot. A cast that couldn't start keeps its card with the engine's reason under the
    /// episode, and the buttons for that reason.
    /// </summary>
    sealed class CastView : View
    {
        readonly TvDialog owner;
        readonly StackPanel root = new() { Spacing = 16 };
        readonly Ellipse dot = new() { Width = 7, Height = 7, VerticalAlignment = VerticalAlignment.Center };
        readonly TextBlock status = new() { Style = Ui.Res<Style>("BodyStrongTextBlockStyle"), TextTrimming = TextTrimming.CharacterEllipsis };
        readonly Grid nowRow = new() { ColumnSpacing = 14 };
        Border cover = new() { Width = 60, Height = 90, CornerRadius = new CornerRadius(4) };
        readonly TextBlock title = Ui.Text("", "ShowTitleStyle", wrap: false);
        readonly TextBlock episode = Ui.Text("", "HazeBodyStyle", wrap: false);
        readonly TextBlock reason = Ui.Text("", "BodyTextBlockStyle");
        readonly Button skip = new() { Height = 32 };
        readonly StackPanel undoNote = new() { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        readonly TextBlock skipped = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly Button undo = new() { Height = 32 };
        readonly Grid upNext = new() { ColumnSpacing = 12, RowSpacing = 10, Padding = new Thickness(0, 12, 0, 0) };
        Border upNextCover = new() { Width = 40, Height = 60, CornerRadius = new CornerRadius(4) };
        uint? upNextCoverFor;
        readonly TextBlock upNextLine = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly TextBlock upNextEpisode = Ui.Text("", "BodyTextBlockStyle", wrap: false);
        readonly ProgressBar upNextCount = new() { Minimum = 0, Maximum = 1, Height = 2, MinHeight = 2 };
        readonly Button upNextCancel = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
        readonly Button upNextPlay = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
        (string Number, uint Seconds)? upNextFrom;
        readonly StackPanel playhead = new() { Spacing = 0 };
        readonly Slider seek = new() { Minimum = 0, Maximum = 1, StepFrequency = 1, SmallChange = 10, LargeChange = 60, IsThumbToolTipEnabled = false };
        readonly TextBlock elapsed = Ui.Text("0:00", "HazeCaptionStyle", wrap: false);
        readonly TextBlock total = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly StackPanel transport = new() { Orientation = Orientation.Horizontal, Spacing = 24, HorizontalAlignment = HorizontalAlignment.Center };
        readonly FontIcon playIcon = new() { Glyph = "", FontSize = 20 };
        readonly Button playButton = new();
        readonly StackPanel settings = new() { Spacing = 4 };
        readonly DropDownButton subtitles = new() { MinWidth = 120 };
        readonly DropDownButton audio = new() { MinWidth = 120 };
        readonly Border subtitlesRow;
        readonly Border audioRow;
        readonly TextBlock volumeNote = Ui.Text("", "HazeCaptionStyle", wrap: false);
        readonly Slider level = new() { Minimum = 0, Maximum = 100, StepFrequency = 1, SmallChange = 5, LargeChange = 10, Width = 160, IsThumbToolTipEnabled = false };
        readonly Button mute = new() { Height = 30, Padding = new Thickness(14, 0, 14, 0) };
        readonly StackPanel volume = new() { Orientation = Orientation.Horizontal, Spacing = 8 };
        readonly ProblemLine problem = new();
        readonly DispatcherQueueTimer settle;
        readonly DispatcherQueueTimer volumeRest;
        string? statusSaid;
        double? held;
        DateTimeOffset heldAt;
        double? levelDragged;
        (double To, DateTimeOffset At)? levelHeld;
        bool quiet;
        uint? coverFor;
        string? tracksDrawn;

        public CastView(TvDialog owner)
        {
            this.owner = owner;
            settle = owner.dialog.DispatcherQueue.CreateTimer();
            settle.Interval = TimeSpan.FromMilliseconds(350);
            settle.IsRepeating = false;
            settle.Tick += (_, _) =>
            {
                if (held is { } to)
                    owner.tv.SendCast(new FfiTvCommand.SeekTo(to));
            };
            // A level dragged is sent as one step once the drag rests, and held on the slider
            // until the TV says where it went.
            volumeRest = owner.dialog.DispatcherQueue.CreateTimer();
            volumeRest.Interval = TimeSpan.FromMilliseconds(200);
            volumeRest.IsRepeating = false;
            volumeRest.Tick += (_, _) =>
            {
                if (levelDragged is not { } to)
                    return;
                levelDragged = null;
                var from = owner.tv.CastCard?.Card.Now?.Volume ?? to;
                levelHeld = (to, DateTimeOffset.UtcNow);
                if (Math.Abs(to - from) >= 1)
                    owner.tv.SendCast(new FfiTvCommand.Volume(to - from));
            };

            var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            header.Children.Add(dot);
            header.Children.Add(status);
            AutomationProperties.SetAccessibilityView(dot, AccessibilityView.Raw);
            AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);
            root.Children.Add(header);

            // What plays: the cover, its title and episode, the reason a cast ended under them,
            // and the skip on offer.
            nowRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            nowRow.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            nowRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            nowRow.Children.Add(cover);
            var words = new StackPanel { Spacing = 3, VerticalAlignment = VerticalAlignment.Center };
            title.FontSize = 18;
            words.Children.Add(title);
            words.Children.Add(episode);
            reason.Margin = new Thickness(0, 6, 0, 0);
            reason.Visibility = Visibility.Collapsed;
            AutomationProperties.SetLiveSetting(reason, AutomationLiveSetting.Assertive);
            words.Children.Add(reason);
            Grid.SetColumn(words, 1);
            nowRow.Children.Add(words);
            skip.VerticalAlignment = VerticalAlignment.Center;
            skip.Click += (_, _) => owner.tv.SendCast(new FfiTvCommand.Skip());
            Grid.SetColumn(skip, 2);
            nowRow.Children.Add(skip);
            // An automatic skip, in the skip's place while it can be undone: "Skipped opening · Undo".
            undo.Content = T.ActionUndo;
            undo.Click += (_, _) => owner.tv.CastUndoSkip();
            undoNote.Children.Add(skipped);
            undoNote.Children.Add(undo);
            undoNote.Visibility = Visibility.Collapsed;
            AutomationProperties.SetLiveSetting(skipped, AutomationLiveSetting.Polite);
            Grid.SetColumn(undoNote, 2);
            nowRow.Children.Add(undoNote);
            root.Children.Add(nowRow);

            // The playhead, dragged or stepped to seek.
            AutomationProperties.SetName(seek, T.PlayerPosition);
            seek.ValueChanged += (_, e) =>
            {
                if (quiet)
                    return;
                held = e.NewValue;
                heldAt = DateTimeOffset.UtcNow;
                ShowTimes(e.NewValue);
                settle.Stop();
                settle.Start();
            };
            playhead.Children.Add(seek);
            Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(elapsed, FontNumeralAlignment.Tabular);
            Microsoft.UI.Xaml.Documents.Typography.SetNumeralAlignment(total, FontNumeralAlignment.Tabular);
            var times = new Grid { Margin = new Thickness(0, -6, 0, 0) };
            total.HorizontalAlignment = HorizontalAlignment.Right;
            times.Children.Add(elapsed);
            times.Children.Add(total);
            playhead.Children.Add(times);
            root.Children.Add(playhead);

            // Up next, near the end: the next episode, its count, Cancel and Play now.
            upNext.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            upNext.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            upNext.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            upNext.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            upNext.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            upNext.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            upNext.BorderBrush = Ui.Res<Brush>("CardStrokeColorDefaultBrush");
            upNext.BorderThickness = new Thickness(0, 1, 0, 0);
            upNext.Children.Add(upNextCover);
            var nextWords = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
            nextWords.Children.Add(upNextLine);
            nextWords.Children.Add(upNextEpisode);
            AutomationProperties.SetLiveSetting(upNextEpisode, AutomationLiveSetting.Polite);
            Grid.SetColumn(nextWords, 1);
            Grid.SetColumnSpan(nextWords, 2);
            upNext.Children.Add(nextWords);
            upNextCount.Foreground = new SolidColorBrush(Colours.Water);
            AutomationProperties.SetAccessibilityView(upNextCount, AccessibilityView.Raw);
            Grid.SetRow(upNextCount, 1);
            Grid.SetColumnSpan(upNextCount, 3);
            upNext.Children.Add(upNextCount);
            upNextCancel.Content = T.ActionCancel;
            upNextCancel.Click += (_, _) => owner.tv.CastCancelNext();
            upNextPlay.Content = T.ActionPlayNow;
            upNextPlay.Style = Ui.Res<Style>("AccentButtonStyle");
            upNextPlay.Click += (_, _) => owner.tv.CastPlayNext();
            var nextButtons = new Grid { ColumnSpacing = 8 };
            nextButtons.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            nextButtons.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            nextButtons.Children.Add(upNextCancel);
            Grid.SetColumn(upNextPlay, 1);
            nextButtons.Children.Add(upNextPlay);
            Grid.SetRow(nextButtons, 2);
            Grid.SetColumnSpan(nextButtons, 3);
            upNext.Children.Add(nextButtons);
            upNext.Visibility = Visibility.Collapsed;
            Motion.QuickOpacity(upNext);
            root.Children.Add(upNext);

            // Back and forward 10 seconds, play or pause, and the next episode.
            transport.Children.Add(Glyph(RemoteView.Ten(back: true), T.PlayerBack10, new FfiTvCommand.Seek(-10)));
            playButton.Content = playIcon;
            RemoteView.Style(playButton, T.ActionPause);
            playButton.Click += (_, _) => owner.tv.SendCast(new FfiTvCommand.TogglePause());
            transport.Children.Add(playButton);
            transport.Children.Add(Glyph(RemoteView.Ten(back: false), T.PlayerForward10, new FfiTvCommand.Seek(10)));
            transport.Children.Add(Glyph(new FontIcon { Glyph = "", FontSize = 18 }, T.PlayerNextEpisode, new FfiTvCommand.Next()));
            Motion.QuickOpacity(transport);
            root.Children.Add(transport);

            // Subtitles, audio while the TV can be given a choice, and the TV's own volume.
            subtitlesRow = Setting(T.PlayerSubtitles, null, subtitles);
            settings.Children.Add(subtitlesRow);
            audioRow = Setting(T.PlayerAudio, null, audio);
            settings.Children.Add(audioRow);
            AutomationProperties.SetName(level, T.TvVolumeLabel);
            level.ValueChanged += (_, e) =>
            {
                if (quiet)
                    return;
                levelDragged = e.NewValue;
                volumeRest.Stop();
                volumeRest.Start();
            };
            mute.Click += (_, _) => owner.tv.SendCast(new FfiTvCommand.Mute(!(owner.tv.CastCard?.Muted ?? false)));
            volume.Children.Add(level);
            volume.Children.Add(mute);
            settings.Children.Add(Setting(T.TvVolumeLabel, volumeNote, volume));
            Motion.QuickOpacity(settings);
            root.Children.Add(settings);

            root.Children.Add(problem.Root);
        }

        public override FrameworkElement Root => root;

        /// <summary>A row on its own card, as the Windows card on the board draws them.</summary>
        static Border Setting(string name, TextBlock? note, FrameworkElement control)
        {
            var row = RemoteView.SettingRow(name, note, control);
            row.MinHeight = 50;
            row.Padding = new Thickness(14, 6, 10, 6);
            return new Border
            {
                Child = row,
                Background = Ui.Res<Brush>("CardBackgroundFillColorDefaultBrush"),
                BorderBrush = Ui.Res<Brush>("CardStrokeColorDefaultBrush"),
                BorderThickness = new Thickness(1),
                CornerRadius = new CornerRadius(4),
            };
        }

        Button Glyph(FrameworkElement content, string name, FfiTvCommand command)
        {
            var button = new Button { Content = content };
            RemoteView.Style(button, name);
            button.Click += (_, _) => owner.tv.SendCast(command);
            return button;
        }

        /// <summary>Space or K pressed on the card: play or pause, while the TV plays. Not on a focused button or menu.</summary>
        public bool Key(global::Windows.System.VirtualKey key, object? focused)
        {
            if (key == global::Windows.System.VirtualKey.Space && focused is ButtonBase)
                return false;
            if (owner.tv.Stage != CastStage.Playing)
                return false;
            owner.tv.SendCast(new FfiTvCommand.TogglePause());
            return true;
        }

        public override void Update()
        {
            var tv = owner.tv;
            var card = tv.CastCard;
            var stage = tv.Stage;
            var now = tv.CastNow;
            var name = tv.CastName;
            dot.Fill = new SolidColorBrush(TvWords.CastDot(stage) switch
            {
                TvWords.Dot.Water => Colours.Water,
                TvWords.Dot.Vermilion => Ui.Vermilion,
                _ => Colours.Haze,
            });
            Status();
            var ended = stage is CastStage.Failed or CastStage.Lost;
            if (ended)
                Ended(card?.Notice);
            else
                owner.Buttons(null,
                    (T.TvCastPlayHere, PlayHere, true, false),
                    (T.TvCastStop, tv.StopCasting, true),
                    T.ActionDone, closeAccent: true);
            problem.Say(tv.Problem);

            reason.Text = ended && card?.Notice is { } notice ? notice.Text() : "";
            reason.Visibility = reason.Text.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            if (now is null)
                return;
            if (coverFor != now.Id)
            {
                coverFor = now.Id;
                nowRow.Children.Remove(cover);
                cover = Ui.Cover(owner.ctx, now.Id, 60, 90);
                cover.CornerRadius = new CornerRadius(4);
                nowRow.Children.Insert(0, cover);
            }
            title.Text = now.Title;
            episode.Text = now.EpisodeLine;
            var playing = stage == CastStage.Playing;
            var undoing = playing && card?.Undo is not null;
            skip.Visibility = playing && now.Skip is not null && !undoing ? Visibility.Visible : Visibility.Collapsed;
            undoNote.Visibility = undoing ? Visibility.Visible : Visibility.Collapsed;
            if (card?.Undo is { } undone)
                skipped.Text = TvWords.CastSkipped(undone);
            UpNext(card?.UpNext, now, ended);
            if (now.Skip is { } offered)
                skip.Content = offered.Label == "Skip ending" ? T.PlayerSkipEnding : T.PlayerSkipOpening;

            // A cast that couldn't start shows no playhead; one lost on the way keeps it where it stopped.
            playhead.Visibility = stage == CastStage.Failed ? Visibility.Collapsed : Visibility.Visible;
            transport.Visibility = ended ? Visibility.Collapsed : Visibility.Visible;
            settings.Visibility = ended ? Visibility.Collapsed : Visibility.Visible;
            transport.Opacity = playing ? 1 : 0.4;
            foreach (var child in transport.Children.OfType<Control>())
                child.IsEnabled = playing;
            playIcon.Glyph = now.Paused ? "" : "";
            var word = now.Paused ? T.ActionPlay : T.ActionPause;
            ToolTipService.SetToolTip(playButton, word);
            AutomationProperties.SetName(playButton, word);
            seek.IsEnabled = playing && now.Duration is not null;

            settings.Opacity = playing ? 1 : 0.4;
            subtitles.IsEnabled = playing && now.Subtitles.Length > 0;
            // Subtitles once the TV has said what it has, never a guess while it starts.
            subtitlesRow.Visibility = playing || now.Subtitles.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            audioRow.Visibility = now.Audio.Length > 0 ? Visibility.Visible : Visibility.Collapsed;
            audio.IsEnabled = playing;
            if (card is not null)
                Volume(card, playing);
            Tracks(now);
            Tick();
        }

        /// <summary>Up next while the engine sends it: the line with its count, the episode, and the count as a bar.</summary>
        void UpNext(FfiCastUpNext? next, FfiNowCard now, bool ended)
        {
            upNext.Visibility = next is not null && !ended ? Visibility.Visible : Visibility.Collapsed;
            if (next is null)
            {
                upNextFrom = null;
                return;
            }
            if (upNextCoverFor != now.Id)
            {
                upNextCoverFor = now.Id;
                upNext.Children.Remove(upNextCover);
                upNextCover = Ui.Cover(owner.ctx, now.Id, 40, 60);
                upNextCover.CornerRadius = new CornerRadius(4);
                upNext.Children.Insert(0, upNextCover);
            }
            upNextLine.Text = TvWords.CastUpNext(next);
            upNextEpisode.Text = TvWords.CastUpNextEpisode(next);
            // The bar fills as the count runs down, from the most seconds it showed for this episode.
            if (next.SecondsLeft is { } left)
            {
                if (upNextFrom is not { } from || from.Number != next.Number || left > from.Seconds)
                    upNextFrom = (next.Number, Math.Max(left, 1));
                upNextCount.Value = 1 - (double)left / upNextFrom!.Value.Seconds;
                upNextCount.Visibility = Visibility.Visible;
            }
            else
                upNextCount.Visibility = Visibility.Collapsed;
        }

        void Status()
        {
            var tv = owner.tv;
            status.Text = TvWords.CastStatus(tv.Stage, tv.CastCard, tv.CastName, tv.CastWaited());
            // Said as the cast changes, not each second the counter moves.
            var said = $"{tv.Stage}";
            if (said != statusSaid)
            {
                if (statusSaid is not null)
                    Announce(status);
                statusSaid = said;
            }
        }

        /// <summary>The buttons of a card that ended, by the engine's reason, with Done last.</summary>
        void Ended(Msg? notice)
        {
            var tv = owner.tv;
            var offers = TvWords.OffersFor(notice).Select(Offer).ToArray();
            owner.Buttons(null,
                offers.Length > 0 ? (offers[0].Text, offers[0].Act, true, false) : null,
                offers.Length > 1 ? (offers[1].Text, offers[1].Act, true) : null,
                T.ActionDone, closeAccent: true);
        }

        (string Text, Action Act) Offer(CastOffer offer) => offer switch
        {
            CastOffer.ChooseSource => (T.PlayerChooseSource, ChooseSource),
            CastOffer.TryAgain => (T.ActionTryAgain, owner.tv.CastAgain),
            CastOffer.ChooseAnother => (owner.Roomy ? T.TvChooseAnother : T.TvAnotherShort, owner.tv.CastElsewhere),
            _ => (T.TvCastPlayHere, PlayHere),
        };

        void PlayHere()
        {
            if (owner.tv.PlayHere() is { } play)
                owner.ctx.Play(play.Id, play.Episode);
        }

        void ChooseSource()
        {
            if (owner.tv.CastChooseSource() is { } play)
                _ = SourcesOnceClosed(owner.ctx, owner.root, play);
        }

        /// <summary>The episode's sources, once Play on TV's dialog has gone: one dialog at a time.</summary>
        static async Task SourcesOnceClosed(Ctx ctx, XamlRoot root, TvPlay play)
        {
            for (var i = 0; i < 40 && Showing is not null; i++)
                await Task.Delay(50);
            try
            {
                await PartySourceDialog.ForEpisode(ctx, root, play.Id, play.Episode).ShowAsync();
            }
            catch (System.Runtime.InteropServices.COMException)
            {
                // Another dialog came up first.
            }
        }

        /// <summary>The TV volume row: a level on a slider, or "Set on the TV" with Mute.</summary>
        void Volume(FfiCastCard card, bool playing)
        {
            var settable = card.VolumeControl == FfiVolumeControl.Level;
            level.Visibility = settable ? Visibility.Visible : Visibility.Collapsed;
            volumeNote.Visibility = settable ? Visibility.Collapsed : Visibility.Visible;
            volumeNote.Text = T.TvCastVolumeOnTv;
            mute.Content = TvWords.MuteWord(card.Muted == true);
            AutomationProperties.SetName(mute, TvWords.MuteWord(card.Muted == true));
            mute.IsEnabled = playing;
            level.IsEnabled = playing;
            if (!settable || levelDragged is not null)
                return;
            var told = card.Card.Now?.Volume ?? 0;
            if (levelHeld is { } h && (Math.Abs(h.To - told) < 1 || DateTimeOffset.UtcNow - h.At > TimeSpan.FromMilliseconds(1500)))
                levelHeld = null;
            quiet = true;
            level.Value = levelHeld?.To ?? told;
            quiet = false;
        }

        /// <summary>The subtitle and audio menus, made again only when the tracks change.</summary>
        void Tracks(FfiNowCard now)
        {
            var key = string.Join("|", now.Subtitles.Concat(now.Audio).Select(t => $"{t.Id},{t.Selected},{t.Language},{t.Title}")) + now.SubtitlesOff;
            if (key == tracksDrawn)
                return;
            tracksDrawn = key;
            subtitles.Content = TvWords.Chosen(now.Subtitles, now.SubtitlesOff);
            audio.Content = TvWords.Chosen(now.Audio, off: false);
            AutomationProperties.SetName(subtitles, T.TvSettingValue(T.PlayerSubtitles, (string)subtitles.Content));
            AutomationProperties.SetName(audio, T.TvSettingValue(T.PlayerAudio, (string)audio.Content));
            subtitles.Flyout = Menu(FfiTvTrackKind.Subtitle, now.Subtitles, now.SubtitlesOff);
            audio.Flyout = Menu(FfiTvTrackKind.Audio, now.Audio, off: false);
        }

        MenuFlyout Menu(FfiTvTrackKind kind, FfiTvTrack[] tracks, bool off)
        {
            var menu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
            var group = kind.ToString();
            if (kind == FfiTvTrackKind.Subtitle)
            {
                var none = new RadioMenuFlyoutItem { Text = T.ValueOff, GroupName = group, IsChecked = off };
                none.Click += (_, _) => owner.tv.SendCast(new FfiTvCommand.Track(kind, null));
                menu.Items.Add(none);
            }
            foreach (var track in tracks)
            {
                var item = new RadioMenuFlyoutItem { Text = TvWords.Track(track), GroupName = group, IsChecked = !off && track.Selected };
                item.Click += (_, _) => owner.tv.SendCast(new FfiTvCommand.Track(kind, track.Id));
                menu.Items.Add(item);
            }
            return menu;
        }

        /// <summary>The counter on the starting line and the playhead, moved on once a second.</summary>
        public override void Tick()
        {
            if (owner.tv.Stage == CastStage.Starting)
                Status();
            var now = owner.tv.CastNow;
            if (now is null)
                return;
            if (held is not null && DateTimeOffset.UtcNow - heldAt > TimeSpan.FromSeconds(3))
                held = null;
            var duration = Math.Max(now.Duration ?? 0, 1);
            var position = Math.Min(held ?? owner.tv.CastPlayhead() ?? now.Position, duration);
            quiet = true;
            seek.Maximum = duration;
            if (held is null)
                seek.Value = position;
            quiet = false;
            ShowTimes(position);
        }

        void ShowTimes(double position)
        {
            var now = owner.tv.CastNow;
            elapsed.Text = AnistreamFfiMethods.PlayClock(position);
            total.Text = now?.Duration is { } d ? AnistreamFfiMethods.PlayClock(d) : "";
            AutomationProperties.SetItemStatus(seek, AnistreamFfiMethods.PlayClock(position));
        }
    }
}
