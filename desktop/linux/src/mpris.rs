//! MPRIS: the episode playing as GNOME's media controls, the lock screen and media keys see it,
//! on the session bus as `org.mpris.MediaPlayer2.anistream`. "Episode 7 · The Lantern Road" is
//! the title, the show the artist, its banner the art. Play, pause, seek and next or previous
//! episode come back as the player's own commands; Raise brings the window forward.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gtk::{gio, glib, prelude::*};

pub const NAME: &str = "org.mpris.MediaPlayer2.anistream";
const PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT: &str = "org.mpris.MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
/// The two properties a client may set.
const VOLUME: &str = "Volume"; // i18n-skip
const RATE: &str = "Rate"; // i18n-skip

const INTERFACES: &str = r#"<node>
  <interface name="org.mpris.MediaPlayer2">
    <method name="Raise"/>
    <method name="Quit"/>
    <property name="CanQuit" type="b" access="read"/>
    <property name="CanRaise" type="b" access="read"/>
    <property name="HasTrackList" type="b" access="read"/>
    <property name="Identity" type="s" access="read"/>
    <property name="DesktopEntry" type="s" access="read"/>
    <property name="SupportedUriSchemes" type="as" access="read"/>
    <property name="SupportedMimeTypes" type="as" access="read"/>
  </interface>
  <interface name="org.mpris.MediaPlayer2.Player">
    <method name="Next"/>
    <method name="Previous"/>
    <method name="Pause"/>
    <method name="PlayPause"/>
    <method name="Stop"/>
    <method name="Play"/>
    <method name="Seek"><arg direction="in" name="Offset" type="x"/></method>
    <method name="SetPosition">
      <arg direction="in" name="TrackId" type="o"/>
      <arg direction="in" name="Position" type="x"/>
    </method>
    <method name="OpenUri"><arg direction="in" name="Uri" type="s"/></method>
    <signal name="Seeked"><arg name="Position" type="x"/></signal>
    <property name="PlaybackStatus" type="s" access="read"/>
    <property name="Rate" type="d" access="readwrite"/>
    <property name="Metadata" type="a{sv}" access="read"/>
    <property name="Volume" type="d" access="readwrite"/>
    <property name="Position" type="x" access="read"/>
    <property name="MinimumRate" type="d" access="read"/>
    <property name="MaximumRate" type="d" access="read"/>
    <property name="CanGoNext" type="b" access="read"/>
    <property name="CanGoPrevious" type="b" access="read"/>
    <property name="CanPlay" type="b" access="read"/>
    <property name="CanPause" type="b" access="read"/>
    <property name="CanSeek" type="b" access="read"/>
    <property name="CanControl" type="b" access="read"/>
  </interface>
</node>"#; // i18n-skip

/// What the media controls ask the player to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Raise,
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    /// By seconds, back when negative.
    SeekBy(f64),
    /// To a position in seconds.
    SeekTo(f64),
    /// Volume, 0 to 1.
    Volume(f64),
    /// The playback speed, 1 for normal.
    Rate(f64),
}

/// The slowest and fastest speeds the player's menu offers, which the controls may ask for.
pub const RATES: (f64, f64) = (0.5, 2.0);

/// The episode as the controls show it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Track {
    pub show: String,
    /// "Episode 7 · The Lantern Road".
    pub title: String,
    pub art: Option<String>,
}

/// Where playback stands.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Media {
    /// `None` while nothing plays: Stopped, and no metadata.
    pub track: Option<Track>,
    /// Counts the episodes this run has played, for each one's track id.
    pub serial: u64,
    pub paused: bool,
    pub position: f64,
    pub length: Option<f64>,
    /// 0 to 1.
    pub volume: f64,
    /// The playback speed.
    pub rate: f64,
    /// Whether there is an episode after the one playing, and one before it.
    pub has_next: bool,
    pub has_previous: bool,
}

