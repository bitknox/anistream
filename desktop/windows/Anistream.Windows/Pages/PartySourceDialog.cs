using System.Globalization;
using System.Text.RegularExpressions;
using Anistream.App;
using Anistream.Engine.Core;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// A party's episode that none of this device's sources found: the viewer is in the room with
/// nothing playing. Choose source lists what each source has for the episode, and the one picked
/// plays with the room; Not now stays in the room, and Leave party leaves it.
/// </summary>
sealed class PartySourceDialog
{
    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly uint id;
    readonly string episode;
    readonly StackPanel body = new() { Spacing = 12, Width = 440 };
    readonly ListView list = new() { SelectionMode = ListViewSelectionMode.Single, MaxHeight = 260 };
    readonly ProgressRing ring = new() { Width = 16, Height = 16, IsActive = true };
    readonly FontIcon mark = new() { Glyph = "\uEA39", FontSize = 16, Opacity = 0 };
    readonly TextBlock line = Ui.Text("", "HazeBodyStyle");
    SourceCandidate[]? candidates;
    bool choosing, failed, openProviders;

    readonly NoSourceWhy why;
    readonly string reason;

    public PartySourceDialog(Ctx ctx, XamlRoot root, uint id, string episode, NoSourceWhy why, string reason)
    {
        this.ctx = ctx;
        this.id = id;
        this.episode = episode;
        this.why = why;
        this.reason = reason;
        dialog = new ContentDialog
        {
            XamlRoot = root,
            Content = body,
            SecondaryButtonText = "Leave party",
            CloseButtonText = "Not now",
        };
        // The actions that fit the answer: nothing to choose before the episode is out, the
        // plugins to add when there are no sources, and the sources' list otherwise.
        switch (why)
        {
            case NoSourceWhy.NotAired:
                dialog.DefaultButton = ContentDialogButton.Close;
                break;
            case NoSourceWhy.NoSources:
                dialog.PrimaryButtonText = "Open Providers";
                openProviders = true;
                break;
            default:
                dialog.PrimaryButtonText = "Choose source";
                break;
        }
        if (why != NoSourceWhy.NotAired)
        {
            dialog.DefaultButton = ContentDialogButton.Primary;
            dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        }
        dialog.PrimaryButtonClick += OnPrimary;
        dialog.SecondaryButtonClick += (_, _) => ctx.Send(new Command.PartyLeave());
        list.SelectionChanged += (_, _) => dialog.IsPrimaryButtonEnabled = list.SelectedIndex >= 0;
        AutomationProperties.SetName(list, "Sources for this episode");
        AutomationProperties.SetLiveSetting(line, AutomationLiveSetting.Polite);
        First();
    }

    /// <summary>The dialog that is up, for a screenshot run to press Choose source in.</summary>
    public static PartySourceDialog? Showing { get; private set; }

    public async Task ShowAsync()
    {
        Showing = this;
        ctx.Batch += Heard;
        try
        {
            await dialog.ShowAsync();
        }
        finally
        {
            ctx.Batch -= Heard;
            if (Showing == this)
                Showing = null;
        }
        if (openProviders)
            ctx.Go("providers");
    }

    string What => ctx.Known.Entry(id) is { } entry ? $"Episode {episode} of {entry.Title}" : $"Episode {episode}";

