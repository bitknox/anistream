using System.Runtime.InteropServices;
using Anistream.App;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;
using Anistream.Windows.Pages;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Anistream.Windows;

/// <summary>
/// Watch parties, the window's side: the dialog that starts one or answers an invite link, the
/// dialog for an episode none of the sources has, what the window says as a party starts and
/// ends, and asking before closing the window leaves one.
/// </summary>
public sealed partial class MainWindow
{
    bool inParty, askedParty;

    /// <summary>The dialog that starts a party, or joins one from an invite.</summary>
    internal async void ShowParty(PartyDialog.For what, Action<PartyDialog>? shown = null)
    {
        if (CloseDialogs())
            await Task.Delay(250);
        try
        {
            await new PartyDialog(ctx, Root.XamlRoot, what).ShowAsync(shown);
        }
        catch (COMException)
        {
            // Another dialog came up first.
        }
    }

    /// <summary>An <c>anistream://party</c> link: its title's page, and the dialog that asks Join.</summary>
    void OpenPartyInvite(string text)
    {
        if (AnistreamFfiMethods.ParsePartyInvite(text) is not { } invite)
        {
            ShowToast(InfoBarSeverity.Warning, T.PartyLinkIncomplete);
            return;
        }
        if (ctx.Known.Party is { } party && party.Server == invite.Server && party.Room == invite.Room)
        {
            ShowToast(InfoBarSeverity.Informational, T.PartyAlreadyIn);
            return;
        }
        if (!Player.Showing)
            OpenTitle(invite.Id);
        ShowParty(new PartyDialog.For.Invite(invite));
    }

    async void ShowPartySource(uint id, string episode, NoSourceWhy why, string reason)
    {
        if (CloseDialogs())
            await Task.Delay(250);
        try
        {
            await new PartySourceDialog(ctx, Root.XamlRoot, id, episode, why, reason).ShowAsync();
        }
        catch (COMException)
        {
        }
    }

    void PartyHeard(IReadOnlyList<Event> batch)
    {
        foreach (var e in batch)
        {
            switch (e)
            {
                case Event.PartyJoined when !inParty:
                    inParty = true;
                    // The player covers the window's toasts: over the video, it says so itself.
                    if (Player.Showing)
                        Player.Say(T.PartyInNote);
                    else
                        ShowToast(InfoBarSeverity.Success, T.PartyInNotePlayer);
                    break;
                case Event.PartyLeft left:
                    // While the dialog is still joining, it says why itself, and over the video
                    // the engine's own notice does.
                    if (inParty && PartyDialog.Showing is null && !closing && !Player.Showing)
                    {
                        if (left.Reason is { } why)
                            ShowToast(InfoBarSeverity.Warning, why.Text());
                        else
                            ShowToast(InfoBarSeverity.Informational, T.PartyLeftNote);
                    }
                    inParty = false;
                    break;
                case Event.PartyNoSource none:
                    ShowPartySource(none.Id, none.Episode, none.Why, none.Reason.Text());
                    break;
            }
        }
        inParty = ctx.Known.Party is not null;
    }

    /// <summary>Closing the window leaves the party: asked first, once per close.</summary>
    bool AskLeavingParty()
    {
        if (ctx.Known.Party is not { } party || askedParty)
            return false;
        askedParty = true;
        AskLeavingParty(party);
        return true;
    }

    async void AskLeavingParty(PartyView party)
    {
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot,
            Title = T.PartyLeaveTitle,
            Content = new TextBlock
            {
                Text = T.PartyLeaveDetail(party.Room),
                TextWrapping = TextWrapping.WrapWholeWords,
            },
            PrimaryButtonText = T.PartyLeaveAndClose,
            CloseButtonText = T.PartyStay,
            // Stay is the default, and the one amber button: Enter and Escape both keep the party.
            DefaultButton = ContentDialogButton.Close,
        };
        ContentDialogResult answer;
        try
        {
            if (CloseDialogs())
                await Task.Delay(250);
            answer = await Dialogs.Show(dialog, first: true);
        }
        catch (COMException)
        {
            askedParty = false;
            return;
        }
        if (answer != ContentDialogResult.Primary)
        {
            askedParty = false;
            return;
        }
        ctx.Send(new Command.PartyLeave());
        // Running downloads are still asked about, as a close without a party would.
        if (!HoldForDownloads())
            CloseNow();
    }

