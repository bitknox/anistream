//! Casting to a TV with Chromecast built in, as the picker and the card read it. The engine casts
//! (`CastSession`): this holds the cast running, what it last said, the line that counts while
//! it starts, and what the card offers when it ends badly. The card is the remote's face, drawn
//! from [`CastView`] in place of the link's card.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anistream_core::ids::AnilistId;
use anistream_engine::{
    cast::{CastHub, protocol::VolumeControl},
    engine::RemoteSender,
    remote::{
        protocol::command::RemoteCommand,
        sender::{CastCard, CastRow, CastSession, LinkPhase, NowCard},
    },
};
use anistream_i18n::t;
use anistream_model::Msg;
use gtk::glib;

use super::{Request, Sheet, Tv};

/// What the sender casts through: the engine's hub, and where its commands go.
pub struct Casting {
    pub hub: Arc<CastHub>,
    pub engine: RemoteSender,
}

/// A seconds counter joins the starting line after this long, as under that it is only noise.
pub const COUNT_AFTER: Duration = Duration::from_secs(4);

/// Where a cast is: starting, playing, or ended in a way the card keeps and explains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Starting,
    Playing,
    /// It never started, or the TV stopped it on a stream: the card says why.
    Failed,
    /// The TV dropped off the network mid-cast.
    Lost,
}

impl Stage {
    pub fn ended(self) -> bool {
        matches!(self, Self::Failed | Self::Lost)
    }
}

/// What a card that has ended offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    ChooseSource,
    PlayHere,
    TryAgain,
    ChooseAnother,
}

/// The cast as the card draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct CastView {
    pub name: String,
    pub stage: Stage,
    /// The line while it starts, with its seconds once it takes a while.
    pub starting: String,
    pub now: Option<NowCard>,
    /// The engine's words for why it ended.
    pub notice: Option<Msg>,
    pub offers: Vec<Offer>,
    pub volume: VolumeControl,
    pub muted: bool,
}

/// The cast running now, and what it last said.
pub struct CastRun {
    session: Arc<CastSession>,
    /// The picker row it was started from, for Try again.
    key: String,
    request: Request,
    card: CastCard,
    asked: Instant,
    /// When the card's playhead was last said, for the seek line.
    card_at: Instant,
    /// It has played once: the player that handed it over has closed.
    played: bool,
    /// The card was opened again after it ended, so closing it now lets it go.
    reopened: bool,
}

/// Whether the TV shows a picture: the engine has said what plays, and the TV has loaded it.
fn playing(card: &CastCard) -> bool {
    card.card.phase == LinkPhase::Connected
        && card.card.now.as_ref().is_some_and(|n| !n.loading && n.duration.is_some())
}

pub fn stage_of(card: &CastCard) -> Stage {
    match &card.notice {
        Some(Msg::NoticeCastLost) => Stage::Lost,
        Some(_) => Stage::Failed,
        None if playing(card) => Stage::Playing,
        None => Stage::Starting,
    }
}

/// What a card that ended offers, by the engine's reason.
pub fn offers_for(notice: &Msg) -> Vec<Offer> {
    match notice {
        Msg::NoticeCastRefused
        | Msg::NoticeCastVideoCodec { .. }
        | Msg::NoticeCastAudioCodec { .. }
        | Msg::NoticeCastContainer => vec![Offer::ChooseSource, Offer::PlayHere],
        Msg::NoticeCastNeverFetches => vec![Offer::TryAgain, Offer::PlayHere],
        Msg::NoticeCastDeepLink => vec![Offer::ChooseSource],
        _ => vec![Offer::PlayHere, Offer::ChooseAnother],
    }
}

/// The starting line: "Starting on Living room", or the engine's phase line, with the seconds
/// once it has taken `COUNT_AFTER`.
fn starting_line(card: &CastCard, waited: Duration) -> String {
    let line = card
        .card
        .phase_line
        .clone()
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| t::tv_starting_on(&card.card.tv_name));
    if waited >= COUNT_AFTER {
        t::tv_cast_counting(&line, waited.as_secs() as i64)
    } else {
        line
    }
}

impl Tv {
    /// The Cast rows of the picker: TVs with Chromecast built in that a browse found.
    pub fn cast_rows(&self) -> Vec<CastRow> {
        self.cast_rows.borrow().clone()
    }

    /// A cast is running, or ended and kept on the card.
    pub fn casting(&self) -> bool {
        self.cast.borrow().is_some()
    }

    /// The TV shows a picture, so the card's controls reach it.
    pub fn cast_playing(&self) -> bool {
        self.cast.borrow().as_ref().is_some_and(|run| stage_of(&run.card) == Stage::Playing)
    }

