//! One local-IPC transport, spelled differently per platform.
//!
//! mpv's JSON IPC is a Unix domain socket on Unix and a named pipe on Windows. The split is
//! confined here: everything above works against [`IpcStream`] and never names a
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_endpoint_is_unique_per_session() {
        let dir = Path::new("/tmp/anistream-test");
        assert_ne!(mpv_endpoint(dir, 1), mpv_endpoint(dir, 2));
    }

    #[cfg(windows)]
    #[test]
    fn windows_endpoints_live_in_the_pipe_namespace() {
        assert!(
            mpv_endpoint(Path::new("ignored"), 7).to_string_lossy().starts_with(r"\\.\pipe\")
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_endpoints_live_under_the_given_directory() {
        let path = mpv_endpoint(Path::new("/run/user/1000"), 7);
        assert!(path.starts_with("/run/user/1000"), "got {}", path.display());
    }
}
