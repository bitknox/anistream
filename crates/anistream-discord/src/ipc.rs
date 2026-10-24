//! Discord's local IPC endpoint: a Unix domain socket on Unix and a named pipe on Windows. The
//! byte protocol on top is identical, so the split is confined here.

use std::path::{Path, PathBuf};

#[cfg(unix)]
pub type IpcStream = tokio::net::UnixStream;

#[cfg(windows)]
pub type IpcStream = tokio::net::windows::named_pipe::NamedPipeClient;

/// Connect to a local endpoint.
///
/// On Windows `path` is a pipe name (`\\.\pipe\something`) rather than a filesystem path, and
/// opening is synchronous: the call either finds the pipe or does not.
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
}
