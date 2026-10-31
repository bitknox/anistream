//! A torrent the person has, rather than one a source named: a magnet link, an info hash or the
//! bytes of a torrent file, as the magnet link the session starts.
//!
//! Nothing here looks anything up. A torrent file is read for what it already says: the hash
//! of its info dictionary, its name and its trackers.

use anistream_core::stream::torrent_link;

/// `input` as a magnet link: a magnet link or a bare info hash as it is, normalised. `None` for
/// anything else, a torrent file's path included, which [`magnet_from_file`] reads.
pub fn magnet_for(input: &str) -> Option<String> {
    torrent_link(input.trim(), None)
}

/// The magnet link for a torrent file's bytes, with its name and trackers. `None` when the
/// bytes are not a torrent file.
pub fn magnet_from_file(bytes: &[u8]) -> Option<String> {
    let torrent = librqbit::torrent_from_bytes::<librqbit::ByteBuf>(bytes).ok()?;
    let mut link = format!("magnet:?xt=urn:btih:{}", torrent.info_hash.as_string());
    if let Some(name) = torrent.info.name.as_ref().and_then(|n| std::str::from_utf8(n).ok()) {
        link.push_str("&dn=");
        link.push_str(&encode(name));
    }
    let mut seen = Vec::new();
    for tracker in torrent.iter_announce() {
        let Ok(tracker) = std::str::from_utf8(tracker) else { continue };
        if !tracker.is_empty() && !seen.contains(&tracker) {
            link.push_str("&tr=");
            link.push_str(&encode(tracker));
            seen.push(tracker);
        }
    }
    Some(link)
}

fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-file torrent made by hand: `ep01.mkv`, one tracker on `t.example`.
    const FILE: &[u8] = b"d8:announce26:udp://t.example:1/announce4:infod6:lengthi10e\
4:name8:ep01.mkv12:piece lengthi16384e6:pieces20:aaaaaaaaaaaaaaaaaaaaee";

    #[test]
    fn a_torrent_file_becomes_the_magnet_link_for_what_it_holds() {
        assert_eq!(
            magnet_from_file(FILE).as_deref(),
            Some(
                "magnet:?xt=urn:btih:fd45c2c75aedb6a1916c201a03f9640e9ee006f6\
&dn=ep01.mkv&tr=udp%3A%2F%2Ft.example%3A1%2Fannounce"
            )
        );
    }

    #[test]
    fn bytes_that_are_not_a_torrent_file_are_refused() {
        assert_eq!(magnet_from_file(b"<html>not a torrent</html>"), None);
        assert_eq!(magnet_from_file(&FILE[..40]), None, "cut short");
    }

    #[test]
    fn a_link_or_a_hash_is_taken_as_it_is() {
        let hash = "fd45c2c75aedb6a1916c201a03f9640e9ee006f6";
        assert_eq!(
            magnet_for(&format!(" {hash} ")),
            Some(format!("magnet:?xt=urn:btih:{hash}"))
        );
        assert_eq!(magnet_for("/home/someone/ep01.torrent"), None);
    }
}
