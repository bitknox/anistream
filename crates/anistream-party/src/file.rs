//! The name a member's file goes by in the room: a readable title, the episode, and a tag with
//! the AniList id, as `Natsukaze Relay, episode 9 [anilist:104321:9]`.
//!
//! Plain Syncplay clients show the name as it is. anistream members read the tag, which is how
//! someone who joins finds the episode in their own sources with nobody choosing. Only a title,
//! an id and an episode travel, never a source, a link or a stream.

/// The longest name sent, in characters. The title is cut to fit.
pub const MAX_NAME: usize = 200;

/// What a file name says of the episode it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomFile {
    /// The readable title; may be empty for a bare tag.
    pub title: String,
    pub id: u32,
    pub episode: String,
}

/// The name a file goes by in the room, under [`MAX_NAME`] characters.
pub fn file_name(title: &str, id: u32, episode: &str) -> String {
    let episode = episode.trim();
    let tail = format!(", episode {episode} [anilist:{id}:{episode}]");
    let room = MAX_NAME.saturating_sub(tail.chars().count());
    let title: String = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = if title.chars().count() > room {
        let cut: String = title.chars().take(room.saturating_sub(1)).collect();
        format!("{}…", cut.trim_end())
    } else {
        title
    };
    format!("{title}{tail}")
}

/// Read the tag from the end of a name. Titles with brackets of their own are safe, as only the
/// last tag counts. `None` for a name without one: a plain Syncplay file.
pub fn parse(name: &str) -> Option<RoomFile> {
    let name = name.trim_end();
    let body = name.strip_suffix(']')?;
    let start = body.rfind("[anilist:")?;
    let (id, episode) = body[start + "[anilist:".len()..].split_once(':')?;
    let id: u32 = id.parse().ok().filter(|id| *id != 0)?;
    let valid = !episode.is_empty()
        && episode.chars().all(|c| c.is_ascii_digit() || c == '.')
        && episode.chars().next().is_some_and(|c| c.is_ascii_digit());
    if !valid {
        return None;
    }
    let before = body[..start].trim_end();
    let title = match before.rfind(", episode ") {
        Some(at) if before[at + ", episode ".len()..].trim() == episode => &before[..at],
        _ => before,
    };
    Some(RoomFile { title: title.trim().to_owned(), id, episode: episode.to_owned() })
}

/// Whether two names are the same episode by their tags, whatever their lengths: sources differ
/// by seconds. `None` when either has no tag, and the name and length decide instead.
pub fn same_episode(a: &str, b: &str) -> Option<bool> {
    let (a, b) = (parse(a)?, parse(b)?);
    Some(a.id == b.id && a.episode == b.episode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_a_title_an_episode_and_a_tag() {
        let name = file_name("Natsukaze Relay", 104321, "9");
        assert_eq!(name, "Natsukaze Relay, episode 9 [anilist:104321:9]");
        assert_eq!(
            parse(&name),
            Some(RoomFile { title: "Natsukaze Relay".into(), id: 104321, episode: "9".into() })
        );
    }

    #[test]
    fn brackets_in_a_title_and_a_half_episode_are_safe() {
        let name = file_name("Kite [Remastered] [anilist:1:1]", 7, "12.5");
        let read = parse(&name).unwrap();
        assert_eq!((read.id, read.episode.as_str()), (7, "12.5"));
        assert_eq!(read.title, "Kite [Remastered] [anilist:1:1]");
    }

    #[test]
    fn a_long_title_is_cut_to_keep_the_name_short() {
        let long = "Very long title ".repeat(30);
        let name = file_name(&long, 55, "3");
        assert!(name.chars().count() <= MAX_NAME, "{}", name.chars().count());
        assert_eq!(parse(&name).map(|f| (f.id, f.episode)), Some((55, "3".into())));
    }

    #[test]
    fn plain_files_and_broken_tags_have_no_tag() {
        for name in [
            "kzm_08_720p.mkv",
            "Show, episode 3",
            "Show [anilist:0:3]",
            "Show [anilist:x:3]",
            "Show [anilist:4:]",
            "Show [anilist:4:3] tail",
            "",
        ] {
            assert_eq!(parse(name), None, "{name}");
        }
    }

    #[test]
    fn tags_decide_sameness_whatever_the_title() {
        let a = file_name("Natsukaze Relay", 5, "9");
        let b = file_name("Another spelling", 5, "9");
        assert_eq!(same_episode(&a, &b), Some(true));
        assert_eq!(same_episode(&a, &file_name("Natsukaze Relay", 5, "10")), Some(false));
        assert_eq!(same_episode(&a, "plain.mkv"), None);
    }
}
