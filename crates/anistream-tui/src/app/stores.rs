//! Plugin stores: joining one, a store's page and its plugins, its members and invites, and
//! publishing to it.

use anistream_model::{
    command::Command,
    notice::Notice,
    stores::{
        InviteLinkView, InvitePreview, InvitedStore, LocalPluginRow, OpenJoinView, Secret,
        StoreInviteRow, StoreMemberRow, StorePluginRow, StorePluginStatus, StoreRow,
        StoreState,
    },
};

use super::{App, Field};
use crate::nav::{Overlay, StageView};

/// A store action waiting for a yes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreConfirm {
    Join(InvitedStore),
    Leave {
        store: String,
        name: String,
    },
    RemoveMember {
        store: String,
        member: String,
        name: String,
        devices: u32,
    },
    CancelInvite {
        store: String,
        invite: String,
        name: String,
    },
    Unpublish {
        store: String,
        plugin: String,
        name: String,
    },
    /// Let anyone with the link join, or only people the owners invite.
    SetOpen {
        store: String,
        name: String,
        open: bool,
    },
    /// Remove one device that joined with the link, or all of them.
    RemoveAnonymous {
        store: String,
        device: Option<String>,
        devices: u32,
    },
}

/// One line of a store's screen: a plugin, or something to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreLine {
    Plugin(usize),
    LinkDevice,
    Invite,
    Members,
    /// Only people the owners invite, or anyone with the link. Owners only.
    WhoCanJoin,
    Publish,
    /// The store's admin page, in a browser. Owners only.
    Admin,
    Leave,
}

impl StoreLine {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Plugin(_) => "",
            Self::LinkDevice => "link another device",
            Self::Invite => "invite someone",
            Self::Members => "members",
            Self::WhoCanJoin => "who can join",
            Self::Publish => "publish a plugin",
            Self::Admin => "open the admin page",
            Self::Leave => "leave this store",
        }
    }
}

/// One row of a store's members screen: the people, their unused invites, and then, while the
/// store has any, its open link and the devices that joined with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberLine {
    Member(usize),
    Invite(usize),
    /// The store's open link, to show and copy.
    OpenLink,
    /// One of the most recent devices that joined with the link.
    Anonymous(usize),
    /// Every device that joined with the link.
    RemoveAnonymous,
}

/// What can be done with a store's plugin from its overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginChoice {
    Install,
    Update(String),
    /// Overwrite a file of the same name that was added by hand.
    Replace,
    Remove,
    /// Stop offering it. Owners only.
    Unpublish,
    Cancel,
}

impl PluginChoice {
    pub fn label(&self) -> String {
        match self {
            Self::Install => "install".into(),
            Self::Update(version) => format!("update to {version}"),
            Self::Replace => "replace the copy added by hand".into(),
            Self::Remove => "remove from this machine".into(),
            Self::Unpublish => "stop offering it in this store".into(),
            Self::Cancel => "cancel".into(),
        }
    }
}

/// Plugin stores.
impl App {
    pub fn store(&self, id: &str) -> Option<&StoreRow> {
        self.stores.iter().find(|s| s.id == id)
    }

    pub(super) fn in_stores(&self) -> bool {
        self.nav.current() == &StageView::Stores
    }

    /// The store whose page is on screen.
    pub fn store_view(&self) -> Option<&str> {
        match self.nav.current() {
            StageView::Store(id) => Some(id),
            _ => None,
        }
    }

    /// The store whose members are on screen.
    pub fn members_view(&self) -> Option<&str> {
        match self.nav.current() {
            StageView::StoreMembers(id) => Some(id),
            _ => None,
        }
    }

    /// What a store's page lists, top to bottom: its plugins, then what can be done there.
    ///
    /// The actions are rows rather than keys. They are used rarely enough that a key nobody
    /// remembers is worse than a line that says what it does.
    pub fn store_lines(&self, store: &StoreRow) -> Vec<StoreLine> {
        let mut lines: Vec<StoreLine> =
            (0..store.plugins.len()).map(StoreLine::Plugin).collect();
        if store.state != StoreState::Revoked {
            // A device that joined with the link has no invite to make: another one joins with
            // the same link.
            if !store.anonymous {
                lines.push(StoreLine::LinkDevice);
            }
            if store.owner {
                lines.extend([
                    StoreLine::Invite,
                    StoreLine::Members,
                    StoreLine::WhoCanJoin,
                    StoreLine::Publish,
                    StoreLine::Admin,
                ]);
            }
        }
        lines.push(StoreLine::Leave);
        lines
    }

