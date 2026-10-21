//! Where the work says what happened: one ordered stream, of the events a front end is told and
//! the messages the engine side leaves itself.
//!
//! One channel for both, so a message keeps its place among the events sent around it, as when
//! the messages were events themselves: the engine side handles it after everything sent before
//! it and before everything sent after. Two channels could not promise that, and a second one
//! read only while the first was empty could wait behind a busy one for as long as it stayed busy.
//!
//! The work only ever holds an [`Outbox`], whichever way it is read. The engine's actor reads
//! an inbox and passes the events on; a reader with no engine side to tell anything, the
//! command line or a test, hands the work a plain event channel, made an `Outbox` with `From`.

use anistream_core::ids::AnilistId;
use anistream_model::{event::Event, playback::PlaybackId};
use tokio::sync::mpsc::{self, error::SendError};

/// Work a command or a task leaves for the engine side to do, in its place among the events.
///
/// These rode the event channel as events, and the loop caught them on their way to the
/// interface, which did nothing with two of them. As messages they never reach a front end.
/// What a front end has to know comes to it as events: the providers rows after a reorder, and
/// that the sources are being reloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Internal {
    /// A setting reached disk: bring the copy the background tasks read up to the config in
    /// effect, so the next download to finish honours the value just written.
    SettingsSaved,
    /// The source order changed and the sources did not: put the live chain in the new order.
    /// Without a rebuild, which would restart the torrent session under a playing episode.
    SourcesReordered,
    /// A setting changed which sources should exist: stand the torrent stack down, rebuild the
    /// chain, and pass it on as [`Event::SourcesChanged`].
    SourcesChanged,
}

/// One item of the engine side's inbox, in the order it was sent.
#[derive(Debug)]
pub(crate) enum Envelope {
    /// For the front end, once the engine side has done its part with it.
    Event(Event),
    /// For the front end too, from a playback, named with the playback that said it.
    Played(Played, Event),
    /// For the engine side alone.
    Internal(Internal),
}

/// The playback an event came from: the title and episode it plays, and its number, so two
/// playbacks of one episode are told apart.
///
/// The events a playback says name none of it — a tick of the playhead, the ending — and the
/// window can be playing one episode while the next is found. Named where they are said, the
/// engine side knows whose they are without guessing from what was started last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Played {
    pub serial: PlaybackId,
    pub id: AnilistId,
    pub episode: String,
}

/// What the work tells what happened through: [`Self::send`] an event, as on the event channel
/// it stands in for, or leave the engine side a message with [`Self::internal`].
#[derive(Debug, Clone)]
pub struct Outbox(Route);

#[derive(Debug, Clone)]
enum Route {
    /// The engine side's inbox: events and messages, in one order.
    Inbox(mpsc::UnboundedSender<Envelope>),
    /// The engine side's inbox, for one playback: its events go in named with it.
    Played(mpsc::UnboundedSender<Envelope>, Played),
    /// Events only, read by whoever made the channel.
    Events(mpsc::UnboundedSender<Event>),
}

impl Outbox {
    /// An inbox for the engine side to read, and the outbox the work writes to it through.
    pub(crate) fn inbox() -> (Self, mpsc::UnboundedReceiver<Envelope>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self(Route::Inbox(tx)), rx)
    }

    /// The same outbox for one playback to answer on: what it says reaches the engine side
    /// named with it, in its place among everything else. Events only, as they were, for a
    /// reader with no engine side to tell.
    ///
    /// Without the desktop's player only a test answers as a playback.
    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub(crate) fn for_playback(&self, played: Played) -> Self {
        match &self.0 {
            Route::Inbox(tx) | Route::Played(tx, _) => Self(Route::Played(tx.clone(), played)),
            Route::Events(_) => self.clone(),
        }
    }

    /// Say what happened. As `UnboundedSender::send`: the event comes back when nobody reads
    /// any more, which is how a task long outlived by its reader learns to stop.
    // The channel's own shape, large event and all, so no caller of the channel it stands in
    // for has to change.
    #[allow(clippy::result_large_err)]
    pub fn send(&self, event: Event) -> Result<(), SendError<Event>> {
        match &self.0 {
            Route::Events(tx) => tx.send(event),
            Route::Inbox(tx) => tx.send(Envelope::Event(event)).map_err(|SendError(sent)| {
                let Envelope::Event(event) = sent else { unreachable!("an event was sent") };
                SendError(event)
            }),
            Route::Played(tx, played) => {
                tx.send(Envelope::Played(played.clone(), event)).map_err(|SendError(sent)| {
                    let Envelope::Played(_, event) = sent else {
                        unreachable!("a playback's event was sent")
                    };
                    SendError(event)
                })
            }
        }
    }

    /// Leave the engine side something to do, after everything sent through here so far.
    ///
    /// Refused, with the message back, when nobody reads any more, and by an outbox that
    /// carries events only: whoever reads that one has no engine side to tell.
    pub(crate) fn internal(&self, message: Internal) -> Result<(), SendError<Internal>> {
        match &self.0 {
            Route::Inbox(tx) | Route::Played(tx, _) => {
                tx.send(Envelope::Internal(message)).map_err(|SendError(sent)| {
                    let Envelope::Internal(message) = sent else {
                        unreachable!("a message was sent")
                    };
                    SendError(message)
                })
            }
            Route::Events(_) => Err(SendError(message)),
        }
    }

    /// A handle that does not keep the channel open, as `UnboundedSender::downgrade`.
    pub(crate) fn downgrade(&self) -> WeakOutbox {
        WeakOutbox(match &self.0 {
            Route::Inbox(tx) => WeakRoute::Inbox(tx.downgrade()),
            Route::Played(tx, played) => WeakRoute::Played(tx.downgrade(), played.clone()),
            Route::Events(tx) => WeakRoute::Events(tx.downgrade()),
        })
    }
}

