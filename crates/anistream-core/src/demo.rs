//! The demo: one work, shared by its makers under a Creative Commons licence, built into the
//! engine so a new install has something to play before any source is added.
//!
//! This is a catalogue, not a source of files. anistream hosts no content: every address here is
//! Wikimedia Commons', the archive the makers' release is held in, and the app plays the files
//! whole, as Commons holds them. Nothing is mirrored, cached on a project server or proxied.
//!
//! None of it can be changed from outside. It is not a plugin: there is no manifest, no store
//! record, no setting and no update path, so a different catalogue, address or host list takes an
//! app release. The source id [`DEMO_SOURCE_ID`] is reserved: [`reserved`] is what the plugin
//! loader and the store client refuse a claim to it with.
//!
//! The licence terms, kept here and in the credit the apps show:
//! - only works whose own file page says CC BY, CC BY-SA or public domain go in (this one is
//!   CC BY 4.0);
//! - a work is played whole: no cut clips, no burned-in subtitles, no re-encodes. Subtitles are a
//!   separate track the viewer turns on, credited separately;
//! - sponsor logos in the episodes are outside the licence, and the credit says so.
//!
//! Each file is pinned by the SHA-1 Commons reports for it. A file whose hash differs from the pin
//! is not played under this credit: see [`DemoPart::sha1`].

use crate::ids::AnilistId;

/// The demo's source id. Reserved: see [`reserved`].
pub const DEMO_SOURCE_ID: &str = "demo";

/// What the demo is called in every list. Reserved with the id.
pub const DEMO_SOURCE_NAME: &str = "Demo";

/// The only hosts the demo may reach, in every request and after every redirect.
pub const DEMO_HOSTS: [&str; 2] = ["upload.wikimedia.org", "commons.wikimedia.org"];

/// The key the demo's title goes by throughout the engine, in place of an AniList id: it has no
/// AniList entry to take one from. Near the top of the range, where AniList's own ids do not go.
/// It is never sent to AniList or to a tracker.
pub const DEMO_ANILIST_ID: AnilistId = AnilistId::new(4_294_000_001);

/// Wikimedia asks for a descriptive User-Agent with a way to reach the project.
pub const DEMO_USER_AGENT: &str =
    concat!("anistream/", env!("CARGO_PKG_VERSION"), " (https://github.com/bitknox/anistream)");

/// The title, as the makers write it.
pub const DEMO_TITLE: &str = "こうしす！";
/// The same in the Latin alphabet, so a search in either finds it.
pub const DEMO_TITLE_ROMAJI: &str = "Koushisu!";
/// A synopsis written for the demo.
pub const DEMO_SYNOPSIS: &str = "A comedy series that teaches information security at the office. \
Its makers share it under CC BY 4.0, and the demo plays it from Wikimedia Commons.";
/// The maker's credit.
pub const DEMO_CREDIT: &str = "こうしす！ by OPAP-JP contributors. Licensed CC BY 4.0. Streamed from Wikimedia Commons. Sponsor logos in the episodes are not covered by the licence.";
/// The subtitles are Commons' own wiki text, credited apart from the episodes.
pub const DEMO_SUBTITLE_CREDIT: &str =
    "Subtitles by Wikimedia Commons contributors, CC BY-SA 4.0.";
/// What the app changes in the work: nothing. It plays each file as Commons holds it.
pub const DEMO_CHANGES: &str = "none";
pub const DEMO_LICENCE_NAME: &str = "CC BY 4.0";
pub const DEMO_LICENCE_URL: &str = "https://creativecommons.org/licenses/by/4.0/";
pub const DEMO_SUBTITLE_LICENCE_URL: &str = "https://creativecommons.org/licenses/by-sa/4.0/";
/// The makers' site. Linked from the credit and never streamed from.
pub const DEMO_MAKER_URL: &str = "https://opap.jp/";
/// The Commons page of the work's first part, which names its licence and its makers.
pub const DEMO_FILE_PAGE_URL: &str = PART_1_PAGE;

