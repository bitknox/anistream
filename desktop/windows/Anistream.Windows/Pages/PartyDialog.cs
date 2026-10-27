using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Pages;

/// <summary>
/// Starting a watch party, or joining one from an invite: the server, with the suggested public
/// one and who runs it, the room the engine named, and the name the room sees. Start party joins
/// and stays up, saying so, until the room answers; a server that can't be reached or refuses
/// says why here, and Try again keeps what was typed. Once in the room the dialog goes, and the
/// party lives on the player's people button.
/// </summary>
sealed class PartyDialog
{
    /// <summary>What the party is started for.</summary>
    public abstract record For
    {
        /// <summary>From a title's More: this episode plays with the room.</summary>
        public sealed record Episode(uint Id, string Number) : For;

        /// <summary>From the player's people button: what plays now plays with the room.</summary>
        public sealed record Playing : For;

        /// <summary>From an invite link: its room, on its server, with its episode.</summary>
        public sealed record Invite(PartyInvite Link) : For;
    }

    readonly Ctx ctx;
    readonly ContentDialog dialog;
    readonly For what;
    readonly TextBlock episode = Ui.Text("", "BodyStrongTextBlockStyle");
    readonly AutoSuggestBox server = new() { Header = "Server", PlaceholderText = "host:port", IsEnabled = false };
    readonly TextBlock serverHint = Ui.Text("", "HazeCaptionStyle");
    readonly Button useSuggested = new() { Content = "Use it", Padding = new Thickness(10, 2, 10, 3), MinHeight = 0, VerticalAlignment = VerticalAlignment.Top };
    readonly TextBox room = new() { Header = "Room", IsEnabled = false };
    readonly TextBox name = new() { Header = "Your name in the room", IsEnabled = false, MaxLength = 40 };
    readonly TextBlock invitePlace = Ui.Text("", "BodyTextBlockStyle");
    readonly ProgressRing ring = new() { Width = 16, Height = 16, IsActive = false, Opacity = 0 };
    // Where the ring is while joining, the error mark once it failed: the line never moves.
    readonly FontIcon mark = new() { Glyph = "\uEA39", FontSize = 16, Opacity = 0 };
    readonly TextBlock status = Ui.Text("", "CaptionTextBlockStyle");
    readonly TextBlock privacy = new()
    {
        Text = "The server sees your name, the room, and when anyone plays, pauses or seeks. Each of you plays the episode from your own sources. anistream hosts no content.",
        TextWrapping = TextWrapping.WrapWholeWords,
    };
    readonly Grid line = new() { ColumnSpacing = 8, VerticalAlignment = VerticalAlignment.Top, Opacity = 0 };
    bool joining, filled;

    /// <summary>The dialog that is up, for an invite opened while it is.</summary>
    public static PartyDialog? Showing { get; private set; }

    public PartyDialog(Ctx ctx, XamlRoot root, For what)
    {
        this.ctx = ctx;
        this.what = what;
        var body = new StackPanel { Spacing = 12, Width = 440 };
        body.Children.Add(episode);
        if (what is For.Invite)
        {
            // An invite names the room: it is shown, not asked, with whose server it is.
            body.Children.Add(invitePlace);
            body.Children.Add(serverHint);
        }
        else
        {
            body.Children.Add(server);
            var hint = new Grid { ColumnSpacing = 4, Margin = new Thickness(0, -8, 0, 0) };
            hint.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            hint.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            hint.Children.Add(serverHint);
            Grid.SetColumn(useSuggested, 1);
            hint.Children.Add(useSuggested);
            body.Children.Add(hint);
            body.Children.Add(room);
            var roomNote = Ui.Text("Anyone who knows the room's name can join it. The invite link carries it, so nobody has to read it out.", "HazeCaptionStyle");
            roomNote.Margin = new Thickness(0, -8, 0, 0);
            body.Children.Add(roomNote);
        }
        body.Children.Add(name);
        // The privacy note and the joining or error line share one place: the line stands in
        // for the note once Start is pressed, so the dialog keeps its height throughout and
        // has no empty band waiting for an error.
        var foot = new Grid();
        privacy.Style = Ui.Res<Style>("HazeCaptionStyle");
        Motion.QuickOpacity(privacy);
        foot.Children.Add(privacy);
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        line.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        line.Children.Add(ring);
        mark.Foreground = Ui.Res<Brush>("SystemFillColorCriticalBrush");
        line.Children.Add(mark);
        Grid.SetColumn(status, 1);
        status.MaxLines = 3;
        status.TextTrimming = TextTrimming.CharacterEllipsis;
        line.Children.Add(status);
        Motion.QuickOpacity(line);
        foot.Children.Add(line);
        body.Children.Add(foot);
        AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);