    void First()
    {
        body.Children.Clear();
        var place = ctx.Known.Party is { } party ? $"You're in {party.Room} on {party.Server}, with nothing playing." : "You're in the room, with nothing playing.";
        switch (why)
        {
            case NoSourceWhy.NotAired:
                // The schedule's own words lead: "Episode 6 airs Tuesday at 13:05", or the
                // episode not out yet with when the next one is.
                var said = Words.Notice(Regex.Replace(reason, @" (\d{1,2}:\d{2})\b", " at $1"));
                var parts = said.Split("; ", 2);
                dialog.Title = parts[0];
                body.Children.Add(Ui.Text(place, "BodyTextBlockStyle"));
                body.Children.Add(Ui.Haze(parts.Length > 1
                    ? $"{Words.Notice(parts[1])}. Your sources won't have episode {episode} before it's out."
                    : $"Your sources won't have it before then. It plays with the room once one of them does."));
                break;
            case NoSourceWhy.NoSources:
                dialog.Title = "You have no sources yet";
                body.Children.Add(Ui.Text(place, "BodyTextBlockStyle"));
                body.Children.Add(Ui.Haze($"Sources come from plugins, which Providers adds. {What} plays with the room once one of them has it."));
                break;
            default:
                dialog.Title = "None of your sources has this episode";
                body.Children.Add(Ui.Text(place, "BodyTextBlockStyle"));
                body.Children.Add(Ui.Haze($"{Words.Notice(reason).TrimEnd('.')}. {What} plays with the room once you choose a source that has it."));
                break;
        }
    }

#if ANISTREAM_TESTING
    public void RehearseChoose() => Choose();
#endif

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (openProviders)
            return;
        if (!choosing || failed)
        {
            args.Cancel = true;
            Choose();
            return;
        }
        if (list.SelectedIndex < 0 || candidates is null)
        {
            args.Cancel = true;
            return;
        }
        var pick = candidates[list.SelectedIndex];
        // In the room already: this play plays with it.
        ctx.Send(new Command.PlaySource(id, episode, pick.ProviderId, pick.Id));
    }

    /// <summary>Step two: what each source lists for the episode, loading in place.</summary>
    void Choose()
    {
        dialog.Title = $"Choose a source for episode {episode}";
        dialog.PrimaryButtonText = "Play with the party";
        dialog.IsPrimaryButtonEnabled = false;
        if (!choosing)
        {
            body.Children.Clear();
            // The ring while looking, the error mark in its place if that failed: the line stays put.
            var waiting = new Grid { ColumnSpacing = 8, MinHeight = 20 };
            waiting.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            waiting.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            waiting.Children.Add(ring);
            waiting.Children.Add(mark);
            Grid.SetColumn(line, 1);
            waiting.Children.Add(line);
            body.Children.Add(waiting);
            body.Children.Add(list);
        }
        choosing = true;
        failed = false;
        candidates = null;
        list.Items.Clear();
        list.Opacity = 0;
        ring.IsActive = true;
        ring.Opacity = 1;
        mark.Opacity = 0;
        line.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        line.Text = "Looking through your sources…";
        ctx.Send(new Command.LoadSources(id, episode));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.Sources s when choosing && s.Id == id && s.Episode == episode:
                    Listed(s.Candidates);
                    break;
                // The engine says why it found nothing as an alert, not as an empty list.
                case Event.Toast t when choosing && candidates is null && !failed && t.V1.Kind == NoticeKind.Alert:
                    Failed(t.V1.Text);
                    break;
                case Event.PartyLeft:
                    dialog.Hide();
                    return;
                case Event.PartyJoined or Event.PartyState when !choosing:
                    First();
                    break;
            }
        }
    }

    void Failed(string why)
    {
        failed = true;
        ring.IsActive = false;
        ring.Opacity = 0;
        mark.Glyph = "\uEA39";
        mark.Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush");
        mark.Opacity = 1;
        line.Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush");
        line.Text = Words.Notice(why);
        dialog.PrimaryButtonText = "Try again";
        dialog.IsPrimaryButtonEnabled = true;
    }

    void Listed(SourceCandidate[] found)
    {
        candidates = found;
        ring.IsActive = false;
        ring.Opacity = 0;
        list.Items.Clear();
        // An answer, not a failure: the mark in the ring's place says so in the line's colour.
        mark.Glyph = "\uE946";
        mark.Foreground = Ui.Res<Brush>("TextFillColorSecondaryBrush");
        mark.Opacity = 1;
        if (found.Length == 0)
        {
            line.Text = $"No source lists episode {episode}. Sources come from plugins, which Providers adds.";
            dialog.PrimaryButtonText = "Open Providers";
            dialog.IsPrimaryButtonEnabled = true;
            openProviders = true;
            return;
        }
        line.Text = found.Length == 1 ? "1 source has it" : $"{found.Length.ToString(CultureInfo.InvariantCulture)} sources have it";
        foreach (var c in found)
        {
            var row = new StackPanel { Padding = new Thickness(0, 4, 0, 4) };
            row.Children.Add(Ui.Text(c.Title, wrap: false));
            row.Children.Add(Ui.Text(Detail(c), "HazeCaptionStyle", wrap: false));
            list.Items.Add(new ListViewItem { Content = row });
        }
        Motion.Show(list);
    }

    static string Detail(SourceCandidate c)
    {
        var parts = new List<string> { c.ProviderId };
        if (c.Quality is { } q)
            parts.Add($"{q}p");
        if (c.Size is { Length: > 0 } size)
            parts.Add(size);
        if (c.Seeders is { } seeders)
            parts.Add($"{seeders} seeders");
        return string.Join(" · ", parts);
    }
}
