using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

namespace Anistream.Windows.Pages;

/// <summary>
/// The source picker, from the title page's More menu: Automatic first, then each source in the
/// order they are tried with how much of the title it lists, or why it lists none, and under a
/// source with several releases a Releases heading and the releases. A check marks the chosen
/// row. Picking one sends the choice and closes the dialog; the episode list follows from the
/// engine's events. Arrow keys move, Enter picks, Esc closes. Until the engine answers, Automatic
/// stands alone and, after 600 ms, two plates stand where the sources will be.
/// </summary>
sealed class SourcePickerDialog
{
    static readonly TimeSpan Silence = TimeSpan.FromMilliseconds(600);

    readonly Ctx ctx;
    readonly uint id;
    readonly ContentDialog dialog;
    readonly ListView list = new()
    {
        SelectionMode = ListViewSelectionMode.None,
        IsItemClickEnabled = true,
        MaxHeight = 440,
        MinWidth = 440,
    };
    readonly StackPanel plates = new() { Spacing = 12, Margin = new Thickness(12, 12, 12, 0), Visibility = Visibility.Collapsed };
    readonly TextBlock empty = Ui.Text(T.SourcesEmpty, "HazeBodyStyle");
    readonly DispatcherQueueTimer silence;
    bool picked;
    bool fixMatch;

