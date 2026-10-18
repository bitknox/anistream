//! The admin page's files, compiled into the server.
//!
//! The page is a React app in `admin/`, built with `bun run build` into `assets/admin/`. The build
//! is committed, so building a store never needs bun; rebuild it after changing the page.

use crate::http::Response;

macro_rules! asset {
    ($path:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/admin/", $path))
    };
}

/// Every file the page is made of: the path below `admin/`, its type, and its bytes.
const FILES: &[(&str, &str, &[u8])] = &[
    ("", "text/html; charset=utf-8", asset!("index.html")),
    ("admin.js", "text/javascript; charset=utf-8", asset!("admin.js")),
    ("admin.css", "text/css; charset=utf-8", asset!("admin.css")),
    ("favicon.svg", "image/svg+xml", asset!("favicon.svg")),
    (
        "fonts/zen-kaku-gothic-new-latin-400-normal.woff2",
        "font/woff2",
        asset!("fonts/zen-kaku-gothic-new-latin-400-normal.woff2"),
    ),
    (
        "fonts/zen-kaku-gothic-new-latin-500-normal.woff2",
        "font/woff2",
        asset!("fonts/zen-kaku-gothic-new-latin-500-normal.woff2"),
    ),
    (
        "fonts/zen-kaku-gothic-new-latin-700-normal.woff2",
        "font/woff2",
        asset!("fonts/zen-kaku-gothic-new-latin-700-normal.woff2"),
    ),
    (
        "fonts/zen-kaku-gothic-new-kana-700-normal.woff2",
        "font/woff2",
        asset!("fonts/zen-kaku-gothic-new-kana-700-normal.woff2"),
    ),
    ("fonts/OFL.txt", "text/plain; charset=utf-8", asset!("fonts/OFL.txt")),
];

/// The page holds owner rights, so it runs nothing it did not come with: no inline script, no
/// outside resource, no framing, and nothing to send a referrer to.
const POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; \
                      img-src 'self' data:; connect-src 'self'; base-uri 'none'; \
                      form-action 'none'; frame-ancestors 'none'";

/// One of the page's files, or `None` for a path that is not one.
pub(crate) fn asset(path: &str) -> Option<Response> {
    let (_, kind, bytes) = FILES.iter().find(|(name, _, _)| *name == path)?;
    let headers = [
        ("content-type", *kind),
        // Revalidated every time: the names are not hashed, so a cached copy could outlive an
        // upgrade of the store it came from.
        ("cache-control", "no-cache"),
        ("content-security-policy", POLICY),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        ("x-content-type-options", "nosniff"),
    ];
    Some(Response {
        status: 200,
        headers: headers.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())).collect(),
        body: bytes.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_references_only_files_that_are_served() {
        // The build names its output; this catches a rebuilt page whose names drifted from the
        // table above, which would otherwise be a blank page in production.
        let page = std::str::from_utf8(asset!("index.html")).unwrap();
        for reference in ["./admin.js", "./admin.css", "./favicon.svg"] {
            assert!(page.contains(reference), "index.html no longer loads {reference}");
            assert!(asset(reference.trim_start_matches("./")).is_some());
        }
        let css = std::str::from_utf8(asset!("admin.css")).unwrap();
        for (name, _, _) in FILES.iter().filter(|(name, _, _)| name.ends_with(".woff2")) {
            assert!(css.contains(&format!("./{name}")), "admin.css does not use {name}");
        }
    }

    #[test]
    fn no_inline_script_is_needed() {
        // The policy forbids inline script; a build that added some would break silently.
        let page = std::str::from_utf8(asset!("index.html")).unwrap();
        assert!(!page.contains("<script>"), "inline script in index.html");
        assert!(asset("../store.db").is_none());
    }
}
