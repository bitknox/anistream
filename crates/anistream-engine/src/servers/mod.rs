//! Media servers: the Jellyfin servers the person runs, as sources.
//!
//! What is kept: the list of servers (address, who is signed in, the libraries read, the matches
//! to AniList) as JSON in the database, and the access token in the secure store. The password is
//! never kept. A saved server joins the source chain as one source; its matches are what it
//! answers from, so nothing searches the server at play time.

mod matching;
mod record;
mod report;

use std::sync::{Arc, Mutex};

use anistream_core::traits::ProviderKind;
use anistream_jellyfin::{
    Address, ClientInfo, Jellyfin, JellyfinError, JellyfinSource, discovery, model::Session,
    source_id,
};
use anistream_model::{
    event::Event,
    servers::{
        CheckedServer, FoundServer, ServerCommand, ServerEvent, ServerFailure, ServerRow,
        ServerStep,
    },
};
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anistream_track::TokenStore;
use tokio::task::AbortHandle;

use crate::{outbox::Outbox, services::Services};

pub use record::{Record, seed_fixture};

/// The servers saved in `store`.
pub fn saved(store: &Store) -> Vec<Record> {
    record::load(store)
}
pub use report::{Report, Reports};

/// The handles the servers need, cheap to clone.
#[derive(Clone)]
pub struct Servers {
    pub(crate) tokens: TokenStore,
    store: Store,
    http: HttpClient,
    quick: Arc<Mutex<Option<AbortHandle>>>,
    /// Where the broadcast goes. The local network's, except in a test.
    target: std::net::SocketAddr,
}

impl Servers {
    pub fn new(tokens: TokenStore, store: Store, http: HttpClient) -> Self {
        Self {
            tokens,
            store,
            http,
            quick: Default::default(),
            target: discovery::broadcast_target(),
        }
    }

    /// The same, with the broadcast aimed at `target`.
    pub fn discovering_at(mut self, target: std::net::SocketAddr) -> Self {
        self.target = target;
        self
    }

    pub fn reports(&self) -> Reports {
        Reports::new(self.clone())
    }

    fn token_key(server: &str) -> String {
        source_id(server)
    }

    /// The device as Jellyfin's dashboard lists it. The id is made once and kept.
    fn client_info(&self) -> ClientInfo {
        const KEY: &str = "servers:device";
        let device_id = match self.store.get_meta_text(KEY).ok().flatten() {
            Some(id) => id,
            None => {
                let id = format!("{:032x}", rand::random::<u128>());
                let _ = self.store.set_meta_text(KEY, &id);
                id
            }
        };
        ClientInfo {
            device: format!("anistream on {}", std::env::consts::OS),
            device_id,
            version: env!("CARGO_PKG_VERSION").to_owned(),
        }
    }

    fn client(&self, address: Address) -> Jellyfin {
        Jellyfin::new(&self.http, address, self.client_info())
    }

    /// A saved server's client, signed in with its kept token.
    pub(crate) fn signed_in(&self, record: &Record) -> Option<Jellyfin> {
        let address = Address::parse(&record.address).ok()?;
        let token = self.tokens.get(&Self::token_key(&record.id)).ok()?;
        Some(self.client(address).with_session(Session {
            token,
            user_id: record.user_id.clone(),
            user_name: record.user_name.clone(),
        }))
    }

    pub(crate) fn records(&self) -> Vec<Record> {
        record::load(&self.store)
    }

    /// The saved servers as sources, in the chain. Called once at launch.
    pub fn join_chain(
        &self,
        registry: &ProviderRegistry,
        config: &anistream_core::config::Config,
    ) {
        for record in self.records() {
            self.put_source(&record, registry, &config.providers.order);
        }
    }

    fn put_source(&self, record: &Record, registry: &ProviderRegistry, order: &[String]) {
        let Some(client) = self.signed_in(record) else { return };
        let source = JellyfinSource::new(&record.id, &record.name, client, record.matches());
        registry.put(Arc::new(source));
        registry.rank_by(|p| crate::sources::rank(p, order));
    }

    fn rows(&self) -> Vec<ServerRow> {
        self.records().iter().map(Record::row).collect()
    }

    fn publish(&self, tx: &Outbox) {
        let _ = tx.send(Event::Server(ServerEvent::Servers(self.rows())));
    }
}