/// One part of the work, as Commons holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoPart {
    /// The episode number the engine lists it as: "1".
    pub number: &'static str,
    /// The part's title.
    pub title: &'static str,
    /// The file, on `upload.wikimedia.org`.
    pub file_url: &'static str,
    /// The Commons file page, which states the part's licence.
    pub page_url: &'static str,
    /// The SHA-1 Commons reports for the file, pinned. The engine asks Commons for it before
    /// handing the file to a player, and plays nothing that differs.
    pub sha1: &'static str,
    /// The English subtitles' TimedText page, when Commons has them.
    pub subtitles_en: Option<&'static str>,
    /// The Japanese subtitles' TimedText page.
    pub subtitles_ja: Option<&'static str>,
    /// The still, on `upload.wikimedia.org`.
    pub still_url: &'static str,
    /// How long it runs, in seconds.
    pub duration_secs: u32,
    /// The picture's height in pixels.
    pub quality: u32,
    /// The Commons API address that reports the file's current hash.
    pub hash_url: &'static str,
}

macro_rules! file_1 {
    () => {
        "%E3%81%93%E3%81%86%E3%81%97%E3%81%99%EF%BC%81_-3.1%E3%80%8C%E3%82%BB%E3%82%AD%E3%83%A5%E3%83%AA%E3%83%86%E3%82%A3%E3%81%AB%E5%AE%8C%E7%92%A7%E3%82%92%E6%B1%82%E3%82%81%E3%82%8B%E3%81%AE%E3%81%AF%E9%96%93%E9%81%95%E3%81%A3%E3%81%A6%E3%81%84%E3%82%8B%E3%81%A0%E3%82%8D%E3%81%86%E3%81%8B%E3%80%8D%28Part1-4%29.webm"
    };
}
const PART_1_PAGE: &str = concat!("https://commons.wikimedia.org/wiki/File:", file_1!());

macro_rules! file_2 {
    () => {
        "%E3%81%93%E3%81%86%E3%81%97%E3%81%99%EF%BC%81_-3.2%E3%80%8C%E3%82%BB%E3%82%AD%E3%83%A5%E3%83%AA%E3%83%86%E3%82%A3%E3%81%AB%E5%AE%8C%E7%92%A7%E3%82%92%E6%B1%82%E3%82%81%E3%82%8B%E3%81%AE%E3%81%AF%E9%96%93%E9%81%95%E3%81%A3%E3%81%A6%E3%81%84%E3%82%8B%E3%81%A0%E3%82%8D%E3%81%86%E3%81%8B_%28Part2-4%29%E3%80%8D.webm"
    };
}
const PART_2_PAGE: &str = concat!("https://commons.wikimedia.org/wiki/File:", file_2!());

/// The parts that Commons holds under CC BY 4.0, in order. Another part goes in only after a check
/// of its own file page.
pub const PARTS: [DemoPart; 2] = [
    DemoPart {
        number: "1",
        title: "Part 1",
        file_url: concat!("https://upload.wikimedia.org/wikipedia/commons/1/1c/", file_1!()),
        page_url: PART_1_PAGE,
        sha1: "555240f79b07384c0b2b07a804493351b7e220a6",
        subtitles_en: Some(concat!(
            "https://commons.wikimedia.org/w/index.php?title=TimedText:",
            file_1!(),
            ".en.srt&action=raw"
        )),
        subtitles_ja: Some(concat!(
            "https://commons.wikimedia.org/w/index.php?title=TimedText:",
            file_1!(),
            ".ja.srt&action=raw"
        )),
        still_url: concat!(
            "https://upload.wikimedia.org/wikipedia/commons/thumb/1/1c/",
            file_1!(),
            "/960px--",
            file_1!(),
            ".jpg"
        ),
        duration_secs: 513,
        quality: 720,
        hash_url: concat!(
            "https://commons.wikimedia.org/w/api.php?action=query&format=json&prop=imageinfo&iiprop=sha1&titles=File:",
            file_1!()
        ),
    },
    DemoPart {
        number: "2",
        title: "Part 2",
        file_url: concat!("https://upload.wikimedia.org/wikipedia/commons/7/74/", file_2!()),
        page_url: PART_2_PAGE,
        sha1: "49f7aa84b30d92b5b0101973e5f12f5bc2d0998a",
        subtitles_en: None,
        subtitles_ja: Some(concat!(
            "https://commons.wikimedia.org/w/index.php?title=TimedText:",
            file_2!(),
            ".ja.srt&action=raw"
        )),
        still_url: concat!(
            "https://upload.wikimedia.org/wikipedia/commons/thumb/7/74/",
            file_2!(),
            "/960px--",
            file_2!(),
            ".jpg"
        ),
        duration_secs: 858,
        quality: 1080,
        hash_url: concat!(
            "https://commons.wikimedia.org/w/api.php?action=query&format=json&prop=imageinfo&iiprop=sha1&titles=File:",
            file_2!()
        ),
    },
];

