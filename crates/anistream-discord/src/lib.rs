//! Discord rich presence, over Discord's local IPC socket.
//!
//! A crate of its own so that a build that wants presence and no mpv (the Mac app's) takes this
//! and not `anistream-player`.
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
//! error — most people watching anime do not have it open. Presence is decoration, so
//! a missing socket, a refused handshake or a dropped connection must never produce a toast, never
//! block playback, and never appear in the failure ladder. The only trace is a debug log.
//!
//! **It reports what you are watching, never where it came from.** The provider, the file, the
//! magnet and the local URL are all deliberately absent from the payload: this is the one part of
//! anistream that publishes to a third party, and "Hoshikuzu · Episode 5" is the whole of what
//! a presence needs to say.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU32, Ordering},
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod ipc;

use ipc::IpcStream;

/// Opcodes. Only these two are needed: a presence connection lives for exactly one episode, so
/// there is nothing long-lived enough to want the keepalive ping.
const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;

/// Discord's IPC protocol version.
const IPC_VERSION: &str = "1";

/// Refuse absurd frames rather than allocating whatever a socket claims.
const MAX_FRAME: u32 = 64 * 1024;

/// Discord's activity type for "Watching".
const TYPE_WATCHING: u8 = 3;

/// `status_display_type` 2: the member list's status line shows `details`, the title, rather than
/// the application's name.
const STATUS_DISPLAY_DETAILS: u8 = 2;

/// The app icon, uploaded as an art asset of this name in the Discord application. Sent wherever
/// there is no cover, so a presence never shows an empty square.
pub const LOGO: &str = "logo";

/// What the player is doing, which the presence says apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
    /// Held waiting for data.
    Buffering,
}

impl PlayState {
    fn words(self) -> &'static str {
        match self {
            Self::Playing => "Watching",
            Self::Paused => "Paused",
            Self::Buffering => "Buffering",
        }
    }

    /// The key of the small image the application holds for the state.
    fn image(self) -> &'static str {
        match self {
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Buffering => "buffering",
        }
    }
}

/// A button under the presence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub label: String,
    pub url: String,
}

/// The richer presence: Discord's own "Watching" with a cover, a progress bar and a button.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Rich {
    /// An https URL of the cover, which Discord fetches itself and shows as the large image.
    pub cover_url: Option<String>,
    pub button: Option<Button>,
    /// Unix seconds the episode started and will end at, were it to play on at the current speed:
    /// what Discord draws its progress bar from. Never sent while paused or buffering.
    pub span: Option<(i64, i64)>,
}

/// What is currently on screen, as a presence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Activity {
    /// The series. Shown as the large line.
    pub title: String,
    /// `Episode 5`, or whatever the episode is called.
    pub detail: String,
    /// Whether it plays, which is worth distinguishing from watching.
    pub state: PlayState,
    /// Unix seconds when this episode started, for Discord's own elapsed timer.
    ///
    /// Sent rather than a formatted duration so the timer keeps counting between updates — without
    /// it a presence would only move when anistream happened to send something.
    pub started_at: Option<i64>,
    /// The richer presence, when asked for. Without it the payload is the plain one.
    pub rich: Option<Rich>,
}

impl Activity {
    /// The `activity` of a SET_ACTIVITY, as it goes on the wire.
    pub fn payload(&self) -> serde_json::Value {
        let playing = self.state == PlayState::Playing;
        let mut payload = serde_json::json!({
            "details": truncate(&self.title),
            "state": truncate(&self.detail),
        });
        let Some(rich) = &self.rich else {
            // A paused episode gets no timestamp: Discord's timer would keep counting while
            // nothing is happening, which is worse than showing no timer at all.
            if let Some(started) = self.started_at.filter(|_| playing) {
                payload["timestamps"] = serde_json::json!({ "start": started });
            }
            payload["assets"] =
                serde_json::json!({ "large_image": LOGO, "large_text": self.state.words() });
            return payload;
        };

        payload["type"] = TYPE_WATCHING.into();
        payload["status_display_type"] = STATUS_DISPLAY_DETAILS.into();
        let mut assets = serde_json::json!({
            "small_image": self.state.image(),
            "small_text": self.state.words(),
        });
        assets["large_image"] = rich.cover_url.as_deref().unwrap_or(LOGO).into();
        assets["large_text"] = truncate(&self.title).into();
        payload["assets"] = assets;
        // Start and end together make the bar; either alone would draw a timer instead.
        if let Some((start, end)) = rich.span.filter(|_| playing) {
            payload["timestamps"] = serde_json::json!({ "start": start, "end": end });
        }
        if let Some(button) = &rich.button {
            payload["buttons"] = serde_json::json!([{
                "label": truncate_to(&button.label, 32),
                "url": button.url,
            }]);
        }
        payload
    }
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
        Self::connect_via(client_id, &candidate_sockets()).await
    }

    /// [`connect`](Self::connect), looking only at these sockets.
    pub async fn connect_via(client_id: &str, sockets: &[PathBuf]) -> Option<Self> {
        for path in sockets {
            let Ok(stream) = ipc::connect(path).await else { continue };
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
        self.command(
            "SET_ACTIVITY",
            serde_json::json!({
                "pid": std::process::id(),
                "activity": activity.payload(),
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
    truncate_to(text, 128)
}

fn truncate_to(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    text.chars().take(limit - 1).collect::<String>() + "…"
}

/// Where Discord's IPC endpoint might be. See [`ipc::discord_endpoints`].
pub fn candidate_sockets() -> Vec<PathBuf> {
    ipc::discord_endpoints()
}

#[cfg(test)]
mod tests;
