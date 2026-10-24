//! What crosses the connection: frames of JSON, and the messages in them.
//!
//! A frame is a four-byte big-endian length and that many bytes of UTF-8 JSON, at most
//! [`MAX_FRAME`]. Every message is an object tagged by its `type`. A message of a type this side
//! does not know is read as `Unknown` and answered, never a reason to drop the connection, so a
//! newer phone and an older TV still work together on what both know.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{Error, command::RemoteCommand, state::RemoteState};

/// The protocol's major version. A change that an older side would misread raises it; adding a
/// message, a command or a field does not. See the design doc.
pub const PROTOCOL: u32 = 1;

/// The largest frame either side sends or takes. State with a full track list is a few KB.
pub const MAX_FRAME: usize = 64 * 1024;

/// What a phone says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// First, on every connection: which protocol, and what to call this phone.
    Hello { protocol: u32, name: String, app: String },
    /// Ask the TV to show a pairing code.
    PairRequest,
    /// The phone's half of the exchange, made from the code the viewer typed.
    PairStart { message: String },
    /// The phone's proof that it reached the same key.
    PairConfirm { mac: String },
    /// Do something on the TV. `id` is the phone's own number, answered in `Ack` or `Rejected`.
    Command { id: u32, command: RemoteCommand },
    /// Forget this phone on the TV.
    Unpair,
    /// Still here: sent only while the phone's remote screen is open and nothing else was sent.
    Ping,
    #[serde(other)]
    Unknown,
}

/// What a TV says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        protocol: u32,
        tv_name: String,
        paired: bool,
    },
    /// A code is on the TV's screen now, for `expires_in` seconds.
    PairCode {
        digits: u8,
        expires_in: u32,
    },
    /// The TV's half of the exchange, and its proof.
    PairReply {
        message: String,
        mac: String,
    },
    /// This phone is paired; state follows.
    Paired,
    PairFailed {
        reason: PairFailure,
    },
    /// The whole of what a remote shows, after every change. Never a delta: a phone that missed
    /// one loses nothing.
    State(RemoteState),
    Ack {
        id: u32,
    },
    Rejected {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u32>,
        reason: Rejection,
    },
    Pong,
    /// This phone was removed on the TV. The connection closes after this.
    Revoked,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairFailure {
    /// The code was wrong. The TV keeps the code for another try, up to its limit.
    WrongCode,
    /// No code is showing: none was asked for, it expired, or too many tries used it up.
    NoCode,
    /// The TV has as many paired phones as it keeps.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    /// Only pairing is open to a phone the TV does not know.
    NotPaired,
    /// No `Hello` yet, or a protocol this TV does not speak.
    Version,
    /// A message or command this TV does not know.
    Unsupported,
    /// The TV is not in a state to do it: nothing playing for a player command.
    NotNow,
    /// As many phones are connected as the TV serves at once.
    Busy,
    #[serde(other)]
    Other,
}

pub(crate) async fn write_frame<W, M>(writer: &mut W, message: &M) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
    M: Serialize,
{
    let body = serde_json::to_vec(message)?;
    if body.len() > MAX_FRAME {
        return Err(Error::Frame("a message too large to send"));
    }
    let len =
        u32::try_from(body.len()).map_err(|_| Error::Frame("a message too large to send"))?;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

/// The next frame, or `None` when the other side closed cleanly between frames.
pub(crate) async fn read_frame<R, M>(reader: &mut R) -> Result<Option<M>, Error>
where
    R: AsyncRead + Unpin,
    M: for<'de> Deserialize<'de>,
{
    match read_raw(reader).await? {
        Some(body) => Ok(Some(serde_json::from_slice(&body)?)),
        None => Ok(None),
    }
}

/// The `id` of a command that could not be read whole, so its refusal can name it.
pub(crate) fn command_id(body: &[u8]) -> Option<u32> {
    #[derive(Deserialize)]
    struct Id {
        id: u32,
    }
    serde_json::from_slice::<Id>(body).ok().map(|i| i.id)
}

/// The next frame's bytes, unread.
pub(crate) async fn read_raw<R>(reader: &mut R) -> Result<Option<Vec<u8>>, Error>
where
    R: AsyncRead + Unpin,
{
    let mut len = [0; 4];
    match reader.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_FRAME {
        return Err(Error::Frame("a frame over the limit"));
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).await?;
    Ok(Some(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_of_an_unknown_type_is_read_not_refused() {
        let read: ClientMessage =
            serde_json::from_str(r#"{"type":"teleport","to":"moon"}"#).unwrap();
        assert_eq!(read, ClientMessage::Unknown);
        let read: ServerMessage = serde_json::from_str(r#"{"type":"fireworks"}"#).unwrap();
        assert_eq!(read, ServerMessage::Unknown);
    }

    #[test]
    fn messages_read_as_the_doc_writes_them() {
        let hello: ClientMessage = serde_json::from_str(
            r#"{"type":"hello","protocol":1,"name":"Kitchen phone","app":"anistream-android/2.0","extra":true}"#,
        )
        .unwrap();
        assert!(matches!(hello, ClientMessage::Hello { protocol: 1, .. }));
        let json = serde_json::to_string(&ServerMessage::Rejected {
            id: Some(4),
            reason: Rejection::NotPaired,
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"rejected","id":4,"reason":"not_paired"}"#);
    }

    #[tokio::test]
    async fn frames_round_trip_and_an_oversized_one_is_refused() {
        let (mut a, mut b) = tokio::io::duplex(1 << 20);
        write_frame(&mut a, &ClientMessage::Ping).await.unwrap();
        let read: Option<ClientMessage> = read_frame(&mut b).await.unwrap();
        assert_eq!(read, Some(ClientMessage::Ping));

        a.write_all(&(MAX_FRAME as u32 + 1).to_be_bytes()).await.unwrap();
        let read: Result<Option<ClientMessage>, _> = read_frame(&mut b).await;
        assert!(matches!(read, Err(Error::Frame(_))));
    }
}