/// The part numbered `number`.
pub fn part(number: &str) -> Option<&'static DemoPart> {
    PARTS.iter().find(|part| part.number == number)
}

/// Whether `claim` is what the demo reserves: its id, or its name, whatever the case or the
/// spacing round it. A plugin or a store entry that claims either is refused.
pub fn reserved(claim: &str) -> bool {
    let claim = claim.trim();
    claim.eq_ignore_ascii_case(DEMO_SOURCE_ID) || claim.eq_ignore_ascii_case(DEMO_SOURCE_NAME)
}

/// The host of an `https` address, lower-cased, when it is a plain one: no user name, no
/// password, and no port but 443. Anything else has no host the demo will talk to.
pub fn plain_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') || authority.contains('\\') {
        return None;
    }
    let host = match authority.rsplit_once(':') {
        Some((host, "443")) => host,
        Some(_) => return None,
        None => authority,
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Whether the demo may talk to `url`: `https`, and exactly one of [`DEMO_HOSTS`]. Checked on
/// every request and on every redirect's target.
pub fn host_allowed(url: &str) -> bool {
    plain_host(url).is_some_and(|host| DEMO_HOSTS.contains(&host.as_str()))
}

/// Every address the catalogue holds, for the checks that walk them.
pub fn urls() -> Vec<&'static str> {
    PARTS
        .iter()
        .flat_map(|part| {
            [part.file_url, part.still_url, part.hash_url, part.page_url]
                .into_iter()
                .chain(part.subtitles_en)
                .chain(part.subtitles_ja)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_address_in_the_catalogue_is_on_a_demo_host() {
        for url in urls() {
            assert!(host_allowed(url), "{url} is not on the demo's hosts");
        }
    }

    #[test]
    fn files_and_stills_are_on_upload_and_subtitles_on_commons() {
        for part in PARTS {
            assert_eq!(plain_host(part.file_url).as_deref(), Some("upload.wikimedia.org"));
            assert_eq!(plain_host(part.still_url).as_deref(), Some("upload.wikimedia.org"));
            for subtitles in [part.subtitles_en, part.subtitles_ja].into_iter().flatten() {
                assert_eq!(plain_host(subtitles).as_deref(), Some("commons.wikimedia.org"));
            }
        }
    }

    #[test]
    fn no_address_is_a_project_host() {
        for url in urls().into_iter().chain([DEMO_LICENCE_URL, DEMO_MAKER_URL]) {
            assert!(!url.contains("anistream.tv"), "{url}");
        }
    }

    #[test]
    fn every_part_is_pinned_by_a_sha1() {
        for part in PARTS {
            assert_eq!(part.sha1.len(), 40);
            assert!(part.sha1.bytes().all(|b| b.is_ascii_hexdigit()), "{}", part.sha1);
        }
    }

    #[test]
    fn the_parts_are_numbered_from_one_in_order() {
        let numbers: Vec<_> = PARTS.iter().map(|p| p.number).collect();
        assert_eq!(numbers, ["1", "2"]);
        assert_eq!(part("2").map(|p| p.title), Some("Part 2"));
        assert!(part("3").is_none());
    }

    #[test]
    fn the_host_check_is_strict() {
        assert!(host_allowed("https://upload.wikimedia.org/a.webm"));
        assert!(host_allowed("https://UPLOAD.wikimedia.org:443/a.webm"));
        for url in [
            "http://upload.wikimedia.org/a.webm",
            "https://upload.wikimedia.org.example/a.webm",
            "https://example.org/upload.wikimedia.org",
            "https://user@upload.wikimedia.org/a.webm",
            "https://upload.wikimedia.org:8443/a.webm",
            "https://upload.wikimedia.org@evil.example/a.webm",
            "https://evil.example\\@upload.wikimedia.org/a",
            "https://anistream.tv/demo.webm",
            "https://thumb.wikimedia.org/a.jpg",
            "//upload.wikimedia.org/a",
            "",
        ] {
            assert!(!host_allowed(url), "{url}");
        }
    }

    #[test]
    fn the_id_and_the_name_are_reserved_whatever_their_case() {
        for claim in ["demo", "Demo", " DEMO ", "dEmO"] {
            assert!(reserved(claim), "{claim}");
        }
        for claim in ["demos", "torrent", "", "de mo"] {
            assert!(!reserved(claim), "{claim}");
        }
    }
}