    SourcePickerDialog(Ctx ctx, uint id, string show, XamlRoot root)
    {
        this.ctx = ctx;
        this.id = id;

        AutomationProperties.SetName(list, T.SourcePickerTitle);
        list.ItemClick += (_, e) => Pick((e.ClickedItem as FrameworkElement)?.Tag as PickerRow);
        // Enter and Space pick the row that has focus, as a click does.
        list.PreviewKeyDown += (_, e) =>
        {
            if (e.Key is global::Windows.System.VirtualKey.Enter or global::Windows.System.VirtualKey.Space
                && dialog.XamlRoot is { } root && FocusManager.GetFocusedElement(root) is FrameworkElement { Tag: PickerRow row })
            {
                e.Handled = true;
                Pick(row);
            }
        };

        AutomationProperties.SetAccessibilityView(plates, AccessibilityView.Raw);
        plates.Children.Add(States.Rows(2, 0, 0));
        empty.Margin = new Thickness(12, 12, 12, 0);
        empty.Visibility = Visibility.Collapsed;

        var body = new StackPanel { Spacing = 8 };
        var name = Ui.Text(show, "ShowTitleStyle", wrap: false);
        name.Margin = new Thickness(0, 0, 0, 4);
        body.Children.Add(name);
        body.Children.Add(list);
        body.Children.Add(plates);
        body.Children.Add(empty);
        // The last row: Wrong title? Fix this match. It closes the picker and opens the match picker in its place.
        if (MatchBook.CanFix(ctx.Known.Providers))
        {
            var fix = new Button { Content = T.MatchSourcePickerRow, HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Left, MinHeight = 44, Margin = new Thickness(0, 8, 0, 0) };
            fix.Click += (_, _) =>
            {
                fixMatch = true;
                dialog.Hide();
            };
            body.Children.Add(fix);
        }

        var queue = DispatcherQueue.GetForCurrentThread();
        silence = queue.CreateTimer();
        silence.Interval = Silence;
        silence.IsRepeating = false;
        silence.Tick += (_, _) =>
        {
            if (ctx.Known.SourceChoices.Loading(id))
                Motion.Show(plates);
        };

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.SourcePickerTitle,
            Content = body,
            CloseButtonText = T.ActionDone,
            DefaultButton = ContentDialogButton.None,
        };
        dialog.Opened += (_, _) => FocusRow(null);
    }

    /// <summary>Opens the picker for title <paramref name="id"/> and asks the engine for what each source has of it.</summary>
    public static async Task Open(Ctx ctx, uint id, string show, XamlRoot root)
    {
        var picker = new SourcePickerDialog(ctx, id, show, root);
        ctx.Batch += picker.Heard;
        ctx.Known.SourceChoices.Asking(id);
        ctx.Send(new Command.LoadSourceOptions(id));
        picker.Fill();
        if (ctx.Known.SourceChoices.Loading(id))
            picker.silence.Start();
        try
        {
            await Dialogs.Show(picker.dialog);
        }
        finally
        {
            ctx.Batch -= picker.Heard;
            picker.silence.Stop();
            ctx.Known.SourceChoices.GiveUp(id);
        }
        if (picker.fixMatch)
            await MatchPickerDialog.Open(ctx, id, show, root, fromMenu: !ctx.Known.Matches.Asks(id));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.SourceOptions s && s.Id == id))
            Fill();
    }

    /// <summary>The rows as the latest listing has them. A row that held focus keeps it.</summary>
    void Fill()
    {
        var choices = ctx.Known.SourceChoices;
        var listed = choices.For(id);
        var held = dialog.XamlRoot is { } root ? FocusManager.GetFocusedElement(root) : null;
        var keep = held is FrameworkElement { Tag: PickerRow focused } && list.Items.Contains(held) ? focused.Key : null;

        list.Items.Clear();
        var rows = SourcePicker.Rows(listed);
        foreach (var row in rows)
            list.Items.Add(Make(row));
        if (listed is not null)
        {
            silence.Stop();
            plates.Visibility = Visibility.Collapsed;
        }
        empty.Visibility = listed is { Length: 0 } ? Visibility.Visible : Visibility.Collapsed;

        // Once the sources arrive the check may have moved; focus goes to where the person was, or to the chosen row.
        if (dialog.IsLoaded)
            FocusRow(keep);
    }

    /// <summary>Focuses the row with <paramref name="key"/>, or the chosen one.</summary>
    void FocusRow(string? key)
    {
        list.UpdateLayout();
        var items = list.Items.OfType<ListViewItem>().Where(i => i.IsEnabled).ToList();
        var target = items.FirstOrDefault(i => key is not null && (i.Tag as PickerRow)?.Key == key)
            ?? items.FirstOrDefault(i => i.Tag is PickerRow.Automatic { Chosen: true } or PickerRow.Source { Chosen: true } or PickerRow.Release { Chosen: true })
            ?? items.FirstOrDefault();
        target?.Focus(FocusState.Keyboard);
    }

    ListViewItem Make(PickerRow row)
    {
        var (title, detail) = SourcePicker.Words(row);
        var item = new ListViewItem { Tag = row, HorizontalContentAlignment = HorizontalAlignment.Stretch, MinHeight = 48 };
        AutomationProperties.SetName(item, SourcePicker.Spoken(row));

        if (row is PickerRow.ReleasesHeading)
        {
            // A heading, not a choice: it takes no focus and no click.
            var heading = Ui.Text(title, "BodyStrongTextBlockStyle", wrap: false);
            AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level2);
            heading.Margin = new Thickness(16, 8, 0, 0);
            item.Content = heading;
            item.IsEnabled = false;
            item.MinHeight = 36;
            return item;
        }

        var chosen = row switch
        {
            PickerRow.Automatic a => a.Chosen,
            PickerRow.Source s => s.Chosen,
            PickerRow.Release r => r.Chosen,
            _ => false,
        };
        var dimmed = row is PickerRow.Source { Dimmed: true };
        var grid = new Grid { ColumnSpacing = 12, Margin = new Thickness(row is PickerRow.Release ? 16 : 0, 0, 0, 0) };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(20) });

        var words = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center, Padding = new Thickness(0, 6, 0, 6) };
        var name = Ui.Text(title, dimmed ? "HazeBodyStyle" : "BodyTextBlockStyle", wrap: row is PickerRow.Release);
        name.MaxLines = 2;
        words.Children.Add(name);
        if (!string.IsNullOrEmpty(detail))
        {
            var line = Ui.Text(detail, "HazeCaptionStyle");
            line.MaxLines = 2;
            line.TextTrimming = TextTrimming.CharacterEllipsis;
            words.Children.Add(line);
        }
        grid.Children.Add(words);

        // The check keeps its place whether shown or not, so the text does not move.
        var check = new FontIcon { Glyph = "", FontSize = 14, VerticalAlignment = VerticalAlignment.Center, Opacity = chosen ? 1 : 0 };
        AutomationProperties.SetAccessibilityView(check, AccessibilityView.Raw);
        Grid.SetColumn(check, 1);
        grid.Children.Add(check);

        item.Content = grid;
        if (row is PickerRow.Source { Selectable: false })
            item.IsEnabled = false;
        return item;
    }

    /// <summary>Sends the choice and closes the dialog. The engine relists the episodes by itself.</summary>
    void Pick(PickerRow? row)
    {
        if (picked)
            return;
        var choices = ctx.Known.SourceChoices;
        switch (row)
        {
            case PickerRow.Automatic { Chosen: false }:
                choices.Chose(id, null);
                ctx.Send(new Command.SetProviderPreference(id, null));
                break;
            case PickerRow.Source { Chosen: false, Selectable: true } s:
                choices.Chose(id, s.Option.ProviderId);
                ctx.Send(new Command.SetProviderPreference(id, s.Option.ProviderId));
                break;
            case PickerRow.Release { Chosen: false } r:
                choices.Chose(id, r.Option.ProviderId, r.Candidate.Id);
                ctx.Send(new Command.PinRelease(id, r.Option.ProviderId, r.Candidate.Id, r.Candidate.Title));
                break;
            case PickerRow.Source { Selectable: false } or PickerRow.ReleasesHeading or null:
                return;
        }
        picked = true;
        dialog.Hide();
    }
}
