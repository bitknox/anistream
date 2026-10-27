using System.Globalization;
using Anistream.Engine.Ffi;

namespace Anistream.App;

/// <summary>
/// What Play on TV plays: the episode, what this PC calls the title, and where this PC was in it,
/// which the TV starts from. <see cref="FromPlayer"/>: asked from the player, which closes once
/// the TV plays it.
/// </summary>
public sealed record TvPlay(uint Id, string Title, string Episode, double? From = null, bool FromPlayer = false);

/// <summary>
/// A pairing under way: the TV, whether a code is to be typed, whether a try is out, the
/// engine's words for the last one that failed, and whether that was the code running out, after
/// which only a new code goes on.
/// </summary>
public sealed record TvPairingStep(string TvName, bool NeedsCode, bool Working, string? Problem = null, bool Expired = false)
{
    /// <summary>The six digits are asked for: the TV answered and needs them.</summary>
    public bool Typing => NeedsCode && TvName.Length > 0;
}

/// <summary>What the TV dialog shows: the TVs to choose from and pairing, or the card of the TV driven.</summary>
public enum TvSheet
{
    Picker,
    Remote,
}

/// <summary>
/// Play on TV: the TVs on the network and the ones paired, pairing with a code, and the card of
/// the TV this PC drives. The engine's sender (<see cref="FfiTvSender"/>) holds every rule: which
/// rows the picker lists and in what order, what a row says under its name, what a failed
/// pairing tells the viewer and what the card reads. This keeps its latest answers for the
/// window, as the Mac's <c>TVRemoteModel</c> does, and hands the platform's DNS-SD browse to it.
///
/// Called on the UI thread; what the engine answers later arrives there too, through the UI
/// thread's synchronization context. <see cref="Changed"/> follows every change.
/// </summary>
public sealed class TvRemote(Func<DateTimeOffset>? clock = null)
{
    readonly Func<DateTimeOffset> clock = clock ?? (() => DateTimeOffset.UtcNow);
    FfiTvSender? sender;
    FfiTvBrowser? browser;
    FfiTvPairing? steps;
    FfiTvLink? link;
    int browsers, attempt;

    public event Action? Changed;

    /// <summary>The dialog up, if any.</summary>
    public TvSheet? Sheet { get; private set; }
    /// <summary>What choosing a TV plays. Null when the picker was opened to pair or connect only.</summary>
    public TvPlay? Request { get; private set; }
    public FfiTvRow[] Rows { get; private set; } = [];
    public FfiPairedTv[] Paired { get; private set; } = [];
    public bool Browsing { get; private set; }
    public TvPairingStep? Pairing { get; private set; }
    /// <summary>The TV this PC drives, by fingerprint, while a link to it is open.</summary>
    public string? Connected { get; private set; }
    /// <summary>The card as the TV last said it, and when, for the playhead to move on from.</summary>
    public FfiRemoteCard? Card { get; private set; }
    public DateTimeOffset CardAt { get; private set; }
    /// <summary>A command that did not get through, in the engine's words.</summary>
    public string? Problem { get; private set; }
    /// <summary>This PC's fingerprint in short, as a TV's settings list it.</summary>
    public string ShortFingerprint { get; private set; } = "";
    /// <summary>
    /// What Play on TV asked the TV to play, until the TV's card names it: the card shows it
    /// starting meanwhile, rather than the TV's last episode or nothing.
    /// </summary>
    public TvPlay? Starting { get; private set; }
    /// <summary>When the link to the TV was lost, while it is: the playhead stops there.</summary>
    public DateTimeOffset? LostAt { get; private set; }

    /// <summary>Once the TV plays what the player here was playing: the window closes its player.</summary>
    public Action? HandedOff { get; set; }

    /// <summary>Whether Play on TV is there: the engine was built with the remote and gave a sender.</summary>
    public bool Available => sender is not null;

    /// <summary>The link is up, and the card's controls reach the TV.</summary>
    public bool Live => Card?.Phase == FfiLinkPhase.Connected;

    /// <summary>The paired TV this PC drives.</summary>
    public FfiPairedTv? ConnectedTv => Paired.FirstOrDefault(p => p.Fingerprint == Connected);

    void Say() => Changed?.Invoke();