impl Media {
    pub fn status(&self) -> &'static str {
        match (&self.track, self.paused) {
            (None, _) => "Stopped",        // i18n-skip
            (Some(_), true) => "Paused",   // i18n-skip
            (Some(_), false) => "Playing", // i18n-skip
        }
    }

    fn track_id(&self) -> String {
        match self.track {
            Some(_) => format!("/tv/anistream/desktop/episode/{}", self.serial),
            None => "/org/mpris/MediaPlayer2/TrackList/NoTrack".into(),
        }
    }

    /// The track's metadata, `a{sv}`, empty while nothing plays.
    pub fn metadata(&self) -> glib::Variant {
        let dict = glib::VariantDict::new(None);
        if let Some(track) = &self.track {
            let id = glib::variant::ObjectPath::try_from(self.track_id())
                .expect("a track id is an object path");
            dict.insert_value("mpris:trackid", &id.to_variant());
            dict.insert_value("xesam:title", &track.title.to_variant());
            dict.insert_value("xesam:artist", &vec![track.show.clone()].to_variant());
            dict.insert_value("xesam:album", &track.show.to_variant());
            if let Some(length) = self.length {
                dict.insert_value("mpris:length", &micros(length).to_variant());
            }
            if let Some(art) = &track.art {
                dict.insert_value("mpris:artUrl", &art.to_variant());
            }
        }
        dict.end()
    }

    fn player_property(&self, name: &str) -> Option<glib::Variant> {
        let playing = self.track.is_some();
        Some(match name {
            "PlaybackStatus" => self.status().to_variant(), // i18n-skip
            "Rate" => self.rate.to_variant(),               // i18n-skip
            "MinimumRate" => RATES.0.to_variant(),
            "MaximumRate" => RATES.1.to_variant(),
            "Metadata" => self.metadata(),        // i18n-skip
            "Volume" => self.volume.to_variant(), // i18n-skip
            "Position" => micros(self.position).to_variant(),
            "CanGoNext" => (playing && self.has_next).to_variant(), // i18n-skip
            "CanGoPrevious" => (playing && self.has_previous).to_variant(), // i18n-skip
            "CanPlay" | "CanPause" => playing.to_variant(),         // i18n-skip
            "CanSeek" => (playing && self.length.is_some()).to_variant(), // i18n-skip
            "CanControl" => true.to_variant(),
            _ => return None,
        })
    }
}

/// The properties a new track or its end changes.
const EVERYTHING: [&str; 7] = [
    "Metadata",       // i18n-skip
    "PlaybackStatus", // i18n-skip
    "CanGoNext",      // i18n-skip
    "CanGoPrevious",  // i18n-skip
    "CanPlay",        // i18n-skip
    "CanPause",       // i18n-skip
    "CanSeek",        // i18n-skip
];

/// The desktop entry the controls take the app's name and icon from: the AppImage's own when
/// this runs as one, as its AppRun writes it.
fn desktop_entry() -> String {
    let appimage =
        std::env::var_os("APPIMAGE").is_some_and(|p| std::path::Path::new(&p).is_file());
    if appimage { format!("{}-appimage", crate::APP_ID) } else { crate::APP_ID.to_owned() }
}

fn micros(seconds: f64) -> i64 {
    (seconds.max(0.0) * 1_000_000.0) as i64
}

/// The MPRIS object, once the bus has it.
pub struct Mpris {
    connection: RefCell<Option<gio::DBusConnection>>,
    media: RefCell<Media>,
    owner: Cell<Option<gio::OwnerId>>,
}

impl Mpris {
    /// Ask the session bus for `name` and answer on it, sending what the controls ask to
    /// `act`. Without a session bus there are no media controls, and nothing else changes.
    pub fn serve(name: &str, act: impl Fn(Action) + 'static) -> Rc<Self> {
        let mpris = Rc::new(Self {
            connection: RefCell::default(),
            media: RefCell::new(Media { volume: 1.0, rate: 1.0, ..Media::default() }),
            owner: Cell::new(None),
        });
        let act: Rc<dyn Fn(Action)> = Rc::new(act);
        let weak = Rc::downgrade(&mpris);
        let owner = gio::bus_own_name(
            gio::BusType::Session,
            name,
            gio::BusNameOwnerFlags::NONE,
            move |connection, _| {
                let Some(mpris) = weak.upgrade() else { return };
                if let Err(error) = mpris.register(&connection, Rc::clone(&act)) {
                    tracing::warn!(%error, "no media controls: registering MPRIS failed");
                    return;
                }
                mpris.connection.replace(Some(connection));
            },
            |_, name| tracing::debug!(name, "media controls on the session bus"),
            |_, name| tracing::debug!(name, "no media controls: the bus name was not ours"),
        );
        mpris.owner.set(Some(owner));
        mpris
    }

