//! Finding servers on the local network: a UDP broadcast to port 7359.

use std::{net::SocketAddr, time::Duration};

use serde::Deserialize;
use tokio::{net::UdpSocket, time::Instant};

/// Jellyfin answers a broadcast of this text on this port.
pub const DISCOVERY_PORT: u16 = 7359;
const QUESTION: &[u8] = b"Who is JellyfinServer?";

/// A server that answered.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Found {
    /// The address it says to reach it at, such as `http://192.168.1.20:8096`.
    pub address: String,
    pub id: String,
    pub name: String,
}

/// How long a search listens: servers answer within a moment, and a person waits no longer.
pub const WINDOW: Duration = Duration::from_secs(4);

/// Broadcast to `target` and collect answers until `window` runs out, handing each new server to
/// `found` as it arrives, once per server id. `target` is the broadcast address on the real
/// network, and a test's own socket in tests.
pub async fn discover(target: SocketAddr, window: Duration, mut found: impl FnMut(Found)) {
    let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)).await else { return };
    if socket.set_broadcast(true).is_err() || socket.send_to(QUESTION, target).await.is_err() {
        return;
    }
    let deadline = Instant::now() + window;
    let mut seen: Vec<String> = Vec::new();
    let mut buf = [0u8; 2048];
    while let Ok(Ok((len, _))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await
    {
        let Ok(server) = serde_json::from_slice::<Found>(&buf[..len]) else { continue };
        if !seen.contains(&server.id) {
            seen.push(server.id.clone());
            found(server);
        }
    }
}

/// The broadcast address of the local network.
pub fn broadcast_target() -> SocketAddr {
    SocketAddr::from(([255, 255, 255, 255], DISCOVERY_PORT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_server_that_answers_is_found_once() {
        let server = UdpSocket::bind(("127.0.0.1", 0)).await.unwrap();
        let target = server.local_addr().unwrap();
        let answering = tokio::spawn(async move {
            let mut buf = [0u8; 64];
            let (len, from) = server.recv_from(&mut buf).await.unwrap();
            assert_eq!(&buf[..len], QUESTION);
            let reply = br#"{"Address":"http://192.168.1.20:8096","Id":"abc","Name":"Mika's Jellyfin"}"#;
            // Twice, as a server with two network cards would.
            server.send_to(reply, from).await.unwrap();
            server.send_to(reply, from).await.unwrap();
            server.send_to(b"not json", from).await.unwrap();
        });
        let mut servers = Vec::new();
        discover(target, Duration::from_millis(300), |s| servers.push(s)).await;
        answering.await.unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "Mika's Jellyfin");
        assert_eq!(servers[0].address, "http://192.168.1.20:8096");
    }
}
