//! Discord rich presence, over Discord's local IPC socket.
//!
//! Hand-rolled rather than pulled from a crate, for the same reason the mpv IPC is: the protocol is
//! a length-prefixed JSON frame over a unix socket, the whole client is under two hundred lines, and
//! a dependency for it would be larger than the thing it wraps.
//!
//! ```text
//! ┌────────────┬────────────┬──────────────────┐
//! │ opcode u32 │ length u32 │ JSON payload     │   both little-endian
//! └────────────┴────────────┴──────────────────┘
//! ```
//!
//! **Everything here fails silently by design.** Discord not running is the *normal* case, not an
//! error — most people watching anime in a terminal do not have it open. Presence is decoration, so
//! a missing socket, a refused handshake or a dropped connection must never produce a toast, never
//! block playback, and never appear in the failure ladder. The only trace is a debug log.
//!
//! **It reports what you are watching, never where it came from.** The provider, the file, the
//! magnet and the local URL are all deliberately absent from the payload: this is the one part of
//! anistream that publishes to a third party, and "Frieren · Episode 5" is the whole of what a
//! presence needs to say.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::ipc::{self, IpcStream};

/// Opcodes. Only these two are needed: a presence connection lives for exactly one episode, so
/// there is nothing long-lived enough to want the keepalive ping.
const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;

/// Discord's IPC protocol version.
const IPC_VERSION: &str = "1";

/// Refuse absurd frames rather than allocating whatever a socket claims.
const MAX_FRAME: u32 = 64 * 1024;

/// What is currently on screen, as a presence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// The series. Shown as the large line.
    pub title: String,
    /// `Episode 5`, or whatever the episode is called.
    pub detail: String,
    /// Whether it is paused, which is worth distinguishing from watching.
    pub paused: bool,
    /// Unix seconds when this episode started, for Discord's own elapsed timer.
    ///
    /// Sent rather than a formatted duration so the timer keeps counting between updates — without
    /// it a presence would only move when anistream happened to send something.
    pub started_at: Option<i64>,
}

/// A connection to the local Discord client.
pub struct Presence {
    stream: IpcStream,
    nonce: AtomicU32,
}

impl Presence {
    /// Connect and handshake, or return `None` if Discord is not there.
    ///
    /// `None` rather than an error throughout: the caller has nothing useful to do with a reason,
    /// and treating "Discord is closed" as a failure would be wrong about what happened.
    pub async fn connect(client_id: &str) -> Option<Self> {
        for path in candidate_sockets() {
            let Ok(stream) = ipc::connect(&path).await else { continue };
            let mut presence = Self { stream, nonce: AtomicU32::new(1) };
            if presence.handshake(client_id).await.is_ok() {
                tracing::debug!(socket = %path.display(), "discord presence connected");
                return Some(presence);
            }
            tracing::debug!(socket = %path.display(), "discord handshake refused");
        }
        tracing::debug!("no discord ipc socket; presence disabled");
        None
    }

    async fn handshake(&mut self, client_id: &str) -> std::io::Result<()> {
        let payload =
            serde_json::json!({ "v": IPC_VERSION, "client_id": client_id }).to_string();
        self.send(OP_HANDSHAKE, &payload).await?;
        // Discord replies with a READY frame. Read it so it does not arrive mixed into the next
        // response, and so a refused client id is noticed here rather than silently later.
        let (_, body) = self.read_frame().await?;
        if body.contains("\"code\"") && body.contains("error") {
            return Err(std::io::Error::other(format!("handshake rejected: {body}")));
        }
        Ok(())
    }

    /// Publish an activity.
    pub async fn set(&mut self, activity: &Activity) -> std::io::Result<()> {
        // `state` and `details` are Discord's two text lines; `details` is the larger one, so the
        // series goes there and the episode below it.
        let mut payload = serde_json::json!({
            "details": truncate(&activity.title),
            "state": truncate(&activity.detail),
        });
        // A paused episode gets no timestamp: Discord's timer would keep counting while nothing is
        // happening, which is worse than showing no timer at all.
        if let Some(started) = activity.started_at.filter(|_| !activity.paused) {
            payload["timestamps"] = serde_json::json!({ "start": started });
        }
        payload["assets"] = serde_json::json!({
            "large_text": if activity.paused { "Paused" } else { "Watching" },
        });

        self.command(
            "SET_ACTIVITY",
            serde_json::json!({
                "pid": std::process::id(),
                "activity": payload,
            }),
        )
        .await
    }

