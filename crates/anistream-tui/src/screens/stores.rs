//! The plugin stores: the list of stores joined, a store's page with its plugins and the owner's
//! actions, and its members and invites; the rows of the overlays that join a store, confirm a
//! store action, act on a plugin, publish one and show an invite link.

use anistream_model::stores::{StorePluginStatus, StoreRow, StoreState};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, PluginChoice, StoreConfirm, StoreLine},
    clock::Clock,
    nav::Focus,
    theme::{Role, glyph},
    widgets::{Hairline, truncate},
};

use super::{
    MIN_TABLE_HEIGHT,
    chrome::{mark_selected, render_placeholder},
    line,
    providers::COL_PROVIDER,
};

// Column offsets for the store tables, named for the same reason the provider ones are.
const COL_STORE_COUNT: u16 = 30;
const COL_STORE_ROLE: u16 = 40;
const COL_STORE_STATE: u16 = 50;
const COL_PLUGIN_VERSION: u16 = 30;
const COL_PLUGIN_STATE: u16 = 44;

/// A store's condition in a few words, and the role to draw it in.
fn store_state(store: &StoreRow) -> (String, Role) {
    match &store.state {
        StoreState::Revoked => ("access removed".into(), Role::Alert),
        StoreState::Offline(_) => (format!("{} offline", glyph::STATE_DEGRADED), Role::State),
        StoreState::Ready => match store.updates() {
            0 => (format!("{} up to date", glyph::STATE_READY), Role::TextDim),
            1 => ("1 update".into(), Role::Obi),
            n => (format!("{n} updates"), Role::Obi),
        },
    }
}

/// Mark the focused row with the obi and return the style its first column should use.
fn focus_row(
    buf: &mut Buffer,
    app: &App,
    area: Rect,
    y: u16,
    selected: bool,
) -> ratatui::style::Style {
    if selected && y < area.bottom() {
        mark_selected(buf, app, area.left(), y, app.nav.focus() == Focus::Stage);
    }
    if selected {
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
    } else {
        app.palette.style(Role::Text)
    }
}

/// The footer every store screen ends with: a hairline, then a line about the focused row.
fn render_footer(buf: &mut Buffer, app: &App, area: Rect, text: &str, role: Role) {
    if area.height < 3 {
        return;
    }
    let y = area.bottom() - 2;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
    line(
        buf,
        area.left() + 2,
        y + 1,
        truncate(text, area.width.saturating_sub(4) as usize),
        app.palette.style(role),
    );
}

