//! The episode a file in a torrent holds, read from its name.
//!
//! A source names a torrent, and a torrent can hold a whole season. When the source does not say
//! which file to play, the session reads the episode number each file's name states and plays the
//! one asked for: `Hoshikuzu no Tabibito - 12.mkv`, `S02E05.mkv`, `Episode 03.mp4`. Nothing here
//! looks at who made a file or ranks one against another.

/// The episode number a file name states, if it states one.
pub fn episode_in(file_name: &str) -> Option<u32> {
    // The last path segment only: a folder named `Season 2` says nothing about the episode.
    let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let name = strip_extension(name.trim());
    let body = without_brackets(name);
    let tokens: Vec<&str> = body
        .split(|c: char| c.is_whitespace() || c == '_' || c == '.')
        .filter(|t| !t.is_empty())
        .collect();

    let mut index = 0;
    // Whether the last token is free to be read as a bare episode number.
    let mut last_free = true;
    while index < tokens.len() {
        let token = tokens[index];
        let lowered = token.to_ascii_lowercase();

        // `Season 2` and `Part 2` name something other than the episode.
        if (lowered == "season" || lowered == "part")
            && tokens.get(index + 1).is_some_and(|next| next.parse::<u32>().is_ok())
        {
            last_free &= index + 2 < tokens.len();
            index += 2;
            continue;
        }
        // `S02E05`.
        if let Some(rest) = lowered.strip_prefix('s')
            && let Some((season, episode)) = rest.split_once('e')
            && season.parse::<u32>().is_ok()
            && let Some(episode) = number(episode)
        {
            return Some(episode);
        }
        // `E05`, `Ep05`, `Episode 05`, `Ep 05`.
        for prefix in ["episode", "ep", "e"] {
            if let Some(rest) = lowered.strip_prefix(prefix) {
                if rest.is_empty() {
                    if let Some(episode) = tokens.get(index + 1).and_then(|next| number(next)) {
                        return Some(episode);
                    }
                } else if let Some(episode) = number(rest) {
                    return Some(episode);
                }
            }
        }
        // `- 12` or `- 12v2`: the dash is the usual separator before the episode.
        if token == "-"
            && let Some(episode) = tokens.get(index + 1).and_then(|next| number(next))
        {
            return Some(episode);
        }
        index += 1;
    }

    // A bare number last, as in `03.mkv` or `Hoshikuzu no Tabibito 03.mkv`, when it is not a
    // year or a resolution.
    tokens
        .last()
        .filter(|_| last_free)
        .and_then(|last| number(last))
        .filter(|n| !(1900..=2100).contains(n))
}

/// `12` or `12v2`, as a number; not `1080p`, `x264` or `2023-2024`.
fn number(token: &str) -> Option<u32> {
    let digits = match token.split_once(['v', 'V']) {
        Some((digits, version)) if version.chars().all(|c| c.is_ascii_digit()) => digits,
        _ => token,
    };
    (!digits.is_empty() && digits.len() <= 4 && digits.chars().all(|c| c.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

/// Strip a trailing container extension.
fn strip_extension(input: &str) -> &str {
    for ext in [".mkv", ".mp4", ".avi", ".webm", ".m4v", ".ts", ".mov"] {
        if input.len() > ext.len() && input[input.len() - ext.len()..].eq_ignore_ascii_case(ext)
        {
            return &input[..input.len() - ext.len()];
        }
    }
    input
}

/// The name with every bracketed or parenthesised part taken out: a checksum, a resolution or a
/// tag never names the episode.
fn without_brackets(input: &str) -> String {
    let mut body = String::with_capacity(input.len());
    let mut depth = 0i32;
    for ch in input.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => {
                depth = (depth - 1).max(0);
                body.push(' ');
            }
            _ if depth == 0 => body.push(ch),
            _ => {}
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_usual_separators_name_the_episode() {
        for (name, episode) in [
            ("Hoshikuzu no Tabibito - 12.mkv", 12),
            ("Hoshikuzu no Tabibito - 07v2.mkv", 7),
            ("Kumo no Ue no Toshokan S02E05.mkv", 5),
            ("kumo.no.ue.no.toshokan.s01e11.mp4", 11),
            ("Episode 03.mp4", 3),
            ("Ep04.mkv", 4),
            ("E09 - The Long Road.mkv", 9),
            ("03.mkv", 3),
            ("Hoshikuzu no Tabibito 10.mkv", 10),
        ] {
            assert_eq!(episode_in(name), Some(episode), "{name}");
        }
    }

    #[test]
    fn checksums_resolutions_and_years_are_not_episodes() {
        assert_eq!(episode_in("Hoshikuzu no Tabibito (1080p) [A1B2C3D4] - 02.mkv"), Some(2));
        assert_eq!(episode_in("Hoshikuzu no Tabibito [1080p].mkv"), None);
        assert_eq!(episode_in("Hoshikuzu no Tabibito 1080p.mkv"), None);
        assert_eq!(episode_in("Hoshikuzu no Tabibito 2023.mkv"), None);
        assert_eq!(episode_in("Hoshikuzu no Tabibito.mkv"), None);
    }

    #[test]
    fn a_season_folder_or_number_is_not_the_episode() {
        assert_eq!(episode_in("Season 2/Hoshikuzu no Tabibito - 04.mkv"), Some(4));
        assert_eq!(episode_in("Hoshikuzu no Tabibito Season 2.mkv"), None);
        assert_eq!(episode_in("Hoshikuzu no Tabibito Season 2 - 06.mkv"), Some(6));
    }
}
