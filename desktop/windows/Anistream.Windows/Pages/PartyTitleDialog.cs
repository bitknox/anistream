using Anistream.App;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows.Pages;

/// <summary>
/// "What are you watching?": the first step of Start a party on the Together page. A search field
/// and the matches under it, each with the episode that comes next; picking one hands its id and
/// episode on, and the party dialog takes it from there.
/// </summary>
sealed class PartyTitleDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly TextBox field = new() { PlaceholderText = T.PartyPickSearch };
    readonly StackPanel results = new() { Spacing = 2 };
    readonly Microsoft.UI.Dispatching.DispatcherQueueTimer typing;
    SearchQuery? asked;

    /// <summary>The title and episode picked, or null when the dialog was cancelled.</summary>
    public (uint Id, string Episode)? Picked { get; private set; }

    public PartyTitleDialog(Ctx ctx, XamlRoot root)
    {
        this.ctx = ctx;
        AutomationProperties.SetName(field, T.PartyPickSearch);
        var body = new StackPanel { Spacing = 10, MinWidth = 480 };
        body.Children.Add(field);
        body.Children.Add(results);
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = T.PartyPickTitle,
            Content = new ScrollViewer { Content = body, MaxHeight = 520, VerticalScrollBarVisibility = ScrollBarVisibility.Auto },
            CloseButtonText = T.ActionCancel,
            DefaultButton = ContentDialogButton.Close,
        };
        typing = dialog.DispatcherQueue.CreateTimer();
        typing.Interval = TimeSpan.FromMilliseconds(350);
        typing.IsRepeating = false;
        typing.Tick += (_, _) => Ask();
        field.TextChanged += (_, _) => { typing.Stop(); typing.Start(); };
    }

    public async Task<(uint Id, string Episode)?> ShowAsync()
    {
        ctx.Batch += Heard;
        try
        {
            await Dialogs.Show(dialog);
        }
        finally
        {
            ctx.Batch -= Heard;
            typing.Stop();
        }
        return Picked;
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
            var episode = (entry.NextEpisode ?? 1).ToString();
            var line = new StackPanel { Spacing = 0, HorizontalAlignment = HorizontalAlignment.Left };
            line.Children.Add(Ui.Text(entry.Title, "BodyStrongTextBlockStyle", wrap: false));
            line.Children.Add(Ui.Text(T.PartyPickNext(episode), "HazeCaptionStyle", wrap: false));
            var row = new Button
            {
                Content = line,
                HorizontalAlignment = HorizontalAlignment.Stretch,
                HorizontalContentAlignment = HorizontalAlignment.Left,
                Style = Ui.Res<Style>("SubtleButtonStyle"),
                MinHeight = 52,
            };
            AutomationProperties.SetName(row, $"{entry.Title}, {T.PartyPickNext(episode)}");
            var id = entry.Id;
            row.Click += (_, _) =>
            {
                Picked = (id, episode);
                dialog.Hide();
            };
            results.Children.Add(row);
        }
    }
}
