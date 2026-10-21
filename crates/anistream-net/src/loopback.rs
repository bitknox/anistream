//! Loopback listeners that can be bound again on their own port.
//!
//! Every server this app runs listens on 127.0.0.1 for one player: the torrent stream servers
//! and the mending proxy. A phone's system may take an app's sockets while it is in the
//! background, and a listener taken that way accepts nothing when the app comes back, while the
//! player still holds a URL with its port in it. [`restore_all`] binds each listener that no
//! longer answers again, on the port it had, so every URL handed out still leads where it did,
//! and the player only has to open its file again. One that still answers is left alone.
//!
//! The registry is the process's: every loopback server here is the app's own, and there is no
//! way for one to be missed by a caller that forgot to pass it along.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, PoisonError, Weak},
};

use tokio::{net::TcpListener, task::JoinHandle};

/// What a listener does with itself once bound: its accept loop, spawned.
type Accept = Box<dyn Fn(TcpListener) -> JoinHandle<()> + Send + Sync>;

/// A listener on a loopback port, and the accept loop serving it.
pub struct Loopback {
    port: u16,
    accept: Accept,
    task: Mutex<Option<JoinHandle<()>>>,
}

/// Every loopback listener alive, held weakly: one dropped is simply gone from it.
static LISTENERS: Mutex<Vec<Weak<Loopback>>> = Mutex::new(Vec::new());

impl Loopback {
    /// Bind a fresh port on 127.0.0.1 and serve it with `accept`, which is given the listener
    /// and spawns its loop. Kept by the registry for [`rebind_all`] until it is dropped.
    pub async fn bind(
        accept: impl Fn(TcpListener) -> JoinHandle<()> + Send + Sync + 'static,
    ) -> std::io::Result<Arc<Self>> {
        // Loopback only: nothing here is for another machine.
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
        let port = listener.local_addr()?.port();
        let accept: Accept = Box::new(accept);
        let task = accept(listener);
        let loopback = Arc::new(Self { port, accept, task: Mutex::new(Some(task)) });
        let mut listeners = LISTENERS.lock().unwrap_or_else(PoisonError::into_inner);
        listeners.retain(|weak| weak.strong_count() > 0);
        listeners.push(Arc::downgrade(&loopback));
        Ok(loopback)
    }

    /// The port it listens on, the same after a rebind.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Bind the port again and serve it afresh: the old listener closed first, so the same
    /// port is free to take. Connections already open are the accept loop's spawned work and
    /// are left as they are.
    pub async fn rebind(&self) -> std::io::Result<()> {
        let old = self.lock().take();
        if let Some(old) = old {
            old.abort();
            // The listener goes with the task: wait for it, so its port is free.
            let _ = old.await;
        }
        let listener =
            TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, self.port))).await?;
        *self.lock() = Some((self.accept)(listener));
        Ok(())
    }

    /// Stop accepting.
    pub fn stop(&self) {
        if let Some(task) = self.lock().take() {
            task.abort();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<JoinHandle<()>>> {
        self.task.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Loopback {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Loopback {
    /// Whether it still serves: its accept loop runs, and a connection to its port is taken.
    async fn answers(&self) -> bool {
        let running = { self.lock().as_ref().is_some_and(|task| !task.is_finished()) };
        running && tokio::net::TcpStream::connect(("127.0.0.1", self.port)).await.is_ok()
    }
}

/// Bind every loopback listener alive that no longer answers again, on its own port, and say
/// how many were. One that cannot be bound is logged and left: what it served ends, and the
/// player says so.
pub async fn restore_all() -> usize {
    let alive: Vec<Arc<Loopback>> = {
        let mut listeners = LISTENERS.lock().unwrap_or_else(PoisonError::into_inner);
        listeners.retain(|weak| weak.strong_count() > 0);
        listeners.iter().filter_map(Weak::upgrade).collect()
    };
    let mut bound = 0;
    for loopback in alive {
        if loopback.answers().await {
            continue;
        }
        match loopback.rebind().await {
            Ok(()) => bound += 1,
            Err(e) => tracing::warn!(port = loopback.port, error = %e, "could not bind again"),
        }
    }
    bound
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    /// A listener that answers each connection with `word`.
    async fn answering(word: &'static [u8]) -> Arc<Loopback> {
        Loopback::bind(move |listener| {
            tokio::spawn(async move {
                while let Ok((mut socket, _)) = listener.accept().await {
                    let _ = socket.write_all(word).await;
                }
            })
        })
        .await
        .unwrap()
    }

    async fn ask(port: u16) -> std::io::Result<Vec<u8>> {
        let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
        let mut answer = Vec::new();
        socket.read_to_end(&mut answer).await?;
        Ok(answer)
    }

    #[tokio::test]
    async fn a_listener_bound_again_answers_on_the_port_it_had() {
        let loopback = answering(b"here").await;
        let port = loopback.port();
        assert_eq!(ask(port).await.unwrap(), b"here");

        // As the system leaves it after taking it: nothing accepts on the port.
        loopback.stop();
        tokio::task::yield_now().await;
        loopback.rebind().await.unwrap();
        assert_eq!(loopback.port(), port, "the same port, so every URL still leads here");
        assert_eq!(ask(port).await.unwrap(), b"here");
    }

    #[tokio::test]
    async fn only_a_listener_that_stopped_answering_is_bound_again() {
        let taken = answering(b"taken").await;
        let fine = answering(b"fine").await;
        let dropped = answering(b"dropped").await;
        let gone = dropped.port();
        drop(dropped);
        taken.stop();
        tokio::task::yield_now().await;

        // Another test's may be among them: this binary's tests run side by side.
        assert!(restore_all().await >= 1, "the one taken");
        assert_eq!(ask(taken.port()).await.unwrap(), b"taken");
        assert_eq!(ask(fine.port()).await.unwrap(), b"fine");
        assert!(ask(gone).await.is_err(), "the one dropped stays gone");
    }
}
