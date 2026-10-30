//! Watch parties, for a front end: the invite link a party is shared by, read and written by
//! the model's rules so every app makes and opens the same links, and, in a testing build, a
//! Syncplay server on the loopback interface for the Fixtures apps to hold a party on offline.
//!
//! The party itself is the engine's, through `Command::PartyJoin` and the `Party*` events.

use anistream_model::party::PartyInvite;

/// The link that invites someone to `invite`'s room, to play its episode there:
/// `anistream://party?server=…&room=…&title=…&episode=…`.
#[uniffi::export]
pub fn party_invite_link(invite: PartyInvite) -> String {
    invite.link()
}

/// The invite a link opened carries, or `None` for a link that is not a whole one.
#[uniffi::export]
pub fn parse_party_invite(link: String) -> Option<PartyInvite> {
    PartyInvite::parse(&link)
}

#[cfg(all(feature = "party", feature = "testing"))]
pub use fake::FfiFakeParty;

#[cfg(all(feature = "party", feature = "testing"))]
mod fake {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex, PoisonError},
    };

    use anistream_model::Msg;
    use anistream_party::fake::{FakeMember, FakeOptions, FakeServer};
    use tokio::runtime::Handle;

    use crate::{engine::FfiEngine, start::FfiError};

    /// A Syncplay server on 127.0.0.1 with members the test or the Fixtures app plays, so a
    /// party can be joined, steered and left with no network. Testing builds only.
    #[derive(uniffi::Object)]
    pub struct FfiFakeParty {
        inner: Mutex<Option<Inner>>,
        handle: Handle,
    }

    struct Inner {
        server: FakeServer,
        members: HashMap<String, FakeMember>,
    }

    #[uniffi::export]
    impl FfiEngine {
        /// A party server for the Fixtures app's watch party: join it with
        /// `Command::PartyJoin` and its `server()`. Testing builds only.
        pub fn start_fake_party(&self) -> Result<Arc<FfiFakeParty>, FfiError> {
            let handle = self.runtime_handle();
            let server = handle
                .block_on(handle.spawn(FakeServer::start(FakeOptions::default())))
                .map_err(|e| FfiError::engine(Msg::StartFailed { reason: e.to_string() }))?
                .map_err(|e| FfiError::engine(Msg::StartFailed { reason: e.to_string() }))?;
            let inner = Inner { server, members: HashMap::new() };
            Ok(Arc::new(FfiFakeParty { inner: Mutex::new(Some(inner)), handle }))
        }
    }

    impl FfiFakeParty {
        fn with<T>(&self, read: impl FnOnce(&mut Inner) -> T) -> Option<T> {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            let _entered = self.handle.enter();
            inner.as_mut().map(read)
        }

        fn member(&self, name: &str, act: impl FnOnce(&FakeMember)) {
            self.with(|inner| inner.members.get(name).map(act));
        }
    }

    #[uniffi::export]
    impl FfiFakeParty {
        /// Where to join it, as `host:port`.
        pub fn server(&self) -> String {
            self.with(|inner| inner.server.address()).unwrap_or_default()
        }

        /// `name` comes into `room`, played by the server.
        pub fn add_member(&self, room: String, name: String) {
            self.with(|inner| {
                let member = inner.server.member(&room, &name);
                inner.members.insert(name, member);
            });
        }

        /// The member opens a file, `duration` seconds long.
        pub fn member_opens(&self, name: String, file: String, duration: f64) {
            self.member(&name, |member| member.open(&file, duration));
        }

        pub fn member_ready(&self, name: String, ready: bool) {
            self.member(&name, |member| member.ready(ready));
        }

        pub fn member_pauses(&self, name: String) {
            self.member(&name, FakeMember::pause);
        }

        pub fn member_plays(&self, name: String) {
            self.member(&name, FakeMember::play);
        }

        /// The member seeks to `to` seconds.
        pub fn member_seeks(&self, name: String, to: f64) {
            self.member(&name, |member| member.seek(to));
        }

        pub fn member_says(&self, name: String, message: String) {
            self.member(&name, |member| member.chat(&message));
        }

        pub fn member_leaves(&self, name: String) {
            self.with(|inner| inner.members.remove(&name).map(|member| member.leave()));
        }

        /// The server goes away, as one lost does: every connection dropped and none taken.
        pub fn go_down(&self, down: bool) {
            self.with(|inner| {
                inner.server.set_down(down);
                if down {
                    inner.server.drop_connections();
                }
            });
        }

        pub fn stop(&self) {
            let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner).take();
            let _entered = self.handle.enter();
            drop(inner);
        }
    }
}