    /// <summary>Takes the engine's sender, browsing with <paramref name="with"/> while a picker is up.</summary>
    public void Attach(FfiTvSender tvSender, FfiTvBrowser with)
    {
        sender = tvSender;
        browser = with;
        ShortFingerprint = tvSender.ShortFingerprint();
        Refresh();
        Watch(tvSender);
    }

    async void Watch(FfiTvSender watched)
    {
        while (sender == watched)
        {
            await watched.Changed();
            if (sender == watched)
                Refresh();
        }
    }

    public void Refresh()
    {
        if (sender is null)
            return;
        Rows = sender.Rows();
        Paired = sender.Paired();
        Say();
    }

    // Play on TV.

    /// <summary>
    /// Play on TV, from a title, an episode or the player: straight to the TV driven already,
    /// otherwise the picker to choose one.
    /// </summary>
    public void Offer(TvPlay request)
    {
        if (!Available)
            return;
        Problem = null;
        Request = request;
        if (Connected is { } fingerprint && Live)
            _ = PlayOn(fingerprint);
        else
            Sheet = TvSheet.Picker;
        Say();
    }

    /// <summary>The picker, to pair or connect, with nothing to play.</summary>
    public void OpenPicker()
    {
        if (!Available)
            return;
        Request = null;
        Problem = null;
        Sheet = TvSheet.Picker;
        Say();
    }

    /// <summary>The card of the TV driven, or the picker when there is none.</summary>
    public void OpenRemote()
    {
        Sheet = Connected is null ? TvSheet.Picker : TvSheet.Remote;
        Say();
    }

    /// <summary>The dialog closed. A pairing under way is let go; the link stays, for the card to come back to.</summary>
    public void SheetClosed()
    {
        Sheet = null;
        Request = null;
        CancelPairing();
        // A session the TV ended while its card was up ends here as the card goes.
        if (Card?.Over == true)
            Disconnect();
        Say();
    }

    /// <summary>Look for TVs while a picker is up. Counted, as Settings and the dialog can both be up.</summary>
    public void StartBrowsing()
    {
        if (sender is null || browser is null)
            return;
        if (++browsers != 1)
            return;
        Browsing = true;
        sender.Browse(browser);
        Refresh();
    }

    public void StopBrowsing()
    {
        if (sender is null || browsers == 0)
            return;
        if (--browsers != 0)
            return;
        Browsing = false;
        sender.StopBrowsing();
        Refresh();
    }

    /// <summary>A row chosen: a paired TV is connected to and plays what was asked; a new one starts pairing.</summary>
    public void Choose(FfiTvRow row)
    {
        switch (row.Kind)
        {
            case FfiTvRowKind.Paired or FfiTvRowKind.PairedAway when row.Fingerprint is { } fingerprint:
                Use(fingerprint);
                break;
            case FfiTvRowKind.New:
                _ = Begin(row.Key);
                break;
        }
    }

    /// <summary>
    /// Pair from <paramref name="target"/>: a row's key, a TV's pairing link or an address typed
    /// in. A TV asked for a code shows one; a link that carried the code pairs at once.
    /// </summary>
    public async Task Begin(string target)
    {
        if (sender is null)
            return;
        CancelPairing();
        var mine = attempt;
        Sheet = TvSheet.Picker;
        Pairing = new TvPairingStep("", NeedsCode: true, Working: true);
        Say();
        try
        {
            var started = await sender.Pair(target);
            if (mine != attempt)
                return;
            steps = started;
            Pairing = new TvPairingStep(started.TvName(), started.NeedsCode(), Working: true);
            Say();
            if (started.NeedsCode())
            {
                await started.RequestCode();
                if (mine != attempt)
                    return;
                Pairing = Pairing with { Working = false };
                Say();
            }
            else
                await Finish(started, null, mine);
        }
        catch (Exception e)
        {
            Fail(e, mine);
        }
    }

    /// <summary>The six digits typed.</summary>
    public async Task Submit(string code)
    {
        if (steps is not { } current || Pairing is not { Working: false } step)
            return;
        var mine = attempt;
        Pairing = step with { Working = true, Problem = null, Expired = false };
        Say();
        try
        {
            await Finish(current, code, mine);
        }
        catch (Exception e)
        {
            Fail(e, mine);
        }
    }

