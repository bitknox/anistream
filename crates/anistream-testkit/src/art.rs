//! Cover, banner and still art for the fixture titles, for a front end that shows images.
//!
//! The native app's design draws each invented title as a stand-in: a traveller under a crescent
//! moon, a tower library above the clouds, a station platform under a full moon, a steel
//! gardener's flower, a running track at summer's end. These are those drawings rendered to PNG,
//! a 460 × 690 poster and a 1600 × 900 wide one each, and the second season's drawn in another
//! tone. [`FakeAniList::serve_art`](crate::FakeAniList::serve_art) serves them at the paths the
//! fixtures' own image URLs name.

use crate::fixtures::{AIRING, AIRING_NO_MAL, FINISHED, MOVIE, ONA, SEQUEL};

/// One title's drawing, as a poster and wide.
struct Art {
    poster: &'static [u8],
    wide: &'static [u8],
}

macro_rules! art {
    ($name:literal) => {
        Art {
            poster: include_bytes!(concat!("../fixtures/art/", $name, "-poster.png")),
            wide: include_bytes!(concat!("../fixtures/art/", $name, "-wide.png")),
        }
    };
}

const STARDUST: Art = art!("stardust");
const STARDUST_SECOND: Art = art!("stardust-2");
const LIBRARY: Art = art!("library");
const PLATFORM: Art = art!("platform");
const GARDENER: Art = art!("gardener");
const RELAY: Art = art!("relay");

fn of(id: u32) -> Option<&'static Art> {
    let id = anistream_core::ids::AnilistId::new(id);
    Some(match id {
        FINISHED => &STARDUST,
        SEQUEL => &STARDUST_SECOND,
        AIRING => &LIBRARY,
        MOVIE => &PLATFORM,
        ONA => &GARDENER,
        AIRING_NO_MAL => &RELAY,
        _ => return None,
    })
}

/// The PNG for an image path under [`crate::fixtures::ART_HOST`], or `None` for a path the
/// fixtures do not name: `cover/900001-xl.jpg` and `cover/900001.jpg` are the poster,
/// `banner/900001.jpg` and an episode's still, `ep/900001/3.jpg`, the wide drawing.
pub fn png(path: &str) -> Option<&'static [u8]> {
    let mut parts = path.trim_start_matches('/').split('/');
    let kind = parts.next()?;
    let id = parts.next()?;
    let id: u32 = id.split(['-', '.']).next()?.parse().ok()?;
    let art = of(id)?;
    match kind {
        "cover" => Some(art.poster),
        "banner" | "ep" => Some(art.wide),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

    #[test]
    fn every_image_the_fixtures_name_has_art() {
        for path in
            ["cover/900001-xl.jpg", "cover/900006.jpg", "banner/900004.jpg", "ep/900001/7.jpg"]
        {
            assert!(png(path).is_some_and(|bytes| bytes.starts_with(PNG)), "{path}");
        }
        assert_ne!(png("cover/900001.jpg"), png("cover/900004.jpg"), "the sequel is its own");
        assert_ne!(
            png("cover/900001.jpg"),
            png("banner/900001.jpg"),
            "a poster is not a banner"
        );
    }

    #[test]
    fn a_path_the_fixtures_do_not_name_has_none() {
        assert!(png("cover/900999.jpg").is_none());
        assert!(png("avatar/900001.jpg").is_none());
        assert!(png("cover").is_none());
    }
}