fn failed(tx: &Outbox, step: ServerStep, error: &JellyfinError) {
    let _ = tx.send(Event::Server(ServerEvent::Failed { step, failure: failure(error) }));
}

/// A failure as the apps are told it.
pub fn failure(error: &JellyfinError) -> ServerFailure {
    match error {
        JellyfinError::Unreachable { address } => {
            ServerFailure::Unreachable { address: address.clone() }
        }
        JellyfinError::NotJellyfin => ServerFailure::NotJellyfin,
        JellyfinError::TooOld { version } => ServerFailure::TooOld { version: version.clone() },
        JellyfinError::BadCredentials => ServerFailure::Credentials,
        JellyfinError::QuickConnectOff => ServerFailure::QuickConnectOff,
        JellyfinError::QuickConnectExpired => ServerFailure::QuickConnectExpired,
        JellyfinError::PlainHttp => ServerFailure::PlainHttp,
        JellyfinError::UntrustedCertificate { host } => {
            ServerFailure::Certificate { host: host.clone() }
        }
        JellyfinError::SignedOut => ServerFailure::SignedOut,
        JellyfinError::BadAddress => ServerFailure::BadAddress,
        JellyfinError::Status { .. } | JellyfinError::Decode(_) => ServerFailure::Other,
    }
}

/// Run one command. Everything that waits on the network runs as a task.
pub fn command(command: ServerCommand, services: &Services, order: Vec<String>, tx: &Outbox) {
    let servers = services.servers.clone();
    let (registry, meta, store) =
        (services.registry.clone(), services.meta.clone(), services.store.clone());
    let tx = tx.clone();
    match command {
        ServerCommand::Load => servers.publish(&tx),
        ServerCommand::Discover => {
            tokio::spawn(async move {
                let found = tx.clone();
                discovery::discover(servers.target, discovery::WINDOW, move |server| {
                    let _ = found.send(Event::Server(ServerEvent::Found(FoundServer {
                        id: server.id,
                        name: server.name,
                        address: server.address,
                    })));
                })
                .await;
                let _ = tx.send(Event::Server(ServerEvent::DiscoveryDone));
            });
        }
        ServerCommand::Check { address } => {
            tokio::spawn(async move {
                match check(&servers, &address).await {
                    Ok((checked, _)) => {
                        let _ = tx.send(Event::Server(ServerEvent::Checked(checked)));
                    }
                    Err(e) => failed(&tx, ServerStep::Address, &e),
                }
            });
        }
        ServerCommand::SignIn { address, user, password } => {
            tokio::spawn(async move {
                let result = async {
                    let (checked, client) = check(&servers, &address).await?;
                    let session = client.sign_in(&user, &password).await?;
                    Ok::<_, JellyfinError>((checked, session))
                }
                .await;
                match result {
                    Ok((checked, session)) => {
                        finish_sign_in(&servers, checked, session, &registry, &order, &tx)
                    }
                    Err(e) => failed(&tx, ServerStep::SignIn, &e),
                }
            });
        }
        ServerCommand::StartQuickConnect { address } => {
            let again = servers.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = quick_connect(&servers, &address, &registry, &order, &tx).await
                {
                    failed(&tx, ServerStep::QuickConnect, &e);
                }
            });
            let mut slot = again.quick.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(old) = slot.replace(handle.abort_handle()) {
                old.abort();
            }
        }
        ServerCommand::CancelQuickConnect => {
            let mut slot = servers.quick.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(old) = slot.take() {
                old.abort();
            }
        }
        ServerCommand::LoadLibraries { server } => {
            tokio::spawn(async move {
                let Some(record) = servers.records().into_iter().find(|r| r.id == server)
                else {
                    return;
                };
                let Some(client) = servers.signed_in(&record) else {
                    failed(&tx, ServerStep::Libraries, &JellyfinError::SignedOut);
                    return;
                };
                match matching::choices(&client, &record).await {
                    Ok(libraries) => {
                        let _ = tx
                            .send(Event::Server(ServerEvent::Libraries { server, libraries }));
                    }
                    Err(e) => failed(&tx, ServerStep::Libraries, &e),
                }
            });
        }
        ServerCommand::SetLibraries { server, libraries } => {
            tokio::spawn(async move {
                let Some(record) = servers.records().into_iter().find(|r| r.id == server)
                else {
                    return;
                };
                let Some(client) = servers.signed_in(&record) else {
                    failed(&tx, ServerStep::Libraries, &JellyfinError::SignedOut);
                    return;
                };
                match client.libraries().await {
                    Ok(all) => record::update(&store, &server, |r| {
                        r.libraries = all
                            .into_iter()
                            .map(|l| record::LibraryRecord {
                                ticked: libraries.contains(&l.id),
                                id: l.id,
                                name: l.name,
                                kind: l.collection_type,
                            })
                            .collect();
                        r.libraries_chosen = true;
                    }),
                    Err(e) => return failed(&tx, ServerStep::Libraries, &e),
                }
                servers.publish(&tx);
                sync(servers, server, registry, meta, store, order, tx);
            });
        }
        ServerCommand::Refresh { server } => {
            sync(servers, server, registry, meta, store, order, tx)
        }
        ServerCommand::SetReport { server, report } => {
            record::update(&store, &server, |r| r.report = report);
            servers.publish(&tx);
        }
        ServerCommand::Remove { server } => {
            record::remove(&store, &server);
            let _ = servers.tokens.clear(&Servers::token_key(&server));
            registry.remove(&source_id(&server));
            servers.publish(&tx);
        }
    }
}