    /// <summary>Ask the TV for a code again, as after one ran out.</summary>
    public async Task RequestCodeAgain()
    {
        if (steps is not { } current || Pairing is not { } step)
            return;
        var mine = attempt;
        Pairing = step with { Working = true, Problem = null, Expired = false };
        Say();
        try
        {
            await current.RequestCode();
            if (mine != attempt)
                return;
            Pairing = Pairing with { Working = false };
            Say();
        }
        catch (Exception e)
        {
            Fail(e, mine);
        }
    }

    public void CancelPairing()
    {
        attempt++;
        if (Pairing is null && steps is null)
            return;
        Pairing = null;
        steps = null;
        Say();
    }

    async Task Finish(FfiTvPairing pairing, string? code, int mine)
    {
        var tv = await pairing.Pair(code);
        if (mine != attempt)
            return;
        CancelPairing();
        Refresh();
        Use(tv.Fingerprint);
    }

    void Fail(Exception e, int mine)
    {
        if (mine != attempt || Pairing is null)
            return;
        Pairing = Pairing with
        {
            Working = false,
            Problem = TvWords.Of(e),
            Expired = e is FfiTvException.Failed { problem: FfiTvProblem.NoCode },
        };
        Say();
    }

    // The link.

    /// <summary>Connect to a paired TV, then play what was asked on it, or show its card.</summary>
    public void Use(string fingerprint)
    {
        Connect(fingerprint);
        if (Request is not null)
            _ = PlayOn(fingerprint);
        else
        {
            Sheet = TvSheet.Remote;
            Say();
        }
    }

    /// <summary>Hold a link to the TV, reading its card until it closes.</summary>
    public void Connect(string fingerprint)
    {
        if (sender is null || (Connected == fingerprint && link is not null))
            return;
        Disconnect();
        if (sender.Link(fingerprint) is not { } opened)
            return;
        link = opened;
        Connected = fingerprint;
        Take(opened.Card());
        Read(opened);
    }

    async void Read(FfiTvLink reading)
    {
        while (await reading.NextCard() is { } card)
        {
            if (link != reading)
                return;
            Take(card);
            // The engine says when the session is over: the TV stopped playing, or stayed away
            // too long. The card stays while it is up, to say so.
            if (card.Over && Sheet != TvSheet.Remote)
                Disconnect();
        }
    }

    /// <summary>
    /// The card the TV sent. The playhead moves on from where and when the TV last said it: a
    /// card that only says the link changed keeps that moment, so the line does not jump back,
    /// and stops where the link was lost.
    /// </summary>
    public void Take(FfiRemoteCard next)
    {
        // Rev counts what the TV said; the arrays in the card would make record equality false every time.
        var same = Card is { } was && was.Rev == next.Rev && (was.Now is null) == (next.Now is null);
        if (next.Phase == FfiLinkPhase.Connected)
            LostAt = null;
        else if (LostAt is null && Card?.Phase == FfiLinkPhase.Connected)
            LostAt = clock();
        Card = next;
        if (!same)
            CardAt = clock();
        if (Starting is { } asked && next.Now is { } now && now.Id == asked.Id && now.Episode == asked.Episode)
            Starting = null;
        if (next.Over)
            Starting = null;
        Say();
    }

    /// <summary>What the TV plays, as the card shows it: what was asked while it starts, else what the TV last said.</summary>
    public FfiNowCard? Now => Starting is not { } asked
        ? Card?.Now
        : new FfiNowCard(
            Id: asked.Id, Title: asked.Title, Episode: asked.Episode, EpisodeLine: T.EpisodeLabel(asked.Episode),
            Loading: true, Position: asked.From ?? 0, Duration: null, Paused: false, Speed: 1,
            Volume: Card?.Now?.Volume, Audio: [], Subtitles: [], SubtitlesOff: false, Skip: null);

    /// <summary>Try the TV again now rather than after the link's wait: the network back, or the window in front.</summary>
    public void RetryNow() => link?.RetryNow();

    async Task PlayOn(string fingerprint)
    {
        if (Request is not { } request || link is not { } held || Connected != fingerprint)
            return;
        Request = null;
        Starting = request;
        Sheet = TvSheet.Remote;
        Say();
        try
        {
            await held.Play(request.Id, request.Title, request.Episode, request.From);
            // The TV playing it already sends nothing new.
            if (Starting == request && Card?.Now is { } now && now.Id == request.Id && now.Episode == request.Episode)
                Starting = null;
            Say();
            if (request.FromPlayer)
                HandedOff?.Invoke();
        }
        catch (Exception e)
        {
            if (Starting == request)
                Starting = null;
            Problem = TvWords.Of(e);
            Say();
        }
    }