#if ANISTREAM_TESTING
    FfiFakeParty? fakeParty;
    const string FixtureRoom = "quiet-harbor-k7m2xq9ad4tnwe3p";

    /// <summary>
    /// --show=party-*: a party on the fake server beside the engine, with members the run plays,
    /// in the state the screenshot is for.
    /// </summary>
    async void RehearseParty(string state)
    {
        await Task.Delay(1500);
        switch (state)
        {
            case "party-nosource" or "party-choose":
            {
                // An episode the sources were asked for and didn't have, in the room.
                fakeParty = ctx.Session.Engine.StartFakeParty();
                ctx.Send(new Command.PartyJoin(fakeParty.Server(), FixtureRoom, "Ada"));
                for (var i = 0; i < 50 && ctx.Known.Party is null; i++)
                    await Task.Delay(200);
                ShowPartySource(900001, "7", NoSourceWhy.NotFound, "could not match this title: sample-source has no episode 7");
                if (state == "party-choose")
                {
                    for (var i = 0; i < 50 && PartySourceDialog.Showing is null; i++)
                        await Task.Delay(200);
                    await Task.Delay(800);
                    PartySourceDialog.Showing?.RehearseChoose();
                }
                return;
            }
            case "party-form":
                ShowParty(new PartyDialog.For.Episode(900001, "7"));
                return;
            case "party-joining":
                // An address nothing answers on: the dialog waits, then says why.
                ShowParty(new PartyDialog.For.Episode(900001, "7"), d => d.Rehearse("10.255.255.1:8999", start: true));
                return;
            case "party-refused":
                ShowParty(new PartyDialog.For.Episode(900001, "7"), d => d.Rehearse("127.0.0.1:9", start: true));
                return;
        }
        fakeParty = ctx.Session.Engine.StartFakeParty();
        var server = fakeParty.Server();
        if (state is "party-invite" or "party-nosource-unaired")
        {
            var (id, episode) = state == "party-invite" ? (900001u, "7") : (900002u, "6");
            OpenLink(AnistreamFfiMethods.PartyInviteLink(new PartyInvite(Server: server, Room: FixtureRoom, Id: id, Episode: episode)));
            if (state != "party-invite")
            {
                await Task.Delay(2500);
                PartyDialog.Showing?.Rehearse(null, start: true);
            }
            return;
        }
        Player.HoldUpNext = true;
        ctx.Send(new Command.PartyJoin(server, FixtureRoom, "Ada"));
        ctx.Play(900001, "7", fromStart: true);
        // Until this device's member has the episode open, for the others to open the same.
        PartyMember? you = null;
        for (var i = 0; i < 100 && you?.File is null; i++)
        {
            await Task.Delay(200);
            you = ctx.Known.PartyMembers?.FirstOrDefault(m => m.You);
        }
        for (var i = 0; i < 50 && you?.Duration is null; i++)
        {
            await Task.Delay(200);
            you = ctx.Known.PartyMembers?.FirstOrDefault(m => m.You);
        }
        var file = you?.File ?? "Stardust Wayfarer · Episode 7";
        var length = you?.Duration ?? 30;
        // Held at the start, as a room waiting to be ready is, and clear of Up next.
        ctx.Send(new Command.Player(new PlayerCommand.TogglePause()));
        fakeParty.AddMember(FixtureRoom, "Mika");
        fakeParty.MemberOpens("Mika", file, length);
        fakeParty.AddMember(FixtureRoom, "Ren");
        fakeParty.MemberOpens("Ren", state == "party-panel" ? "Stardust Wayfarer · Episode 8" : file, length);
        fakeParty.MemberReady("Mika", true);
        fakeParty.MemberReady("Ren", false);
        await Task.Delay(800);
        switch (state)
        {
            case "party-panel":
                Player.ShowParty();
                break;
            case "party-ready":
                fakeParty.MemberReady("Ren", true);
                await Task.Delay(400);
                ctx.Send(new Command.PartyReady(true));
                await Task.Delay(600);
                Player.ShowParty();
                break;
            case "party-chat":
                fakeParty.MemberSays("Mika", "Starting in a minute, grabbing tea");
                break;
            case "party-left":
                Player.ShowParty();
                await Task.Delay(1500);
                ctx.Send(new Command.PartyLeave());
                break;
            case "party-lost":
                fakeParty.GoDown(true);
                await Task.Delay(2500);
                Player.ShowParty();
                break;
            case "party-close":
                // What the title bar's close does in a party (a window's Close() skips it).
                AskLeavingParty();
                break;
        }
    }
#endif
}
