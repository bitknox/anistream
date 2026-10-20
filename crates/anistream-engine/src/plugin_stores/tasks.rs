//! What a front end asks of the plugin stores, each task run off the UI thread with what it needs
//! moved into it: joining and leaving, installing and removing, invites, members, and publishing.

use std::sync::Arc;

use anistream_core::config::{Config, Paths};
use anistream_model::{event::Event, notice::Notice, stores::Secret};
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;

use crate::{
    data, host::Host, jobs::JobGuard, outbox::Outbox, plugin_stores, services::Services,
    sources, state::EngineState,
};

/// What a plugin store task needs, moved into the task that runs it.
pub struct StoreTaskContext {
    stores: plugin_stores::Stores,
    registry: ProviderRegistry,
    config: Config,
    http: HttpClient,
    paths: Paths,
    host: Arc<dyn Host>,
    tx: Outbox,
}

impl StoreTaskContext {
    fn send(&self, update: Event) {
        let _ = self.tx.send(update);
    }

    fn toast(&self, toast: Notice) {
        self.send(Event::Toast(toast));
    }

    /// The store rows as they now stand, from disk.
    fn rows(&self) {
        match self.stores.rows() {
            Ok(rows) => self.send(Event::Stores(rows)),
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    async fn members(&self, store: &str) {
        match self.stores.members(store).await {
            Ok((members, invites)) => {
                self.send(Event::StoreMembers { store: store.to_owned(), members, invites });
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// The joined stores, as they are on disk and then as each store says.
    pub async fn load_stores(self) {
        // What is on disk answers at once; the network fills in behind it.
        self.rows();
        match self.stores.refresh().await {
            Ok(rows) => self.send(Event::Stores(rows)),
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// What a pasted invite link would join, asked of the store it names before it is spent.
    pub async fn preview_store(self, link: Secret) {
        self.send(Event::StorePreview(self.stores.preview(&link.0).await));
    }

    /// Spend an invite link, and join its store.
    pub async fn join_store(self, link: Secret) {
        match self.stores.join(&link.0).await {
            Ok(joined) => {
                self.rows();
                self.send(Event::StoreJoined { store: joined.id.clone() });
                self.toast(Notice::info(format!(
                    "joined {} as {}",
                    joined.name, joined.member.name
                )));
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Leave a store; what was installed from it stays.
    pub async fn leave_store(self, store: String) {
        match self.stores.leave(&store).await {
            Ok(name) => {
                self.rows();
                self.toast(Notice::info(format!("left {name} — its plugins stay installed")));
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Install a store's plugin, or update it, and bring it into the running chain.
    pub async fn install_plugin(self, store: String, plugin: String) {
        match self.stores.install(&store, &plugin).await {
            Ok((entry, path)) => {
                self.rows();
                self.load_installed(&entry.name, &entry.version, &path).await;
            }
            Err(e) => {
                // The install refreshed the list on its way, so the screen catches up
                // with whatever made it refuse.
                self.rows();
                self.toast(Notice::alert(format!("{plugin} not installed: {e}")));
            }
        }
    }

    /// Delete an installed plugin, and take it out of the chain.
    pub async fn remove_plugin(self, plugin: String) {
        match self.stores.remove(&plugin).await {
            Ok(()) => {
                self.registry.remove(&plugin);
                self.send(Event::Providers(data::provider_rows(&self.registry)));
                self.rows();
                self.toast(Notice::info(format!("removed {plugin}")));
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Make an invite link to a store.
    pub async fn create_invite(self, store: String, name: Option<String>) {
        match self.stores.invite(&store, name).await {
            Ok(view) => self.send(Event::InviteLink(view)),
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// A store's members and its open invites.
    pub async fn load_members(self, store: String) {
        self.members(&store).await
    }

    /// Take a member's access away, and list who is left.
    pub async fn remove_member(self, store: String, member: String) {
        match self.stores.remove_member(&store, &member).await {
            Ok(()) => self.toast(Notice::info("removed — their devices stopped working here")),
            Err(e) => self.toast(Notice::alert(e)),
        }
        self.members(&store).await;
    }

    /// Cancel an invite, and list what is left.
    pub async fn cancel_invite(self, store: String, invite: String) {
        match self.stores.cancel_invite(&store, &invite).await {
            Ok(()) => self.toast(Notice::info("invite cancelled — the link no longer works")),
            Err(e) => self.toast(Notice::alert(e)),
        }
        self.members(&store).await;
    }

    /// The local plugins a store's owner could publish there.
    pub async fn load_local_plugins(self, store: String) {
        match self.stores.local_plugins(&store).await {
            Ok(plugins) => self.send(Event::LocalPlugins { store, plugins }),
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Publish a local plugin to a store.
    pub async fn publish_plugin(self, store: String, plugin: String) {
        match self.stores.publish(&store, &plugin).await {
            Ok(entry) => {
                self.rows();
                self.toast(Notice::info(format!("published {} {}", entry.name, entry.version)));
            }
            Err(e) => self.toast(Notice::alert(format!("{plugin} not published: {e}"))),
        }
    }

    /// Stop offering a plugin in a store.
    pub async fn unpublish_plugin(self, store: String, plugin: String) {
        match self.stores.unpublish(&store, &plugin).await {
            Ok(()) => {
                self.rows();
                self.toast(Notice::info(format!("{plugin} is no longer offered")));
            }
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Open a store's admin page in the browser, signed in by a link that works once.
    pub async fn open_admin(self, store: String) {
        match self.stores.admin_link(&store).await {
            Ok(link) => match self.host.open_url(&link) {
                Ok(()) => self.toast(Notice::info(
                    "opened the admin page in your browser — the link works once, for ten minutes",
                )),
                Err(e) => self.toast(Notice::alert(format!(
                    "could not open a browser ({e}) — on the store's machine, `anistream store admin` prints a link"
                ))),
            },
            Err(e) => self.toast(Notice::alert(e)),
        }
    }

    /// Bring a just-installed plugin into the running chain, so it is usable without a restart.
    async fn load_installed(&self, name: &str, version: &str, path: &std::path::Path) {
        if !sources::plugins_enabled(&self.config) {
            self.toast(Notice::info(format!(
                "installed {name} {version} — \"plugins\" is off in providers.order, so it is not used"
            )));
            return;
        }
        match sources::load_plugin(&self.config, &self.http, &self.paths, path).await {
            Ok(provider) => {
                // Only ever swaps a plugin for a plugin. Whatever the id, a store's download does
                // not get to take a built-in source's place in the chain.
                let id = provider.manifest().id.clone();
                if self.registry.get(&id).is_some_and(|existing| {
                    existing.manifest().kind != anistream_core::traits::ProviderKind::Plugin
                }) {
                    self.toast(Notice::alert(format!(
                        "installed {name}, but {id} is a built-in source's name — not loaded"
                    )));
                    return;
                }
                // Where the order puts it, as the next launch would, rather than at the end. An
                // update was swapped in place and keeps it.
                self.registry.put(provider);
                sources::arrange(&self.registry, &self.config);
                self.send(Event::Providers(data::provider_rows(&self.registry)));
                self.toast(Notice::info(format!("installed {name} {version}")));
            }
            Err(e) => self
                .toast(Notice::alert(format!("installed {name}, but it does not load: {e}"))),
        }
    }
}

/// Run a plugin store task off the UI thread, with what it needs moved into it.
pub fn store_task<W, F>(
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
    work: W,
) where
    W: FnOnce(StoreTaskContext) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let context = StoreTaskContext {
        stores: services.stores.clone(),
        registry: services.registry.clone(),
        config: engine.config.effective().clone(),
        http: services.http.clone(),
        paths: services.paths.clone(),
        host: services.host.clone(),
        tx: tx.clone(),
    };
    tokio::spawn(async move {
        let _job = job;
        work(context).await;
    });
}
