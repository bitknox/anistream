//! The watch party rooms this device was in: the last ten, newest first.
//!
//! A row is a place and a title, with who was there. It holds no source and no stream, so it is
//! safe to keep and to show.

use anistream_core::ids::AnilistId;

use crate::{Result, Store};

/// How many rooms are kept.
pub const KEPT: usize = 10;

/// One room this device was in.
#[derive(Debug, Clone, PartialEq)]
pub struct PartyRoom {
    /// `host:port`.
    pub server: String,
    pub room: String,
    pub anilist_id: AnilistId,
    pub title: String,
    /// The last episode the room reached.
    pub episode: String,
    /// Who was there besides this device's member.
    pub names: Vec<String>,
    pub cover_url: Option<String>,
    pub last_at: i64,
}

impl Store {
    /// Keep `room` as the newest, in place of the same room before it, and let go of all but
    /// the last ten.
    pub fn remember_party_room(&self, room: &PartyRoom) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute(
                "INSERT INTO party_room
                    (server, room, anilist_id, title, episode, names, cover_url, last_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(server, room) DO UPDATE SET
                    anilist_id = excluded.anilist_id,
                    title      = excluded.title,
                    episode    = excluded.episode,
                    names      = excluded.names,
                    cover_url  = excluded.cover_url,
                    last_at    = excluded.last_at",
                rusqlite::params![
                    room.server,
                    room.room,
                    room.anilist_id.get(),
                    room.title,
                    room.episode,
                    room.names.join("\n"),
                    room.cover_url,
                    room.last_at,
                ],
            )?;
            tx.execute(
                "DELETE FROM party_room WHERE rowid NOT IN
                    (SELECT rowid FROM party_room ORDER BY last_at DESC, rowid DESC LIMIT ?1)",
                [KEPT as i64],
            )?;
            Ok(())
        })
    }

    /// The rooms kept, newest first.
    pub fn party_rooms(&self) -> Result<Vec<PartyRoom>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT server, room, anilist_id, title, episode, names, cover_url, last_at
                 FROM party_room ORDER BY last_at DESC, rowid DESC",
            )?;
            let rows = statement
                .query_map([], |r| {
                    let names: String = r.get(5)?;
                    Ok(PartyRoom {
                        server: r.get(0)?,
                        room: r.get(1)?,
                        anilist_id: AnilistId::new(r.get::<_, u32>(2)?),
                        title: r.get(3)?,
                        episode: r.get(4)?,
                        names: names
                            .lines()
                            .filter(|n| !n.is_empty())
                            .map(str::to_owned)
                            .collect(),
                        cover_url: r.get(6)?,
                        last_at: r.get(7)?,
                    })
                })?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Let go of one room.
    pub fn forget_party_room(&self, server: &str, room: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "DELETE FROM party_room WHERE server = ?1 AND room = ?2",
                rusqlite::params![server, room],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(n: u32, at: i64) -> PartyRoom {
        PartyRoom {
            server: "party.example:8999".into(),
            room: format!("kite-aaaa-{n:04}"),
            anilist_id: AnilistId::new(n + 1),
            title: format!("Title {n}"),
            episode: "3".into(),
            names: vec!["Mika".into(), "Ren".into()],
            cover_url: None,
            last_at: at,
        }
    }

    #[test]
    fn the_last_ten_rooms_are_kept_newest_first() {
        let store = Store::open_in_memory().unwrap();
        for n in 0..12 {
            store.remember_party_room(&room(n, 100 + i64::from(n))).unwrap();
        }
        let rooms = store.party_rooms().unwrap();
        assert_eq!(rooms.len(), KEPT);
        assert_eq!(rooms[0].title, "Title 11");
        assert_eq!(rooms[9].title, "Title 2");
        assert_eq!(rooms[0].names, ["Mika", "Ren"]);
    }

    #[test]
    fn a_room_seen_again_is_one_row_with_its_latest_episode() {
        let store = Store::open_in_memory().unwrap();
        store.remember_party_room(&room(1, 10)).unwrap();
        store.remember_party_room(&room(2, 20)).unwrap();
        let mut again = room(1, 30);
        again.episode = "4".into();
        store.remember_party_room(&again).unwrap();
        let rooms = store.party_rooms().unwrap();
        assert_eq!(rooms.len(), 2);
        assert_eq!(
            (rooms[0].room.as_str(), rooms[0].episode.as_str()),
            (again.room.as_str(), "4")
        );
        store.forget_party_room(&again.server, &again.room).unwrap();
        assert_eq!(store.party_rooms().unwrap().len(), 1);
    }
}
