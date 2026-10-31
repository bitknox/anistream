using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
using Windows.ApplicationModel.DataTransfer;
using Windows.System;

namespace Anistream.Windows.Pages;

/// <summary>
/// Watch together as a place of its own. Out of a party: start one, join with a link or code, and the
/// last rooms. In one: the lobby, which says what the room is watching, who is ready and what is
/// left to do. One amber on the page, on the next thing to do; once you are ready it turns tonal.
/// </summary>
public sealed partial class TogetherPage : Page
{
    readonly Ctx ctx = Ctx.Current;
    readonly TextBox join = new() { PlaceholderText = T.TogetherJoinField, MinWidth = 280 };
    readonly TextBlock joinProblem = Ui.Text("", "HazeCaptionStyle");
    bool inLobby;

    public TogetherPage()
    {
        InitializeComponent();
        AutomationProperties.SetName(join, T.TogetherJoinField);
        join.KeyDown += (_, e) =>
        {
            if (e.Key == VirtualKey.Enter)
                Join(join.Text);
        };
        ctx.Hear(this, Heard);
        // Ctrl+V outside a field pastes into Join.
        KeyboardAccelerators.Add(Paste());
    }

    KeyboardAccelerator Paste()
    {
        var key = new KeyboardAccelerator { Key = VirtualKey.V, Modifiers = VirtualKeyModifiers.Control };
        key.Invoked += async (_, e) =>
        {
            if (FocusManager.GetFocusedElement(XamlRoot) is TextBox || ctx.Known.Party is not null)
                return;
            e.Handled = true;
            await PasteIntoJoin();
        };
        return key;
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ctx.Send(new Command.LoadPartyForm());
        ctx.Send(new Command.Party(new PartyCommand.LoadRecent()));
        Show();
    }

    void Heard(IReadOnlyList<Event> batch)
    {
        if (batch.Any(e => e is Event.PartyJoined or Event.PartyState or Event.PartyMembers or Event.PartyLeft
            or Event.PartyRecent or Event.PartyGlimpse or Event.PartyLookFailed or Event.PartyForm or Event.PartyLastOut))
            Show();
    }

    async Task PasteIntoJoin()
    {
        var data = Clipboard.GetContent();
        if (data.Contains(StandardDataFormats.Text))
            join.Text = (await data.GetTextAsync()).Trim();
        join.Focus(FocusState.Programmatic);
    }

    /// <summary>A link or a code. A code implies the suggested server; a link carries its own.</summary>
    void Join(string text)
    {
        if (AnistreamFfiMethods.ParsePartyTarget(text) is not { } target)
        {
            joinProblem.Text = T.PartyCodeShape("kite-7qm4-xr9d");
            joinProblem.Visibility = Visibility.Visible;
            return;
        }
        joinProblem.Visibility = Visibility.Collapsed;
        var server = target.Server.Length > 0 ? target.Server : ctx.Known.PartyForm?.SuggestedServer ?? "";
        ctx.Send(new Command.Party(new PartyCommand.Lobby(server, target.Room, ctx.Known.PartyForm?.Name, null)));
    }

