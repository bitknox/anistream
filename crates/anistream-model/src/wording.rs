//! The words the app uses for counts, sizes and spans of time.
//!
//! Plain functions over plain numbers, so every front end says a wait, a size or a count the same
//! way rather than each writing its own and drifting: the terminal's activity row, the download
//! queue and the notices the work sends all read from here.

/// Render a countdown in the largest unit that still reads at a glance.
///
/// Raw minutes are useless past an hour or so — `9698m` tells you nothing, while `6d 17h`
/// is immediately legible. Seasonal shows are often days away, so this matters on every
/// list, not just the calendar.
pub fn countdown(seconds: i64) -> String {
    if seconds <= 0 {
        return "now".into();
    }
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days >= 1 {
        let rem_hours = hours % 24;
        if rem_hours > 0 && days < 7 {
            format!("{days}d {rem_hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours >= 1 {
        let rem_minutes = minutes % 60;
        if rem_minutes > 0 { format!("{hours}h {rem_minutes}m") } else { format!("{hours}h") }
    } else {
        format!("{minutes}m")
    }
}

/// How long ago something happened: `2d ago`, or `just now` under a minute.
///
/// Separate from [`countdown`] because that renders `now` at zero, and "out now ago" is not a
/// thing anyone says.
pub fn ago(seconds: i64) -> String {
    if seconds < 60 { "just now".into() } else { format!("{} ago", countdown(seconds)) }
}

/// A count and its noun, singular when the count is one: `1 episode`, `3 episodes`.
///
/// One helper rather than a `format!` at each call site, because each call site was written
/// for the plural and "queued 1 episodes" is what the first count of one printed.
pub fn plural(count: usize, noun: &str) -> String {
    if count == 1 { format!("1 {noun}") } else { format!("{count} {noun}s") }
}

/// Bytes at human scale, two significant figures.
///
/// Its own function because a download's size is the number the screen is most often read for, and
/// `1288490188 bytes` communicates nothing at a glance. Decimal units, as every size and rate on
/// screen is: a `kB` that meant 1024 in one place and 1000 in another would make two screens
/// disagree about the same file.
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [(&str, u64); 4] =
        [("GB", 1_000_000_000), ("MB", 1_000_000), ("kB", 1_000), ("B", 1)];
    for (unit, scale) in UNITS {
        if bytes >= scale {
            let value = bytes as f64 / scale as f64;
            // One decimal below ten, none above: `9.4 GB` and `140 MB` are both two digits of real
            // information, and `140.3 MB` is one of them plus noise.
            return if value < 10.0 && scale > 1 {
                format!("{value:.1} {unit}")
            } else {
                format!("{} {unit}", value.round() as u64)
            };
        }
    }
    "0 B".into()
}

/// A calendar row's time: `in 4d 20h` ahead, `3h ago` once aired.
pub fn calendar_time(secs: i64) -> String {
    if secs <= 0 { ago(-secs) } else { format!("in {}", countdown(secs)) }
}

/// `7s`, then `1m 05s`: seconds are what a wait is felt in, and minutes only once there are any.
pub fn clock(span: std::time::Duration) -> String {
    let secs = span.as_secs().max(1);
    if secs < 60 { format!("{secs}s") } else { format!("{}m {:02}s", secs / 60, secs % 60) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_of_one_are_singular() {
        assert_eq!(plural(1, "episode"), "1 episode");
        assert_eq!(plural(0, "episode"), "0 episodes");
        assert_eq!(plural(6, "title"), "6 titles");
    }

    #[test]
    fn sizes_read_at_human_scale() {
        // Two digits of real information, never a third of noise.
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1_400_000_000), "1.4 GB");
        assert_eq!(human_bytes(14_000_000_000), "14 GB");
        assert_eq!(human_bytes(700_000_000), "700 MB");
        assert_eq!(human_bytes(9_400_000), "9.4 MB");
    }

    #[test]
    fn long_waits_read_in_minutes() {
        assert_eq!(clock(std::time::Duration::from_secs(7)), "7s");
        assert_eq!(clock(std::time::Duration::from_secs(65)), "1m 05s");
    }
}