    pub fn cast_view(&self) -> Option<CastView> {
        let cast = self.cast.borrow();
        let run = cast.as_ref()?;
        let stage = stage_of(&run.card);
        let now = run.card.card.now.clone().or_else(|| Some(self.asked_card(run)));
        Some(CastView {
            name: run.card.card.tv_name.clone(),
            stage,
            starting: starting_line(&run.card, run.asked.elapsed()),
            now,
            notice: run.card.notice.clone(),
            offers: run.card.notice.as_ref().map(offers_for).unwrap_or_default(),
            volume: run.card.volume_control,
            muted: run.card.muted.unwrap_or(false),
        })
    }

    /// What was asked of the TV, for the card to show before the engine has said what plays.
    fn asked_card(&self, run: &CastRun) -> NowCard {
        let request = &run.request;
        NowCard {
            id: request.id,
            title: request.title.clone(),
            episode_line: t::tv_episode(&request.episode),
            episode: request.episode.clone(),
            loading: true,
            position: request.from.unwrap_or(0.0),
            duration: None,
            paused: false,
            speed: 1.0,
            volume: None,
            audio: Vec::new(),
            subtitles: Vec::new(),
            subtitles_off: false,
            skip: None,
        }
    }

    /// Where the TV's playhead is now: moved on from the card on this computer's clock while it
    /// plays, and held where the card last said once it ends.
    pub fn cast_playhead(&self) -> Option<f64> {
        let cast = self.cast.borrow();
        let run = cast.as_ref()?;
        let now = run.card.card.now.as_ref()?;
        if stage_of(&run.card) != Stage::Playing {
            return Some(now.position);
        }
        Some(now.position_after(run.card_at.elapsed().as_secs_f64()))
    }

    /// What the sidebar and the header button say of the cast: the words, the dot, and whether
    /// it is down.
    pub fn cast_standing(&self) -> Option<(String, &'static str, bool)> {
        let view = self.cast_view()?;
        Some(match view.stage {
            Stage::Starting => (view.starting, "haze", false),
            Stage::Playing => (t::tv_status_playing(&view.name), "water", false),
            Stage::Failed => (t::tv_cast_couldnt_start(&view.name), "vermilion", true),
            Stage::Lost => (t::tv_cast_stopped(), "vermilion", true),
        })
    }

    /// A Cast row chosen: the request is cast to that TV, and the card is up at once.
    pub fn choose_cast(&self, row: &CastRow) {
        let Some(request) = self.request.borrow().clone() else { return };
        self.start_cast(row.key.clone(), request);
    }

    fn start_cast(&self, key: String, request: Request) {
        let Some(sender) = self.sender.clone() else { return };
        let started = {
            let _entered = self.handle.enter();
            sender.cast(&key, request.id, request.episode.clone(), request.from)
        };
        let session = match started {
            Ok(session) => session,
            Err(problem) => {
                self.problem.replace(Some(problem));
                self.changed();
                return;
            }
        };
        self.request.replace(None);
        self.problem.replace(None);
        let turn = self.cast_turn.get() + 1;
        self.cast_turn.set(turn);
        let card = session.card();
        self.cast.replace(Some(CastRun {
            session: Arc::clone(&session),
            key,
            request,
            card,
            asked: Instant::now(),
            card_at: Instant::now(),
            played: false,
            reopened: false,
        }));
        self.sheet.set(Some(Sheet::Remote));
        self.watch_cast(&session, turn);
        self.changed();
    }

    /// Read the card again whenever the engine says it may read differently.
    fn watch_cast(&self, session: &CastSession, turn: u64) {
        let mut changes = session.changes();
        let (tx, rx) = async_channel::unbounded::<()>();
        self.handle.spawn(async move {
            while changes.changed().await.is_ok() {
                if tx.send(()).await.is_err() {
                    return;
                }
            }
        });
        let me = self.me.clone();
        glib::spawn_future_local(async move {
            while rx.recv().await.is_ok() {
                // A burst of changes, read once.
                while rx.try_recv().is_ok() {}
                let Some(tv) = me.upgrade() else { return };
                if !tv.read_cast(turn) {
                    return;
                }
            }
        });
    }

    /// Take the engine's card. False once this cast is not the one shown.
    fn read_cast(&self, turn: u64) -> bool {
        if self.cast_turn.get() != turn {
            return false;
        }
        let (stage, notice, hand_off) = {
            let mut cast = self.cast.borrow_mut();
            let Some(run) = cast.as_mut() else { return false };
            let next = run.session.card();
            if next.card.now != run.card.card.now {
                run.card_at = Instant::now();
            }
            run.card = next;
            let stage = stage_of(&run.card);
            let hand_off = stage == Stage::Playing && !run.played && run.request.from_player;
            run.played |= stage == Stage::Playing;
            (stage, run.card.notice.clone(), hand_off)
        };
        if hand_off && let Some(handed_off) = self.handed_off.borrow().as_ref() {
            handed_off();
        }
        match (stage, &notice) {
            // Stopped from the card, or played out: nothing to say.
            (Stage::Starting | Stage::Playing, _) if self.cast_over() => self.finish_cast(),
            (_, Some(Msg::NoticeCastTakenOver)) => self.taken_over(),
            _ => self.changed(),
        }
        true
    }