    /// <summary>Stop driving the TV. What it plays plays on.</summary>
    public void Disconnect()
    {
        var held = link;
        link = null;
        held?.HangUp();
        Connected = null;
        Card = null;
        Starting = null;
        LostAt = null;
        Say();
    }

    /// <summary>Send the TV a command from the card.</summary>
    public async Task Send(FfiTvCommand command)
    {
        if (link is not { } held)
            return;
        Problem = null;
        Say();
        try
        {
            await held.Send(command);
        }
        catch (Exception e)
        {
            Problem = TvWords.Of(e);
            Say();
        }
    }

    /// <summary>Forget a paired TV, on both sides when it can be reached.</summary>
    public async Task Forget(string fingerprint)
    {
        if (sender is null)
            return;
        if (Connected == fingerprint)
        {
            Disconnect();
            if (Sheet == TvSheet.Remote)
                Sheet = null;
        }
        Paired = [.. Paired.Where(p => p.Fingerprint != fingerprint)];
        Say();
        await sender.Forget(fingerprint);
        Refresh();
    }

    /// <summary>Pair again after the TV let this PC go: forget it here, then the picker.</summary>
    public async Task PairAgain()
    {
        if (Connected is { } fingerprint)
            await Forget(fingerprint);
        OpenPicker();
    }

    /// <summary>Where the TV's playhead is now: moved on from the card on this PC's clock while it plays.</summary>
    public double? Playhead(DateTimeOffset? at = null)
    {
        if (Starting is not null || Card?.Now is not { } now)
            return null;
        var date = at ?? clock();
        var until = LostAt is { } lost && lost < date ? lost : date;
        return AnistreamFfiMethods.TvPlayhead(now, Math.Max(0, (until - CardAt).TotalSeconds));
    }
}

/// <summary>What Play on TV says, from the shared catalogue.</summary>
public static class TvWords
{
    /// <summary>
    /// What a TV lists this PC as: the kind of device, never the name it was given, which is often
    /// its owner's. Sent to the TV, which shows it as it is, so it is not translated here.
    /// </summary>
    public const string Device = "PC";

    public static string PickerTitle(TvPlay? request) => request is null ? T.TvSheetTitlePair : T.TvPlayOn;

    public static string PickerLine(TvPlay? request) => request switch
    {
        null => T.TvOpenOnTv,
        { Title.Length: 0 } => T.EpisodeLabel(request.Episode),
        _ => T.TvPickerEpisodeOf(request.Title, request.Episode),
    };

    public static string PairingTitle(TvPairingStep step) =>
        step.Typing ? T.TvPairWith(step.TvName) : step.TvName.Length == 0 ? T.TvConnectingToTv : T.TvPairingWith(step.TvName);

    public static string PairingLine(TvPairingStep step) =>
        step.Typing ? T.TvTypeCode(step.TvName) : T.TvPairingMoment;

    public static string Looking => T.TvLooking;
    public static string NoneFound => T.TvNoneFound;

    /// <summary>What to check when no TV answers. The picker's line says it already when it is pairing, with nothing to play.</summary>
    public static string NoneLine(TvPlay? request) => request is null
        ? T.TvNoneFoundShort
        : T.TvNoneFoundDetail;

    public static string Address => T.TvAddressHint;
    public static string Nothing => T.TvAfterPairing;
    public static string StartingLine => T.TvStarting;

    /// <summary>
    /// What to say when the TV link fails: the catalogue's line for the engine's problem.
    /// </summary>
    public static string Of(Exception e) => e is FfiTvException.Failed failed ? failed.problem switch
    {
        FfiTvProblem.WrongCode => T.TvProblemWrongCode,
        FfiTvProblem.NoCode => T.TvProblemNoCode,
        FfiTvProblem.Full => T.TvProblemFull,
        FfiTvProblem.Busy => T.TvProblemBusy,
        FfiTvProblem.Version => T.TvProblemVersion,
        FfiTvProblem.NotNow => T.TvProblemNotNow,
        FfiTvProblem.Unpaired => T.TvProblemUnpaired,
        FfiTvProblem.KeyChanged => T.TvProblemKeyChanged,
        FfiTvProblem.Unreachable => T.TvProblemUnreachable,
        FfiTvProblem.Gone => T.TvProblemGone,
        FfiTvProblem.NotALink => T.TvProblemNotALink,
        _ => T.TvTalkFailed,
    } : T.TvTalkFailed;