async fn check(
    servers: &Servers,
    typed: &str,
) -> Result<(CheckedServer, Jellyfin), JellyfinError> {
    let address = Address::parse(typed)?;
    let client = servers.client(address.clone());
    let info = client.public_info().await?;
    let quick_connect = client.quick_connect_enabled().await.unwrap_or(false);
    Ok((
        CheckedServer {
            id: info.id,
            name: info.server_name,
            version: info.version,
            address: address.base().to_owned(),
            quick_connect,
        },
        client,
    ))
}

/// Poll an approval: every two seconds, for as long as the server keeps the code.
async fn quick_connect(
    servers: &Servers,
    address: &str,
    registry: &ProviderRegistry,
    order: &[String],
    tx: &Outbox,
) -> Result<(), JellyfinError> {
    let (checked, client) = check(servers, address).await?;
    let started = client.quick_connect_start().await?;
    let _ =
        tx.send(Event::Server(ServerEvent::QuickConnectCode { code: started.code.clone() }));
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if client.quick_connect_approved(&started.secret).await? {
            break;
        }
    }
    let session = client.sign_in_with_quick_connect(&started.secret).await?;
    finish_sign_in(servers, checked, session, registry, order, tx);
    Ok(())
}

/// Keep the server and its token, put it in the chain, and say so.
fn finish_sign_in(
    servers: &Servers,
    checked: CheckedServer,
    session: Session,
    registry: &ProviderRegistry,
    order: &[String],
    tx: &Outbox,
) {
    if servers.tokens.set(&Servers::token_key(&checked.id), &session.token).is_err() {
        failed(tx, ServerStep::SignIn, &JellyfinError::SignedOut);
        return;
    }
    let record = Record::new(&checked, &session);
    record::upsert(&servers.store, record.clone());
    servers.put_source(&record, registry, order);
    let _ = tx.send(Event::Server(ServerEvent::SignedIn { server: checked.id }));
    servers.publish(tx);
}

/// Match the ticked libraries' series to AniList, keep the matches, and give the source them.
fn sync(
    servers: Servers,
    server: String,
    registry: ProviderRegistry,
    meta: anistream_meta::Meta,
    store: Store,
    order: Vec<String>,
    tx: Outbox,
) {
    tokio::spawn(async move {
        let Some(record) = servers.records().into_iter().find(|r| r.id == server) else {
            return;
        };
        let Some(client) = servers.signed_in(&record) else {
            failed(&tx, ServerStep::Libraries, &JellyfinError::SignedOut);
            return;
        };
        match matching::match_all(&client, &record, &meta, &store).await {
            Ok((matches, series)) => {
                record::update(&store, &server, |r| r.set_matches(matches, series));
                if let Some(updated) = servers.records().into_iter().find(|r| r.id == server) {
                    registry.remove(&source_id(&server));
                    servers.put_source(&updated, &registry, &order);
                }
            }
            Err(e) => {
                if matches!(e, JellyfinError::SignedOut) {
                    record::update(&store, &server, |r| r.signed_out = true);
                }
                failed(&tx, ServerStep::Libraries, &e);
            }
        }
        servers.publish(&tx);
    });
}

/// Whether a source in the chain is a media server's.
pub fn is_server(provider: &dyn anistream_core::traits::Provider) -> bool {
    provider.manifest().kind == ProviderKind::Server
}