    void Show()
    {
        Stack.Children.Clear();
        var heading = Ui.Text(T.TogetherTitle, "PageTitleStyle");
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level1);
        Stack.Children.Add(heading);
        if (ctx.Known.Party is { } party)
        {
            inLobby = true;
            Lobby(party);
        }
        else
        {
            inLobby = false;
            Place();
        }
    }

    // --- Out of a party ---

    void Place()
    {
        Stack.Children.Add(Ui.Haze(T.TogetherLine));
        Stack.Children.Add(Ui.Haze(T.LegalHostsNoContent));

        Stack.Children.Add(Section(T.TogetherStart));
        Stack.Children.Add(Ui.Haze(T.TogetherStartLine));
        // A party starts from a title: its page has Watch together, and search finds the title.
        Stack.Children.Add(Ui.Label(T.PartyStartParty, (_, _) => ctx.Window.Go("search"), accent: true));

        Stack.Children.Add(Section(T.TogetherJoin));
        Stack.Children.Add(Ui.Haze(T.TogetherJoinLine));
        var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        row.Children.Add(join);
        row.Children.Add(Ui.Label(T.TogetherPaste, async (_, _) => await PasteIntoJoin()));
        row.Children.Add(Ui.Label(T.TogetherJoinAction, (_, _) => Join(join.Text)));
        Stack.Children.Add(row);
        joinProblem.Visibility = Visibility.Collapsed;
        Stack.Children.Add(joinProblem);
        Stack.Children.Add(Ui.Haze(T.TogetherPasteShortcutHint));

        Stack.Children.Add(Section(T.TogetherRecent));
        var recent = ctx.Known.PartyRecent;
        if (recent.Length == 0)
            Stack.Children.Add(Ui.Haze(T.TogetherRecentEmpty));
        foreach (var room in recent)
            Stack.Children.Add(Recent(room));
        if (ctx.Known.PartyLookFailure is { } failure)
            Stack.Children.Add(new InfoBar { Severity = InfoBarSeverity.Warning, IsOpen = true, IsClosable = false, Message = failure });
    }

    UIElement Recent(PartyRecentRoom room)
    {
        var grid = new Grid { ColumnSpacing = 12, MinHeight = 56 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(Ui.Text(room.Title, "BodyStrongTextBlockStyle", wrap: false));
        var when = DateTimeOffset.FromUnixTimeSeconds(room.LastAt).LocalDateTime.ToString("d");
        text.Children.Add(Ui.Text(T.TogetherRecentRow(room.Episode, string.Join(", ", room.Names), when), "HazeCaptionStyle", wrap: false));
        grid.Children.Add(text);
        var rejoin = Ui.Label(T.TogetherRejoin, (_, _) =>
            ctx.Send(new Command.Party(new PartyCommand.Lobby(room.Server, room.Room, ctx.Known.PartyForm?.Name, null))));
        Grid.SetColumn(rejoin, 1);
        grid.Children.Add(rejoin);
        var forget = Ui.Label(T.TogetherForgetRoom, (_, _) =>
            ctx.Send(new Command.Party(new PartyCommand.ForgetRoom(room.Server, room.Room))));
        Grid.SetColumn(forget, 2);
        grid.Children.Add(forget);
        return grid;
    }

    // --- In a party: the lobby ---

    void Lobby(PartyView party)
    {
        var members = ctx.Known.PartyMembers ?? [];
        if (party.Connection is PartyConnection.Reconnecting retry)
            Stack.Children.Add(new InfoBar { Severity = InfoBarSeverity.Warning, IsOpen = true, IsClosable = false, Message = T.PartyConnectionLost(retry.Attempt, retry.Of) });
        if (party.Starter is { Length: > 0 } starter)
            Stack.Children.Add(Ui.Haze(T.PartyInvitedBy(starter)));

        // What the room is watching, from its playlist: nobody picks a title to join.
        Stack.Children.Add(Section(T.PartyRoomWatching));
        if (party.RoomEpisode is { } room)
        {
            Stack.Children.Add(Ui.Text(room.Title.Length > 0 ? room.Title : room.File, "SubtitleTextBlockStyle"));
            Stack.Children.Add(Ui.Haze(T.EpisodeNumber(long.TryParse(room.Episode, out var n) ? n : 0)));
            if (room.Found == PartyFound.Name)
                Stack.Children.Add(Ui.Haze(T.PartyRoomFoundByName));
            else if (room.Found == PartyFound.FileName)
            {
                Stack.Children.Add(Ui.Haze(T.PartyRoomMatched));
                Stack.Children.Add(Ui.Label(T.PartyRoomNotThis, (_, _) => ctx.Send(new Command.Party(new PartyCommand.ChooseTitle(null)))));
            }
            else if (room.Found == PartyFound.Unmatched)
            {
                Stack.Children.Add(Ui.Haze(T.PartyRoomPlainFile(room.PlainBy ?? "", room.File)));
                if (party.Guesses.Length > 0)
                    Stack.Children.Add(Ui.Haze(T.PartyRoomGuesses));
                foreach (var guess in party.Guesses)
                {
                    var id = guess.Id;
                    Stack.Children.Add(Ui.Label(guess.Title, (_, _) => ctx.Send(new Command.Party(new PartyCommand.ChooseTitle(id)))));
                }
            }
        }
        else
        {
            Stack.Children.Add(Ui.Text(T.PartyRoomNothing, "SubtitleTextBlockStyle"));
            Stack.Children.Add(Ui.Haze(T.PartyRoomNothingLine));
            Stack.Children.Add(Ui.Label(T.PartyRoomPick, (_, _) => ctx.Window.Go("search")));
        }

        // Your own sources, checked for that episode. Never anyone else's.
        switch (party.Sources)
        {
            case PartySources.Checking:
                Stack.Children.Add(Ui.Haze(T.PartyLookingIn));
                break;
            case PartySources.Found:
                Stack.Children.Add(Ui.Haze(T.PartyLobbySourcesFound));
                break;
            case PartySources.Missing missing:
                Stack.Children.Add(Ui.Text(missing.Reason.Text(), "BodyTextBlockStyle"));
                Stack.Children.Add(Ui.Label(T.PartyLobbySourcesMissing, (_, _) => ctx.Window.Go("providers")));
                break;
        }

        // Members: you first, then in the order they joined. A state in words, never a source.
        Stack.Children.Add(Section(T.PartyWaitingMembers));
        foreach (var member in members.OrderBy(m => m.You ? 0 : 1))
            Stack.Children.Add(MemberRow(member));
        var ready = members.Count(m => m.Ready == true);
        Stack.Children.Add(Ui.Haze(T.PartyLobbyCount(ready, members.Length)));
        Stack.Children.Add(Ui.Haze(party.StartingIn is { } secs ? T.PartyStarting(secs) : party.Playing || members.Length > 0 && ready == members.Length ? T.PartyAllReady : T.PartyStartsLine));

        // Ready is the one amber; once ready it is tonal with a check and the word.
        var actions = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Margin = new Thickness(0, 4, 0, 0) };
        var readyButton = party.Ready
            ? Ui.Label("✓ " + T.PartyReadyOn, (_, _) => ctx.Send(new Command.PartyReady(false)))
            : Ui.Label(T.PartyReadyAction, (_, _) => ctx.Send(new Command.PartyReady(true)), accent: true);
        actions.Children.Add(readyButton);
        if (party.CanStartAnyway)
            actions.Children.Add(Ui.Label(T.PartyStartAnyway, (_, _) => ctx.Send(new Command.Party(new PartyCommand.StartAnyway()))));
        if (!party.Playing)
            actions.Children.Add(Ui.Label(T.TogetherOpenPlayer, (_, _) => ctx.Send(new Command.Party(new PartyCommand.OpenPlayer()))));
        Stack.Children.Add(actions);

        // Invite: the https link and the code, for reading out.
        if (party.Link is { } link)
        {
            Stack.Children.Add(Section(T.PartyShareInvite));
            var invite = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            invite.Children.Add(Ui.Label(T.PartyCopyInvite, (_, _) =>
            {
                if (Ui.TryCopy(link))
                    ctx.Window.Say(T.PartyInviteCopied);
            }));
            Stack.Children.Add(invite);
            if (party.Code is { } code)
                Stack.Children.Add(Ui.Text(T.PartyCode + " " + code, "BodyStrongTextBlockStyle"));
        }
        var leave = Ui.Label(T.PartyLeave, (_, _) => ctx.Send(new Command.PartyLeave()));
        leave.Margin = new Thickness(0, 12, 0, 0);
        Stack.Children.Add(leave);
    }

    static UIElement MemberRow(PartyMember member)
    {
        var state = member.Ready switch
        {
            true => "✓ " + T.PartyMemberReady,
            false => T.PartyMemberNotReady,
            null => member.File is null ? T.PartyMemberNoFile : T.PartyMemberJoining,
        };
        var grid = new Grid { ColumnSpacing = 12, Height = 44 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        // Initials on a raised circle, never a colour of their own.
        var circle = new Border
        {
            Width = 32, Height = 32, CornerRadius = new CornerRadius(16), VerticalAlignment = VerticalAlignment.Center,
            Background = Ui.Res<Microsoft.UI.Xaml.Media.Brush>("CardBackgroundFillColorSecondaryBrush"),
            Child = new TextBlock
            {
                Text = member.Name.Length > 0 ? member.Name[..1].ToUpperInvariant() : "?",
                HorizontalAlignment = HorizontalAlignment.Center, VerticalAlignment = VerticalAlignment.Center,
            },
        };
        AutomationProperties.SetAccessibilityView(circle, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        grid.Children.Add(circle);
        var name = Ui.Text(member.You ? T.PartyMemberYou(member.Name) : member.Name, "BodyTextBlockStyle", wrap: false);
        name.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(name, 1);
        grid.Children.Add(name);
        var status = Ui.Haze(state, wrap: false);
        status.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(status, 2);
        grid.Children.Add(status);
        AutomationProperties.SetName(grid, $"{member.Name}, {state}");
        return grid;
    }

    static TextBlock Section(string text)
    {
        var block = Ui.Text(text, "BodyStrongTextBlockStyle");
        block.Margin = new Thickness(0, 10, 0, 0);
        AutomationProperties.SetHeadingLevel(block, AutomationHeadingLevel.Level2);
        return block;
    }
}
