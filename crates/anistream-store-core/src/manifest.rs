//! What a plugin is, readable without running it.
//!
//! A plugin says what it is by returning its manifest from `describe()`, so learning a plugin's id,
//! version and hosts means loading it in a WebAssembly runtime and calling it. That works in the
//! reference server and in the app, which both have wasmtime, and nowhere else: a Worker or a
//! serverless function cannot load a component, and neither can a browser.
//!
//! So a plugin can also carry its manifest as data, in a custom section named
//! [`SECTION`] at the top level of the `.wasm` file. `anistream plugin stamp` adds it by loading
//! the plugin, asking, and writing the answer down; the app stamps every plugin it publishes. A
//! store that cannot run plugins reads the section, and a store that can checks the section
//! against what the plugin says when asked. Neither is trusted further than that: every member's
//! app loads a plugin before installing it and refuses one whose answer differs from the index.

use serde::{Deserialize, Serialize};

/// The custom section's name.
pub const SECTION: &str = "anistream:manifest";

/// The largest manifest section read. A manifest is a few hundred bytes.
const MAX_SECTION: usize = 64 * 1024;

/// What a plugin says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub translations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("not a WebAssembly file")]
    NotWasm,
    #[error("the WebAssembly file is cut short or damaged")]
    Truncated,
    #[error("the plugin's manifest section is not a manifest: {0}")]
    Malformed(String),
}

/// The manifest a plugin carries, if it carries one.
pub fn read(wasm: &[u8]) -> Result<Option<PluginInfo>, ManifestError> {
    let mut found = None;
    for section in sections(wasm)? {
        let section = section?;
        if section.id == 0
            && let Some((name, payload)) = custom(section.payload)
            && name == SECTION
        {
            if payload.len() > MAX_SECTION {
                return Err(ManifestError::Malformed("larger than 64 KiB".into()));
            }
            // The last one wins, as it would for a file stamped twice by something other than
            // `stamp`, which replaces rather than appends.
            found = Some(
                serde_json::from_slice(payload)
                    .map_err(|e| ManifestError::Malformed(e.to_string()))?,
            );
        }
    }
    Ok(found)
}

/// `wasm` with `info` as its manifest section, replacing any it had.
///
/// Custom sections may appear anywhere in a module or a component, so the section goes at the end
/// and nothing that runs is touched.
pub fn stamp(wasm: &[u8], info: &PluginInfo) -> Result<Vec<u8>, ManifestError> {
    let mut out = Vec::with_capacity(wasm.len() + 512);
    out.extend_from_slice(&wasm[..8.min(wasm.len())]);
    for section in sections(wasm)? {
        let section = section?;
        let ours =
            section.id == 0 && custom(section.payload).is_some_and(|(n, _)| n == SECTION);
        if !ours {
            out.extend_from_slice(section.raw);
        }
    }
    let payload =
        serde_json::to_vec(info).map_err(|e| ManifestError::Malformed(e.to_string()))?;
    let mut body = Vec::with_capacity(payload.len() + SECTION.len() + 5);
    leb128(&mut body, SECTION.len() as u32);
    body.extend_from_slice(SECTION.as_bytes());
    body.extend_from_slice(&payload);
    out.push(0);
    leb128(&mut out, body.len() as u32);
    out.extend_from_slice(&body);
    Ok(out)
}

struct Section<'a> {
    id: u8,
    payload: &'a [u8],
    /// The whole section, header included, for copying it unchanged.
    raw: &'a [u8],
}

/// The top-level sections of a module or a component. Both share the outer format: an 8-byte
/// preamble, then `id, size, payload` repeated.
fn sections(
    wasm: &[u8],
) -> Result<impl Iterator<Item = Result<Section<'_>, ManifestError>>, ManifestError> {
    if wasm.len() < 8 || &wasm[..4] != b"\0asm" {
        return Err(ManifestError::NotWasm);
    }
    let mut at = 8;
    Ok(std::iter::from_fn(move || {
        if at >= wasm.len() {
            return None;
        }
        let start = at;
        let id = wasm[at];
        at += 1;
        let (size, read) = match read_leb128(&wasm[at..]) {
            Some(v) => v,
            None => {
                at = wasm.len();
                return Some(Err(ManifestError::Truncated));
            }
        };
        at += read;
        let end = at.checked_add(size as usize).filter(|end| *end <= wasm.len());
        let Some(end) = end else {
            at = wasm.len();
            return Some(Err(ManifestError::Truncated));
        };
        let section = Section { id, payload: &wasm[at..end], raw: &wasm[start..end] };
        at = end;
        Some(Ok(section))
    }))
}

