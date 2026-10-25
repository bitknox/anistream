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
        true => "Ready",
        false => "Not ready",
        null => "No ready state",
    };

    /// <summary>A second line for a member whose episode is not this device's, or null.</summary>
    public static string? MemberFile(PartyMember member) => member switch
    {
        { You: true } => null,
        { FileMatches: false } => "Playing a different file",
        { File: null } => "Nothing open yet",
        _ => null,
    };

    /// <summary>The member's name, with "(you)" for this device's.</summary>
    public static string MemberName(PartyMember member) => member.You ? $"{member.Name} (you)" : member.Name;

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
            return "Nobody else is here yet. Send the invite link.";
        if (AllReady(members))
            return "Everyone is ready";
        var waiting = members.Where(m => m.Ready == false).Select(m => m.You ? "you" : m.Name).ToList();
        return waiting.Count switch
        {
            1 when waiting[0] == "you" => "Play starts once you're ready",
            1 => $"Play starts once {waiting[0]} is ready",
            2 => $"Play starts once {waiting[0]} and {waiting[1]} are ready",
            _ => $"Play starts once {waiting.Count} more are ready",
        };
    }

    /// <summary>How the connection stands, for the panel's header.</summary>
    public static string ConnectionLine(PartyView party) => party.Connection switch
    {
        PartyConnection.Reconnecting r => $"Connection lost. Trying again, {r.Attempt} of {r.Of}",
        _ => (party.Encrypted ? "Connected, encrypted" : "Connected, not encrypted")
             + (party.LatencyMs is { } ms ? $" · {ms.ToString(CultureInfo.InvariantCulture)} ms" : ""),
    };

    /// <summary>The line under the server field: whose the suggested server is, so the viewer knows before joining.</summary>
    public static string ServerHint(string typed, PartyForm form) =>
        string.Equals(typed.Trim(), form.SuggestedServer, StringComparison.OrdinalIgnoreCase)
            ? $"A public server run by {form.SuggestedRunBy}, not by anistream."
            : $"No server of your own? {form.SuggestedServer} is a public one, run by {form.SuggestedRunBy}.";

    /// <summary>
    /// What is wrong with a server as typed, or null when it reads as <c>host</c>,
    /// <c>host:port</c> or <c>[v6]:port</c>. Empty is not wrong, only not yet chosen.
    /// </summary>
    public static string? ServerProblem(string typed)
    {
        var text = typed.Trim();
        if (text.Length == 0)
            return null;
        const string shape = "Write the server as a host and a port, like party.example:8999.";
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
