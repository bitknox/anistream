using Anistream.Engine.Core;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

namespace Anistream.App;

/// <summary>
/// What protects torrent traffic, and what a failed check does: the words for each choice, as
/// the Mac (<c>ProtectionWords</c>) and Linux (<c>pages::torrents</c>) say them, the turn-on
/// dialog's draft, and the commands it sends. No protection and Warn only each ask the viewer to
/// confirm first, and go to the engine acknowledged.
/// </summary>
public static class TorrentWords
{
    /// <summary>The choices in the order the other apps list them.</summary>
    public static readonly TorrentProtection[] Order = [TorrentProtection.Proxy, TorrentProtection.Device, TorrentProtection.None];

    public static readonly FailedCheck[] FailedChecks = [FailedCheck.Pause, FailedCheck.Stop, FailedCheck.Warn];

    public static string Label(TorrentProtection protection) => protection switch
    {
        TorrentProtection.Proxy => "SOCKS5 proxy",
        TorrentProtection.Device => "VPN on this device",
        _ => "No protection",
    };

    public static string Detail(TorrentProtection protection) => protection switch
    {
        TorrentProtection.Proxy => "Torrent traffic goes through a proxy you run or rent.",
        TorrentProtection.Device => "Uses the VPN already running on this PC.",
        _ => "Your address is visible to everyone in the torrent.",
    };

    public static string Label(FailedCheck check) => check switch
    {
        FailedCheck.Pause => "Pause torrents",
        FailedCheck.Stop => "Stop torrents",
        _ => "Warn only",
    };

    public const string Exposed = "Other people can see your IP address while you download and share a torrent. Choose what protects it before torrents start.";
    public const string IndexNote = "The RSS search address anistream asks for torrents. anistream hosts no content.";
    public const string CheckNote = "anistream checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.";
    public const string OffNote = "Off by default. Turning this on asks how to protect your connection first.";

    public const string NoneTitle = "Use torrents without protection?";
    public const string NoneMessage = "Other people can see your IP address while you download and share a torrent, and anistream does not check the connection.";
    /// <summary>The confirmation's button: what pressing it does, turning torrents on or changing what protects them.</summary>
    public static string NoneAction(bool turningOn) => turningOn ? "Turn on without protection" : "Use without protection";

    public const string FailedNote = "What happens to torrents when the check finds traffic leaving outside your VPN or proxy.";
    public const string WarnTitle = "Keep torrents running if a check fails?";
    public const string WarnMessage = "anistream will warn you instead of pausing. If your VPN or proxy is down, other people can see your IP address.";

    /// <summary>Edit proxy's line over its form.</summary>
    public const string EditNote = "Torrents keep using the saved proxy until the new one passes the connection check.";
    /// <summary>Under the password, wherever a saved proxy's details are open again.</summary>
    public const string PasswordKept = "Leave the password empty to keep the saved one.";
    /// <summary>While a proxy is checked before it is saved.</summary>
    public const string CheckingProxy = "Checking the proxy";
    /// <summary>Beside Protect with once a proxy passed its check and was saved.</summary>
    public const string ProxySaved = "Proxy saved";

    /// <summary>What a form was doing with the proxy it checked: what stays when the check fails.</summary>
    public enum Kept { EditedProxy, Protection, TurningOn }

    /// <summary>Why a proxy was not saved, from the check's <paramref name="view"/> of it, and what stays: Android's words.</summary>
    public static string NotSaved(GuardView view, Kept kept)
    {
        var why = view is GuardView.Leaking
            ? "Torrent traffic left outside this proxy"
            : "The check could not be made through this proxy";
        return kept switch
        {
            Kept.EditedProxy => $"Not saved. {why}, so the saved proxy stays.",
            Kept.Protection => $"Not saved. {why}, so the saved protection stays.",
            _ => $"Torrents are still off. {why}.",
        };
    }

    /// <summary>What protects torrent traffic now, under the switch while torrents are on.</summary>
    public static string OnNote(ProtectionView view) => (view.Protection, view.Proxy) switch
    {
        (TorrentProtection.Proxy, { } proxy) => $"Protected by a SOCKS5 proxy at {proxy.Host}:{proxy.Port}.",
        (TorrentProtection.Proxy, null) => "Protected by a SOCKS5 proxy.",
        (TorrentProtection.Device, _) => "Protected by the VPN on this device.",
        _ => "Not protected. Your address is visible to everyone in the torrent.",
    };

    /// <summary>
    /// The connection check's last word: the line under Last check, the word beside it, and
    /// whether it failed. <paramref name="ago"/> is how many seconds since it was said.
    /// </summary>
    public static (string Note, string Word, bool Failed) Guard(GuardView? view, long? ago) => view switch
    {
        GuardView.Protected p => ($"Leaving through {p.Exit}" + (ago is { } s ? $" · {AnistreamFfiMethods.Ago(s)}" : ""), "Passed", false),
        GuardView.Checking => ("Checking where torrent traffic leaves from", "Checking", false),
        GuardView.Leaking => ("Traffic left outside the protection. Torrents are paused.", "Failed", true),
        GuardView.Unreachable => ("The check could not be made. Torrents are paused.", "Failed", true),
        GuardView.Unprotected => ("No protection is set", "Unprotected", false),
        _ => ("Not checked yet", "Waiting", false),
    };
}

