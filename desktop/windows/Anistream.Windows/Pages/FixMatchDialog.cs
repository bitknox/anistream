using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// "Which title is this folder?": the match picker the other way round. The search field starts
/// with the parsed title; the results are AniList titles, and one click picks one. A result whose
/// episode count does not fit the files is dimmed with the reason, and can still be picked.
/// </summary>
sealed class FixMatchDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly LocalGroup group;
    readonly TextBox field = new();
    readonly StackPanel results = new() { Spacing = 2 };
    readonly StackPanel body = new() { Spacing = 10, MinWidth = 480 };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer typing;
    SearchQuery? asked;

    public FixMatchDialog(Ctx ctx, XamlRoot root, LocalGroup group)
    {
        this.ctx = ctx;
        this.group = group;
        AutomationProperties.SetName(field, T.NavSearch);
        field.Text = group.Name;
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.LibraryFixTitle,
            Content = new ScrollViewer { Content = body, MaxHeight = 520, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
            SecondaryButtonText = T.LibraryLeaveUnmatched,
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Close,
        };
        dialog.SecondaryButtonClick += (_, _) => ctx.Send(new Command.Local(new LocalCommand.Keep(group.Id)));
        body.Children.Add(Ui.Haze($"{group.Name} · {T.LibraryFiles(group.Files)}"));
        body.Children.Add(field);
        body.Children.Add(results);
        typing = dialog.DispatcherQueue.CreateTimer();
        typing.Interval = TimeSpan.FromMilliseconds(350);
        typing.IsRepeating = false;
        typing.Tick += (_, _) => Ask();
        field.TextChanged += (_, _) => { typing.Stop(); typing.Start(); };
    }

    public async Task ShowAsync()
    {
        ctx.Batch += Heard;
        Ask();
        try
        {
            await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Batch -= Heard;
            typing.Stop();
        }
    }

    void Ask()
    {
        var text = field.Text.Trim();
        if (text.Length == 0)
            return;
        asked = Searches.Text(text);
        ctx.Send(new Command.Search(asked));
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.Content c && asked is { } q && Searches.Same(c.View, new ContentFor.Search(q))))
            Show();
    }

    void Show()
    {
        results.Children.Clear();
        if (asked is null)
            return;
        foreach (var entry in ctx.Known.ListEntries(new ContentFor.Search(asked)).Take(8))
        {
            var fits = entry.Episodes is not { } n || n == 0 || n == group.Files;
            var line = new StackPanel { Spacing = 0, Opacity = fits ? 1 : 0.6, HorizontalAlignment = HorizontalAlignment.Left };
            line.Children.Add(Ui.Text(entry.Title, "BodyStrongTextBlockStyle", wrap: false));
            line.Children.Add(Ui.Text(Wording.MetaLine(entry), "HazeCaptionStyle", wrap: false));
            if (!fits)
                line.Children.Add(Ui.Text(T.LibraryFixCountMismatch(entry.Episodes ?? 0, group.Files), "HazeCaptionStyle", wrap: false));
            var row = new Button
            {
                Content = line,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Left,
                Style = Ui.Res<Style>("SubtleButtonStyle"),
                MinHeight = 52,
            };
            AutomationProperties.SetName(row, $"{entry.Title}, {Wording.MetaLine(entry)}");
            row.Click += (_, _) =>
            {
                ctx.Send(new Command.Local(new LocalCommand.Choose(group.Id, entry.Id)));
                dialog.Hide();
            };
            results.Children.Add(row);
        }
    }
}