    pub(super) fn apply_stores(&mut self, rows: Vec<StoreRow>) {
        self.stores = rows;
        self.stores_loaded = true;
        // One past the last store is the "join a store" row.
        self.cursors.stores.selected = self.cursors.stores.selected.min(self.stores.len());

        // A store that was left takes its screens with it, rather than leaving a page about
        // something that no longer exists.
        let gone = |app: &Self, id: &str| app.store(id).is_none();
        if let Some(id) = self.members_view().map(str::to_owned)
            && gone(self, &id)
        {
            self.nav.close_all_overlays();
            self.nav.pop();
        }
        if let Some(id) = self.store_view().map(str::to_owned) {
            if gone(self, &id) {
                self.nav.close_all_overlays();
                self.nav.pop();
            } else if let Some(store) = self.store(&id) {
                let last = self.store_lines(store).len().saturating_sub(1);
                self.cursors.store_lines.selected = self.cursors.store_lines.selected.min(last);
            }
        }
    }

    pub(super) fn apply_store_preview(&mut self, outcome: InvitePreview) {
        match outcome {
            // Asked from the store list with nothing open. Anywhere else, a confirmation that
            // took over would be answering a question the user is not looking at — and an open
            // "leave" or "remove" would quietly become a "join" under the same Enter.
            InvitePreview::Store(_) if !self.in_stores() || self.nav.has_overlay() => {
                self.push_toast(Notice::info(
                    "the invite checked out — paste it again to join",
                ));
            }
            InvitePreview::Store(preview) => {
                self.store_confirm = Some(StoreConfirm::Join(preview));
                self.nav.open_overlay(Overlay::StoreConfirm);
            }
            InvitePreview::Failed { reason } => {
                self.push_toast(Notice::alert(reason));
                // Back to the prompt with the link still in it, so a link cut short in the
                // copy can be finished rather than pasted again — when the user is still here.
                if self.in_stores() && !self.nav.has_overlay() {
                    self.nav.open_overlay(Overlay::JoinStore);
                }
            }
        }
    }

    /// A store just joined.
    pub(super) fn apply_store_joined(&mut self, store: String) {
        // Into the store just joined, but only from the list it was joined from. A join
        // that finishes after the user has gone elsewhere must not drag them back.
        if self.nav.current() == &StageView::Stores && self.store(&store).is_some() {
            self.cursors.store_lines.selected = 0;
            self.nav.push(StageView::Store(store));
        }
    }

    /// An invite link just made, shown until it is dismissed.
    pub(super) fn apply_invite_link(&mut self, view: InviteLinkView) {
        self.invite_link = Some(view);
        self.invite_copied = false;
        self.nav.open_overlay(Overlay::InviteLink);
    }

    /// A store's members and its unused invites, for the members screen.
    pub(super) fn apply_store_members(
        &mut self,
        store: String,
        members: Vec<StoreMemberRow>,
        invites: Vec<StoreInviteRow>,
    ) {
        if self.members_for.as_deref() == Some(store.as_str()) {
            self.store_members = members;
            self.store_invites = invites;
            self.clamp_member_cursor();
        }
    }

    /// Who can join a store, for its members screen, and the link just made when the owner
    /// opened it from its page.
    pub(super) fn apply_open_join(&mut self, store: String, view: OpenJoinView) {
        let opened = self.opening.as_deref() == Some(store.as_str());
        if opened {
            self.opening = None;
        }
        let here = self.members_for.as_deref() == Some(store.as_str())
            || self.store_view() == Some(store.as_str());
        if !here {
            return;
        }
        let link = view.link.clone();
        self.open_join = Some((store, view));
        self.clamp_member_cursor();
        // Just opened from the store's page, with nothing else open: the link it was opened
        // for, shown and copied, as a new invite link is.
        if opened
            && let Some(link) = link
            && !self.nav.has_overlay()
        {
            self.show_open_link(link);
        }
    }

    fn clamp_member_cursor(&mut self) {
        let last = self.member_lines().len().saturating_sub(1);
        self.cursors.members.selected = self.cursors.members.selected.min(last);
    }

    /// Who can join the store whose members are on screen, once its server has said.
    pub fn open_join_view(&self) -> Option<&OpenJoinView> {
        let (store, view) = self.open_join.as_ref()?;
        (self.members_for.as_deref() == Some(store.as_str())).then_some(view)
    }

    /// The members screen's rows, top to bottom.
    pub fn member_lines(&self) -> Vec<MemberLine> {
        let mut lines: Vec<MemberLine> = (0..self.store_members.len())
            .map(MemberLine::Member)
            .chain((0..self.store_invites.len()).map(MemberLine::Invite))
            .collect();
        if let Some(view) = self.open_join_view() {
            if view.open {
                lines.push(MemberLine::OpenLink);
            }
            lines.extend((0..view.recent.len()).map(MemberLine::Anonymous));
            if view.devices > 0 {
                lines.push(MemberLine::RemoveAnonymous);
            }
        }
        lines
    }

    /// The open link overlay, with the link on its way to the clipboard.
    fn show_open_link(&mut self, link: Secret) {
        self.clipboard = Some(link.0);
        self.open_link_copied = false;
        self.nav.open_overlay(Overlay::OpenLink);
    }

    /// The text to hand the terminal's clipboard, once: whoever runs the app copies it and then
    /// says whether that went through, as with an invite link.
    pub fn take_clipboard(&mut self) -> Option<String> {
        self.clipboard.take()
    }

