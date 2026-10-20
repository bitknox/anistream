//! Muxing sidecar subtitles into a finished download.
//!
//! Fansub releases routinely ship `Episode.mkv` beside `Episode.ass`, or a `Subs/` folder holding
//! one file per language. That works while the folder structure survives, and stops working the
//! moment the video is copied anywhere on its own — onto a phone, into a different player, across a
//! network share. Muxing them in makes the file self-contained.
//!
//! **Stream copy only.** `-c copy` re-encodes nothing, so this costs seconds and cannot degrade the
//! video. Anything that would require a real transcode is refused rather than attempted quietly.
//!
//! **Never fatal.** A failed remux leaves the original untouched and the subtitles where they were,
//! which mpv finds by itself. The download is already complete and playable; this is an improvement
//! on it, not a step in it.

use std::path::{Path, PathBuf};

/// Subtitle extensions worth muxing.
///
/// Text formats only. Muxing an image-based `.sup` into Matroska works but doubles as a trap: it is
/// large, cannot be restyled, and is nearly always a rip artefact rather than what a fansub shipped.
const SUBTITLE_EXTENSIONS: &[&str] = &["ass", "ssa", "srt", "vtt"];

/// Folders a release puts its subtitles in.
const SUBTITLE_DIRECTORIES: &[&str] = &["subs", "subtitles"];

#[derive(Debug, thiserror::Error)]
pub enum RemuxError {
    #[error("ffmpeg is not installed or not on PATH")]
    NotInstalled,
    #[error("ffmpeg exited with {code}: {message}")]
    Failed { code: i32, message: String },
    #[error("{0}")]
    Io(String),
}

/// What a merge did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merge {
    /// Muxed in place. No path comes back: the caller passed one in and it is unchanged.
    Merged { tracks: usize },
    /// No sidecar subtitles, or the container already carries them.
    NothingToDo,
}

/// Find the sidecar subtitles belonging to `video`.
///
/// Matched on the video's stem, so an episode in a season folder does not adopt its neighbours'
/// subtitles — which is the obvious failure of "every `.ass` in this directory".
fn sidecars_for(video: &Path) -> Vec<PathBuf> {
    let Some(stem) = video.file_stem().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    let Some(parent) = video.parent() else {
        return Vec::new();
    };

    let mut found = Vec::new();
    let mut search = vec![parent.to_path_buf()];
    // Releases commonly keep subtitles one level down.
    if let Ok(entries) = std::fs::read_dir(parent) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if entry.path().is_dir() && SUBTITLE_DIRECTORIES.contains(&name.as_str()) {
                search.push(entry.path());
            }
        }
    }

    for dir in search {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(extension) =
                path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase)
            else {
                continue;
            };
            if !SUBTITLE_EXTENSIONS.contains(&extension.as_str()) {
                continue;
            }
            // The stem has to match, but not exactly: `Episode 01.eng.ass` belongs to
            // `Episode 01.mkv`, and requiring equality would miss every language-tagged file.
            let Some(candidate) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            if candidate == stem || candidate.starts_with(&format!("{stem}.")) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Mux any sidecar subtitles into `video`, in place.