        dialog = new ContentDialog
        {
            XamlRoot = root,
            Title = what is For.Invite ? "Join the party?" : "Watch together",
            Content = body,
            PrimaryButtonText = what is For.Invite ? "Join party" : "Start party",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary,
            IsPrimaryButtonEnabled = false,
        };
        dialog.PrimaryButtonStyle = Ui.Res<Style>("AccentButtonStyle");
        dialog.PrimaryButtonClick += OnPrimary;
        dialog.Closing += (_, _) =>
        {
            // Cancel while the room is still answering: no party behind the viewer's back.
            if (joining)
                ctx.Send(new Command.PartyLeave());
            joining = false;
        };
        server.TextChanged += (_, args) =>
        {
            if (args.Reason == AutoSuggestionBoxTextChangeReason.UserInput)
                server.ItemsSource = Recent(server.Text);
            Update();
        };
        server.SuggestionChosen += (_, args) => server.Text = args.SelectedItem as string ?? server.Text;
        useSuggested.Click += (_, _) =>
        {
            if (ctx.Known.PartyForm is { } form)
                server.Text = form.SuggestedServer;
        };
        room.TextChanged += (_, _) => Update();
        name.TextChanged += (_, _) => Update();
        ShowEpisode();
        if (what is For.Invite invite)
            invitePlace.Text = $"Room {invite.Link.Room} on {invite.Link.Server}";
    }

    public async Task ShowAsync(Action<PartyDialog>? shown = null)
    {
        Showing = this;
        ctx.Known.PartyForm = null;
        ctx.Batch += Heard;
        ctx.Send(new Command.LoadPartyForm());
        Update();
        try
        {
            var open = dialog.ShowAsync();
            shown?.Invoke(this);
            await open;
        }
        finally
        {
            ctx.Batch -= Heard;
            if (Showing == this)
                Showing = null;
        }
    }

    public void Hide() => dialog.Hide();

#if ANISTREAM_TESTING
    /// <summary>A screenshot run's typing: the server, then Start party.</summary>
    public async void Rehearse(string? serverText, bool start)
    {
        while (!filled)
            await Task.Delay(100);
        if (serverText is not null)
            server.Text = serverText;
        Update();
        if (start && dialog.IsPrimaryButtonEnabled)
            Join();
    }
