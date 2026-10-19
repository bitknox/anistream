//! Golden files: text compared against a committed copy, rewritten only when asked.
//!
//! A golden lives at `tests/goldens/<name>.txt` in the crate whose test calls
//! [`assert_golden`]. The name may hold slashes, to group goldens in directories. With
//! `ANISTREAM_BLESS=1` set the file is written instead of compared, so an intended change is
//! accepted by rerunning the suite and reviewing the diff in version control.
//!
//! Line endings are compared as `\n` whatever the checkout did to them, and the text always
//! ends in exactly one newline as written, so neither a Windows checkout nor an editor that
//! adds a final newline makes a golden fail.

use std::path::Path;

/// The variable that turns a comparison into a rewrite. Any value but empty or `0` counts.
pub const BLESS_VAR: &str = "ANISTREAM_BLESS";

/// Lines of unchanged text shown around each change in a mismatch.
const CONTEXT: usize = 3;

/// Compare `actual` against the golden called `name`, or rewrite it under `ANISTREAM_BLESS=1`.
///
/// Panics on a mismatch, with a line diff, and on a missing golden, naming the variable that
/// writes it. A missing golden fails rather than writing itself, so a suite that has lost its
/// goldens cannot pass in CI by blessing them as it goes.
///
/// The calling crate is found through `CARGO_MANIFEST_DIR`, which cargo and nextest both set
/// for the tests they run.
#[track_caller]
pub fn assert_golden(name: &str, actual: &str) {
    let Some(root) = std::env::var_os("CARGO_MANIFEST_DIR") else {
        panic!("CARGO_MANIFEST_DIR is unset: run golden tests through cargo");
    };
    let dir = Path::new(&root).join("tests").join("goldens");
    if let Err(message) = check_golden(&dir, name, actual, blessing()) {
        panic!("{message}");
    }
}

/// [`assert_golden`]'s work, with the directory and the blessing passed in rather than taken
/// from the environment. `Err` carries the message a test would fail with.
pub fn check_golden(dir: &Path, name: &str, actual: &str, bless: bool) -> Result<(), String> {
    let path = dir.join(format!("{name}.txt"));
    let actual = normalised(actual);
    if bless {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, &actual)
            .map_err(|e| format!("writing {}: {e}", path.display()))?;
        eprintln!("blessed {}", path.display());
        return Ok(());
    }

    let expected = match std::fs::read_to_string(&path) {
        Ok(text) => normalised(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "no golden at {}; run with {BLESS_VAR}=1 to write it",
                path.display()
            ));
        }
        Err(e) => return Err(format!("reading {}: {e}", path.display())),
    };
    if expected == actual {
        return Ok(());
    }
    Err(format!(
        "golden {name:?} differs from {} (- golden, + actual); rerun with {BLESS_VAR}=1 to \
         accept the change:\n{}",
        path.display(),
        diff(&expected, &actual)
    ))
}

fn blessing() -> bool {
    std::env::var(BLESS_VAR).is_ok_and(|v| !v.is_empty() && v != "0")
}

fn normalised(text: &str) -> String {
    let mut text = text.replace("\r\n", "\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// One line of a diff: unchanged, only in the golden, or only in the actual text.
struct Line<'a> {
    tag: char,
    /// One-based line numbers in the golden and the actual text, at this point in the walk.
    golden: usize,
    actual: usize,
    text: &'a str,
}

/// A line diff in hunks, each headed by where it starts in both texts.
///
/// A longest-common-subsequence walk: quadratic, which is nothing at the size of a screen and
/// keeps the kit free of a diff dependency. Removals come before additions at each change, so a
/// changed line reads as the old one directly above the new.
fn diff(expected: &str, actual: &str) -> String {
    let old: Vec<&str> = expected.lines().collect();
    let new: Vec<&str> = actual.lines().collect();
    let (n, m) = (old.len(), new.len());
    let mut common = vec![vec![0_u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            common[i][j] = if old[i] == new[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }

    let mut lines = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        let (golden, actual) = (i + 1, j + 1);
        if i < n && j < m && old[i] == new[j] {
            lines.push(Line { tag: ' ', golden, actual, text: old[i] });
            (i, j) = (i + 1, j + 1);
        } else if i < n && (j == m || common[i + 1][j] >= common[i][j + 1]) {
            lines.push(Line { tag: '-', golden, actual, text: old[i] });
            i += 1;
        } else {
            lines.push(Line { tag: '+', golden, actual, text: new[j] });
            j += 1;
        }
    }

    let mut shown = vec![false; lines.len()];
    for (index, line) in lines.iter().enumerate() {
        if line.tag != ' ' {
            let end = (index + CONTEXT).min(lines.len() - 1);
            shown[index.saturating_sub(CONTEXT)..=end].fill(true);
        }
    }
    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        if !shown[index] {
            continue;
        }
        if index == 0 || !shown[index - 1] {
            out.push_str(&format!(
                "@@ golden line {}, actual line {} @@\n",
                line.golden, line.actual
            ));
        }
        out.push(line.tag);
        out.push(' ');
        out.push_str(line.text);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blessing_writes_the_golden_and_then_it_matches() {
        let dir = tempfile::tempdir().unwrap();
        let text = "first\nsecond";
        let missing = check_golden(dir.path(), "screens/home", text, false).unwrap_err();
        assert!(
            missing.contains(BLESS_VAR),
            "a missing golden says how to write it: {missing}"
        );

        check_golden(dir.path(), "screens/home", text, true).unwrap();
        let written = std::fs::read_to_string(dir.path().join("screens/home.txt")).unwrap();
        assert_eq!(written, "first\nsecond\n", "written with one final newline");
        check_golden(dir.path(), "screens/home", text, false).unwrap();
        check_golden(dir.path(), "screens/home", "first\nsecond\n", false).unwrap();
    }

    #[test]
    fn a_mismatch_shows_the_changed_line_in_context() {
        let dir = tempfile::tempdir().unwrap();
        let golden: String = (1..=20).map(|n| format!("row {n}\n")).collect();
        check_golden(dir.path(), "rows", &golden, true).unwrap();

        let changed = golden.replace("row 12\n", "row twelve\n");
        let message = check_golden(dir.path(), "rows", &changed, false).unwrap_err();
        assert!(message.contains("- row 12\n+ row twelve\n"), "{message}");
        assert!(message.contains("@@ golden line 9, actual line 9 @@"), "{message}");
        assert!(message.contains("  row 9\n") && message.contains("  row 15\n"), "{message}");
        assert!(!message.contains("row 8\n") && !message.contains("row 16\n"), "{message}");
    }

    #[test]
    fn lines_added_and_removed_are_both_shown() {
        let shown = diff("a\nb\nc\n", "a\nc\nd\n");
        assert_eq!(shown, "@@ golden line 1, actual line 1 @@\n  a\n- b\n  c\n+ d\n");
    }

    #[test]
    fn a_windows_checkout_still_matches() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("crlf.txt"), "one\r\ntwo\r\n").unwrap();
        check_golden(dir.path(), "crlf", "one\ntwo\n", false).unwrap();
    }
}
