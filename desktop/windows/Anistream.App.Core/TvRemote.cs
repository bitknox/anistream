using System.Globalization;
using Anistream.Engine.Ffi;
using Anistream.Engine.Model;

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
/// <summary>Where a cast is: starting, playing, or ended in a way the card keeps and explains.</summary>
public enum CastStage
{
    Starting,
    Playing,
    /// <summary>It never started, or the TV stopped it on a stream: the card says why.</summary>
    Failed,
    /// <summary>The TV dropped off the network mid-cast.</summary>
    Lost,
}

/// <summary>What a cast's card offers once it has ended.</summary>
public enum CastOffer
{
    ChooseSource,
    PlayHere,
    TryAgain,
    ChooseAnother,
}

public enum TvSheet
{
    Picker,
    Remote,
    /// <summary>The card of the TV with Chromecast built in this PC casts to.</summary>
    Cast,
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
    FfiCastBrowser? castBrowser;
    FfiCastSession? cast;
    FfiTvPairing? steps;
    FfiTvLink? link;
    int browsers, attempt;

    public event Action? Changed;

    /// <summary>The dialog up, if any.</summary>
    public TvSheet? Sheet { get; private set; }
    /// <summary>What choosing a TV plays. Null when the picker was opened to pair or connect only.</summary>
    public TvPlay? Request { get; private set; }
    public FfiTvRow[] Rows { get; private set; } = [];
    /// <summary>The TVs with Chromecast built in the browse found, for the picker's Chromecast group.</summary>
    public FfiCastRow[] CastRows { get; private set; } = [];
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