#endif

    string[] Recent(string typed) =>
        (ctx.Known.PartyForm?.RecentServers ?? [])
            .Where(s => s.Contains(typed.Trim(), StringComparison.OrdinalIgnoreCase))
            .ToArray();

    void ShowEpisode()
    {
        (uint Id, string Number)? ep = what switch
        {
            For.Episode e => (e.Id, e.Number),
            For.Invite i => (i.Link.Id, i.Link.Episode),
            _ => null,
        };
        episode.Text = ep is { } x
            ? ctx.Known.Entry(x.Id) is { } entry ? $"Episode {x.Number} of {entry.Title}" : $"Episode {x.Number}"
            : "What's playing now plays with the room.";
    }

    void Fill(PartyForm form)
    {
        if (filled)
            return;
        filled = true;
        server.Text = form.RecentServers.FirstOrDefault() ?? "";
        room.Text = form.Room;
        name.Text = form.Name;
        server.IsEnabled = room.IsEnabled = name.IsEnabled = true;
        Update();
    }

    /// <summary>Whether Start can be pressed, and the line under the server.</summary>
    void Update()
    {
        var form = ctx.Known.PartyForm;
        var (serverText, roomText) = what is For.Invite i ? (i.Link.Server, i.Link.Room) : (server.Text, room.Text);
        if (form is not null)
        {
            serverHint.Text = what is For.Invite
                ? Parties.ServerHint(serverText, form) is var h && h.StartsWith("A public", StringComparison.Ordinal) ? h : "A server the person who invited you chose. anistream doesn't run it."
                : Parties.ServerProblem(serverText) ?? Parties.ServerHint(serverText, form);
            var suggested = string.Equals(serverText.Trim(), form.SuggestedServer, StringComparison.OrdinalIgnoreCase);
            // Faded rather than removed, so the line keeps its height.
            useSuggested.Opacity = suggested ? 0 : 1;
            useSuggested.IsEnabled = !suggested && !joining;
            AutomationProperties.SetName(useSuggested, $"Use {form.SuggestedServer}");
        }
        else
            useSuggested.Opacity = 0;
        if (!joining)
        {
            if (ctx.Known.Offline)
                Say("A party needs a connection. You're offline.", error: true);
            else if (form is null)
                Say("Getting things ready…", busy: true);
            else if (status.Text is "Getting things ready…" or "A party needs a connection. You're offline.")
                Say("");
        }
        dialog.IsPrimaryButtonEnabled = form is not null && !joining && !ctx.Known.Offline
            && serverText.Trim().Length > 0 && Parties.ServerProblem(serverText) is null
            && Parties.RoomOk(roomText) && name.Text.Trim().Length > 0;
    }

    void Say(string text, bool busy = false, bool error = false)
    {
        status.Text = text;
        ring.IsActive = busy;
        ring.Opacity = busy ? 1 : 0;
        mark.Opacity = error ? 1 : 0;
        var saying = text.Length > 0;
        line.Opacity = saying ? 1 : 0;
        privacy.Opacity = saying ? 0 : 1;
        AutomationProperties.SetAccessibilityView(privacy, saying ? AccessibilityView.Raw : AccessibilityView.Content);
        ToolTipService.SetToolTip(status, error ? text : null);
        status.Foreground = error ? Ui.Res<Brush>("SystemFillColorCriticalBrush") : Ui.Res<Brush>("TextFillColorSecondaryBrush");
    }

    void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        args.Cancel = true;
        Join();
    }

    void Join()
    {
        var who = name.Text.Trim();
        var (serverText, roomText) = what is For.Invite i ? (i.Link.Server, i.Link.Room) : (server.Text.Trim(), room.Text.Trim());
        joining = true;
        server.IsEnabled = room.IsEnabled = name.IsEnabled = false;
        Say($"Joining {roomText} on {serverText}…", busy: true);
        Update();
        // The room first, and the episode once in it: a server that can't be reached leaves
        // nothing playing behind the dialog.
        ctx.Send(new Command.PartyJoin(serverText, roomText, who));
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.PartyForm f:
                    Fill(f.V1);
                    break;
                case Event.PartyJoined when joining:
                    joining = false;
                    // In the room: the episode plays with it, or says it has no source here.
                    if (what is For.Episode ep)
                        ctx.Send(new Command.PlayParty(ep.Id, ep.Number));
                    else if (what is For.Invite inv)
                        ctx.Send(new Command.PlayParty(inv.Link.Id, inv.Link.Episode));
                    dialog.Hide();
                    return;
                case Event.PartyLeft left when joining:
                    joining = false;
                    server.IsEnabled = room.IsEnabled = name.IsEnabled = true;
                    dialog.PrimaryButtonText = "Try again";
                    Say(left.Reason is { } why ? AnistreamFfiMethods.MsgEnglish(why) : "The party ended before it began.", error: true); // i18n P2: replace with the catalogue
                    break;
                case Event.Connectivity:
                    break;
                case Event.Detail or Event.Content:
                    ShowEpisode();
                    break;
            }
        }
        Update();
    }
}