    /// <summary>A track as the card names it: its language in this PC's language, and its title.</summary>
    public static string Track(FfiTvTrack track, CultureInfo? culture = null)
    {
        var language = Language(track.Language, culture ?? CultureInfo.CurrentUICulture);
        return (language, track.Title) switch
        {
            ({ } l, { Length: > 0 } t) => T.ShowSettingsTrackWith(l, t),
            ({ } l, _) => l,
            (null, { Length: > 0 } t) => t,
            _ => T.TvTrackN(track.Id),
        };
    }

    static string? Language(string? code, CultureInfo culture)
    {
        if (code is not { Length: > 0 })
            return null;
        try
        {
            var name = CultureInfo.GetCultureInfo(code).DisplayName;
            return string.Equals(name, code, StringComparison.OrdinalIgnoreCase) || name.StartsWith("Unknown", StringComparison.Ordinal) ? null : name;
        }
        catch (CultureNotFoundException)
        {
            return null;
        }
    }

    public static string Chosen(FfiTvTrack[] tracks, bool off) =>
        off ? T.ValueOff : tracks.FirstOrDefault(t => t.Selected) is { } on ? Track(on) : T.ValueNone;

    public static string? Volume(FfiNowCard now) =>
        now.Volume is { } v ? T.TvVolumeOf((long)Math.Round(v)) : null;

    public static string Status(FfiRemoteCard? card) => card?.PhaseLine ?? T.TvStatusConnected;

    /// <summary>
    /// What the title bar's button says: the TV and what it plays while the link is up, and how
    /// the link stands while it is not.
    /// </summary>
    public static string BarLine(FfiRemoteCard card, bool playing) => card.Phase switch
    {
        FfiLinkPhase.Connected => playing ? T.TvStatusPlaying(card.TvName) : card.TvName,
        FfiLinkPhase.Connecting => T.TvStatusConnecting(card.TvName),
        FfiLinkPhase.Retrying => T.TvStatusReconnecting(card.TvName),
        _ => T.TvPairAgainWith(card.TvName),
    };

    /// <summary>Pairing's way to a fresh code, the one way on once the code ran out.</summary>
    public static string NewCode => T.ActionNewCode;

    /// <summary>The TV let this PC go: Pair again in place of Choose another TV.</summary>
    public static bool Lost(FfiRemoteCard? card) => card?.Phase is FfiLinkPhase.Unpaired or FfiLinkPhase.KeyChanged;

    /// <summary>The dot beside the link's line: water while connected, vermilion when the TV let this PC go, haze while it connects.</summary>
    public static Dot DotFor(FfiLinkPhase? phase) => phase switch
    {
        FfiLinkPhase.Connected => Dot.Water,
        FfiLinkPhase.Unpaired or FfiLinkPhase.KeyChanged => Dot.Vermilion,
        _ => Dot.Haze,
    };

    public enum Dot
    {
        Water,
        Vermilion,
        Haze,
    }

    /// <summary>What Settings says under a paired TV's name.</summary>
    public static string Note(FfiPairedTv tv, bool connected, DateTimeOffset now, CultureInfo? culture = null)
    {
        if (connected)
            return T.TvConnectedAt(tv.Address);
        var paired = T.TvPairedOn(DateTimeOffset.FromUnixTimeSeconds(tv.PairedAt).ToLocalTime().ToString("d MMM", culture ?? CultureInfo.CurrentCulture));
        return tv.LastSeen is { } seen ? T.TvPairedSeen(paired, Wording.Ago(Math.Max(0, now.ToUnixTimeSeconds() - seen))) : paired;
    }

    public static string Footnote(string shortFingerprint) =>
        T.TvDeviceCode(shortFingerprint);

    public static string PairNote => T.TvNonePairedDetail;

    /// <summary>The six digits typed, once there are six.</summary>
    public static string? SixDigits(string typed)
    {
        var digits = new string([.. typed.Where(char.IsAsciiDigit)]);
        return digits.Length == 6 ? digits : null;
    }
}