    /// The engine ended the cast with nothing to say.
    fn cast_over(&self) -> bool {
        self.cast.borrow().as_ref().is_some_and(|run| run.card.card.over)
    }

    /// Let the cast go, and close the card.
    fn finish_cast(&self) {
        self.drop_cast();
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
    }

    fn drop_cast(&self) -> Option<CastRun> {
        self.cast_turn.set(self.cast_turn.get() + 1);
        self.cast.take()
    }

    /// Someone else started something on the TV: the card closes, and a notice offers to carry
    /// on here.
    fn taken_over(&self) {
        let Some(run) = self.drop_cast() else { return };
        let (id, episode) = Self::episode_of(&run);
        self.taken.replace(Some((id, episode)));
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
        if let Some(say) = self.on_taken_over.borrow().as_ref() {
            say(&crate::say::msg(&Msg::NoticeCastTakenOver));
        }
    }

    /// What the notice for a TV taken over offers: the episode, to play on this computer.
    pub fn take_taken(&self) -> Option<(AnilistId, String)> {
        self.taken.take()
    }

    /// Run `f` with the words of the notice when someone else takes the TV.
    pub fn on_taken_over(&self, f: impl Fn(&str) + 'static) {
        self.on_taken_over.replace(Some(Box::new(f)));
    }

    fn episode_of(run: &CastRun) -> (AnilistId, String) {
        run.card.card.now.as_ref().map_or_else(
            || (run.request.id, run.request.episode.clone()),
            |now| (now.id, now.episode.clone()),
        )
    }

    /// Send the TV a command from the card: the engine's playback, or the TV's own volume.
    pub fn cast_send(&self, command: RemoteCommand) {
        if let Some(run) = self.cast.borrow().as_ref() {
            let _entered = self.handle.enter();
            run.session.command(command);
        }
    }

    /// Stop casting from the card. The person chose it, so there is nothing to say.
    pub fn cast_stop(&self) {
        let Some(run) = self.drop_cast() else { return };
        {
            let _entered = self.handle.enter();
            run.session.stop();
        }
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
    }

    /// Play here: the cast stops and the episode is the caller's to play on this computer, from
    /// where the engine kept it in history.
    pub fn cast_play_here(&self) -> Option<(AnilistId, String)> {
        let run = self.drop_cast()?;
        let episode = Self::episode_of(&run);
        if !stage_of(&run.card).ended() {
            let _entered = self.handle.enter();
            run.session.play_here();
        }
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
        Some(episode)
    }

    /// Try again: the same episode to the same TV.
    pub fn cast_try_again(&self) {
        let Some(run) = self.drop_cast() else { return };
        self.start_cast(run.key, run.request);
    }

    /// Choose another TV: the picker, with the same episode to cast.
    pub fn cast_choose_another(&self) {
        let Some(run) = self.drop_cast() else { return };
        self.request.replace(Some(run.request));
        self.sheet.set(Some(Sheet::Picker));
        self.changed();
    }

    /// Choose source: the card goes, and the episode is the caller's to list sources for.
    pub fn cast_choose_source(&self) -> Option<(AnilistId, String)> {
        let run = self.drop_cast()?;
        if self.sheet.get() == Some(Sheet::Remote) {
            self.sheet.set(None);
        }
        self.changed();
        Some(Self::episode_of(&run))
    }

    /// The card is opened again from the header button.
    pub(super) fn cast_reopened(&self) {
        if let Some(run) = self.cast.borrow_mut().as_mut() {
            run.reopened = true;
        }
    }

    /// The sheet closed with a cast on it: running, it goes on. One that ended stays for the
    /// header button to say so, until the card has been opened again and closed.
    pub(super) fn cast_sheet_closed(&self) {
        let dismissed = self
            .cast
            .borrow()
            .as_ref()
            .is_some_and(|run| stage_of(&run.card).ended() && run.reopened);
        if dismissed {
            self.drop_cast();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_that_ended_offers_by_why() {
        use Offer::*;
        let codec = Msg::NoticeCastVideoCodec { codec: "AV1".into() };
        assert_eq!(offers_for(&Msg::NoticeCastRefused), vec![ChooseSource, PlayHere]);
        assert_eq!(offers_for(&codec), vec![ChooseSource, PlayHere]);
        assert_eq!(offers_for(&Msg::NoticeCastContainer), vec![ChooseSource, PlayHere]);
        assert_eq!(offers_for(&Msg::NoticeCastNeverFetches), vec![TryAgain, PlayHere]);
        assert_eq!(offers_for(&Msg::NoticeCastNotSameNetwork), vec![PlayHere, ChooseAnother]);
        assert_eq!(offers_for(&Msg::NoticeCastDeepLink), vec![ChooseSource]);
        assert_eq!(offers_for(&Msg::NoticeCastLost), vec![PlayHere, ChooseAnother]);
    }
}
