//! One local-IPC transport, spelled differently per platform.
//!
//! Both things anistream talks to locally — mpv's JSON IPC and Discord's rich presence — use a
//! Unix domain socket on Unix and a named pipe on Windows. The byte protocol on top is identical,
//! so the split is confined here: everything above works against [`IpcStream`] and never names a
//! platform.
//!
//! Halves come from [`tokio::io::split`] rather than a socket-specific `into_split`, because that
//! is the one splitting API both types support.

use std::path::{Path, PathBuf};

#[cfg(unix)]
pub type IpcStream = tokio::net::UnixStream;

#[cfg(windows)]
pub type IpcStream = tokio::net::windows::named_pipe::NamedPipeClient;

pub type ReadHalf = tokio::io::ReadHalf<IpcStream>;
pub type WriteHalf = tokio::io::WriteHalf<IpcStream>;

/// Connect to a local endpoint.
///
/// On Windows `path` is a pipe name (`\\.\pipe\something`) rather than a filesystem path, and
/// opening is synchronous — the call either finds the pipe or does not. Callers retry, which also
/// covers `ERROR_PIPE_BUSY` from a server that has not looped back round to accept yet.
pub async fn connect(path: &Path) -> std::io::Result<IpcStream> {
    #[cfg(unix)]
    {
        tokio::net::UnixStream::connect(path).await
    }
    #[cfg(windows)]
    {
        tokio::net::windows::named_pipe::ClientOptions::new().open(path)
    }
}

/// Split a stream into halves that can be owned independently.
pub fn split(stream: IpcStream) -> (ReadHalf, WriteHalf) {
    tokio::io::split(stream)
}

/// Where to ask mpv to put its IPC endpoint for one session.
///
/// `dir` is only meaningful on Unix, where the endpoint is a real file. On Windows the pipe
/// namespace is flat and global, so the directory is ignored and the name carries the uniqueness
/// instead.
pub fn mpv_endpoint(dir: &Path, unique: u64) -> PathBuf {
    #[cfg(unix)]
    {
        dir.join(format!("mpv-{unique}.sock"))
    }
    #[cfg(windows)]
    {
        let _ = dir;
        PathBuf::from(format!(r"\\.\pipe\mpv-{unique}"))
    }
}

/// Remove an endpoint left behind by a finished session.
///
/// A no-op on Windows: a named pipe is owned by the process that created it and disappears with
/// it, so there is nothing on disk to unlink.
pub async fn remove_endpoint(path: &Path) {
    #[cfg(unix)]
    {
        let _ = tokio::fs::remove_file(path).await;
    }
    #[cfg(windows)]
    {
        let _ = path;
    }
}

/// Blocking form of [`remove_endpoint`], for `Drop`.
pub fn remove_endpoint_blocking(path: &Path) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(path);
    }
    #[cfg(windows)]
    {
        let _ = path;
    }
}

/// Where Discord's IPC endpoint might be.
///
/// Numbered `0`–`9` because a second Discord instance takes the next slot. On Unix the directory
/// varies by platform *and* by how Discord was installed — Flatpak and Snap each nest it further —
/// so a short list of candidates is far more robust than getting one path right. On Windows the
/// pipe namespace is flat and there is only one place to look.
pub fn discord_endpoints() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        (0..10).map(|i| PathBuf::from(format!(r"\\.\pipe\discord-ipc-{i}"))).collect()
    }
    #[cfg(unix)]
    {
        let mut bases: Vec<PathBuf> = Vec::new();
        // macOS puts it in the per-user temporary directory; Linux in the runtime dir.
        for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
            if let Ok(value) = std::env::var(key)
                && !value.is_empty()
            {
                bases.push(PathBuf::from(value));
            }
        }
        bases.push(PathBuf::from("/tmp"));

        let mut paths = Vec::new();
        for base in bases {
            for nested in [
                "",
                "app/com.discordapp.Discord",
                "snap.discord",
                ".flatpak/dev.vencord.Vesktop/xdg-run",
            ] {
                let dir = if nested.is_empty() { base.clone() } else { base.join(nested) };
                for index in 0..10 {
                    paths.push(dir.join(format!("discord-ipc-{index}")));
                }
            }
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_endpoint_is_unique_per_session() {
        let dir = Path::new("/tmp/anistream-test");
        assert_ne!(mpv_endpoint(dir, 1), mpv_endpoint(dir, 2));
    }

    #[test]
    fn discord_candidates_cover_ten_slots() {
        // A second Discord instance takes the next slot, so one path would miss it.
        let paths = discord_endpoints();
        for index in 0..10 {
            let suffix = format!("discord-ipc-{index}");
            assert!(
                paths.iter().any(|p| p.to_string_lossy().ends_with(&suffix)),
                "no candidate for slot {index}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_endpoints_live_in_the_pipe_namespace() {
        assert!(mpv_endpoint(Path::new("ignored"), 7).to_string_lossy().starts_with(r"\\.\pipe\"));
        assert!(
            discord_endpoints()
                .iter()
                .all(|p| p.to_string_lossy().starts_with(r"\\.\pipe\"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_endpoints_live_under_the_given_directory() {
        let path = mpv_endpoint(Path::new("/run/user/1000"), 7);
        assert!(path.starts_with("/run/user/1000"), "got {}", path.display());
    }
}
