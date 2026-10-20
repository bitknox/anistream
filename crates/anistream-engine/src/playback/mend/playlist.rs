//! The HLS side of the proxy: telling a playlist from media, pointing every URI a playlist
//! names back through the proxy with every other byte kept, and reading the order of its
//! segments.

/// A media playlist with every fetchable URI pointed back at this proxy.
///
/// Segment URIs, initialisation segments and key URIs are all rewritten: a key that still
/// pointed upstream would be fetched by mpv without the stream's referer and refused, and a
/// nested playlist has to come back through here or its own segments would go direct and
/// unmended.
///
/// **Rewriting rather than parsing** is deliberate. A full HLS parser (`m3u8-rs` is the good
/// one) would round-trip the playlist through its own model and re-serialise it, which drops
/// any tag the model does not know — including whatever the spec adds next. Every byte here
/// survives except the URIs, which is the only change a proxy has any business making.
pub fn rewrite_playlist(body: &str, base: &str, proxy: &dyn Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(body.len() + 256);

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }

        if trimmed.starts_with('#') {
            // Any tag carrying a `URI="…"` attribute, rather than an enumerated list of the
            // ones known today: `EXT-X-KEY`, `-MAP`, `-MEDIA`, `-I-FRAME-STREAM-INF`,
            // `-SESSION-KEY` and `-PART` all use the same attribute, and a tag added to the
            // spec next year would otherwise slip through pointing upstream. The value is
            // replaced in place and every other byte of the tag is kept.
            if let Some(rewritten) = rewrite_uri_attribute(trimmed, base, proxy) {
                out.push_str(&rewritten);
                out.push('\n');
                continue;
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }

        // A bare line in a playlist is a URI: a segment, or a nested playlist.
        out.push_str(&proxy(&resolve_url(base, trimmed)));
        out.push('\n');
    }

    out
}

/// Rewrite the `URI="…"` attribute of one tag, if it has one.
fn rewrite_uri_attribute(
    tag: &str,
    base: &str,
    proxy: &dyn Fn(&str) -> String,
) -> Option<String> {
    let start = tag.find("URI=\"")? + 5;
    let end = start + tag[start..].find('"')?;
    let absolute = resolve_url(base, &tag[start..end]);
    Some(format!("{}{}{}", &tag[..start], proxy(&absolute), &tag[end..]))
}

/// Resolve a possibly-relative URI against the URL it was found in.
///
/// Hand-rolled rather than pulled from a URL crate: playlists use three forms — absolute,
/// root-relative and path-relative — and this handles exactly those without adding a
/// dependency for the sake of it.
pub fn resolve_url(base: &str, reference: &str) -> String {
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return reference.to_owned();
    }

    let scheme_end = base.find("://").map(|i| i + 3).unwrap_or(0);
    let authority_end =
        base[scheme_end..].find('/').map(|i| scheme_end + i).unwrap_or(base.len());

    if let Some(rooted) = reference.strip_prefix('/') {
        return format!("{}/{}", &base[..authority_end], rooted);
    }

    // Path-relative: everything up to the last slash, then the reference. Query strings on the
    // base are not part of the directory.
    let path = base.split('?').next().unwrap_or(base);
    let directory = path.rfind('/').map(|i| &path[..=i]).unwrap_or(base);
    format!("{directory}{reference}")
}

/// The segments a media playlist lists, in order, as absolute URLs.
pub(super) fn segment_urls(body: &str, base: &str) -> Vec<String> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| resolve_url(base, line))
        .collect()
}

/// Whether a response is an HLS playlist rather than media.
///
/// The body is trusted over the content type: image CDNs serving video segments are exactly
/// the kind of host that also mislabels a playlist.
pub(super) fn looks_like_playlist(content_type: &str, body: &[u8]) -> bool {
    if body.starts_with(b"#EXTM3U") {
        return true;
    }
    let lowered = content_type.to_ascii_lowercase();
    (lowered.contains("mpegurl") || lowered.contains("m3u"))
        && body.len() < 4 * 1024 * 1024
        && std::str::from_utf8(body).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::mend::encode_path;

    #[test]
    fn a_tag_this_code_never_heard_of_still_has_its_uri_rewritten() {
        // Any tag with a URI attribute, rather than a list of the ones known today — a tag
        // added to the spec later would otherwise quietly point upstream, unmended.
        let proxy = |u: &str| format!("proxied:{u}");
        let out = rewrite_playlist(
            "#EXTM3U\n#EXT-X-SOMETHING-NEW:URI=\"future.ts\",FLAG=1\n",
            "https://cdn.test/a/index.m3u8",
            &proxy,
        );
        assert!(out.contains("proxied:https://cdn.test/a/future.ts"), "{out}");
        assert!(out.contains("FLAG=1"), "the rest of the tag must survive: {out}");
    }

    #[test]
    fn every_uri_in_a_playlist_comes_back_through_the_proxy() {
        let playlist = "#EXTM3U\n\
             #EXT-X-VERSION:3\n\
             #EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0x00\n\
             #EXT-X-MAP:URI=\"init.mp4\"\n\
             #EXTINF:10.0,\n\
             seg1.ts\n\
             #EXTINF:10.0,\n\
             https://other.test/seg2.ts\n";
        let proxy = |u: &str| format!("http://127.0.0.1:1/m/t/{}", encode_path(u));
        let out = rewrite_playlist(playlist, "https://cdn.test/hls/index.m3u8", &proxy);

        // Segments, the key and the init segment all route back — a key fetched directly
        // would go without the stream's referer and be refused.
        assert!(out.contains(&proxy("https://cdn.test/hls/seg1.ts")), "{out}");
        assert!(out.contains(&proxy("https://other.test/seg2.ts")), "{out}");
        assert!(out.contains(&proxy("https://cdn.test/hls/key.bin")), "{out}");
        assert!(out.contains(&proxy("https://cdn.test/hls/init.mp4")), "{out}");
        // Tags keep their other attributes exactly.
        assert!(out.contains("METHOD=AES-128"), "{out}");
        assert!(out.contains("IV=0x00"), "{out}");
        assert!(out.contains("#EXTINF:10.0,"), "{out}");
    }

    #[test]
    fn relative_and_absolute_references_both_resolve() {
        let base = "https://cdn.test/a/b/index.m3u8?token=1";
        assert_eq!(resolve_url(base, "seg.ts"), "https://cdn.test/a/b/seg.ts");
        assert_eq!(resolve_url(base, "/root/seg.ts"), "https://cdn.test/root/seg.ts");
        assert_eq!(resolve_url(base, "https://x.test/s.ts"), "https://x.test/s.ts");
    }
}
