using Anistream.Engine.Core;
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
        TorrentProtection.Proxy => T.TorrentsProtectProxy,
        TorrentProtection.Device => T.TorrentsProtectVpn,
        _ => T.TorrentsProtectNone,
    };

    public static string Detail(TorrentProtection protection) => protection switch
    {
        TorrentProtection.Proxy => T.TorrentsProtectProxyDetail,
        TorrentProtection.Device => T.TorrentsProtectVpnDetail,
        _ => T.TorrentsProtectNoneDetail,
    };

    public static string Label(FailedCheck check) => check switch
    {
        FailedCheck.Pause => T.TorrentsFailPause,
        FailedCheck.Stop => T.TorrentsFailStop,
        _ => T.TorrentsFailWarn,
    };

    public static string Exposed => T.TorrentsRisk;
    public static string CheckNote => T.TorrentsCheckNote;
    public static string OffNote => T.TorrentsOffDetail;

    public static string NoneTitle => T.TorrentsUnprotectedTitle;
    public static string NoneMessage => T.TorrentsUnprotectedDetail;
    /// <summary>The confirmation's button: what pressing it does, turning torrents on or changing what protects them.</summary>
    public static string NoneAction(bool turningOn) => turningOn ? T.TorrentsTurnOnUnprotected : T.TorrentsUseUnprotected;

    public static string FailedNote => T.TorrentsKeepRunningDetail;
    public static string WarnTitle => T.TorrentsKeepRunningTitle;
    public static string WarnMessage => T.TorrentsWarnInstead;

    /// <summary>Edit proxy's line over its form.</summary>
    public static string EditNote => T.TorrentsProxyKeepsSaved;
    /// <summary>Under the password, wherever a saved proxy's details are open again.</summary>
    public static string PasswordKept => T.TorrentsPasswordKeep;
    /// <summary>While a proxy is checked before it is saved.</summary>
    public static string CheckingProxy => T.TorrentsCheckingProxy;
    /// <summary>Beside Protect with once a proxy passed its check and was saved.</summary>
    public static string ProxySaved => T.TorrentsProxySaved;

    /// <summary>What a form was doing with the proxy it checked: what stays when the check fails.</summary>
    public enum Kept { EditedProxy, Protection, TurningOn }

    /// <summary>Why a proxy was not saved, from the check's <paramref name="view"/> of it, and what stays: Android's words.</summary>
    public static string NotSaved(GuardView view, Kept kept)
    {
        var why = view is GuardView.Leaking ? T.TorrentsLeftOutsideProxy : T.TorrentsCheckNotMadeProxy;
        return kept switch
        {
            Kept.EditedProxy => T.TorrentsProxyNotSaved(why),
            Kept.Protection => T.TorrentsProtectionNotSaved(why),
            _ => T.TorrentsStillOff(why),
        };
    }

    /// <summary>What protects torrent traffic now, under the switch while torrents are on.</summary>
    public static string OnNote(ProtectionView view) => (view.Protection, view.Proxy) switch
    {
        (TorrentProtection.Proxy, { } proxy) => T.TorrentsSummaryProxyAt(proxy.Host, proxy.Port.ToString(System.Globalization.CultureInfo.InvariantCulture)),
        (TorrentProtection.Proxy, null) => T.TorrentsSummaryProxy,
        (TorrentProtection.Device, _) => T.TorrentsSummaryVpn,
        _ => T.TorrentsSummaryNone,
    };

    /// <summary>
    /// The connection check's last word: the line under Last check, the word beside it, and
    /// whether it failed. <paramref name="ago"/> is how many seconds since it was said.
    /// </summary>
    public static (string Note, string Word, bool Failed) Guard(GuardView? view, long? ago) => view switch
    {
        GuardView.Protected p => (T.TorrentsCheckPassedExit(p.Exit) + (ago is { } s ? $" · {Wording.Ago(s)}" : ""), T.TorrentsCheckPassed, false),
        GuardView.Checking => (T.TorrentsCheckRunningLine, T.TorrentsCheckRunning, false),
        GuardView.Leaking => (T.TorrentsCheckLeaked(T.TorrentsFailPauseResult), T.TorrentsCheckFailed, true),
        GuardView.Unreachable => (T.TorrentsCheckUnmade(T.TorrentsFailPauseResult), T.TorrentsCheckFailed, true),
        GuardView.Unprotected => (T.TorrentsCheckNoneLine, T.TorrentsCheckNone, false),
        _ => (T.TorrentsCheckWaitingLine, T.TorrentsCheckWaiting, false),
    };
}

/// <summary>
/// The turn-on dialog's choices before they are sent. A password left empty keeps the one kept.
/// </summary>
public sealed record TorrentDraft(
    TorrentProtection Protection, string Host, string Port, string User, string Password)
{
    /// <summary>The draft the dialog opens on: what the engine last said, or <paramref name="start"/>'s choice.</summary>
    public static TorrentDraft From(SettingsView? settings, bool turningOn, TorrentProtection? start = null)
    {
        var proxy = settings?.Torrent.Proxy;
        return new(
            start ?? settings?.Torrent.Protection ?? TorrentProtection.Proxy,
            proxy?.Host ?? "",
            proxy is { } p ? p.Port.ToString(System.Globalization.CultureInfo.InvariantCulture) : "",
            proxy?.User ?? "",
            "");
    }

    /// <summary>The proxy as the engine takes it, when the fields make one.</summary>
    public TorrentProxy? Proxy =>
        Protection == TorrentProtection.Proxy && Host.Trim() is { Length: > 0 } host
            && ushort.TryParse(Port.Trim(), out var port) && port > 0
            ? new TorrentProxy(host, port, User.Trim() is { Length: > 0 } user ? user : null, Password.Length > 0 ? Password : null)
            : null;

    /// <summary>Whether it can be sent: a proxy needs its host and port.</summary>
    public bool Complete => Protection != TorrentProtection.Proxy || Proxy is not null;
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
    /// What the dialog sends: the protection, and torrents on when
    /// they are turned on here. No protection goes acknowledged, the viewer having confirmed it.
    /// </summary>
    public static IEnumerable<Command> Apply(TorrentDraft draft, SettingsView? settings, bool turningOn)
    {
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