/// <summary>
/// The turn-on dialog's choices before they are sent. A password left empty keeps the one kept.
/// <see cref="NeedsIndex"/> is turning torrents on, which the engine refuses without an index.
/// </summary>
public sealed record TorrentDraft(
    string Index, TorrentProtection Protection, string Host, string Port, string User, string Password, bool NeedsIndex)
{
    /// <summary>The draft the dialog opens on: what the engine last said, or <paramref name="start"/>'s choice.</summary>
    public static TorrentDraft From(SettingsView? settings, bool turningOn, TorrentProtection? start = null)
    {
        var proxy = settings?.Torrent.Proxy;
        return new(
            settings?.Sources.Index ?? "",
            start ?? settings?.Torrent.Protection ?? TorrentProtection.Proxy,
            proxy?.Host ?? "",
            proxy is { } p ? p.Port.ToString(System.Globalization.CultureInfo.InvariantCulture) : "",
            proxy?.User ?? "",
            "",
            turningOn);
    }

    /// <summary>The proxy as the engine takes it, when the fields make one.</summary>
    public TorrentProxy? Proxy =>
        Protection == TorrentProtection.Proxy && Host.Trim() is { Length: > 0 } host
            && ushort.TryParse(Port.Trim(), out var port) && port > 0
            ? new TorrentProxy(host, port, User.Trim() is { Length: > 0 } user ? user : null, Password.Length > 0 ? Password : null)
            : null;

    /// <summary>Whether it can be sent: the index when one is needed, and a proxy with its host and port.</summary>
    public bool Complete =>
        (!NeedsIndex || Index.Trim().Length > 0) && (Protection != TorrentProtection.Proxy || Proxy is not null);
}

public static class Torrents
{
    /// <summary>
    /// <paramref name="mode"/> as the engine takes it, with what is kept: the proxy's fields when
    /// it is the proxy and none are given, and the failed check as it stands unless
    /// <paramref name="onFailure"/> changes it.
    /// </summary>
    public static Command Protect(ProtectionView? current, TorrentProtection mode, TorrentProxy? proxy = null,
        FailedCheck? onFailure = null, bool acknowledged = false)
    {
        proxy ??= current?.Proxy is { } kept ? new TorrentProxy(kept.Host, kept.Port, kept.User, null) : null;
        return new Command.SetTorrentProtection(
            mode,
            mode == TorrentProtection.Proxy ? proxy : null,
            onFailure ?? current?.OnFailure ?? FailedCheck.Pause,
            acknowledged);
    }

    /// <summary>
    /// What the dialog sends: the index when it changed, the protection, and torrents on when
    /// they are turned on here. No protection goes acknowledged, the viewer having confirmed it.
    /// </summary>
    public static IEnumerable<Command> Apply(TorrentDraft draft, SettingsView? settings, bool turningOn)
    {
        var typed = draft.Index.Trim();
        if (typed.Length > 0 && typed != (settings?.Sources.Index ?? ""))
            yield return new Command.SetSetting(SettingKey.TorrentIndex, new SettingValue.Str(typed));
        yield return Protect(settings?.Torrent, draft.Protection, draft.Proxy, acknowledged: draft.Protection == TorrentProtection.None);
        if (turningOn)
            yield return new Command.SetSetting(SettingKey.Torrents, new SettingValue.Bool(true));
    }

    /// <summary>
    /// What a failed check does: pause or stop as a setting, Warn only through the protection
    /// command, acknowledged once the viewer confirmed it. Null when it is already so.
    /// </summary>
    public static Command? FailedCheckCommand(ProtectionView? current, FailedCheck check)
    {
        if (current is null || check == current.OnFailure)
            return null;
        return check == FailedCheck.Warn
            ? Protect(current, current.Protection, onFailure: FailedCheck.Warn, acknowledged: true)
            : new Command.SetSetting(SettingKey.VpnOnFailure, new SettingValue.Str(check == FailedCheck.Stop ? "stop" : "pause"));
    }
}

/// <summary>A protection to check before it is saved: the choice, and the proxy when it is one.</summary>
public sealed record TrialAsk(TorrentProtection Mode, TorrentProxy? Proxy);

/// <summary>
/// Checks before anything is saved, one at a time. The engine answers each
/// <see cref="Command.CheckTorrentProtection"/> with an unnumbered
/// <see cref="Event.ProtectionChecked"/>, each from its own task, so two in flight could answer
/// in either order: a check is only sent once the one before it has answered, and an answer
/// goes to the ask that was sent, never to one made since. Asking again replaces what waits;
/// <see cref="Drop"/> forgets the ask, its answer then going nowhere.
/// </summary>
public sealed class ProtectionTrials(Action<Command> send)
{
    int outstanding;
    TrialAsk? queued, sent;
    Action<TrialAsk, GuardView>? answer;

    /// <summary>Whether an ask waits for its verdict.</summary>
    public bool Checking => queued is not null || sent is not null;

    /// <summary>Checks <paramref name="ask"/>, and calls <paramref name="answered"/> with it and its verdict.</summary>
    public void Ask(TrialAsk ask, Action<TrialAsk, GuardView> answered)
    {
        answer = answered;
        sent = null;
        queued = ask;
        Pump();
    }

    /// <summary>Done waiting: the form closed. A verdict still to come is dropped.</summary>
    public void Drop()
    {
        queued = null;
        sent = null;
        answer = null;
    }

    /// <summary>The engine's verdict on the check it was sent last.</summary>
    public void Heard(GuardView view)
    {
        if (outstanding > 0)
            outstanding--;
        if (sent is { } mine && outstanding == 0)
        {
            var answered = answer;
            sent = null;
            answer = null;
            answered?.Invoke(mine, view);
        }
        Pump();
    }

    void Pump()
    {
        if (queued is not { } next || outstanding > 0)
            return;
        queued = null;
        sent = next;
        outstanding++;
        send(new Command.CheckTorrentProtection(next.Mode, next.Proxy));
    }
}