    fn register(
        self: &Rc<Self>,
        connection: &gio::DBusConnection,
        act: Rc<dyn Fn(Action)>,
    ) -> Result<(), glib::Error> {
        let node = gio::DBusNodeInfo::for_xml(INTERFACES)?;
        let root = node.lookup_interface(ROOT).expect("the root interface");
        let player = node.lookup_interface(PLAYER).expect("the player interface");
        connection
            .register_object(PATH, &root)
            .method_call({
                let act = Rc::clone(&act);
                move |_, _, _, _, method, _, invocation| {
                    if method == "Raise" {
                        act(Action::Raise);
                    }
                    invocation.return_value(None);
                }
            })
            .property(|_, _, _, _, name| match name {
                "CanQuit" | "HasTrackList" => false.to_variant(),
                "CanRaise" => true.to_variant(),
                "Identity" => "anistream".to_variant(),
                "DesktopEntry" => desktop_entry().to_variant(),
                _ => Vec::<String>::new().to_variant(),
            })
            .build()?;
        let media = Rc::downgrade(self);
        let set_media = Rc::downgrade(self);
        let set_act = Rc::clone(&act);
        connection
            .register_object(PATH, &player)
            .method_call({
                move |_, _, _, _, method, params, invocation| {
                    let action = match method {
                        "Next" => Some(Action::Next),
                        "Previous" => Some(Action::Previous),
                        "Pause" => Some(Action::Pause),
                        "PlayPause" => Some(Action::PlayPause),
                        "Stop" => Some(Action::Stop),
                        "Play" => Some(Action::Play),
                        "Seek" => params
                            .get::<(i64,)>()
                            .map(|(offset,)| Action::SeekBy(offset as f64 / 1_000_000.0)),
                        "SetPosition" => params
                            .get::<(glib::variant::ObjectPath, i64)>()
                            .map(|(_, at)| Action::SeekTo(at as f64 / 1_000_000.0)),
                        _ => None,
                    };
                    if let Some(action) = action {
                        act(action);
                    }
                    invocation.return_value(None);
                }
            })
            .property(move |_, _, _, _, name| {
                media
                    .upgrade()
                    .and_then(|m| m.media.borrow().player_property(name))
                    .unwrap_or_else(|| false.to_variant())
            })
            .set_property(move |_, _, _, _, name, value| {
                let Some(mpris) = set_media.upgrade() else { return false };
                match (name, value.get::<f64>()) {
                    (VOLUME, Some(volume)) => {
                        let volume = volume.clamp(0.0, 1.0);
                        mpris.media.borrow_mut().volume = volume;
                        set_act(Action::Volume(volume));
                        true
                    }
                    (RATE, Some(rate)) if rate > 0.0 => {
                        set_act(Action::Rate(rate.clamp(RATES.0, RATES.1)));
                        true
                    }
                    _ => false,
                }
            })
            .build()?;
        Ok(())
    }

    /// An episode is starting: its show, its line and its art. A new track, unless it is the
    /// one already playing.
    pub fn track(&self, track: Track) {
        {
            let mut media = self.media.borrow_mut();
            if media.track.as_ref() == Some(&track) {
                return;
            }
            media.serial += 1;
            media.position = 0.0;
            media.length = None;
            media.track = Some(track);
        }
        self.changed(&EVERYTHING);
    }

    /// The engine named the episode playing: the same track, its line and show said again.
    pub fn retitle(&self, show: &str, title: &str) {
        {
            let mut media = self.media.borrow_mut();
            let Some(track) = media.track.as_mut() else { return };
            if track.show == show && track.title == title {
                return;
            }
            track.show = show.to_owned();
            track.title = title.to_owned();
        }
        self.changed(&["Metadata"]); // i18n-skip
    }

    /// Where the player stands: paused or not, the position and the length. Says what changed,
    /// and a seek when the position jumped.
    pub fn state(&self, paused: bool, position: Option<f64>, length: Option<f64>) {
        let mut said: Vec<&str> = Vec::new();
        let mut seeked = None;
        {
            let mut media = self.media.borrow_mut();
            if media.track.is_none() {
                return;
            }
            if media.paused != paused {
                media.paused = paused;
                said.push("PlaybackStatus"); // i18n-skip
            }
            if let Some(length) = length
                && media.length.is_none_or(|l| (l - length).abs() > 0.5)
            {
                media.length = Some(length);
                said.extend(["Metadata", "CanSeek"]); // i18n-skip
            }
            if let Some(position) = position {
                // mpv reports a few times a second: more than 2 s from the last is a seek.
                if (position - media.position).abs() > 2.0 {
                    seeked = Some(position);
                }
                media.position = position;
            }
        }
        if !said.is_empty() {
            self.changed(&said);
        }
        if let (Some(position), Some(connection)) = (seeked, self.connection.borrow().as_ref())
        {
            let _ = connection.emit_signal(
                None,
                PATH,
                PLAYER,
                "Seeked", // i18n-skip
                Some(&(micros(position),).to_variant()),
            );
        }
    }

    /// The speed mpv plays at.
    pub fn rate(&self, rate: f64) {
        {
            let mut media = self.media.borrow_mut();
            if (media.rate - rate).abs() < 0.001 {
                return;
            }
            media.rate = rate;
        }
        self.changed(&["Rate"]); // i18n-skip
    }