    /// The link the open link overlay shows.
    pub fn open_link(&self) -> Option<&str> {
        let (_, view) = self.open_join.as_ref()?;
        view.link.as_ref().map(|link| link.0.as_str())
    }

    /// The plugins that could be published to a store, for the publish picker.
    pub(super) fn apply_local_plugins(&mut self, store: String, plugins: Vec<LocalPluginRow>) {
        // Only for the store still on screen; a late answer for another is dropped.
        let here = self.nav.current() == &StageView::Store(store.clone());
        if self.publish_for.as_deref() != Some(store.as_str()) || !here {
            return;
        }
        if plugins.is_empty() {
            self.push_toast(Notice::info(
                "no plugins in the plugin directory to publish — `anistream --plugins` shows where it is",
            ));
            return;
        }
        self.local_plugins = plugins;
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::PublishPick);
    }

    pub(super) fn open_stores(&mut self) -> Option<Command> {
        self.cursors.stores.selected = 0;
        self.nav.push(StageView::Stores);
        Some(Command::LoadStores)
    }

    /// Enter on the store list: into a store, or the join prompt from the last row.
    pub(super) fn open_store_row(&mut self) -> Option<Command> {
        match self.stores.get(self.cursors.stores.selected) {
            Some(store) => {
                let id = store.id.clone();
                self.cursors.store_lines.selected = 0;
                self.nav.push(StageView::Store(id));
            }
            None => {
                self.field_mut(Field::JoinLink).clear();
                self.nav.open_overlay(Overlay::JoinStore);
            }
        }
        None
    }

    /// Enter on a pasted invite link: the store it names, checked before anything is joined.
    pub(super) fn submit_join_link(&mut self) -> Option<Command> {
        // A link that wrapped in a chat window arrives with spaces and line breaks in
        // it, and a URL has no place for either.
        let link: String =
            self.text(Field::JoinLink).chars().filter(|c| !c.is_whitespace()).collect();
        if link.is_empty() {
            return None;
        }
        self.nav.close_overlay();
        Some(Command::PreviewStore { link: Secret(link) })
    }

    /// Enter on an invite's name: the invite, made for the store on screen.
    pub(super) fn submit_invite_name(&mut self) -> Option<Command> {
        let name = self.text(Field::InviteName).trim().to_owned();
        if name.is_empty() {
            return None;
        }
        let store = self.store_view()?.to_owned();
        self.field_mut(Field::InviteName).clear();
        self.nav.close_overlay();
        Some(Command::CreateInvite { store, name: Some(name) })
    }

    fn open_store_line(&mut self, id: &str) -> Option<Command> {
        let store = self.store(id)?.clone();
        let line = *self.store_lines(&store).get(self.cursors.store_lines.selected)?;
        match line {
            StoreLine::Plugin(i) => {
                let plugin = store.plugins.get(i)?.id.clone();
                self.show_plugin(&store.id, &plugin);
                None
            }
            StoreLine::LinkDevice => {
                Some(Command::CreateInvite { store: store.id, name: None })
            }
            StoreLine::Invite => {
                self.field_mut(Field::InviteName).clear();
                self.nav.open_overlay(Overlay::InviteName);
                None
            }
            StoreLine::Members => {
                self.members_for = Some(store.id.clone());
                self.store_members.clear();
                self.store_invites.clear();
                self.open_join = None;
                self.cursors.members.selected = 0;
                self.nav.push(StageView::StoreMembers(store.id.clone()));
                Some(Command::LoadMembers { store: store.id })
            }
            StoreLine::WhoCanJoin => {
                self.store_confirm = Some(StoreConfirm::SetOpen {
                    store: store.id,
                    name: store.name,
                    open: !store.open,
                });
                self.nav.open_overlay(Overlay::StoreConfirm);
                None
            }
            StoreLine::Publish => {
                self.publish_for = Some(store.id.clone());
                self.local_plugins.clear();
                Some(Command::LoadLocalPlugins { store: store.id })
            }
            // No status line of its own: the activity row says it is opening, and a toast says
            // how it went. A status set here was never cleared, so it outlived both.
            StoreLine::Admin => Some(Command::OpenAdmin { store: store.id }),
            StoreLine::Leave => {
                self.store_confirm =
                    Some(StoreConfirm::Leave { store: store.id, name: store.name });
                self.nav.open_overlay(Overlay::StoreConfirm);
                None
            }
        }
    }

    /// Enter on a store's page: its highlighted line.
    pub(super) fn enter_store_line(&mut self) -> Option<Command> {
        let id = self.store_view()?.to_owned();
        self.open_store_line(&id)
    }

    /// Open the plugin overlay for one of a store's plugins, with its choices as they stand now.
    pub fn show_plugin(&mut self, store: &str, plugin: &str) {
        self.plugin_action = Some((store.to_owned(), plugin.to_owned()));
        self.plugin_options = self.current_plugin_choices();
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::PluginAction);
    }

    /// The store and plugin the plugin overlay is about.
    pub fn plugin_in_action(&self) -> Option<(&StoreRow, &StorePluginRow)> {
        let (store_id, plugin_id) = self.plugin_action.as_ref()?;
        let store = self.store(store_id)?;
        let plugin = store.plugins.iter().find(|p| &p.id == plugin_id)?;
        Some((store, plugin))
    }

    /// What the plugin overlay offers, in order, as it was when the overlay opened.
    pub fn plugin_choices(&self) -> &[PluginChoice] {
        &self.plugin_options
    }

    /// What the plugin overlay would offer for the plugin it is about, from the rows as they are.
    fn current_plugin_choices(&self) -> Vec<PluginChoice> {
        let Some((store, plugin)) = self.plugin_in_action() else { return Vec::new() };
        // A store that refused this device cannot be downloaded from, but what it installed
        // can still be removed.
        let live = store.state != StoreState::Revoked;
        let mut choices = Vec::new();
        match &plugin.status {
            StorePluginStatus::Available if live => choices.push(PluginChoice::Install),
            StorePluginStatus::Update { .. } if live => {
                choices.push(PluginChoice::Update(plugin.version.clone()));
            }
            StorePluginStatus::AddedByHand if live => choices.push(PluginChoice::Replace),
            _ => {}
        }
        if matches!(
            plugin.status,
            StorePluginStatus::Installed | StorePluginStatus::Update { .. }
        ) {
            choices.push(PluginChoice::Remove);
        }
        if store.owner && live {
            choices.push(PluginChoice::Unpublish);
        }
        choices.push(PluginChoice::Cancel);
        choices
    }

    pub(super) fn confirm_plugin_choice(&mut self) -> Option<Command> {
        let choice = self.plugin_options.get(self.cursors.overlay.selected)?.clone();
        self.plugin_options.clear();
        let name = self.plugin_in_action().map(|(_, p)| p.name.clone()).unwrap_or_default();
        let (store, plugin) = self.plugin_action.clone()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        match choice {
            PluginChoice::Install | PluginChoice::Update(_) | PluginChoice::Replace => {
                Some(Command::InstallPlugin { store, plugin })
            }
            PluginChoice::Remove => Some(Command::RemovePlugin { plugin }),
            PluginChoice::Unpublish => {
                self.store_confirm = Some(StoreConfirm::Unpublish { store, plugin, name });
                self.nav.open_overlay(Overlay::StoreConfirm);
                None
            }
            PluginChoice::Cancel => None,
        }
    }

    pub(super) fn confirm_store_action(&mut self) -> Option<Command> {
        let confirm = self.store_confirm.take()?;
        self.nav.close_overlay();
        Some(match confirm {
            StoreConfirm::Join(preview) => {
                self.field_mut(Field::JoinLink).clear();
                Command::JoinStore { link: preview.link }
            }
            StoreConfirm::Leave { store, .. } => Command::LeaveStore { store },
            StoreConfirm::RemoveMember { store, member, .. } => {
                Command::RemoveMember { store, member }
            }
            StoreConfirm::CancelInvite { store, invite, .. } => {
                Command::CancelInvite { store, invite }
            }
            StoreConfirm::Unpublish { store, plugin, .. } => {
                Command::UnpublishPlugin { store, plugin }
            }
            StoreConfirm::SetOpen { store, open, .. } => {
                if open {
                    self.opening = Some(store.clone());
                }
                Command::SetOpenJoin { store, open }
            }
            StoreConfirm::RemoveAnonymous { store, device, .. } => {
                Command::RemoveAnonymous { store, device }
            }
        })
    }

    /// The plugin picked, published to the store on screen.
    pub(super) fn confirm_publish(&mut self) -> Option<Command> {
        let plugin = self.local_plugins.get(self.cursors.overlay.selected)?.id.clone();
        let store = self.publish_for.clone()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        Some(Command::PublishPlugin { store, plugin })
    }

    /// Enter on the members screen: remove a member, cancel an unused invite, show the open
    /// link, or remove devices that joined with it.
    fn open_member_row(&mut self, store: &str) -> Option<Command> {
        let line = *self.member_lines().get(self.cursors.members.selected)?;
        let store = store.to_owned();
        self.store_confirm = Some(match line {
            MemberLine::Member(at) => {
                let member = self.store_members.get(at)?;
                if member.you {
                    self.push_toast(Notice::info(
                        "that is you — a store always keeps its owner",
                    ));
                    return None;
                }
                StoreConfirm::RemoveMember {
                    store,
                    member: member.id.clone(),
                    name: member.name.clone(),
                    devices: member.devices,
                }
            }
            MemberLine::Invite(at) => {
                let invite = self.store_invites.get(at)?;
                StoreConfirm::CancelInvite {
                    store,
                    invite: invite.id.clone(),
                    name: invite.name.clone(),
                }
            }
            MemberLine::OpenLink => {
                let link = self.open_join_view()?.link.clone()?;
                self.show_open_link(link);
                return None;
            }
            MemberLine::Anonymous(at) => {
                let device = self.open_join_view()?.recent.get(at)?;
                StoreConfirm::RemoveAnonymous {
                    store,
                    device: Some(device.id.clone()),
                    devices: 1,
                }
            }
            MemberLine::RemoveAnonymous => {
                let devices = self.open_join_view()?.devices;
                StoreConfirm::RemoveAnonymous { store, device: None, devices }
            }
        });
        self.nav.open_overlay(Overlay::StoreConfirm);
        None
    }

    /// Enter on a store's members: the highlighted row.
    pub(super) fn enter_member_row(&mut self) -> Option<Command> {
        let id = self.members_view()?.to_owned();
        self.open_member_row(&id)
    }

    /// A store's members, asked for again.
    pub(super) fn refresh_members(&mut self) -> Option<Command> {
        let store = self.members_view()?.to_owned();
        Some(Command::LoadMembers { store })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    fn store_row(id: &str, owner: bool, plugins: Vec<StorePluginRow>) -> StoreRow {
        StoreRow {
            id: id.into(),
            name: format!("store {id}"),
            url: "https://plugins.example".into(),
            host: "plugins.example".into(),
            checked_at: None,
            owners: Vec::new(),
            open: false,
            anonymous: false,
            fingerprint: "9f3c 1a2b 77de 0c41".into(),
            member: "Johan".into(),
            owner,
            state: StoreState::Ready,
            plugins,
        }
    }

    fn store_plugin(id: &str, status: StorePluginStatus) -> StorePluginRow {
        StorePluginRow {
            id: id.into(),
            name: id.into(),
            version: "2.0.0".into(),
            hosts: vec!["api.example".into()],
            status,
        }
    }

    #[test]
    fn enter_on_providers_opens_the_stores_from_the_rail_too() {
        let mut a = app();
        a.go_to_section(Section::Providers);
        a.nav.focus_rail();
        assert_eq!(a.handle(Action::Open, 20), Some(Command::LoadStores));
        assert_eq!(a.nav.current(), &StageView::Stores);
        assert_eq!(a.nav.focus(), Focus::Stage);
    }

    #[test]
    fn joining_is_paste_check_confirm() {
        let mut a = at_stores(Vec::new());
        // With nothing joined, the only row is "join a store".
        assert_eq!(a.handle(Action::Open, 20), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinStore));
        assert!(a.is_typing());

        // A link that wrapped in a chat window, pasted with its line break.
        a.paste("https://plugins.example/join/abc\n  #key");
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::PreviewStore {
                link: Secret("https://plugins.example/join/abc#key".into())
            })
        );
        assert!(!a.nav.has_overlay());

        a.apply(Event::StorePreview(InvitePreview::Store(InvitedStore {
            link: Secret("https://plugins.example/join/abc#key".into()),
            name: "Our plugins".into(),
            url: "https://plugins.example".into(),
            host: "plugins.example".into(),
            fingerprint: "9f3c".into(),
            already_joined: false,
            owners: Vec::new(),
            plugins: Vec::new(),
            open: false,
            anonymous: false,
        })));
        assert_eq!(a.nav.overlay(), Some(&Overlay::StoreConfirm));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::JoinStore {
                link: Secret("https://plugins.example/join/abc#key".into())
            })
        );
        assert!(a.text(Field::JoinLink).is_empty(), "the spent link is not kept");

        // Joined: the new store opens, because the user is still on the list it was joined from.
        a.apply(Event::Stores(vec![store_row("abc", false, Vec::new())]));
        a.apply(Event::StoreJoined { store: "abc".into() });
        assert_eq!(a.nav.current(), &StageView::Store("abc".into()));
    }

    #[test]
    fn a_bad_link_says_why_and_reopens_the_prompt_with_the_link_kept() {
        let mut a = at_stores(Vec::new());
        a.handle(Action::Open, 20);
        type_text(&mut a, "https://plugins.example/join/abc");
        a.handle(Action::Open, 20);
        a.apply(Event::StorePreview(InvitePreview::Failed {
            reason: "the link is missing the store's key".into(),
        }));
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinStore));
        assert_eq!(a.text(Field::JoinLink), "https://plugins.example/join/abc");
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("missing the store's key")));
    }

    #[test]
    fn a_dismissed_join_prompt_forgets_the_link() {
        let mut a = at_stores(Vec::new());
        a.handle(Action::Open, 20);
        type_text(&mut a, "https://plugins.example/join/abc#key");
        a.handle(Action::Back, 20);
        assert!(a.text(Field::JoinLink).is_empty());
    }

    #[test]
    fn a_join_that_finishes_elsewhere_does_not_drag_the_user_back() {
        let mut a = at_stores(Vec::new());
        a.go_to_section(Section::Home);
        a.apply(Event::Stores(vec![store_row("abc", false, Vec::new())]));
        a.apply(Event::StoreJoined { store: "abc".into() });
        assert_eq!(a.nav.current(), &StageView::Section(Section::Home));
    }

    #[test]
    fn a_plugin_is_installed_from_its_overlay() {
        let mut a = at_stores(vec![store_row(
            "abc",
            false,
            vec![store_plugin("example", StorePluginStatus::Available)],
        )]);
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.current(), &StageView::Store("abc".into()));
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::PluginAction));
        assert_eq!(a.plugin_choices(), vec![PluginChoice::Install, PluginChoice::Cancel]);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::InstallPlugin { store: "abc".into(), plugin: "example".into() })
        );
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn an_update_offers_the_new_version_and_removal() {
        let mut a = at_stores(vec![store_row(
            "abc",
            false,
            vec![store_plugin(
                "example",
                StorePluginStatus::Update { from: "1.0.0".into(), new_hosts: Vec::new() },
            )],
        )]);
        a.handle(Action::Open, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.plugin_choices(),
            vec![
                PluginChoice::Update("2.0.0".into()),
                PluginChoice::Remove,
                PluginChoice::Cancel
            ]
        );
        a.handle(Action::Down, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::RemovePlugin { plugin: "example".into() })
        );
    }

    #[test]
    fn a_store_that_removed_this_device_can_only_have_its_plugins_removed() {
        let mut store = store_row(
            "abc",
            true,
            vec![
                store_plugin(
                    "kept",
                    StorePluginStatus::Update { from: "1.0.0".into(), new_hosts: Vec::new() },
                ),
                store_plugin("never", StorePluginStatus::Available),
            ],
        );
        store.state = StoreState::Revoked;
        let mut a = at_stores(vec![store.clone()]);
        a.handle(Action::Open, 20);
        // No device, invite or owner rows: only the plugins and leaving.
        assert_eq!(
            a.store_lines(&store),
            vec![StoreLine::Plugin(0), StoreLine::Plugin(1), StoreLine::Leave]
        );
        a.handle(Action::Open, 20);
        assert_eq!(a.plugin_choices(), vec![PluginChoice::Remove, PluginChoice::Cancel]);
        a.handle(Action::Back, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(a.plugin_choices(), vec![PluginChoice::Cancel]);
    }

    #[test]
    fn a_new_invite_link_starts_uncopied() {
        // Copied is said of the link on screen; the next one has not been anywhere yet.
        let link = |n: &str| InviteLinkView {
            store: "abc".into(),
            link: Secret(format!("https://plugins.example/join/{n}#k")),
            for_whom: None,
            expires_at: 0,
        };
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.apply(Event::InviteLink(link("one")));
        a.invite_copied = true;
        a.apply(Event::InviteLink(link("two")));
        assert!(!a.invite_copied);
    }

    #[test]
    fn an_owner_invites_someone_by_name() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        let lines = a.store_lines(a.store("abc").unwrap());
        assert_eq!(
            lines,
            vec![
                StoreLine::LinkDevice,
                StoreLine::Invite,
                StoreLine::Members,
                StoreLine::WhoCanJoin,
                StoreLine::Publish,
                StoreLine::Admin,
                StoreLine::Leave
            ]
        );
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::InviteName));
        // An empty name is a slip, not an answer.
        assert_eq!(a.handle(Action::Open, 20), None);
        type_text(&mut a, "Alex");
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::CreateInvite { store: "abc".into(), name: Some("Alex".into()) })
        );

        a.apply(Event::InviteLink(InviteLinkView {
            store: "abc".into(),
            link: Secret("https://plugins.example/join/x#y".into()),
            for_whom: Some("Alex".into()),
            expires_at: 0,
        }));
        assert_eq!(a.nav.overlay(), Some(&Overlay::InviteLink));
        a.handle(Action::Open, 20);
        assert!(!a.nav.has_overlay());
        assert!(a.invite_link.is_none());
    }

    #[test]
    fn an_owner_opens_the_admin_page_from_the_store() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        for _ in 0..5 {
            a.handle(Action::Down, 20);
        }
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::OpenAdmin { store: "abc".into() })
        );
    }

    #[test]
    fn a_member_has_no_admin_page_row() {
        let a = at_stores(vec![store_row("abc", false, Vec::new())]);
        assert!(!a.store_lines(a.store("abc").unwrap()).contains(&StoreLine::Admin));
    }

    #[test]
    fn linking_a_device_is_one_enter() {
        let mut a = at_stores(vec![store_row("abc", false, Vec::new())]);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::CreateInvite { store: "abc".into(), name: None })
        );
    }

    #[test]
    fn removing_a_member_asks_first_and_ignores_stray_keys() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::LoadMembers { store: "abc".into() })
        );
        assert_eq!(a.nav.current(), &StageView::StoreMembers("abc".into()));

        let member = |id: &str, you| StoreMemberRow {
            id: id.into(),
            name: id.into(),
            owner: you,
            devices: 1,
            last_seen: None,
            you,
        };
        a.apply(Event::StoreMembers {
            store: "abc".into(),
            members: vec![member("me", true), member("alex", false)],
            invites: vec![StoreInviteRow {
                id: "inv".into(),
                name: "sam".into(),
                device: false,
                expires_at: 0,
            }],
        });

        // Yourself: refused with a reason, not a confirmation.
        assert_eq!(a.handle(Action::Open, 20), None);
        assert!(!a.nav.has_overlay());

        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::StoreConfirm));
        // A stray key must not move the cursor off the person being removed.
        a.handle(Action::Down, 20);
        assert_eq!(a.member_cursor(), 1);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::RemoveMember { store: "abc".into(), member: "alex".into() })
        );

        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::CancelInvite { store: "abc".into(), invite: "inv".into() })
        );
    }

    #[test]
    fn leaving_asks_first_and_the_page_goes_with_the_store() {
        let mut a = at_stores(vec![store_row("abc", false, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        assert_eq!(a.handle(Action::Open, 20), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::StoreConfirm));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::LeaveStore { store: "abc".into() })
        );

        a.apply(Event::Stores(Vec::new()));
        assert_eq!(a.nav.current(), &StageView::Stores);
    }

    #[test]
    fn publishing_picks_from_local_plugins_for_the_store_on_screen() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        for _ in 0..4 {
            a.handle(Action::Down, 20);
        }
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::LoadLocalPlugins { store: "abc".into() })
        );
        let local = LocalPluginRow {
            id: "example".into(),
            name: "Example".into(),
            version: "1.0.0".into(),
            hosts: Vec::new(),
            published: None,
        };
        // An answer for another store is not offered here.
        a.apply(Event::LocalPlugins { store: "other".into(), plugins: vec![local.clone()] });
        assert!(!a.nav.has_overlay());

        a.apply(Event::LocalPlugins { store: "abc".into(), plugins: vec![local] });
        assert_eq!(a.nav.overlay(), Some(&Overlay::PublishPick));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::PublishPlugin { store: "abc".into(), plugin: "example".into() })
        );
    }

    #[test]
    fn nothing_to_publish_says_so() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        for _ in 0..4 {
            a.handle(Action::Down, 20);
        }
        a.handle(Action::Open, 20);
        a.apply(Event::LocalPlugins { store: "abc".into(), plugins: Vec::new() });
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("no plugins")));
    }

    /// The store screens each keep a cursor of their own: stepping back out of a store finds the
    /// store list where it was, and a store's page opens at its top.
    #[test]
    fn stepping_back_out_of_a_store_finds_the_store_list_where_it_was() {
        let mut a = at_stores(vec![
            store_row("a", false, Vec::new()),
            store_row("b", false, Vec::new()),
        ]);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.current(), &StageView::Store("b".into()));
        a.handle(Action::Down, 20);
        assert_eq!(a.store_line_cursor(), 1);
        a.handle(Action::Back, 20);
        assert_eq!(a.nav.current(), &StageView::Stores);
        assert_eq!(a.store_cursor(), 1, "the store list is where it was left");
        a.handle(Action::Open, 20);
        assert_eq!(a.store_line_cursor(), 0, "a store's page opens at its top");
    }

    #[test]
    fn the_store_cursor_stops_at_the_join_row() {
        let mut a = at_stores(vec![
            store_row("a", false, Vec::new()),
            store_row("b", false, Vec::new()),
        ]);
        for _ in 0..5 {
            a.handle(Action::Down, 20);
        }
        assert_eq!(a.store_cursor(), 2, "one past the stores is the join row");
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinStore));
    }

    #[test]
    fn refreshing_a_store_screen_asks_the_stores_again() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        assert_eq!(a.handle(Action::Refresh, 20), Some(Command::LoadStores));
        a.handle(Action::Open, 20);
        assert_eq!(a.handle(Action::Refresh, 20), Some(Command::LoadStores));
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Refresh, 20),
            Some(Command::LoadMembers { store: "abc".into() })
        );
    }

    #[test]
    fn a_store_list_arriving_does_not_reorder_an_open_plugin_overlay() {
        let mut a = at_stores(vec![store_row(
            "abc",
            true,
            vec![store_plugin("example", StorePluginStatus::Installed)],
        )]);
        a.handle(Action::Open, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.plugin_choices(),
            [PluginChoice::Remove, PluginChoice::Unpublish, PluginChoice::Cancel]
        );
        a.handle(Action::Down, 20);
        // The refresh that opening the screen asked for lands now, with an update waiting.
        a.apply(Event::Stores(vec![store_row(
            "abc",
            true,
            vec![store_plugin(
                "example",
                StorePluginStatus::Update { from: "1.0.0".into(), new_hosts: Vec::new() },
            )],
        )]));
        a.handle(Action::Open, 20);
        assert_eq!(
            a.store_confirm,
            Some(StoreConfirm::Unpublish {
                store: "abc".into(),
                plugin: "example".into(),
                name: "example".into()
            }),
            "the choice under the cursor is still the one that was there"
        );
    }

    #[test]
    fn a_late_preview_does_not_take_over_an_open_question() {
        let mut a = at_stores(vec![store_row("abc", false, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        let leave = a.store_confirm.clone();
        assert!(matches!(leave, Some(StoreConfirm::Leave { .. })));
        a.apply(Event::StorePreview(InvitePreview::Store(InvitedStore {
            link: Secret("x".into()),
            name: "Other".into(),
            url: "https://other.example".into(),
            host: "other.example".into(),
            fingerprint: "0000".into(),
            already_joined: false,
            owners: Vec::new(),
            plugins: Vec::new(),
            open: false,
            anonymous: false,
        })));
        assert_eq!(a.store_confirm, leave);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::LeaveStore { store: "abc".into() })
        );
    }

    fn open_view(open: bool, devices: u32) -> OpenJoinView {
        OpenJoinView {
            open,
            link: open.then(|| Secret("https://plugins.example/join/abc?open#key".into())),
            devices,
            recent: (0..devices.min(2))
                .map(|i| anistream_model::stores::AnonymousDeviceRow {
                    id: format!("d{i}"),
                    joined_at: 0,
                    last_seen: None,
                })
                .collect(),
        }
    }

    #[test]
    fn an_owner_opens_the_store_to_its_link_and_is_shown_the_link() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        for _ in 0..3 {
            a.handle(Action::Down, 20);
        }
        assert_eq!(a.handle(Action::Open, 20), None);
        assert_eq!(
            a.store_confirm,
            Some(StoreConfirm::SetOpen {
                store: "abc".into(),
                name: "store abc".into(),
                open: true
            })
        );
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::SetOpenJoin { store: "abc".into(), open: true })
        );

        // The answer brings the new link, shown and on its way to the clipboard.
        a.apply(Event::OpenJoin { store: "abc".into(), view: open_view(true, 0) });
        assert_eq!(a.nav.overlay(), Some(&Overlay::OpenLink));
        assert_eq!(a.open_link(), Some("https://plugins.example/join/abc?open#key"));
        assert_eq!(
            a.take_clipboard().as_deref(),
            Some("https://plugins.example/join/abc?open#key")
        );
        assert_eq!(a.take_clipboard(), None, "copied once");
        a.handle(Action::Open, 20);
        assert!(!a.nav.has_overlay());

        // Closing is asked the other way round, once the row says the store is open.
        let mut open = store_row("abc", true, Vec::new());
        open.open = true;
        a.apply(Event::Stores(vec![open]));
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::SetOpenJoin { store: "abc".into(), open: false })
        );
        a.apply(Event::OpenJoin { store: "abc".into(), view: open_view(false, 3) });
        assert!(!a.nav.has_overlay(), "nothing to show when it closes");
    }

    #[test]
    fn the_members_screen_shows_the_link_and_removes_devices_that_joined_with_it() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::LoadMembers { store: "abc".into() })
        );
        a.apply(Event::StoreMembers {
            store: "abc".into(),
            members: vec![StoreMemberRow {
                id: "m".into(),
                name: "Johan".into(),
                owner: true,
                devices: 1,
                last_seen: None,
                you: true,
            }],
            invites: Vec::new(),
        });
        assert_eq!(a.member_lines(), vec![MemberLine::Member(0)]);
        // Another store's answer is not this screen's.
        a.apply(Event::OpenJoin { store: "other".into(), view: open_view(true, 2) });
        assert_eq!(a.member_lines().len(), 1);

        a.apply(Event::OpenJoin { store: "abc".into(), view: open_view(true, 5) });
        assert_eq!(
            a.member_lines(),
            vec![
                MemberLine::Member(0),
                MemberLine::OpenLink,
                MemberLine::Anonymous(0),
                MemberLine::Anonymous(1),
                MemberLine::RemoveAnonymous,
            ]
        );
        a.handle(Action::Down, 20);
        assert_eq!(a.handle(Action::Open, 20), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::OpenLink));
        assert!(a.take_clipboard().is_some());
        a.handle(Action::Back, 20);

        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::RemoveAnonymous { store: "abc".into(), device: Some("d0".into()) })
        );
        a.handle(Action::Bottom, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.store_confirm,
            Some(StoreConfirm::RemoveAnonymous {
                store: "abc".into(),
                device: None,
                devices: 5
            })
        );
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::RemoveAnonymous { store: "abc".into(), device: None })
        );

        // All gone: the section goes, and the cursor stays on a row that exists.
        a.apply(Event::OpenJoin { store: "abc".into(), view: open_view(false, 0) });
        assert_eq!(a.member_lines(), vec![MemberLine::Member(0)]);
        assert_eq!(a.member_cursor(), 0);
    }

    #[test]
    fn a_device_that_joined_with_the_link_has_no_other_device_to_link() {
        let mut row = store_row("abc", false, Vec::new());
        row.anonymous = true;
        row.member = String::new();
        let a = at_stores(vec![row]);
        assert_eq!(a.store_lines(a.store("abc").unwrap()), vec![StoreLine::Leave]);
    }
}
