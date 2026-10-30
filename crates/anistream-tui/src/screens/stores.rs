//! The plugin stores: the list of stores joined, a store's page with its plugins and the owner's
//! actions, and its members and invites; the rows of the overlays that join a store, confirm a
//! store action, act on a plugin, publish one and show an invite link.

use anistream_i18n::t;
use anistream_model::stores::{StorePluginRow, StorePluginStatus, StoreRow, StoreState};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, Field, MemberLine, PluginChoice, StoreConfirm, StoreLine},
    clock::Clock,
    nav::Focus,
    theme::{Role, glyph},
    widgets::{Cells, Hairline, truncate},
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
        StoreState::Revoked => (t::stores_state_revoked(), Role::Alert),
        StoreState::Offline(_) => {
            (format!("{} {}", glyph::STATE_DEGRADED, t::stores_state_offline()), Role::State)
        }
        StoreState::Ready => match store.updates() {
            0 => (
                format!("{} {}", glyph::STATE_READY, t::stores_state_up_to_date()),
                Role::TextDim,
            ),
            n => (t::stores_state_updates(n as i64), Role::State),
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
        render_placeholder(
            buf,
            app,
            area,
            &t::stores_list_heading(),
            &t::stores_needs_larger_terminal(),
        );
        return;
    }
    line(
        buf,
        area.left(),
        area.top(),
        t::stores_list_heading(),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    let head = area.top() + 2;
    for (label, x) in [
        (t::stores_column_store(), COL_PROVIDER),
        (t::stores_column_plugins(), COL_STORE_COUNT),
        (t::stores_column_you(), COL_STORE_ROLE),
        (t::stores_column_state(), COL_STORE_STATE),
    ] {
        if x < area.width {
            line(buf, area.left() + x, head, label, app.palette.style(Role::TextDim));
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
            t::stores_none_joined(),
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
            role_text(store.owner),
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
        line(buf, area.left() + COL_PROVIDER, join_y, t::stores_join_row(), style);
    }

    let (footer, role) = match app.stores.get(app.cursors.stores.selected) {
        Some(store) => match &store.state {
            StoreState::Revoked => (t::stores_footer_revoked(), Role::Alert),
            StoreState::Offline(reason) => {
                (t::stores_footer_offline(&store.url, reason), Role::State)
            }
            StoreState::Ready => (
                glyph::dotted(&[
                    &store.url,
                    &t::stores_key(&store.fingerprint),
                    &if store.anonymous {
                        t::stores_joined_without_name()
                    } else {
                        t::stores_joined_as(&store.member)
                    },
                ]),
                Role::TextDim,
            ),
        },
        None => (t::stores_footer_join(), Role::TextDim),
    };
    render_footer(buf, app, area, &footer, role);
}

/// One store: its plugins, then what can be done there.
pub(super) fn render_store(buf: &mut Buffer, app: &App, area: Rect, id: &str) {
    let Some(store) = app.store(id) else {
        render_placeholder(buf, app, area, &t::stores_page_heading(), &t::stores_loading());
        return;
    };
    if area.height < MIN_TABLE_HEIGHT + 6 || area.width < 40 {
        render_placeholder(buf, app, area, &store.name, &t::stores_needs_larger_terminal());
        return;
    }
    line(
        buf,
        area.left(),
        area.top(),
        truncate(&store.name, area.width.saturating_sub(10) as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    if store.owner {
        let tag = t::stores_owner_tag();
        line(
            buf,
            area.right().saturating_sub(tag.cells() as u16 + 1),
            area.top(),
            tag,
            app.palette.style(Role::TextDim),
        );
    }
    let (note, role) = match &store.state {
        StoreState::Ready if store.open => (
            glyph::dotted(&[
                &store.url,
                &t::stores_key(&store.fingerprint),
                &t::stores_anyone_can_join(),
            ]),
            Role::TextDim,
        ),
        StoreState::Ready => {
            (glyph::dotted(&[&store.url, &t::stores_key(&store.fingerprint)]), Role::TextDim)
        }
        StoreState::Offline(reason) => (t::stores_note_offline(reason), Role::State),
        StoreState::Revoked => (t::stores_note_revoked(), Role::Alert),
    };
    line(
        buf,
        area.left(),
        area.top() + 1,
        truncate(&note, area.width as usize),
        app.palette.style(role),
    );

    let head = area.top() + 3;
    for (label, x) in [
        (t::stores_column_plugin(), COL_PROVIDER),
        (t::stores_column_version(), COL_PLUGIN_VERSION),
        (t::stores_column_state(), COL_PLUGIN_STATE),
    ] {
        if x < area.width {
            line(buf, area.left() + x, head, label, app.palette.style(Role::TextDim));
        }
    }
    Hairline::new(&app.palette).render(Rect { y: head + 1, height: 1, ..area }, buf);

    let lines = app.store_lines(store);
    let last_row = area.bottom().saturating_sub(3);
    let mut y = head + 2;
    if store.plugins.is_empty() && y < last_row {
        let empty =
            if store.owner { t::stores_empty_owner() } else { t::stores_empty_member() };
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
                    StorePluginStatus::Available => {
                        (t::stores_plugin_not_installed(), Role::TextDim)
                    }
                    StorePluginStatus::Installed => (
                        format!("{} {}", glyph::STATE_READY, t::stores_plugin_installed()),
                        Role::State,
                    ),
                    StorePluginStatus::Update { from, .. } => {
                        (t::stores_plugin_update_from(from), Role::State)
                    }
                    StorePluginStatus::AddedByHand => {
                        (t::stores_plugin_added_by_hand(), Role::TextDim)
                    }
                    StorePluginStatus::OtherStore(_) => {
                        (t::stores_plugin_from_another_store(), Role::TextDim)
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
                if matches!(action, StoreLine::WhoCanJoin) && COL_PLUGIN_VERSION < area.width {
                    line(
                        buf,
                        area.left() + COL_PLUGIN_VERSION,
                        y,
                        truncate(
                            &who_can_join(store.open),
                            area.width.saturating_sub(COL_PLUGIN_VERSION + 1) as usize,
                        ),
                        app.palette.style(Role::TextDim),
                    );
                }
            }
        }
        y += 1;
    }

    let (footer, role) = match lines.get(app.cursors.store_lines.selected) {
        Some(StoreLine::Plugin(at)) => match store.plugins.get(*at) {
            Some(plugin) => match &plugin.status {
                StorePluginStatus::Update { new_hosts, .. } if !new_hosts.is_empty() => {
                    (t::stores_footer_update_hosts(&new_hosts.join(", ")), Role::Alert)
                }
                _ => (t::stores_footer_may_contact(&hosts_text(&plugin.hosts)), Role::TextDim),
            },
            None => (String::new(), Role::TextDim),
        },
        Some(StoreLine::LinkDevice) => (t::stores_footer_link_device(), Role::TextDim),
        Some(StoreLine::Invite) => (t::stores_footer_invite(), Role::TextDim),
        Some(StoreLine::Members) => (t::stores_footer_members(), Role::TextDim),
        Some(StoreLine::WhoCanJoin) if store.open => {
            (t::stores_footer_close_join(), Role::TextDim)
        }
        Some(StoreLine::WhoCanJoin) => (t::stores_footer_open_join(), Role::TextDim),
        Some(StoreLine::Publish) => (t::stores_footer_publish(), Role::TextDim),
        Some(StoreLine::Admin) => (t::stores_footer_admin(), Role::TextDim),
        Some(StoreLine::Leave) => (t::stores_footer_leave(), Role::TextDim),
        None => (String::new(), Role::TextDim),
    };
    render_footer(buf, app, area, &footer, role);
}

/// The plugins a store a link leads to offers, a line each, with what each may contact and where
/// it stands here: at most a handful, then how many more. `label` is the first line's label, and
/// `indent` the width the rows after it are set in by.
fn offered_rows(plugins: &[StorePluginRow], label: &str, indent: usize) -> Vec<String> {
    const SHOWN: usize = 5;
    if plugins.is_empty() {
        return Vec::new();
    }
    let pad = " ".repeat(indent);
    let mut rows = vec![format!("{label}{}", t::stores_offers_count(plugins.len() as i64))]; // i18n-skip
    rows.extend(plugins.iter().take(SHOWN).map(|plugin| {
        let here = match &plugin.status {
            StorePluginStatus::Available => String::new(),
            StorePluginStatus::Installed => t::stores_offered_installed(),
            StorePluginStatus::Update { from, .. } => t::stores_offered_installed_version(from),
            StorePluginStatus::AddedByHand => t::stores_offered_added_by_hand(),
            StorePluginStatus::OtherStore(other) => t::stores_offered_from_other(other),
        };
        let here = if here.is_empty() { here } else { format!(" · {here}") }; // i18n-skip
        format!(
            "{pad}{}{here}", // i18n-skip
            t::stores_offered_plugin(&plugin.name, &plugin.version, &hosts_text(&plugin.hosts))
        )
    }));
    if plugins.len() > SHOWN {
        rows.push(format!("{pad}{}", t::stores_offered_more((plugins.len() - SHOWN) as i64))); // i18n-skip
    }
    rows
}

/// A store's role in a few words.
fn role_text(owner: bool) -> String {
    if owner { t::stores_role_owner() } else { t::stores_role_member() }
}

/// Who can join a store, in a few words.
fn who_can_join(open: bool) -> String {
    if open { t::stores_who_open() } else { t::stores_who_invite_only() }
}

fn hosts_text(hosts: &[String]) -> String {
    if hosts.is_empty() { t::stores_hosts_none() } else { hosts.join(", ") }
}

/// How long ago a moment was, coarsely. Exactness would be noise on a "last seen" column.
fn ago(clock: &Clock, at: i64) -> String {
    let span = clock.now_epoch().saturating_sub(at).max(0);
    match span {
        0..3_600 => t::stores_ago_just_now(),
        3_600..86_400 => t::stores_ago_today(),
        86_400..172_800 => t::stores_ago_yesterday(),
        _ => t::stores_ago_days(span / 86_400),
    }
}

/// [`ago`] for after a word ("Joined just now"), so it starts in lower case.
fn ago_inline(clock: &Clock, at: i64) -> String {
    let span = clock.now_epoch().saturating_sub(at).max(0);
    match span {
        0..3_600 => t::stores_ago_inline_just_now(),
        3_600..86_400 => t::stores_ago_inline_today(),
        86_400..172_800 => t::stores_ago_inline_yesterday(),
        _ => t::stores_ago_inline_days(span / 86_400),
    }
}

/// How long until a moment, coarsely.
fn until(clock: &Clock, at: i64) -> String {
    let span = at.saturating_sub(clock.now_epoch());
    match span {
        ..=0 => t::stores_expired(),
        1..86_400 => t::stores_expires_today(),
        86_400..172_800 => t::stores_expires_tomorrow(),
        _ => t::stores_expires_days(span / 86_400),
    }
}

/// Who has joined a store and which invites are still open, for its owner.
pub(super) fn render_store_members(buf: &mut Buffer, app: &App, area: Rect, id: &str) {
    let name = app.store(id).map_or_else(t::stores_unnamed, |s| s.name.clone());
    if area.height < MIN_TABLE_HEIGHT + 4 || area.width < 40 {
        render_placeholder(
            buf,
            app,
            area,
            &t::stores_members_placeholder_heading(),
            &t::stores_needs_larger_terminal(),
        );
        return;
    }
    line(
        buf,
        area.left(),
        area.top(),
        truncate(&t::stores_members_heading(&name), area.width as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    let head = area.top() + 2;
    for (label, x) in [
        (t::stores_column_name(), COL_PROVIDER),
        (t::stores_column_role(), COL_STORE_COUNT),
        (t::stores_column_devices(), COL_STORE_ROLE),
        (t::stores_column_last_seen(), COL_STORE_STATE),
    ] {
        if x < area.width {
            line(buf, area.left() + x, head, label, app.palette.style(Role::TextDim));
        }
    }
    Hairline::new(&app.palette).render(Rect { y: head + 1, height: 1, ..area }, buf);

    let last_row = area.bottom().saturating_sub(3);
    let mut y = head + 2;
    if app.store_members.is_empty() {
        line(
            buf,
            area.left() + COL_PROVIDER,
            y,
            t::stores_loading(),
            app.palette.style(Role::TextDim),
        );
        return;
    }
    let lines = app.member_lines();
    let selected = lines.get(app.cursors.members.selected).copied();
    let at = |line: MemberLine| lines.iter().position(|l| *l == line);
    let dim = app.palette.style(Role::TextDim);
    for (i, member) in app.store_members.iter().enumerate() {
        if y >= last_row {
            break;
        }
        let style = focus_row(buf, app, area, y, selected == Some(MemberLine::Member(i)));
        let label =
            if member.you { t::stores_member_you(&member.name) } else { member.name.clone() };
        line(
            buf,
            area.left() + COL_PROVIDER,
            y,
            truncate(&label, (COL_STORE_COUNT - COL_PROVIDER - 2) as usize),
            style,
        );
        line(buf, area.left() + COL_STORE_COUNT, y, role_text(member.owner), dim);
        line(buf, area.left() + COL_STORE_ROLE, y, member.devices.to_string(), dim);
        let seen = member.last_seen.map_or_else(t::stores_seen_never, |at| ago(&app.clock, at));
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
        line(buf, area.left(), y, t::stores_invites_heading(), dim);
        y += 1;
        for (i, invite) in app.store_invites.iter().enumerate() {
            if y >= last_row {
                break;
            }
            let style = focus_row(buf, app, area, y, selected == Some(MemberLine::Invite(i)));
            let label = if invite.device {
                t::stores_invite_another_device(&invite.name)
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
                dim,
            );
            y += 1;
        }
    }

    // The store's link and the devices that joined with it: counted, never named.
    if let Some(view) = app.open_join_view()
        && (view.open || view.devices > 0)
        && y + 2 < last_row
    {
        y += 1;
        let joined = t::stores_joined_devices(i64::from(view.devices));
        let heading = if view.open {
            t::stores_open_heading(&joined)
        } else {
            t::stores_closed_heading(&joined)
        };
        line(buf, area.left(), y, truncate(&heading, area.width as usize), dim);
        y += 1;
        let mut row = |y: &mut u16, line_at: Option<usize>, label: String, note: String| {
            if *y >= last_row {
                return;
            }
            let style =
                focus_row(buf, app, area, *y, line_at == Some(app.cursors.members.selected));
            line(
                buf,
                area.left() + COL_PROVIDER,
                *y,
                truncate(&label, (COL_STORE_STATE - COL_PROVIDER - 2) as usize),
                style,
            );
            line(
                buf,
                area.left() + COL_STORE_STATE,
                *y,
                truncate(&note, area.width.saturating_sub(COL_STORE_STATE + 1) as usize),
                dim,
            );
            *y += 1;
        };
        if view.open {
            row(
                &mut y,
                at(MemberLine::OpenLink),
                t::stores_open_link_row(),
                t::stores_open_link_note(),
            );
        }
        for (i, device) in view.recent.iter().enumerate() {
            let seen = device.last_seen.map_or_else(t::stores_anonymous_never_seen, |at| {
                t::stores_anonymous_seen(&ago_inline(&app.clock, at))
            });
            row(
                &mut y,
                at(MemberLine::Anonymous(i)),
                t::stores_anonymous_joined(&ago_inline(&app.clock, device.joined_at)),
                seen,
            );
        }
        if view.devices > 0 {
            let all = t::stores_remove_devices(i64::from(view.devices));
            row(&mut y, at(MemberLine::RemoveAnonymous), all, String::new());
        }
    }

    let footer = match selected {
        Some(MemberLine::Member(i)) => match app.store_members.get(i) {
            Some(member) if member.you => t::stores_footer_you(),
            _ => t::stores_footer_remove_member(),
        },
        Some(MemberLine::Invite(_)) => t::stores_footer_cancel_invite(),
        Some(MemberLine::OpenLink) => t::stores_footer_show_link(),
        Some(MemberLine::Anonymous(_)) => t::stores_footer_remove_anonymous(),
        Some(MemberLine::RemoveAnonymous) => t::stores_footer_remove_all_anonymous(),
        None => String::new(),
    };
    render_footer(buf, app, area, &footer, Role::TextDim);
}

/// How many characters of an invite link fit on one overlay row.
pub(super) fn link_width(band_width: u16) -> usize {
    // The band's two-cell indent each side, and the label's own margin.
    (band_width as usize).saturating_sub(7).max(16)
}

/// A link in row-sized pieces.
///
/// Cut at fixed widths rather than wrapped at words: a link has no words, and a selection across
/// the rows pastes back as one link because the join prompt drops whitespace.
fn link_chunks(link: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = link.chars().collect();
    chars.chunks(width.max(1)).map(|chunk| chunk.iter().collect()).collect()
}

/// The invite link in row-sized pieces.
pub(super) fn invite_link_chunks(app: &App, width: usize) -> Vec<String> {
    app.invite_link.as_ref().map_or_else(Vec::new, |link| link_chunks(&link.link.0, width))
}

/// The store's open link in row-sized pieces.
pub(super) fn open_link_chunks(app: &App, width: usize) -> Vec<String> {
    app.open_link().map_or_else(Vec::new, |link| link_chunks(link, width))
}

pub(super) fn open_link_rows(app: &App, width: usize) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> =
        open_link_chunks(app, width).into_iter().map(|chunk| (String::new(), chunk)).collect();
    rows.push((String::new(), String::new()));
    rows.push((
        String::new(),
        if app.open_link_copied { t::stores_link_copied() } else { t::stores_link_select() },
    ));
    rows.push((String::new(), t::stores_open_link_warning()));
    rows.push((String::new(), t::stores_open_link_lasts()));
    rows.push((String::new(), t::stores_link_close()));
    rows
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
        if app.invite_copied { t::stores_link_copied() } else { t::stores_link_select() },
    ));
    rows.push((
        String::new(),
        t::stores_invite_works_once(&until(&app.clock, view.expires_at)),
    ));
    rows.push((String::new(), t::stores_invite_preview_safe()));
    rows.push((String::new(), t::stores_link_close()));
    rows
}

pub(super) fn store_confirm_heading(app: &App) -> String {
    match &app.store_confirm {
        Some(StoreConfirm::Join(preview)) => t::stores_confirm_join(&preview.name),
        Some(StoreConfirm::Leave { name, .. }) => t::stores_confirm_leave(name),
        Some(StoreConfirm::RemoveMember { name, .. }) => t::stores_confirm_remove_member(name),
        Some(StoreConfirm::CancelInvite { name, .. }) => t::stores_confirm_cancel_invite(name),
        Some(StoreConfirm::Unpublish { name, .. }) => t::stores_confirm_unpublish(name),
        Some(StoreConfirm::SetOpen { name, open: true, .. }) => t::stores_confirm_open(name),
        Some(StoreConfirm::SetOpen { name, open: false, .. }) => t::stores_confirm_close(name),
        Some(StoreConfirm::RemoveAnonymous { device: Some(_), .. }) => {
            t::stores_confirm_remove_device()
        }
        Some(StoreConfirm::RemoveAnonymous { devices, .. }) => {
            t::stores_confirm_remove_devices(i64::from(*devices))
        }
        None => t::stores_confirm_fallback(),
    }
}

pub(super) fn store_confirm_rows(app: &App) -> Vec<(String, String)> {
    let row = |text: &str| (String::new(), text.to_owned());
    match &app.store_confirm {
        Some(StoreConfirm::Join(preview)) => {
            // The labels are set in one column, as wide as the longest and three cells more.
            let labels = [
                t::stores_label_address(),
                t::stores_label_key(),
                t::stores_label_run_by(),
                t::stores_label_device(),
                t::stores_label_offers(),
            ];
            let indent = labels.iter().map(|l| l.cells()).max().unwrap_or(0) + 3;
            let [address, key, run_by, device, offers] = labels;
            let labelled = |label: &str, value: &str| row(&format!("{label:<indent$}{value}")); // i18n-skip
            let fingerprint = if preview.anonymous {
                t::stores_key_matches_link(&preview.fingerprint)
            } else {
                t::stores_key_matches_invite(&preview.fingerprint)
            };
            let mut rows = vec![labelled(&address, &preview.url), labelled(&key, &fingerprint)];
            if !preview.owners.is_empty() {
                rows.push(labelled(&run_by, &preview.owners.join(", ")));
            }
            rows.extend(
                offered_rows(&preview.plugins, &format!("{offers:<indent$}"), indent) // i18n-skip
                    .into_iter()
                    .map(|text| row(&text)),
            );
            if preview.open {
                rows.push(row(&t::stores_join_open()));
            }
            if preview.anonymous {
                rows.push(row(&t::stores_join_anonymous()));
            }
            if preview.already_joined {
                rows.push(row(&t::stores_join_already()));
            }
            // An invite sends a name for this device, which the person joining can change here.
            if !preview.anonymous {
                rows.push(row(""));
                let name = super::overlays::field_text(app, Field::DeviceLabel);
                rows.push(labelled(&device, &format!("{name}▏"))); // i18n-skip
                rows.push(row(&format!("{:indent$}{}", "", t::stores_join_device_note()))); // i18n-skip
            }
            rows.push(row(""));
            rows.push(row(&t::stores_join_keys()));
            rows
        }
        Some(StoreConfirm::Leave { .. }) => vec![
            row(&t::stores_leave_plugins()),
            row(&t::stores_leave_revoked()),
            row(""),
            row(&t::stores_leave_keys()),
        ],
        Some(StoreConfirm::RemoveMember { devices, .. }) => vec![
            row(&t::stores_remove_member_devices(i64::from(*devices))),
            row(&t::stores_remove_member_plugins()),
            row(""),
            row(&t::stores_remove_member_keys()),
        ],
        Some(StoreConfirm::CancelInvite { .. }) => vec![
            row(&t::stores_cancel_invite_link()),
            row(""),
            row(&t::stores_cancel_invite_keys()),
        ],
        Some(StoreConfirm::Unpublish { .. }) => {
            vec![row(&t::stores_unpublish_members()), row(""), row(&t::stores_unpublish_keys())]
        }
        Some(StoreConfirm::SetOpen { open: true, .. }) => vec![
            row(&t::stores_open_anyone()),
            row(&t::stores_open_link_new()),
            row(&t::stores_open_members_see()),
            row(""),
            row(&t::stores_open_keys()),
        ],
        Some(StoreConfirm::SetOpen { open: false, .. }) => vec![
            row(&t::stores_close_link()),
            row(&t::stores_close_devices()),
            row(""),
            row(&t::stores_close_keys()),
        ],
        Some(StoreConfirm::RemoveAnonymous { device, .. }) => vec![
            row(&if device.is_some() {
                t::stores_anonymous_one_stops()
            } else {
                t::stores_anonymous_all_stop()
            }),
            row(&t::stores_anonymous_plugins()),
            row(&t::stores_anonymous_still_joins()),
            row(""),
            row(&t::stores_anonymous_keys()),
        ],
        None => Vec::new(),
    }
}

/// The plugin overlay's rows, and how many of them are facts before the choices begin.
pub(super) fn plugin_action_rows(app: &App) -> (Vec<(String, String)>, usize) {
    let Some((_, plugin)) = app.plugin_in_action() else { return (Vec::new(), 0) };
    let mut rows = vec![
        (String::new(), t::stores_plugin_may_contact(&hosts_text(&plugin.hosts))),
        (String::new(), t::stores_plugin_cannot()),
    ];
    match &plugin.status {
        StorePluginStatus::Update { new_hosts, .. } if !new_hosts.is_empty() => {
            rows.push((String::new(), t::stores_plugin_update_adds(&new_hosts.join(", "))))
        }
        StorePluginStatus::AddedByHand => {
            rows.push((String::new(), t::stores_plugin_added_by_hand_note(&plugin.id)))
        }
        StorePluginStatus::OtherStore(_) => {
            rows.push((String::new(), t::stores_plugin_other_store_note()))
        }
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
                None => t::stores_publish_new(),
                Some(version) if *version == plugin.version => t::stores_publish_published(),
                Some(version) => t::stores_publish_replaces(version),
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
                owners: Vec::new(),
                open: false,
                anonymous: false,
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
                owners: Vec::new(),
                open: false,
                anonymous: false,
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
                owners: Vec::new(),
                open: false,
                anonymous: false,
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
            owners: Vec::new(),
            plugins: Vec::new(),
            open: false,
            anonymous: false,
            device_label: "Computer".into(),
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
        // The line on the Sources screen belongs to that screen; it names the stores either way.
        assert!(text.to_lowercase().contains("plugin stores"), "{text}");
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
            t::stores_plugin_not_installed(),
            t::stores_plugin_installed(),
            t::stores_plugin_update_from("1.0.0"),
            t::stores_plugin_added_by_hand(),
            t::stores_line_invite(),
            t::stores_line_members(),
            t::stores_line_publish(),
            t::stores_line_leave(),
            t::stores_key("9f3c 1a2b 77de 0c41"),
        ] {
            assert!(text.contains(&expected), "missing {expected:?}:\n{text}");
        }
        // The first plugin is focused, and the footer says what it may contact.
        assert!(
            text.contains(&t::stores_footer_may_contact("api.example, cdn.example")),
            "{text}"
        );
    }

    #[test]
    fn a_member_sees_no_owner_actions() {
        let mut app = app_with_stores();
        app.stores[0].owner = false;
        app.go_to_section(Section::Providers);
        app.nav.push(StageView::Store("aaaa".into()));
        let text = text_of(&render_to_buffer(&mut app, 140, 40));
        assert!(text.contains(&t::stores_line_link_device()));
        assert!(!text.contains(&t::stores_line_invite()));
        assert!(!text.contains(&t::stores_line_publish()));
    }

    #[test]
    fn the_plugin_overlay_states_its_hosts_before_offering_anything() {
        let mut app = app_with_stores();
        app.go_to_section(Section::Providers);
        app.nav.push(StageView::Store("aaaa".into()));
        app.nav.open_overlay(Overlay::PluginAction);
        let text = text_of(&render_to_buffer(&mut app, 140, 40));
        let hosts = text
            .find(&t::stores_plugin_may_contact("api.example, cdn.example"))
            .expect("hosts shown");
        let adds = text
            .find(&t::stores_plugin_update_adds("else.example"))
            .expect("new host called out");
        let offer = text.find(&t::stores_choice_update("1.4.0")).expect("update offered");
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