    /// Whether there is an episode after the one playing, and one before it, for the controls'
    /// next and previous buttons.
    pub fn neighbours(&self, next: bool, previous: bool) {
        {
            let mut media = self.media.borrow_mut();
            if media.has_next == next && media.has_previous == previous {
                return;
            }
            media.has_next = next;
            media.has_previous = previous;
        }
        self.changed(&["CanGoNext", "CanGoPrevious"]); // i18n-skip
    }

    /// Nothing plays any more.
    pub fn stopped(&self) {
        let was = self.media.borrow_mut().track.take();
        if was.is_some() {
            self.changed(&EVERYTHING);
        }
    }

    /// Tell the bus what changed: `PropertiesChanged` with the new values.
    fn changed(&self, names: &[&str]) {
        let Some(connection) = self.connection.borrow().clone() else { return };
        let dict = glib::VariantDict::new(None);
        {
            let media = self.media.borrow();
            for name in names {
                if let Some(value) = media.player_property(name) {
                    dict.insert_value(name, &value);
                }
            }
        }
        let body = (PLAYER, dict.end(), Vec::<String>::new()).to_variant();
        if let Err(error) = connection.emit_signal(
            None,
            PATH,
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged", // i18n-skip
            Some(&body),
        ) {
            tracing::debug!(%error, "media controls not told");
        }
    }

    #[cfg(test)]
    fn media(&self) -> Media {
        self.media.borrow().clone()
    }
}

impl Drop for Mpris {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            gio::bus_unown_name(owner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing() -> Media {
        Media {
            track: Some(Track {
                show: "The Lantern Road".into(),
                title: "Episode 7 · Night Market".into(),
                art: Some("https://art.example.test/banner.jpg".into()),
            }),
            serial: 3,
            paused: false,
            position: 61.5,
            length: Some(1420.0),
            volume: 1.0,
            rate: 1.0,
            has_next: true,
            has_previous: false,
        }
    }

    #[test]
    fn an_episode_is_titled_by_its_line_and_credited_to_its_show() {
        let metadata = playing().metadata();
        let dict = glib::VariantDict::new(Some(&metadata));
        let title: String = dict.lookup("xesam:title").unwrap().unwrap();
        let artist: Vec<String> = dict.lookup("xesam:artist").unwrap().unwrap();
        let length: i64 = dict.lookup("mpris:length").unwrap().unwrap();
        let art: String = dict.lookup("mpris:artUrl").unwrap().unwrap();
        assert_eq!(title, "Episode 7 · Night Market");
        assert_eq!(artist, ["The Lantern Road"]);
        assert_eq!(length, 1_420_000_000);
        assert!(art.ends_with("banner.jpg"));
        assert!(dict.contains("mpris:trackid"));
    }

    #[test]
    fn nothing_playing_is_stopped_with_no_metadata() {
        let media = Media::default();
        assert_eq!(media.status(), "Stopped");
        assert_eq!(media.metadata().n_children(), 0);
        assert_eq!(media.player_property("CanSeek"), Some(false.to_variant()));
        let paused = Media { paused: true, ..playing() };
        assert_eq!(paused.status(), "Paused");
        assert_eq!(playing().player_property("Position"), Some(61_500_000i64.to_variant()));
    }

    #[test]
    fn next_and_previous_follow_the_episodes_and_the_rate_the_speed() {
        let first = playing();
        assert_eq!(first.player_property("CanGoNext"), Some(true.to_variant()));
        assert_eq!(first.player_property("CanGoPrevious"), Some(false.to_variant()));
        let fast = Media { rate: 1.5, ..playing() };
        assert_eq!(fast.player_property("Rate"), Some(1.5f64.to_variant()));
        assert_eq!(Media::default().player_property("CanGoNext"), Some(false.to_variant()));
    }

    #[test]
    fn a_new_episode_gets_a_new_track_and_the_same_one_does_not() {
        // Never on a bus: no connection, so nothing is said, only kept.
        let mpris = Mpris {
            connection: RefCell::default(),
            media: RefCell::new(Media::default()),
            owner: Cell::new(None),
        };
        let track = playing().track.unwrap();
        mpris.track(track.clone());
        mpris.state(false, Some(30.0), Some(1420.0));
        mpris.track(track.clone());
        assert_eq!(mpris.media().serial, 1);
        assert_eq!(mpris.media().position, 30.0);
        mpris.retitle("The Lantern Road", "Episode 7 · Renamed");
        assert_eq!(mpris.media().serial, 1, "a retitle is the same track");
        mpris.track(Track { title: "Episode 8".into(), ..track });
        assert_eq!(mpris.media().serial, 2);
        assert_eq!(mpris.media().length, None);
        mpris.stopped();
        assert_eq!(mpris.media().status(), "Stopped");
    }
}