/// A custom section's name and data.
fn custom(payload: &[u8]) -> Option<(&str, &[u8])> {
    let (len, read) = read_leb128(payload)?;
    let name_end = read.checked_add(len as usize).filter(|end| *end <= payload.len())?;
    let name = std::str::from_utf8(&payload[read..name_end]).ok()?;
    Some((name, &payload[name_end..]))
}

fn read_leb128(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut value: u32 = 0;
    for (i, byte) in bytes.iter().take(5).enumerate() {
        value |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
    }
    None
}

fn leb128(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(version: &str) -> PluginInfo {
        PluginInfo {
            id: "example".into(),
            name: "Example".into(),
            version: version.into(),
            allowed_hosts: vec!["api.example".into()],
            translations: vec!["sub".into()],
        }
    }

    /// The smallest component: the preamble a component starts with, and one custom section of
    /// someone else's so stamping has something to keep.
    fn component() -> Vec<u8> {
        let mut wasm = b"\0asm\x0d\x00\x01\x00".to_vec();
        let mut body = Vec::new();
        leb128(&mut body, 4);
        body.extend_from_slice(b"name");
        body.extend_from_slice(b"\x01\x02\x03");
        wasm.push(0);
        leb128(&mut wasm, body.len() as u32);
        wasm.extend_from_slice(&body);
        wasm
    }

    #[test]
    fn a_stamp_is_read_back() {
        let stamped = stamp(&component(), &info("1.0.0")).unwrap();
        assert_eq!(read(&stamped).unwrap(), Some(info("1.0.0")));
        // Everything that was there is still there, in order.
        assert!(stamped.starts_with(&component()));
    }

    #[test]
    fn stamping_again_replaces_rather_than_adds() {
        let once = stamp(&component(), &info("1.0.0")).unwrap();
        let twice = stamp(&once, &info("2.0.0")).unwrap();
        assert_eq!(read(&twice).unwrap(), Some(info("2.0.0")));
        assert_eq!(twice.len(), once.len(), "one manifest section, not two");
    }

    #[test]
    fn an_unstamped_plugin_has_no_manifest() {
        assert_eq!(read(&component()).unwrap(), None);
    }

    #[test]
    fn a_core_module_can_be_stamped_too() {
        let module = b"\0asm\x01\x00\x00\x00".to_vec();
        assert_eq!(
            read(&stamp(&module, &info("1.0.0")).unwrap()).unwrap(),
            Some(info("1.0.0"))
        );
    }

    #[test]
    fn damage_is_reported_not_skipped() {
        assert_eq!(read(b"not wasm at all").unwrap_err(), ManifestError::NotWasm);
        let mut cut = stamp(&component(), &info("1.0.0")).unwrap();
        cut.truncate(cut.len() - 3);
        assert_eq!(read(&cut).unwrap_err(), ManifestError::Truncated);

        let mut junk = component();
        let mut body = Vec::new();
        leb128(&mut body, SECTION.len() as u32);
        body.extend_from_slice(SECTION.as_bytes());
        body.extend_from_slice(b"{ not json");
        junk.push(0);
        leb128(&mut junk, body.len() as u32);
        junk.extend_from_slice(&body);
        assert!(matches!(read(&junk), Err(ManifestError::Malformed(_))));
    }

    #[test]
    fn leb128_round_trips_at_the_byte_boundaries() {
        for value in [0, 1, 127, 128, 16_383, 16_384, u32::MAX] {
            let mut out = Vec::new();
            leb128(&mut out, value);
            assert_eq!(read_leb128(&out), Some((value, out.len())));
        }
    }
}