/// Events only, for a reader with no engine side to tell anything: the command line, a probe,
/// a test.
impl From<mpsc::UnboundedSender<Event>> for Outbox {
    fn from(tx: mpsc::UnboundedSender<Event>) -> Self {
        Self(Route::Events(tx))
    }
}

/// An [`Outbox`] that does not keep its channel open.
#[derive(Debug, Clone)]
pub(crate) struct WeakOutbox(WeakRoute);

#[derive(Debug, Clone)]
enum WeakRoute {
    Inbox(mpsc::WeakUnboundedSender<Envelope>),
    Played(mpsc::WeakUnboundedSender<Envelope>, Played),
    Events(mpsc::WeakUnboundedSender<Event>),
}

impl WeakOutbox {
    /// The outbox again, while anything else still holds it open.
    pub(crate) fn upgrade(&self) -> Option<Outbox> {
        Some(Outbox(match &self.0 {
            WeakRoute::Inbox(tx) => Route::Inbox(tx.upgrade()?),
            WeakRoute::Played(tx, played) => Route::Played(tx.upgrade()?, played.clone()),
            WeakRoute::Events(tx) => Route::Events(tx.upgrade()?),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_and_messages_come_out_in_the_order_they_went_in() {
        let (outbox, mut inbox) = Outbox::inbox();
        outbox.send(Event::SkipCleared).unwrap();
        outbox.internal(Internal::SettingsSaved).unwrap();
        outbox.send(Event::PlaybackKept).unwrap();
        outbox.internal(Internal::SourcesReordered).unwrap();
        let order: Vec<&str> = std::iter::from_fn(|| inbox.try_recv().ok())
            .map(|envelope| match envelope {
                Envelope::Event(Event::SkipCleared) => "skip cleared",
                Envelope::Event(Event::PlaybackKept) => "kept",
                Envelope::Internal(Internal::SettingsSaved) => "saved",
                Envelope::Internal(Internal::SourcesReordered) => "reordered",
                other => panic!("not sent: {other:?}"),
            })
            .collect();
        assert_eq!(order, ["skip cleared", "saved", "kept", "reordered"]);
    }

    #[test]
    fn a_playback_s_events_come_named_with_it_and_in_their_place() {
        let (outbox, mut inbox) = Outbox::inbox();
        let played =
            Played { serial: PlaybackId::new(3), id: AnilistId::new(7), episode: "2".into() };
        let playing = outbox.for_playback(played.clone());
        outbox.send(Event::SkipCleared).unwrap();
        playing.send(Event::PlaybackKept).unwrap();
        playing.internal(Internal::SettingsSaved).unwrap();
        assert!(matches!(inbox.try_recv(), Ok(Envelope::Event(Event::SkipCleared))));
        assert!(matches!(
            inbox.try_recv(),
            Ok(Envelope::Played(from, Event::PlaybackKept)) if from == played
        ));
        assert!(matches!(inbox.try_recv(), Ok(Envelope::Internal(Internal::SettingsSaved))));

        // A reader of events alone hears the playback as it always did.
        let (tx, mut rx) = mpsc::unbounded_channel();
        Outbox::from(tx).for_playback(played).send(Event::SkipCleared).unwrap();
        assert!(matches!(rx.try_recv(), Ok(Event::SkipCleared)));
    }

    #[test]
    fn a_closed_outbox_hands_back_what_was_sent() {
        let (outbox, inbox) = Outbox::inbox();
        let weak = outbox.downgrade();
        drop(inbox);
        assert!(matches!(outbox.send(Event::SkipCleared), Err(SendError(Event::SkipCleared))));
        assert_eq!(
            outbox.internal(Internal::SourcesChanged),
            Err(SendError(Internal::SourcesChanged))
        );
        drop(outbox);
        assert!(weak.upgrade().is_none(), "nothing holds it open");

        // One that carries events refuses a message: nobody there would act on it.
        let (tx, mut rx) = mpsc::unbounded_channel();
        let events = Outbox::from(tx);
        events.send(Event::SkipCleared).unwrap();
        assert!(events.internal(Internal::SettingsSaved).is_err());
        assert!(matches!(rx.try_recv(), Ok(Event::SkipCleared)));
        assert!(rx.try_recv().is_err());
    }
}
