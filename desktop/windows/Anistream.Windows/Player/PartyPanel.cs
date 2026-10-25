using Anistream.App;
using Anistream.Engine.Model;
using Anistream.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;

namespace Anistream.Windows.Player;

/// <summary>
/// The party, from the player's people button: how the connection stands, who is in the room
/// and who is ready, this member's own ready switch, the invite link to copy, the server and
/// room as text for someone joining with Syncplay itself, and Leave party. It follows the
/// engine while it is open, and each part keeps its place as the room changes.
/// </summary>
sealed class PartyPanel
{
    readonly Ctx ctx;
    public Flyout Flyout { get; }
    readonly TextBlock connection = Ui.Text("", "HazeCaptionStyle");
    readonly ProgressRing reconnecting = new() { Width = 12, Height = 12, IsActive = false, Opacity = 0 };
    readonly StackPanel members = new() { Spacing = 2 };
    readonly TextBlock readyLine = Ui.Text("", "HazeCaptionStyle");
    readonly ToggleSwitch ready = new() { OnContent = "I'm ready", OffContent = "I'm ready" };
    readonly Button copy = new() { HorizontalAlignment = HorizontalAlignment.Stretch };
    readonly TextBlock copyText = new() { Text = "Copy invite link" };
    readonly TextBlock inviteNote = Ui.Text("", "HazeCaptionStyle");
    readonly TextBlock place = new() { IsTextSelectionEnabled = true, TextWrapping = TextWrapping.Wrap };
    readonly TextBlock motd = Ui.Text("", "HazeCaptionStyle");
    readonly Button leave = new() { Content = "Leave party", HorizontalAlignment = HorizontalAlignment.Stretch };
    bool fromEngine, leaving;
    int copied;

    public PartyPanel(Ctx ctx)
    {
        this.ctx = ctx;
        var body = new StackPanel { Spacing = 10, Width = 320 };

        var head = new StackPanel { Spacing = 2 };
        head.Children.Add(Ui.Text("Watch party", "BodyStrongTextBlockStyle"));
        // The ring follows the line, so the line starts where the title does.
        var status = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
        status.Children.Add(connection);
        reconnecting.VerticalAlignment = VerticalAlignment.Center;
        status.Children.Add(reconnecting);
        head.Children.Add(status);
        body.Children.Add(head);

        body.Children.Add(members);
        body.Children.Add(readyLine);
        AutomationProperties.SetLiveSetting(readyLine, AutomationLiveSetting.Polite);
        AutomationProperties.SetName(ready, "I'm ready");
        ready.Toggled += (_, _) =>
        {
            if (!fromEngine)
                ctx.Send(new Command.PartyReady(ready.IsOn));
        };
        body.Children.Add(ready);

        body.Children.Add(new Border { Height = 1, Background = Ui.Res<Brush>("DividerStrokeColorDefaultBrush"), Margin = new Thickness(0, 2, 0, 2) });

        var copyContent = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        copyContent.Children.Add(new FontIcon { Glyph = "", FontSize = 14 });
        copyContent.Children.Add(copyText);
        copy.Content = copyContent;
        copy.Click += (_, _) => Copy();
        body.Children.Add(copy);
        body.Children.Add(inviteNote);

        var plain = new StackPanel { Spacing = 2 };
        plain.Children.Add(Ui.Text("For Syncplay's own app", "CaptionTextBlockStyle"));
        place.Style = Ui.Res<Style>("HazeCaptionStyle");
        plain.Children.Add(place);
        body.Children.Add(plain);
        body.Children.Add(motd);

        leave.Click += (_, _) =>
        {
            leaving = true;
            leave.Content = "Leaving…";
            leave.IsEnabled = false;
            ctx.Send(new Command.PartyLeave());
        };
        body.Children.Add(leave);

        Flyout = new Flyout { Content = body, Placement = FlyoutPlacementMode.BottomEdgeAlignedRight };
        Flyout.Opening += (_, _) =>
        {
            leaving = false;
            leave.Content = "Leave party";
            Show();
            ctx.Batch += Heard;
        };
        Flyout.Closed += (_, _) => ctx.Batch -= Heard;
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.PartyLeft))
        {
            Flyout.Hide();
            return;
        }
        if (batch.Any(e => e is Event.PartyState or Event.PartyJoined or Event.PartyMembers))
            Show();
    }

    void Show()
    {
        if (ctx.Known.Party is not { } party)
            return;
        var lost = party.Connection is PartyConnection.Reconnecting;
        connection.Text = Parties.ConnectionLine(party);
        reconnecting.IsActive = lost;
        reconnecting.Opacity = lost ? 1 : 0;

        members.Children.Clear();
        if (ctx.Known.PartyMembers is { } list)
        {
            foreach (var member in list)
                members.Children.Add(Row(member));
            readyLine.Text = Parties.ReadyLine(list);
        }
        else
        {
            // Before the room lists its members: one row's height, waiting.
            var wait = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, MinHeight = 36 };
            wait.Children.Add(new ProgressRing { Width = 16, Height = 16, IsActive = true });
            wait.Children.Add(Ui.Text("Waiting for the room's members", "HazeBodyStyle"));
            members.Children.Add(wait);
            readyLine.Text = "";
        }

        fromEngine = true;
        ready.IsOn = party.Ready;
        fromEngine = false;
        ready.IsEnabled = !lost && !leaving;

        copy.IsEnabled = party.Invite is not null && !leaving;
        inviteNote.Text = party.Invite is null
            ? "The invite link comes once an episode plays."
            : "It names the room and the episode. Each friend plays it from their own sources.";
        place.Text = $"Server {party.Server}\nRoom {party.Room}";
        motd.Text = party.Motd ?? "";
        motd.Visibility = party.Motd is { Length: > 0 } ? Visibility.Visible : Visibility.Collapsed;
        leave.IsEnabled = !leaving;
    }

    static FrameworkElement Row(PartyMember member)
    {
        var row = new Grid { ColumnSpacing = 10, MinHeight = 36, Padding = new Thickness(0, 2, 0, 2) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(20) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var isReady = member.Ready == true;
        var mark = new FontIcon
        {
            // A filled check for ready, an empty circle for not yet.
            Glyph = isReady ? "" : "",
            FontSize = 14,
            Opacity = isReady ? 1 : 0.6,
            VerticalAlignment = VerticalAlignment.Center,
        };
        row.Children.Add(mark);
        var names = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        names.Children.Add(Ui.Text(Parties.MemberName(member), wrap: false));
        if (Parties.MemberFile(member) is { } file)
            names.Children.Add(Ui.Text(file, "HazeCaptionStyle", wrap: false));
        Grid.SetColumn(names, 1);
        row.Children.Add(names);
        var state = Ui.Text(Parties.MemberState(member), isReady ? "CaptionTextBlockStyle" : "HazeCaptionStyle", wrap: false);
        state.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(state, 2);
        row.Children.Add(state);
        AutomationProperties.SetName(row, $"{Parties.MemberName(member)}, {Parties.MemberState(member)}");
        return row;
    }

    async void Copy()
    {
        if (ctx.Known.Party?.Invite is not { } link)
            return;
        var package = new global::Windows.ApplicationModel.DataTransfer.DataPackage();
        package.SetText(link);
        global::Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
        var mine = ++copied;
        copyText.Text = "Copied";
        await Task.Delay(2000);
        if (mine == copied)
            copyText.Text = "Copy invite link";
    }
}
