//! Names that stay the same from one run to the next.

/// Stable, filesystem-safe hash of a URL.
///
/// Not cryptographic — this only needs to avoid collisions between cover URLs and produce
/// the same name across runs so the cache survives a restart.
///
/// Down here rather than beside either of its callers: the artwork fetch names the cover cache's
/// files with it and the interface names the same files, and neither may depend on the other.
pub fn stable_hash(input: &str) -> String {
    // FNV-1a: tiny, stable across releases, and good enough for cache keys.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_keys_are_stable_and_distinct() {
        // Stable across runs, or the disk cache is useless.
        let a = stable_hash("https://s4.anilist.co/cover/154587.jpg");
        assert_eq!(a, stable_hash("https://s4.anilist.co/cover/154587.jpg"));
        assert_ne!(a, stable_hash("https://s4.anilist.co/cover/154588.jpg"));
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()), "must be filename-safe");
    }
}
