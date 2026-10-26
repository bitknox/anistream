using Anistream.App;
using Anistream.Engine.Ffi;
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
    readonly DispatcherQueueTimer clock;
    Action? primary, secondary;
    View? shown;
    bool closing;

    public static TvDialog? Showing { get; private set; }

    TvDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        tv = ctx.Tv;
        dialog = new ContentDialog { XamlRoot = root, Content = body };
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
            it.tv.Changed -= it.Update;
            it.shown?.Leave();
            it.tv.StopBrowsing();
            Showing = null;
            it.tv.SheetClosed();
        }
    }

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
            : shown as PickerView ?? new PickerView(this);
        if (next != shown)
        {
            shown?.Leave();
            shown = next;
            body.Children.Clear();
            body.Children.Add(next.Root);
            Motion.FadeIn(next.Root);
        }
        next.Update();
    }

    /// <summary>The dialog's title and its three buttons, for the view up.</summary>
    void Buttons(string title, (string Text, Action Act, bool Enabled, bool Accent)? first, (string Text, Action Act, bool Enabled)? second, string close)
    {
        dialog.Title = title;
        primary = first?.Act;
        secondary = second?.Act;
        dialog.PrimaryButtonText = first?.Text ?? "";
        dialog.IsPrimaryButtonEnabled = first?.Enabled ?? false;
        dialog.PrimaryButtonStyle = first is { Accent: true } ? Ui.Res<Style>("AccentButtonStyle") : Ui.Res<Style>("DefaultButtonStyle");
        dialog.DefaultButton = first is { Accent: true } ? ContentDialogButton.Primary : ContentDialogButton.None;
        dialog.SecondaryButtonText = second?.Text ?? "";
        dialog.IsSecondaryButtonEnabled = second?.Enabled ?? false;
        dialog.CloseButtonText = close;
    }

    static TextBlock Problem(string? text)
    {
        var block = Ui.Text(text ?? "", "CaptionTextBlockStyle");
        block.Foreground = new SolidColorBrush(Ui.Vermilion);
        block.Visibility = text is null ? Visibility.Collapsed : Visibility.Visible;
        AutomationProperties.SetLiveSetting(block, AutomationLiveSetting.Assertive);
        return block;
    }

    static void Say(TextBlock block, string? text)
    {
        block.Text = text ?? "";
        block.Visibility = text is null ? Visibility.Collapsed : Visibility.Visible;
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
    /// on throughout, and a TV found replaces the row.
    /// </summary>
    sealed class PickerView : View
    {
        readonly TvDialog owner;
        readonly StackPanel root = new() { Spacing = 0 };
        readonly TextBlock line = Ui.Haze("");
        readonly StackPanel rows = new();
        readonly StackPanel typed = new() { Spacing = 6, Margin = new Thickness(0, 14, 0, 0), Visibility = Visibility.Collapsed };
        readonly TextBox address = new() { PlaceholderText = "192.168.1.20:47811", Header = "Address" };
        readonly DateTimeOffset opened = DateTimeOffset.UtcNow;
        string? drawn;
        bool typing;

        public PickerView(TvDialog owner)
        {
            this.owner = owner;
            root.Children.Add(line);
            var card = Rows(rows);
            card.Margin = new Thickness(0, 16, 0, 0);
            root.Children.Add(card);
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
                typing ? ("Connect", Connect, address.Text.Trim().Length > 0, true) : null,
                typing ? null : ("Type an address…", Type, true),
                "Done");
            // The rows again only when they read differently, so focus stays where it was.
            var now = string.Join("|", tv.Rows.Select(r => $"{r.Key},{r.Name},{r.Detail},{r.Kind},{r.Fingerprint == tv.Connected}"));
            if (now == drawn && tv.Rows.Length > 0)
                return;
            drawn = now;
            rows.Children.Clear();
            if (tv.Rows.Length == 0)
            {
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
            if (owner.tv.Rows.Length == 0)
            {
                var stage = Stage();
                if (rows.Children is [Grid { Tag: string was }] && was != stage)
                {
                    rows.Children.Clear();
                    rows.Children.Add(Searching());
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
                    grid.Children.Add(none);
                    break;
                }
            }
            if (stage != "quiet")
            {
                Motion.FadeIn(grid);
                AutomationProperties.SetLiveSetting(grid, AutomationLiveSetting.Polite);
                AutomationProperties.SetName(grid, stage == "looking" ? TvWords.Looking : $"{TvWords.NoneFound}. {TvWords.NoneLine(owner.tv.Request)}");
            }
            return grid;
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
                FfiTvRowKind.New => new TextBlock { Text = "Pair", Style = Ui.Res<Style>("BodyStrongTextBlockStyle") },
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
            AutomationProperties.SetName(button, $"{row.Name}, {row.Detail}" + (connected ? ", connected" : ""));
            if (row.Kind == FfiTvRowKind.New)
                AutomationProperties.SetHelpText(button, "Pairs with this TV");
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
        readonly TextBlock problem = Problem(null);
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
            AutomationProperties.SetName(typed, "Pairing code");
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
            code.Tapped += (_, _) => typed.Focus(FocusState.Pointer);
            root.Children.Add(code);
            waiting.Children.Add(new ProgressRing { IsActive = true, Width = 16, Height = 16 });
            waiting.Children.Add(Ui.Haze("Waiting for the TV", wrap: false));
            root.Children.Add(waiting);
            problem.Margin = new Thickness(0, 12, 0, 0);
            root.Children.Add(problem);
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
                var next = focused && i == Math.Min(text.Length, 5);
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
            ((TextBlock)waiting.Children[1]).Text = step.TvName.Length == 0 ? "Waiting for the TV" : $"Waiting for {step.TvName}";
            typed.IsReadOnly = step.Working;
            working.Visibility = typing && step.Working ? Visibility.Visible : Visibility.Collapsed;
            if (step.Problem != lastProblem)
            {
                lastProblem = step.Problem;
                Say(problem, step.Problem);
                if (step.Problem is not null)
                {
                    // Another try starts from empty boxes.
                    typed.Text = "";
                    if (typing)
                        typed.Focus(FocusState.Programmatic);
                }
                // The spinner stopped: the problem in its place.
                if (step.Problem is not null && !typing)
                    waiting.Visibility = Visibility.Collapsed;
            }
            else if (step.Problem is not null && !typing)
                waiting.Visibility = Visibility.Collapsed;
            owner.Buttons(TvWords.PairingTitle(step),
                typing ? ("Pair", Submit, TvWords.SixDigits(typed.Text) is not null && !step.Working, true) : null,
                typing ? ("Show new code", Again, !step.Working) : null,
                "Back");
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
        readonly Slider seek = new() { Minimum = 0, Maximum = 1, StepFrequency = 1, IsThumbToolTipEnabled = false, Margin = new Thickness(0, 10, 0, 0) };
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
        readonly TextBox search = new() { PlaceholderText = "Search on the TV", Margin = new Thickness(0, 14, 0, 0) };
        readonly TextBlock problem = Problem(null);
        readonly DispatcherQueueTimer settle;
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
            AutomationProperties.SetName(seek, "Position");
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
            var times = new Grid { Margin = new Thickness(0, -6, 0, 0) };
            remaining.HorizontalAlignment = HorizontalAlignment.Right;
            times.Children.Add(elapsed);
            times.Children.Add(remaining);
            playing.Children.Add(times);

            // Back and forward 10 seconds, play or pause, and the next episode, as the player's bar draws them.
            transport.Children.Add(Glyph(Ten(back: true), "Back 10 seconds", new FfiTvCommand.Seek(-10)));
            playButton.Content = playIcon;
            Style(playButton, "Pause");
            playButton.Click += (_, _) => _ = owner.tv.Send(new FfiTvCommand.TogglePause());
            transport.Children.Add(playButton);
            transport.Children.Add(Glyph(Ten(back: false), "Forward 10 seconds", new FfiTvCommand.Seek(10)));
            transport.Children.Add(Glyph(new FontIcon { Glyph = "", FontSize = 18 }, "Next episode", new FfiTvCommand.Next()));
            playing.Children.Add(transport);

            // Subtitles, audio and the TV's own volume.
            settings.Children.Add(SettingRow("Subtitles", null, subtitles));
            settings.Children.Add(Hairline());
            settings.Children.Add(SettingRow("Audio", null, audio));
            settings.Children.Add(Hairline());
            volume.Children.Add(Glyph(new FontIcon { Glyph = "", FontSize = 14 }, "Volume down", new FfiTvCommand.Volume(-5), small: true));
            volume.Children.Add(Glyph(new FontIcon { Glyph = "", FontSize = 14 }, "Volume up", new FfiTvCommand.Volume(5), small: true));
            settings.Children.Add(SettingRow("TV volume", volumeNote, volume));
            AutomationProperties.SetName(subtitles, "Subtitles");
            AutomationProperties.SetName(audio, "Audio");
            Motion.QuickOpacity(transport);
            rowsCard = Rows(settings);
            Motion.QuickOpacity(rowsCard);
            rowsCard.Margin = new Thickness(0, 12, 0, 0);
            playing.Children.Add(rowsCard);
            root.Children.Add(playing);

            nothing.Margin = new Thickness(0, 14, 0, 0);
            root.Children.Add(nothing);

            // A search typed here lands in the TV's search, as each letter is typed.
            AutomationProperties.SetName(search, "Search on the TV");
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
            problem.Margin = new Thickness(0, 10, 0, 0);
            root.Children.Add(problem);
        }

        public override FrameworkElement Root => root;

        static FrameworkElement Ten(bool back)
        {
            var icon = new FontIcon { Glyph = "", FontSize = 20, RenderTransformOrigin = new global::Windows.Foundation.Point(0.5, 0.5) };
            if (back)
                icon.RenderTransform = new ScaleTransform { ScaleX = -1 };
            var grid = new Grid();
            grid.Children.Add(icon);
            grid.Children.Add(new TextBlock { Text = "10", Style = Ui.Res<Style>("SkipDigitsStyle") });
            return grid;
        }

        static void Style(Button button, string name)
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

        static Grid SettingRow(string name, TextBlock? note, FrameworkElement control)
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
            owner.Buttons(card?.TvName ?? "TV",
                TvWords.Lost(card) ? ("Pair again", () => _ = tv.PairAgain(), true, false) : ("Choose another TV…", tv.OpenPicker, true, false),
                ("Disconnect", () =>
                {
                    tv.Disconnect();
                    owner.Close();
                }, true),
                "Done");

            playing.Visibility = now is null ? Visibility.Collapsed : Visibility.Visible;
            // What to do from here is said only while the TV can hear it; otherwise the status
            // line says what the link is doing.
            nothing.Visibility = now is null && live ? Visibility.Visible : Visibility.Collapsed;
            search.IsEnabled = live;
            Say(problem, tv.Problem);
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
                skip.Content = Words.Sentence(offered.Label);
            skip.IsEnabled = live;

            var controls = live && !now.Loading;
            transport.Opacity = controls ? 1 : 0.4;
            foreach (var child in transport.Children.OfType<Control>())
                child.IsEnabled = controls;
            playIcon.Glyph = now.Paused ? "" : "";
            var name = now.Paused ? "Play" : "Pause";
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
            subtitles.Flyout = Menu(FfiTvTrackKind.Subtitle, now.Subtitles, now.SubtitlesOff);
            audio.Flyout = Menu(FfiTvTrackKind.Audio, now.Audio, off: false);
        }

        MenuFlyout Menu(FfiTvTrackKind kind, FfiTvTrack[] tracks, bool off)
        {
            var menu = new MenuFlyout { Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
            var group = kind.ToString();
            if (kind == FfiTvTrackKind.Subtitle)
            {
                var none = new RadioMenuFlyoutItem { Text = "Off", GroupName = group, IsChecked = off };
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
}
