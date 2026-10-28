using System.Globalization;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// The words of a watch party, as the dialog that starts one and the panel in the player say
/// them: who is ready, how the connection stands, which server is suggested and who runs it.
/// The party itself is the engine's.
/// </summary>
public static class Parties
{
    /// <summary>What the panel says of a member, beside their name.</summary>
    public static string MemberState(PartyMember member) => member.Ready switch
    {
        true => T.PartyMemberReady,
        false => T.PartyMemberNotReady,
        null => T.PartyMemberNoReadyState,
    };

    /// <summary>A second line for a member whose episode is not this device's, or null.</summary>
    public static string? MemberFile(PartyMember member) => member switch
    {
        { You: true } => null,
        { FileMatches: false } => T.PartyMemberOtherFile,
        { File: null } => T.PartyMemberNoFile,
        _ => null,
    };

    /// <summary>The member's name, with "(you)" for this device's.</summary>
    public static string MemberName(PartyMember member) => member.You ? T.PartyMemberYou(member.Name) : member.Name;

    /// <summary>
    /// Every member is ready, and there are at least two: when play starts on its own. A member
    /// whose client has no ready state does not hold the room up.
    /// </summary>
    public static bool AllReady(IReadOnlyCollection<PartyMember> members) =>
        members.Count >= 2 && members.All(m => m.Ready != false);

    /// <summary>The line under the members: everyone ready, who the room waits for, or alone.</summary>
    public static string ReadyLine(IReadOnlyCollection<PartyMember> members)
    {
        if (members.Count < 2)
            return T.PartyAlone;
        if (AllReady(members))
            return T.PartyAllReady;
        var waiting = members.Where(m => m.Ready == false).ToList();
        return waiting.Count switch
        {
            1 when waiting[0].You => T.PartyWaitYou,
            1 => T.PartyWaitOne(waiting[0].Name),
            2 => T.PartyWaitTwo(Named(waiting[0]), Named(waiting[1])),
            _ => T.PartyWaitMany(waiting.Count),
        };
    }

    static string Named(PartyMember member) => member.You ? T.PartyYou : member.Name;

    /// <summary>How the connection stands, for the panel's header.</summary>
    public static string ConnectionLine(PartyView party)
    {
        if (party.Connection is PartyConnection.Reconnecting r)
            return T.PartyConnectionLost(r.Attempt, r.Of);
        var line = party.Encrypted ? T.PartyConnectedEncrypted : T.PartyConnectedPlain;
        return party.LatencyMs is { } ms ? T.PartyConnectedLatency(line, (long)ms) : line;
    }

    /// <summary>The line under the server field: whose the suggested server is, so the viewer knows before joining.</summary>
    public static string ServerHint(string typed, PartyForm form) =>
        string.Equals(typed.Trim(), form.SuggestedServer, StringComparison.OrdinalIgnoreCase)
            ? T.PartyServerPublic(form.SuggestedRunBy)
            : T.PartyServerSuggest(form.SuggestedServer, form.SuggestedRunBy);

    /// <summary>
    /// What is wrong with a server as typed, or null when it reads as <c>host</c>,
    /// <c>host:port</c> or <c>[v6]:port</c>. Empty is not wrong, only not yet chosen.
    /// </summary>
    public static string? ServerProblem(string typed)
    {
        var text = typed.Trim();
        if (text.Length == 0)
            return null;
        var shape = T.PartyServerShape;
        if (text.Contains(' ') || text.Contains('/'))
            return shape;
        string host, port;
        if (text.StartsWith('['))
        {
            var close = text.IndexOf(']');
            if (close < 0)
                return shape;
            host = text[1..close];
            var rest = text[(close + 1)..];
            if (rest.Length > 0 && (rest[0] != ':' || rest.Length == 1))
                return shape;
            port = rest.Length > 0 ? rest[1..] : "";
        }
        else if (text.Split(':') is [var h, var p])
        {
            if (p.Length == 0)
                return shape;
            (host, port) = (h, p);
        }
        else if (!text.Contains(':'))
            (host, port) = (text, "");
        else
            return shape;
        if (host.Length == 0)
            return shape;
        if (port.Length > 0 && !(ushort.TryParse(port, NumberStyles.None, CultureInfo.InvariantCulture, out var n) && n > 0))
            return shape;
        return null;
    }

    /// <summary>A room name is anything but blank; the server takes it as it is.</summary>
    public static bool RoomOk(string room) => room.Trim().Length > 0;
}