///
/// Writes to a temporary file and renames over the original only on success, so an interrupted
/// remux cannot leave a truncated video where a complete one was.
pub async fn merge_sidecar_subtitles(video: &Path) -> Result<Merge, RemuxError> {
    let subtitles = sidecars_for(video);
    if subtitles.is_empty() {
        return Ok(Merge::NothingToDo);
    }
    // Matroska is the only container here that reliably carries ASS, which is what fansubs use.
    // Muxing into MP4 would silently convert styling away, so it is refused as nothing-to-do
    // rather than done badly.
    let container =
        video.extension().and_then(|e| e.to_str()).unwrap_or_default().to_lowercase();
    if container != "mkv" {
        tracing::info!(
            container = %container,
            "sidecar subtitles left alone: only Matroska carries ASS styling faithfully"
        );
        return Ok(Merge::NothingToDo);
    }

    let temporary = video.with_extension("remux.mkv");
    let mut command = tokio::process::Command::new("ffmpeg");
    command.arg("-nostdin").arg("-y").arg("-loglevel").arg("error");
    command.arg("-i").arg(video);
    for subtitle in &subtitles {
        command.arg("-i").arg(subtitle);
    }
    // Map the video's own streams, then one subtitle stream per input file.
    command.arg("-map").arg("0");
    for index in 1..=subtitles.len() {
        command.arg("-map").arg(format!("{index}:0"));
    }
    command.arg("-c").arg("copy");
    // Label each track with the language the filename claims, when it claims one. Unlabelled
    // subtitle tracks are what make a player pick the wrong one.
    for (slot, subtitle) in subtitles.iter().enumerate() {
        if let Some(language) = language_of(subtitle) {
            command.arg(format!("-metadata:s:s:{slot}")).arg(format!("language={language}"));
        }
    }
    command.arg(&temporary);

    let output = command.output().await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RemuxError::NotInstalled
        } else {
            RemuxError::Io(e.to_string())
        }
    })?;

    if !output.status.success() {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(RemuxError::Failed {
            code: output.status.code().unwrap_or(-1),
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    tokio::fs::rename(&temporary, video)
        .await
        .map_err(|e| RemuxError::Io(format!("replacing {}: {e}", video.display())))?;

    tracing::info!(
        video = %video.display(),
        tracks = subtitles.len(),
        "subtitles muxed into the download"
    );
    Ok(Merge::Merged { tracks: subtitles.len() })
}

/// The language a subtitle filename claims, as an ISO 639 code if it looks like one.
///
/// `Episode 01.eng.ass` → `eng`. Guessing beyond that would be worse than leaving it unset: a
/// wrongly-labelled track is harder to notice than an unlabelled one.
fn language_of(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let tag = stem.rsplit('.').next()?;
    let looks_like_a_code =
        (2..=3).contains(&tag.len()) && tag.chars().all(|c| c.is_ascii_alphabetic());
    looks_like_a_code.then(|| tag.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("anistream-remux-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn a_matching_sidecar_is_found() {
        let dir = scratch("basic");
        let video = dir.join("Episode 01.mkv");
        touch(&video);
        touch(&dir.join("Episode 01.ass"));
        assert_eq!(sidecars_for(&video), vec![dir.join("Episode 01.ass")]);
    }

    #[test]
    fn a_neighbours_subtitles_are_not_adopted() {
        // The failure mode of "every .ass in this directory": a season folder would give every
        // episode every other episode's subtitles.
        let dir = scratch("neighbour");
        let video = dir.join("Episode 01.mkv");
        touch(&video);
        touch(&dir.join("Episode 01.ass"));
        touch(&dir.join("Episode 02.ass"));
        assert_eq!(sidecars_for(&video), vec![dir.join("Episode 01.ass")]);
    }

    #[test]
    fn language_tagged_files_are_matched_and_labelled() {
        let dir = scratch("languages");
        let video = dir.join("Episode 01.mkv");
        touch(&video);
        touch(&dir.join("Episode 01.eng.ass"));
        touch(&dir.join("Episode 01.spa.srt"));
        let found = sidecars_for(&video);
        assert_eq!(found.len(), 2, "got {found:?}");
        assert_eq!(language_of(&dir.join("Episode 01.eng.ass")).as_deref(), Some("eng"));
        assert_eq!(language_of(&dir.join("Episode 01.spa.srt")).as_deref(), Some("spa"));
    }

    #[test]
    fn a_subs_folder_is_searched() {
        let dir = scratch("subsdir");
        let video = dir.join("Episode 01.mkv");
        touch(&video);
        std::fs::create_dir_all(dir.join("Subs")).unwrap();
        touch(&dir.join("Subs/Episode 01.ass"));
        assert_eq!(sidecars_for(&video), vec![dir.join("Subs/Episode 01.ass")]);
    }

    #[test]
    fn a_bare_stem_is_not_read_as_a_language() {
        // `Episode 01.ass` has no language tag, and inventing one would mislabel the track.
        let dir = scratch("nolang");
        assert_eq!(language_of(&dir.join("Episode 01.ass")), None);
        // Nor is a long trailing word a code.
        assert_eq!(language_of(&dir.join("Episode.english.ass")), None);
    }

    #[test]
    fn image_subtitles_and_unrelated_files_are_ignored() {
        let dir = scratch("ignored");
        let video = dir.join("Episode 01.mkv");
        touch(&video);
        touch(&dir.join("Episode 01.sup"));
        touch(&dir.join("Episode 01.txt"));
        touch(&dir.join("Episode 01.nfo"));
        assert!(sidecars_for(&video).is_empty());
    }

    #[tokio::test]
    async fn a_video_with_no_sidecars_is_left_completely_alone() {
        let dir = scratch("noop");
        let video = dir.join("Episode 01.mkv");
        touch(&video);
        assert_eq!(merge_sidecar_subtitles(&video).await.unwrap(), Merge::NothingToDo);
        assert_eq!(std::fs::read(&video).unwrap(), b"x", "the file must not be touched");
    }

    #[tokio::test]
    async fn a_non_matroska_container_is_declined_rather_than_mangled() {
        // Muxing ASS into MP4 loses the styling silently, which is worse than not doing it.
        let dir = scratch("mp4");
        let video = dir.join("Episode 01.mp4");
        touch(&video);
        touch(&dir.join("Episode 01.ass"));
        assert_eq!(merge_sidecar_subtitles(&video).await.unwrap(), Merge::NothingToDo);
    }

    #[tokio::test]
    async fn a_failed_remux_leaves_the_original_intact() {
        // The input here is not a real Matroska file, so ffmpeg refuses it — which is exactly the
        // case that must not destroy the download.
        let dir = scratch("failure");
        let video = dir.join("Episode 01.mkv");
        touch(&video);
        touch(&dir.join("Episode 01.ass"));

        match merge_sidecar_subtitles(&video).await {
            // ffmpeg absent is a legitimate outcome on a machine without it.
            Err(RemuxError::NotInstalled) => {}
            Err(RemuxError::Failed { .. }) => {
                assert_eq!(std::fs::read(&video).unwrap(), b"x", "original was damaged");
                assert!(
                    !dir.join("Episode 01.remux.mkv").exists(),
                    "temporary file left behind"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