/// Every store this machine has joined, and the way to join another.
pub(super) fn render_stores(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height < MIN_TABLE_HEIGHT + 4 || area.width < 40 {
        render_placeholder(buf, app, area, "PLUGIN STORES", "needs a larger terminal");
        return;
    }
    line(
        buf,
        area.left(),
        area.top(),
        glyph::eyebrow("plugin stores"),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    let head = area.top() + 2;
    for (label, x) in [
        ("store", COL_PROVIDER),
        ("plugins", COL_STORE_COUNT),
        ("you", COL_STORE_ROLE),
        ("state", COL_STORE_STATE),
    ] {
        if x < area.width {
            line(
                buf,
                area.left() + x,
                head,
                glyph::eyebrow(label),
                app.palette.style(Role::TextDim),
            );
        }
    }
    Hairline::new(&app.palette).render(Rect { y: head + 1, height: 1, ..area }, buf);

    let mut y = head + 2;
    let last_row = area.bottom().saturating_sub(3);
    if app.stores.is_empty() && app.stores_loaded {
        line(
            buf,
            area.left() + COL_PROVIDER,
            y,
            "none joined yet",
            app.palette.style(Role::TextDim),
        );
        y += 1;
    }
    for (i, store) in app.stores.iter().enumerate() {
        if y >= last_row {
            break;
        }
        let style = focus_row(buf, app, area, y, i == app.cursors.stores.selected);
        line(
            buf,
            area.left() + COL_PROVIDER,
            y,
            truncate(&store.name, (COL_STORE_COUNT - COL_PROVIDER - 2) as usize),
            style,
        );
        line(
            buf,
            area.left() + COL_STORE_COUNT,
            y,
            store.plugins.len().to_string(),
            app.palette.style(Role::TextDim),
        );
        line(
            buf,
            area.left() + COL_STORE_ROLE,
            y,
            if store.owner { "owner" } else { "member" },
            app.palette.style(Role::TextDim),
        );
        let (state, role) = store_state(store);
        line(
            buf,
            area.left() + COL_STORE_STATE,
            y,
            truncate(&state, area.width.saturating_sub(COL_STORE_STATE + 1) as usize),
            app.palette.style(role),
        );
        y += 1;
    }

    // The join row, set apart from the stores by a blank line.
    let join_y = y + 1;
    if join_y < last_row {
        let style =
            focus_row(buf, app, area, join_y, app.cursors.stores.selected == app.stores.len());
        line(buf, area.left() + COL_PROVIDER, join_y, "+ join a store", style);
    }

    let (footer, role) = match app.stores.get(app.cursors.stores.selected) {
        Some(store) => match &store.state {
            StoreState::Revoked => (
                "this store no longer accepts this device — its owner can send a new invite"
                    .to_owned(),
                Role::Alert,
            ),
            StoreState::Offline(reason) => (format!("{} — {reason}", store.url), Role::State),
            StoreState::Ready => (
                glyph::dotted(&[
                    &store.url,
                    &format!("key {}", store.fingerprint),
                    &format!("joined as {}", store.member),
                ]),
                Role::TextDim,
            ),
        },
        None => ("paste the invite link a store's owner sent you".to_owned(), Role::TextDim),
    };
    render_footer(buf, app, area, &footer, role);
}

/// One store: its plugins, then what can be done there.
pub(super) fn render_store(buf: &mut Buffer, app: &App, area: Rect, id: &str) {
    let Some(store) = app.store(id) else {
        render_placeholder(buf, app, area, "PLUGIN STORE", "loading…");
        return;
    };
    if area.height < MIN_TABLE_HEIGHT + 6 || area.width < 40 {
        render_placeholder(buf, app, area, &store.name, "needs a larger terminal");
        return;
    }
    line(
        buf,
        area.left(),
        area.top(),
        truncate(&glyph::eyebrow(&store.name), area.width.saturating_sub(10) as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    if store.owner {
        line(
            buf,
            area.right().saturating_sub(6),
            area.top(),
            "owner",
            app.palette.style(Role::TextDim),
        );
    }
    let (note, role) = match &store.state {
        StoreState::Ready => {
            (glyph::dotted(&[&store.url, &format!("key {}", store.fingerprint)]), Role::TextDim)
        }
        StoreState::Offline(reason) => {
            (format!("offline, showing the last list it sent — {reason}"), Role::State)
        }
        StoreState::Revoked => (
            "this store no longer accepts this device — what it installed keeps working".into(),
            Role::Alert,
        ),
    };
    line(
        buf,
        area.left(),
        area.top() + 1,
        truncate(&note, area.width as usize),
        app.palette.style(role),
    );

    let head = area.top() + 3;
    for (label, x) in
        [("plugin", COL_PROVIDER), ("version", COL_PLUGIN_VERSION), ("state", COL_PLUGIN_STATE)]
    {
        if x < area.width {
            line(
                buf,
                area.left() + x,
                head,
                glyph::eyebrow(label),
                app.palette.style(Role::TextDim),
            );
        }
    }
    Hairline::new(&app.palette).render(Rect { y: head + 1, height: 1, ..area }, buf);

    let lines = app.store_lines(store);
    let last_row = area.bottom().saturating_sub(3);
    let mut y = head + 2;
    if store.plugins.is_empty() && y < last_row {
        let empty = if store.owner {
            "nothing published yet — publish a plugin from below"
        } else {
            "nothing published yet"
        };
        line(buf, area.left() + COL_PROVIDER, y, empty, app.palette.style(Role::TextDim));
        y += 1;
    }
    for (i, entry) in lines.iter().enumerate() {
        // The actions start after a blank line, so the table reads as a table.
        if !matches!(entry, StoreLine::Plugin(_))
            && i > 0
            && matches!(lines[i - 1], StoreLine::Plugin(_))
        {
            y += 1;
        }
        if !matches!(entry, StoreLine::Plugin(_)) && i == 0 {
            y += 1;
        }
        if y >= last_row {
            break;
        }
        let style = focus_row(buf, app, area, y, i == app.cursors.store_lines.selected);
        match entry {
            StoreLine::Plugin(at) => {
                let Some(plugin) = store.plugins.get(*at) else { continue };
                line(
                    buf,
                    area.left() + COL_PROVIDER,
                    y,
                    truncate(&plugin.name, (COL_PLUGIN_VERSION - COL_PROVIDER - 2) as usize),
                    style,
                );
                line(
                    buf,
                    area.left() + COL_PLUGIN_VERSION,
                    y,
                    truncate(
                        &plugin.version,
                        (COL_PLUGIN_STATE - COL_PLUGIN_VERSION - 1) as usize,
                    ),
                    app.palette.style(Role::TextDim),
                );
                let (state, role) = match &plugin.status {
                    StorePluginStatus::Available => ("not installed".to_owned(), Role::TextDim),
                    StorePluginStatus::Installed => {
                        (format!("{} installed", glyph::STATE_READY), Role::State)
                    }
                    StorePluginStatus::Update { from, .. } => {
                        (format!("update from {from}"), Role::Obi)
                    }
                    StorePluginStatus::AddedByHand => {
                        ("added by hand".to_owned(), Role::TextDim)
                    }
                    StorePluginStatus::OtherStore(_) => {
                        ("from another store".to_owned(), Role::TextDim)
                    }
                };
                line(
                    buf,
                    area.left() + COL_PLUGIN_STATE,
                    y,
                    truncate(&state, area.width.saturating_sub(COL_PLUGIN_STATE + 1) as usize),
                    app.palette.style(role),
                );
            }
            action => {
                let style = if matches!(action, StoreLine::Leave)
                    && i != app.cursors.store_lines.selected
                {
                    app.palette.style(Role::TextDim)
                } else {
                    style
                };
                line(buf, area.left() + COL_PROVIDER, y, action.label(), style);
            }
        }
        y += 1;
    }

    let (footer, role) = match lines.get(app.cursors.store_lines.selected) {
        Some(StoreLine::Plugin(at)) => match store.plugins.get(*at) {
            Some(plugin) => match &plugin.status {
                StorePluginStatus::Update { new_hosts, .. } if !new_hosts.is_empty() => (
                    format!("the update also wants to contact  {}", new_hosts.join(", ")),
                    Role::Alert,
                ),
                _ => (format!("may contact  {}", hosts_text(&plugin.hosts)), Role::TextDim),
            },
            None => (String::new(), Role::TextDim),
        },
        Some(StoreLine::LinkDevice) => (
            "a one-time link that joins another of your computers as you".into(),
            Role::TextDim,
        ),
        Some(StoreLine::Invite) => ("a one-time link for someone new".into(), Role::TextDim),
        Some(StoreLine::Members) => {
            ("who has joined, and removing someone's access".into(), Role::TextDim)
        }
        Some(StoreLine::Publish) => {
            ("offer a plugin from this machine's plugin directory".into(), Role::TextDim)
        }
        Some(StoreLine::Admin) => {
            ("manage members, invites and plugins in your browser".into(), Role::TextDim)
        }
        Some(StoreLine::Leave) => (
            "plugins installed from it stay on this machine and stop updating".into(),
            Role::TextDim,
        ),
        None => (String::new(), Role::TextDim),
    };
    render_footer(buf, app, area, &footer, role);
}

fn hosts_text(hosts: &[String]) -> String {
    if hosts.is_empty() { "nothing".into() } else { hosts.join(", ") }
}

/// How long ago a moment was, coarsely. Exactness would be noise on a "last seen" column.
fn ago(clock: &Clock, at: i64) -> String {
    let span = clock.now_epoch().saturating_sub(at).max(0);
    match span {
        0..3_600 => "just now".into(),
        3_600..86_400 => "today".into(),
        86_400..172_800 => "yesterday".into(),
        _ => format!("{} days ago", span / 86_400),
    }
}

/// How long until a moment, coarsely.
fn until(clock: &Clock, at: i64) -> String {
    let span = at.saturating_sub(clock.now_epoch());
    match span {
        ..=0 => "expired".into(),
        1..86_400 => "expires today".into(),
        86_400..172_800 => "expires tomorrow".into(),
        _ => format!("expires in {} days", span / 86_400),
    }
}

/// Who has joined a store and which invites are still open, for its owner.
pub(super) fn render_store_members(buf: &mut Buffer, app: &App, area: Rect, id: &str) {
    let name = app.store(id).map_or("plugin store", |s| s.name.as_str());
    if area.height < MIN_TABLE_HEIGHT + 4 || area.width < 40 {
        render_placeholder(buf, app, area, "MEMBERS", "needs a larger terminal");
        return;
    }
    line(
        buf,
        area.left(),
        area.top(),
        truncate(&glyph::eyebrow(&format!("members · {name}")), area.width as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    let head = area.top() + 2;
    for (label, x) in [
        ("name", COL_PROVIDER),
        ("role", COL_STORE_COUNT),
        ("devices", COL_STORE_ROLE),
        ("last seen", COL_STORE_STATE),
    ] {
        if x < area.width {
            line(
                buf,
                area.left() + x,
                head,
                glyph::eyebrow(label),
                app.palette.style(Role::TextDim),
            );
        }
    }
    Hairline::new(&app.palette).render(Rect { y: head + 1, height: 1, ..area }, buf);

    let last_row = area.bottom().saturating_sub(3);
    let mut y = head + 2;
    if app.store_members.is_empty() {
        line(buf, area.left() + COL_PROVIDER, y, "loading…", app.palette.style(Role::TextDim));
        return;
    }
    for (i, member) in app.store_members.iter().enumerate() {
        if y >= last_row {
            break;
        }
        let style = focus_row(buf, app, area, y, i == app.cursors.members.selected);
        let label =
            if member.you { format!("{} (you)", member.name) } else { member.name.clone() };
        line(
            buf,
            area.left() + COL_PROVIDER,
            y,
            truncate(&label, (COL_STORE_COUNT - COL_PROVIDER - 2) as usize),
            style,
        );
        let dim = app.palette.style(Role::TextDim);
        line(
            buf,
            area.left() + COL_STORE_COUNT,
            y,
            if member.owner { "owner" } else { "member" },
            dim,
        );
        line(buf, area.left() + COL_STORE_ROLE, y, member.devices.to_string(), dim);
        let seen =
            member.last_seen.map_or_else(|| "never".to_owned(), |at| ago(&app.clock, at));
        line(
            buf,
            area.left() + COL_STORE_STATE,
            y,
            truncate(&seen, area.width.saturating_sub(COL_STORE_STATE + 1) as usize),
            dim,
        );
        y += 1;
    }

    if !app.store_invites.is_empty() && y + 2 < last_row {
        y += 1;
        line(
            buf,
            area.left(),
            y,
            glyph::eyebrow("invites not used yet"),
            app.palette.style(Role::TextDim),
        );
        y += 1;
        for (i, invite) in app.store_invites.iter().enumerate() {
            if y >= last_row {
                break;
            }
            let at = app.store_members.len() + i;
            let style = focus_row(buf, app, area, y, at == app.cursors.members.selected);
            let label = if invite.device {
                format!("{}, another device", invite.name)
            } else {
                invite.name.clone()
            };
            line(
                buf,
                area.left() + COL_PROVIDER,
                y,
                truncate(&label, (COL_STORE_STATE - COL_PROVIDER - 2) as usize),
                style,
            );
            line(
                buf,
                area.left() + COL_STORE_STATE,
                y,
                truncate(
                    &until(&app.clock, invite.expires_at),
                    area.width.saturating_sub(COL_STORE_STATE + 1) as usize,
                ),
                app.palette.style(Role::TextDim),
            );
            y += 1;
        }
    }

    let footer = if app.cursors.members.selected < app.store_members.len() {
        match app.store_members.get(app.cursors.members.selected) {
            Some(member) if member.you => "you — a store always keeps its owner",
            _ => "↵ remove their access",
        }
    } else {
        "↵ cancel this invite"
    };
    render_footer(buf, app, area, footer, Role::TextDim);
}

/// How many characters of an invite link fit on one overlay row.
pub(super) fn link_width(band_width: u16) -> usize {
    // The band's two-cell indent each side, and the label's own margin.
    (band_width as usize).saturating_sub(7).max(16)
}

/// The invite link in row-sized pieces.
///
/// Cut at fixed widths rather than wrapped at words: a link has no words, and a selection across
/// the rows pastes back as one link because the join prompt drops whitespace.
pub(super) fn invite_link_chunks(app: &App, width: usize) -> Vec<String> {
    let Some(link) = &app.invite_link else { return Vec::new() };
    let chars: Vec<char> = link.link.0.chars().collect();
    chars.chunks(width.max(1)).map(|chunk| chunk.iter().collect()).collect()
}

pub(super) fn invite_link_rows(app: &App, width: usize) -> Vec<(String, String)> {
    let Some(view) = &app.invite_link else { return Vec::new() };
    let mut rows: Vec<(String, String)> = invite_link_chunks(app, width)
        .into_iter()
        .map(|chunk| (String::new(), chunk))
        .collect();
    rows.push((String::new(), String::new()));
    rows.push((
        String::new(),
        if app.invite_copied {
            "copied to the clipboard where the terminal allows it — or select it to copy".into()
        } else {
            "select it to copy".into()
        },
    ));
    rows.push((String::new(), format!("works once, {}", until(&app.clock, view.expires_at))));
    rows.push((
        String::new(),
        "opening it in a browser shows how to join and does not use it up".into(),
    ));
    rows.push((String::new(), "enter closes".into()));
    rows
}

pub(super) fn store_confirm_heading(app: &App) -> String {
    let question = match &app.store_confirm {
        Some(StoreConfirm::Join(preview)) => format!("join {}?", preview.name),
        Some(StoreConfirm::Leave { name, .. }) => format!("leave {name}?"),
        Some(StoreConfirm::RemoveMember { name, .. }) => format!("remove {name}?"),
        Some(StoreConfirm::CancelInvite { name, .. }) => {
            format!("cancel the invite for {name}?")
        }
        Some(StoreConfirm::Unpublish { name, .. }) => format!("stop offering {name}?"),
        None => "confirm".into(),
    };
    glyph::eyebrow(&question)
}

pub(super) fn store_confirm_rows(app: &App) -> Vec<(String, String)> {
    let row = |text: &str| (String::new(), text.to_owned());
    match &app.store_confirm {
        Some(StoreConfirm::Join(preview)) => {
            let mut rows = vec![
                row(&format!("address   {}", preview.url)),
                row(&format!("key       {}  — matches the invite", preview.fingerprint)),
            ];
            if preview.already_joined {
                rows.push(row("already joined — joining again gives this device a new token"));
            }
            rows.push(row(""));
            rows.push(row("enter joins · esc cancels"));
            rows
        }
        Some(StoreConfirm::Leave { .. }) => vec![
            row("plugins installed from it stay on this machine and stop updating"),
            row("this device's access to the store is revoked"),
            row(""),
            row("enter leaves · esc stays"),
        ],
        Some(StoreConfirm::RemoveMember { devices, .. }) => vec![
            row(&match devices {
                1 => "their device stops working with this store at once".to_owned(),
                n => format!("their {n} devices stop working with this store at once"),
            }),
            row("plugins they already installed stay on their machines"),
            row(""),
            row("enter removes · esc keeps them"),
        ],
        Some(StoreConfirm::CancelInvite { .. }) => {
            vec![row("the link stops working"), row(""), row("enter cancels it · esc keeps it")]
        }
        Some(StoreConfirm::Unpublish { .. }) => vec![
            row("members who installed it keep it, and it stops updating"),
            row(""),
            row("enter withdraws it · esc keeps it"),
        ],
        None => Vec::new(),
    }
}

/// The plugin overlay's rows, and how many of them are facts before the choices begin.
pub(super) fn plugin_action_rows(app: &App) -> (Vec<(String, String)>, usize) {
    let Some((_, plugin)) = app.plugin_in_action() else { return (Vec::new(), 0) };
    let mut rows = vec![
        (String::new(), format!("may contact   {}", hosts_text(&plugin.hosts))),
        (
            String::new(),
            "cannot        read files, open sockets or see your environment".into(),
        ),
    ];
    match &plugin.status {
        StorePluginStatus::Update { new_hosts, .. } if !new_hosts.is_empty() => {
            rows.push((String::new(), format!("the update adds   {}", new_hosts.join(", "))))
        }
        StorePluginStatus::AddedByHand => rows.push((
            String::new(),
            format!("{}.wasm is already in the plugin directory, added by hand", plugin.id),
        )),
        StorePluginStatus::OtherStore(_) => rows.push((
            String::new(),
            "installed from another store, which keeps it updated".into(),
        )),
        _ => {}
    }
    rows.push((String::new(), String::new()));
    let facts = rows.len();
    rows.extend(app.plugin_choices().iter().map(|choice| {
        let key = match choice {
            PluginChoice::Install | PluginChoice::Update(_) | PluginChoice::Replace => "↵",
            _ => "",
        };
        (key.to_owned(), choice.label())
    }));
    (rows, facts)
}

pub(super) fn publish_rows(app: &App) -> Vec<(String, String)> {
    app.local_plugins
        .iter()
        .map(|plugin| {
            let state = match &plugin.published {
                None => "new".to_owned(),
                Some(version) if *version == plugin.version => "published".to_owned(),
                Some(version) => format!("replaces {version}"),
            };
            (state, format!("{}  {}", plugin.name, plugin.version))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::catalog::ContentList;

    use crate::{
        app::Field,
        nav::{Overlay, Section, StageView},
        screens::{
            fixtures::{app_with, display_width, text_of},
            render_to_buffer,
        },
    };

    /// An app holding one store in every state worth drawing.
    fn app_with_stores() -> App {
        use anistream_model::stores::{
            InviteLinkView, InvitedStore, LocalPluginRow, Secret, StoreInviteRow,
            StoreMemberRow, StorePluginRow,
        };
        let plugin = |id: &str, status| StorePluginRow {
            id: id.into(),
            name: format!("{id} with a name long enough to need cutting short"),
            version: "1.4.0-beta.12+build.2026".into(),
            hosts: vec!["api.example".into(), "cdn.example".into()],
            status,
        };
        let mut app = app_with(ContentList::Entries(Vec::new()));
        app.apply(anistream_model::event::Event::Stores(vec![
            StoreRow {
                id: "aaaa".into(),
                name: "Our plugins, run from a closet in a flat in Malmö".into(),
                url: "https://plugins.example/with/a/long/path".into(),
                host: "plugins.example".into(),
                checked_at: None,
                fingerprint: "9f3c 1a2b 77de 0c41".into(),
                member: "Johan".into(),
                owner: true,
                state: StoreState::Ready,
                plugins: vec![
                    plugin("one", StorePluginStatus::Available),
                    plugin("two", StorePluginStatus::Installed),
                    plugin(
                        "three",
                        StorePluginStatus::Update {
                            from: "1.0.0".into(),
                            new_hosts: vec!["else.example".into()],
                        },
                    ),
                    plugin("four", StorePluginStatus::AddedByHand),
                    plugin("five", StorePluginStatus::OtherStore("bbbb".into())),
                ],
            },
            StoreRow {
                id: "bbbb".into(),
                name: "Sam's".into(),
                url: "https://sam.example".into(),
                host: "sam.example".into(),
                checked_at: None,
                fingerprint: "0000 1111 2222 3333".into(),
                member: "Johan".into(),
                owner: false,
                state: StoreState::Revoked,
                plugins: Vec::new(),
            },
            StoreRow {
                id: "cccc".into(),
                name: "Offline".into(),
                url: "https://down.example".into(),
                host: "down.example".into(),
                checked_at: None,
                fingerprint: "4444 5555 6666 7777".into(),
                member: "Johan".into(),
                owner: false,
                state: StoreState::Offline("could not connect".into()),
                plugins: Vec::new(),
            },
        ]));
        app.store_members = vec![
            StoreMemberRow {
                id: "m1".into(),
                name: "Johan".into(),
                owner: true,
                devices: 2,
                last_seen: Some(0),
                you: true,
            },
            StoreMemberRow {
                id: "m2".into(),
                name: "Alex, whose name is also rather long".into(),
                owner: false,
                devices: 1,
                last_seen: None,
                you: false,
            },
        ];
        app.store_invites = vec![StoreInviteRow {
            id: "i1".into(),
            name: "Sam".into(),
            device: false,
            expires_at: i64::MAX / 2,
        }];
        app.members_for = Some("aaaa".into());
        app.invite_link = Some(InviteLinkView {
            store: "aaaa".into(),
            link: Secret(format!(
                "https://plugins.example/join/k3TzQe9WbA1xLr7uVn2c0g#{}",
                "G".repeat(43)
            )),
            for_whom: Some("Alex".into()),
            expires_at: i64::MAX / 2,
        });
        app.invite_copied = true;
        app.store_confirm = Some(crate::app::StoreConfirm::Join(InvitedStore {
            link: Secret("https://plugins.example/join/x#y".into()),
            name: "Our plugins".into(),
            url: "https://plugins.example".into(),
            host: "plugins.example".into(),
            fingerprint: "9f3c 1a2b 77de 0c41".into(),
            already_joined: true,
        }));
        app.show_plugin("aaaa", "three");
        app.nav.close_overlay();
        app.local_plugins = vec![LocalPluginRow {
            id: "one".into(),
            name: "One".into(),
            version: "2.0.0".into(),
            hosts: Vec::new(),
            published: Some("1.4.0".into()),
        }];
        app.publish_for = Some("aaaa".into());
        app.set_text(Field::JoinLink, "https://plugins.example/join/".repeat(4));
        app.set_text(Field::InviteName, "Alex".into());
        app
    }

    fn store_views() -> [StageView; 4] {
        [
            StageView::Stores,
            StageView::Store("aaaa".into()),
            StageView::Store("bbbb".into()),
            StageView::StoreMembers("aaaa".into()),
        ]
    }

    const STORE_OVERLAYS: [Overlay; 6] = [
        Overlay::JoinStore,
        Overlay::StoreConfirm,
        Overlay::PluginAction,
        Overlay::InviteName,
        Overlay::InviteLink,
        Overlay::PublishPick,
    ];

    #[test]
    fn store_screens_survive_every_size_without_overflowing() {
        for (width, height) in
            [(200, 50), (120, 34), (80, 24), (60, 20), (40, 15), (24, 8), (8, 4), (1, 1)]
        {
            for view in store_views() {
                let mut app = app_with_stores();
                app.go_to_section(Section::Providers);
                app.nav.push(view.clone());
                let buf = render_to_buffer(&mut app, width, height);
                for (row, line) in text_of(&buf).lines().enumerate() {
                    assert!(
                        display_width(line) <= width as usize,
                        "{view:?} at {width}x{height}: row {row} overflows:\n{line}"
                    );
                }
                for overlay in STORE_OVERLAYS {
                    let mut app = app_with_stores();
                    app.go_to_section(Section::Providers);
                    app.nav.push(view.clone());
                    app.nav.open_overlay(overlay.clone());
                    let buf = render_to_buffer(&mut app, width, height);
                    for (row, line) in text_of(&buf).lines().enumerate() {
                        assert!(
                            display_width(line) <= width as usize,
                            "{overlay:?} over {view:?} at {width}x{height}: row {row}:\n{line}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn providers_says_stores_exist_even_with_nothing_joined() {
        let mut app = app_with(ContentList::Entries(Vec::new()));
        app.go_to_section(Section::Providers);
        let text = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(text.contains("PLUGIN STORES"), "{text}");
        assert!(text.contains("invite link"), "{text}");
    }

    #[test]
    fn a_store_page_lists_plugins_then_the_owners_actions() {
        let mut app = app_with_stores();
        app.go_to_section(Section::Providers);
        app.nav.push(StageView::Stores);
        app.nav.push(StageView::Store("aaaa".into()));
        let text = text_of(&render_to_buffer(&mut app, 140, 40));
        for expected in [
            "not installed",
            "installed",
            "update from 1.0.0",
            "added by hand",
            "invite someone",
            "members",
            "publish a plugin",
            "leave this store",
            "key 9f3c 1a2b 77de 0c41",
        ] {
            assert!(text.contains(expected), "missing {expected:?}:\n{text}");
        }
        // The first plugin is focused, and the footer says what it may contact.
        assert!(text.contains("may contact  api.example, cdn.example"), "{text}");
    }

    #[test]
    fn a_member_sees_no_owner_actions() {
        let mut app = app_with_stores();
        app.stores[0].owner = false;
        app.go_to_section(Section::Providers);
        app.nav.push(StageView::Store("aaaa".into()));
        let text = text_of(&render_to_buffer(&mut app, 140, 40));
        assert!(text.contains("link another device"));
        assert!(!text.contains("invite someone"));
        assert!(!text.contains("publish a plugin"));
    }

    #[test]
    fn the_plugin_overlay_states_its_hosts_before_offering_anything() {
        let mut app = app_with_stores();
        app.go_to_section(Section::Providers);
        app.nav.push(StageView::Store("aaaa".into()));
        app.nav.open_overlay(Overlay::PluginAction);
        let text = text_of(&render_to_buffer(&mut app, 140, 40));
        let hosts = text.find("may contact   api.example, cdn.example").expect("hosts shown");
        let adds = text.find("the update adds   else.example").expect("new host called out");
        let offer = text.find("update to 1.4.0").expect("update offered");
        assert!(hosts < offer && adds < offer, "facts come before the choice:\n{text}");
    }

    #[test]
    fn the_invite_link_is_shown_whole() {
        let mut app = app_with_stores();
        app.go_to_section(Section::Providers);
        app.nav.push(StageView::Store("aaaa".into()));
        app.nav.open_overlay(Overlay::InviteLink);
        // Narrow enough that the link has to span rows.
        let text = text_of(&render_to_buffer(&mut app, 60, 30));
        let link = app.invite_link.as_ref().unwrap().link.0.clone();
        let joined: String = text.chars().filter(|c| !c.is_whitespace() && *c != '▌').collect();
        assert!(joined.contains(&link), "the whole link must be on screen:\n{text}");
    }
}