    /// <summary>
    /// Takes the engine's sender, browsing with <paramref name="with"/> while a picker is up, and
    /// for TVs with Chromecast built in with <paramref name="casts"/>.
    /// </summary>
    public void Attach(FfiTvSender tvSender, FfiTvBrowser with, FfiCastBrowser? casts = null)
    {
        sender = tvSender;
        browser = with;
        castBrowser = casts;
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
        CastRows = castBrowser is null ? [] : sender.CastRows();
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

    /// <summary>The card of the cast or the TV driven, or the picker when there is neither.</summary>
    public void OpenRemote()
    {
        if (cast is not null)
            CastReopened();
        Sheet = cast is not null ? TvSheet.Cast : Connected is null ? TvSheet.Picker : TvSheet.Remote;
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
        CastSheetClosed();
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
        if (castBrowser is not null)
            sender.BrowseCast(castBrowser);
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

    // Casting to a TV with Chromecast built in.

    /// <summary>The cast's card as the engine last said it, while there is a cast or its ending is shown.</summary>
    public FfiCastCard? CastCard { get; private set; }
    /// <summary>When the card's playhead was last said, for the playhead to move on from.</summary>
    public DateTimeOffset CastCardAt { get; private set; }
    /// <summary>When the TV was picked, for the starting line's seconds.</summary>
    public DateTimeOffset CastPicked { get; private set; }
    /// <summary>What the cast plays, for Play here and the card while the TV starts.</summary>
    public TvPlay? CastPlay { get; private set; }
    /// <summary>The TV the cast went to, for Try again and the card's lines.</summary>
    public FfiCastRow? CastRow { get; private set; }
    /// <summary>A cast has reached a TV from this PC once: Windows' firewall has asked by then.</summary>
    public bool CastServed { get; set; }

    /// <summary>The cast's TV first played: the window keeps that, so the firewall note goes.</summary>
    public Action? Served { get; set; }
    /// <summary>Someone else took the TV: the card closed, and the window offers Play here.</summary>
    public Action<TvPlay, Msg>? TakenOver { get; set; }

    bool castPlayed;
    bool castReopened;
    /// <summary>What the TV last said it played, kept where it stopped once a card comes without it.</summary>
    FfiNowCard? castLastNow;

    /// <summary>A cast is running, or its ending is shown on the card and the title bar.</summary>
    public bool Casting => cast is not null;

    /// <summary>
    /// A Chromecast row chosen: cast what was asked to it, and show its card at once. A player
    /// that asked closes once the TV plays it.
    /// </summary>
    public void Cast(FfiCastRow row)
    {
        if (sender is null || Request is not { } request)
            return;
        Problem = null;
        try
        {
            var session = sender.Cast(row.Key, request.Id, request.Episode, request.From);
            DropCast();
            cast = session;
            CastPlay = request;
            CastRow = row;
            CastPicked = clock();
            castPlayed = false;
            castReopened = false;
            Request = null;
            Sheet = TvSheet.Cast;
            TakeCast(session.Card());
            ReadCast(session);
        }
        catch (Exception e)
        {
            Problem = TvWords.Of(e);
            Say();
        }
    }

    async void ReadCast(FfiCastSession reading)
    {
        try
        {
            while (await reading.NextCard() is { } card)
            {
                if (cast != reading)
                    return;
                TakeCast(card);
            }
        }
        catch (Exception e)
        {
            AnistreamFfiMethods.LogLine(FfiLogLevel.Warn, "cast", $"the cast's card stopped: {e.Message}");
        }
    }

    /// <summary>
    /// The card the engine sent. Someone else taking the TV closes it, with Play here offered by
    /// the window; a cast stopped without a reason goes; any other ending stays on the card.
    /// </summary>
    public void TakeCast(FfiCastCard next)
    {
        var was = CastCard?.Card.Now;
        var now = next.Card.Now;
        // A TV that dropped off sends no playhead: the card keeps the one it had, where it got to.
        if (now is not null)
            castLastNow = now;
        else if (was is not null && Stage == CastStage.Playing)
            castLastNow = was with { Position = CastPlayhead() ?? was.Position, Paused = true };
        if (was is null || now is null || was.Position != now.Position || was.Paused != now.Paused || was.Loading != now.Loading)
            CastCardAt = clock();
        CastCard = next;
        if (next.Card.Over && next.Notice is Msg.NoticeCastTakenOver taken && CastPlay is { } play)
        {
            DropCast();
            Say();
            TakenOver?.Invoke(play, taken);
            return;
        }
        if (next.Card.Over && next.Notice is null)
        {
            DropCast();
            Say();
            return;
        }
        if (!castPlayed && Stage == CastStage.Playing)
        {
            castPlayed = true;
            if (!CastServed)
            {
                CastServed = true;
                Served?.Invoke();
            }
            if (CastPlay?.FromPlayer == true)
                HandedOff?.Invoke();
        }
        Say();
    }

    /// <summary>How the cast stands, for the card's line, dot and buttons.</summary>
    public CastStage Stage => TvWords.StageOf(CastCard);

    /// <summary>What the TV plays, as the card shows it: what was asked while it starts.</summary>
    public FfiNowCard? CastNow
    {
        get
        {
            var told = CastCard?.Card.Now ?? castLastNow;
            if (told is not null && (Stage is CastStage.Playing or CastStage.Lost || CastPlay is null))
                return told;
            if (CastPlay is not { } asked)
                return told;
            return new FfiNowCard(
                Id: asked.Id, Title: asked.Title, Episode: asked.Episode,
                EpisodeLine: told?.EpisodeLine is { Length: > 0 } line ? line : T.EpisodeLabel(asked.Episode),
                Loading: Stage == CastStage.Starting, Position: told?.Position ?? asked.From ?? 0, Duration: told?.Duration,
                Paused: false, Speed: 1, Volume: told?.Volume, Audio: told?.Audio ?? [], Subtitles: told?.Subtitles ?? [],
                SubtitlesOff: told?.SubtitlesOff ?? false, Skip: null);
        }
    }

    /// <summary>Where the TV's playhead is now: moved on from the card on this PC's clock while it plays.</summary>
    public double? CastPlayhead(DateTimeOffset? at = null)
    {
        if ((CastCard?.Card.Now ?? castLastNow) is not { } now)
            return null;
        if (Stage != CastStage.Playing)
            return now.Position;
        return AnistreamFfiMethods.TvPlayhead(now, Math.Max(0, ((at ?? clock()) - CastCardAt).TotalSeconds));
    }

    /// <summary>The seconds since the TV was picked, for the starting line's counter.</summary>
    public TimeSpan CastWaited(DateTimeOffset? at = null) => (at ?? clock()) - CastPicked;

    /// <summary>The TV's name as the card says it.</summary>
    public string CastName => CastCard?.Card.TvName is { Length: > 0 } name ? name : CastRow?.Name ?? "";

    /// <summary>A control on the cast's card.</summary>
    public void SendCast(FfiTvCommand command) => cast?.Command(command);

    /// <summary>Play here: the cast stops, and the episode is the window's to play on this PC from where history has it.</summary>
    public TvPlay? PlayHere()
    {
        var play = CastPlay;
        var held = cast;
        var ended = Stage is CastStage.Failed or CastStage.Lost;
        DropCast();
        if (!ended)
            held?.PlayHere();
        Say();
        return play;
    }

    /// <summary>Stop casting from the card. The person chose it, so there is nothing to say.</summary>
    public void StopCasting()
    {
        var held = cast;
        DropCast();
        held?.Stop();
        Say();
    }

    /// <summary>Try again: the same episode to the same TV.</summary>
    public void CastAgain()
    {
        if (CastPlay is not { } play || CastRow is not { } row)
            return;
        DropCast();
        Request = play;
        Cast(row);
    }

    /// <summary>Choose another TV: the picker, with the same episode to cast.</summary>
    public void CastElsewhere()
    {
        var play = CastPlay;
        DropCast();
        Request = play;
        Sheet = TvSheet.Picker;
        Say();
    }

    /// <summary>Choose source: the card goes, and the episode is the window's to list sources for.</summary>
    public TvPlay? CastChooseSource()
    {
        var play = CastPlay;
        DropCast();
        Say();
        return play;
    }

    /// <summary>The card is opened again from the title bar.</summary>
    void CastReopened() => castReopened = cast is not null;

    /// <summary>
    /// The dialog closed with a cast on it: running, it goes on. One that ended stays for the
    /// title bar to say so, until its card has been opened again and closed.
    /// </summary>
    void CastSheetClosed()
    {
        if (cast is not null && Stage is CastStage.Failed or CastStage.Lost && castReopened)
            DropCast();
    }

    void DropCast()
    {
        cast = null;
        CastCard = null;
        CastPlay = null;
        CastRow = null;
        castPlayed = false;
        castReopened = false;
        castLastNow = null;
        if (Sheet == TvSheet.Cast)
            Sheet = null;
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

    // Casting.

    /// <summary>A seconds counter joins the starting line after this long; under it, it is only noise.</summary>
    public static readonly TimeSpan CountAfter = TimeSpan.FromSeconds(4);

    /// <summary>How a cast stands by its card: the engine's reason once it ended, else whether the TV shows a picture.</summary>
    public static CastStage StageOf(FfiCastCard? card) => card switch
    {
        null => CastStage.Starting,
        { Notice: Msg.NoticeCastLost } => CastStage.Lost,
        { Notice: not null } => CastStage.Failed,
        { Card: { Phase: FfiLinkPhase.Connected, Now: { Loading: false, Duration: not null } } } => CastStage.Playing,
        _ => CastStage.Starting,
    };

    /// <summary>
    /// A notice the casting card says itself, with the reason under the episode, or the window's
    /// own notice for a TV someone else took: the engine's toast for it is left out.
    /// </summary>
    public static bool OnCastCard(Msg? notice) => notice is Msg.NoticeCastVideoCodec or Msg.NoticeCastAudioCodec
        or Msg.NoticeCastContainer or Msg.NoticeCastDeepLink or Msg.NoticeCastNeverFetches or Msg.NoticeCastRefused
        or Msg.NoticeCastTakenOver or Msg.NoticeCastNotSameNetwork or Msg.NoticeCastLost;

    /// <summary>What a card that ended offers, by the engine's reason.</summary>
    public static CastOffer[] OffersFor(Msg? notice) => notice switch
    {
        Msg.NoticeCastRefused or Msg.NoticeCastVideoCodec or Msg.NoticeCastAudioCodec or Msg.NoticeCastContainer =>
            [CastOffer.ChooseSource, CastOffer.PlayHere],
        Msg.NoticeCastNeverFetches => [CastOffer.TryAgain, CastOffer.PlayHere],
        Msg.NoticeCastDeepLink => [CastOffer.ChooseSource],
        _ => [CastOffer.PlayHere, CastOffer.ChooseAnother],
    };

    /// <summary>
    /// The line while a cast starts: the engine's phase line, else "Starting on Living room",
    /// with the seconds once it has taken <see cref="CountAfter"/>.
    /// </summary>
    public static string CastStarting(FfiCastCard? card, string name, TimeSpan waited)
    {
        var line = card?.Card.PhaseLine is { Length: > 0 } phase ? phase : T.TvStartingOn(name);
        return waited >= CountAfter ? T.TvCastCounting(line, (long)Math.Floor(waited.TotalSeconds)) : line;
    }

    /// <summary>The card's header line for a cast, as its stage has it.</summary>
    public static string CastStatus(CastStage stage, FfiCastCard? card, string name, TimeSpan waited) => stage switch
    {
        CastStage.Playing => T.TvStatusPlaying(name),
        CastStage.Failed => T.TvCastCouldntStart(name),
        CastStage.Lost => T.TvCastStopped,
        _ => CastStarting(card, name, waited),
    };

    /// <summary>The dot beside a cast's line: water while it plays, vermilion once it ended, haze while it starts.</summary>
    public static Dot CastDot(CastStage stage) => stage switch
    {
        CastStage.Playing => Dot.Water,
        CastStage.Failed or CastStage.Lost => Dot.Vermilion,
        _ => Dot.Haze,
    };

    /// <summary>What a Chromecast row says under its name, until the engine knows the route before a pick.</summary>
    public static string CastRoute(FfiCastRow row) => T.TvCastThroughPc;

    /// <summary>The TV volume row's value: the level for a TV that has one, "Set on the TV" for one that doesn't.</summary>
    public static string CastVolume(FfiCastCard card) => card.VolumeControl == FfiVolumeControl.MuteOnly || card.Card.Now?.Volume is null
        ? T.TvCastVolumeOnTv
        : T.TvVolumeOf((long)Math.Round(card.Card.Now.Volume.Value));

    /// <summary>The mute button's word: Unmute while the TV is muted.</summary>
    public static string MuteWord(bool muted) => muted ? T.TvCastUnmute : T.TvCastMute;

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