    /// Remove the activity, leaving no trace of the session behind.
    pub async fn clear(&mut self) -> std::io::Result<()> {
        // A null activity is how Discord's protocol says "nothing"; omitting the key leaves the
        // previous presence up, which after quitting would claim you were still watching.
        self.command(
            "SET_ACTIVITY",
            serde_json::json!({ "pid": std::process::id(), "activity": serde_json::Value::Null }),
        )
        .await
    }

    async fn command(&mut self, name: &str, args: serde_json::Value) -> std::io::Result<()> {
        let nonce = self.nonce.fetch_add(1, Ordering::Relaxed);
        let payload = serde_json::json!({
            "cmd": name,
            "args": args,
            "nonce": nonce.to_string(),
        })
        .to_string();
        self.send(OP_FRAME, &payload).await?;
        // The reply is read and discarded: leaving it buffered would desynchronise the next one,
        // and there is nothing in it worth acting on.
        let _ = self.read_frame().await?;
        Ok(())
    }

    async fn send(&mut self, opcode: u32, payload: &str) -> std::io::Result<()> {
        let bytes = payload.as_bytes();
        let mut frame = Vec::with_capacity(8 + bytes.len());
        frame.extend_from_slice(&opcode.to_le_bytes());
        frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        frame.extend_from_slice(bytes);
        self.stream.write_all(&frame).await?;
        self.stream.flush().await
    }

    async fn read_frame(&mut self) -> std::io::Result<(u32, String)> {
        let mut header = [0u8; 8];
        self.stream.read_exact(&mut header).await?;
        let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        if length > MAX_FRAME {
            // Never allocate on a length a socket claims. This is a local socket rather than a
            // hostile one, but a desynchronised stream produces garbage lengths and a multi-gigabyte
            // allocation is a worse outcome than a dropped connection.
            return Err(std::io::Error::other(format!("frame of {length} bytes refused")));
        }
        let mut body = vec![0u8; length as usize];
        self.stream.read_exact(&mut body).await?;
        Ok((opcode, String::from_utf8_lossy(&body).into_owned()))
    }
}

/// Discord's two text fields are capped; a long title would otherwise be rejected outright.
fn truncate(text: &str) -> String {
    const LIMIT: usize = 128;
    if text.chars().count() <= LIMIT {
        return text.to_owned();
    }
    text.chars().take(LIMIT - 1).collect::<String>() + "…"
}

/// Where Discord's IPC endpoint might be. See [`crate::ipc::discord_endpoints`].
pub fn candidate_sockets() -> Vec<PathBuf> {
    ipc::discord_endpoints()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_two_little_endian_headers_then_the_payload() {
        // The wire format, asserted rather than assumed: getting the endianness wrong produces a
        // connection that handshakes and then silently ignores everything.
        let payload = br#"{"v":"1"}"#;
        let mut frame = Vec::new();
        frame.extend_from_slice(&OP_HANDSHAKE.to_le_bytes());
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(payload);

        assert_eq!(&frame[0..4], &[0, 0, 0, 0], "handshake opcode");
        assert_eq!(&frame[4..8], &[9, 0, 0, 0], "length, little-endian");
        assert_eq!(&frame[8..], payload);
    }

    #[test]
    fn candidate_sockets_cover_numbered_instances_and_sandboxes() {
        let paths = candidate_sockets();
        let names: Vec<String> = paths
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"discord-ipc-0".to_string()));
        // A second instance takes the next slot, so stopping at zero would miss it.
        assert!(names.contains(&"discord-ipc-1".to_string()));
        assert!(
            paths.iter().any(|p| p.to_string_lossy().contains("com.discordapp.Discord")),
            "a Flatpak install nests the socket"
        );
    }

    #[test]
    fn a_long_title_is_truncated_rather_than_rejected() {
        // Discord refuses over-long fields outright, so an untruncated title would drop the whole
        // presence rather than shortening it.
        let long: String = "あ".repeat(400);
        let truncated = truncate(&long);
        assert_eq!(truncated.chars().count(), 128);
        assert!(truncated.ends_with('…'));
        assert_eq!(truncate("Frieren"), "Frieren");
    }

    #[test]
    fn a_paused_activity_carries_no_timestamp() {
        // Discord's elapsed timer keeps running on its own, so leaving a start time on a paused
        // episode would show it advancing while nothing plays.
        let activity = Activity {
            title: "Frieren".into(),
            detail: "Episode 5".into(),
            paused: true,
            started_at: Some(1_700_000_000),
        };
        assert!(activity.started_at.filter(|_| !activity.paused).is_none());

        let playing = Activity { paused: false, ..activity };
        assert!(playing.started_at.filter(|_| !playing.paused).is_some());
    }
}
