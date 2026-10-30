//! The Providers screen: each source and how it stands, the plugin stores line, and the metadata
//! ladder's rungs — every place the user looks to find out which source broke and why.

use anistream_i18n::t;
use anistream_model::stores::{StoreRow, StoreState};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::App,
    clock::Clock,
    nav::Focus,
    theme::{Role, glyph},
    widgets::{Cells, Hairline, truncate, wrap},
};

use super::{
    MIN_TABLE_HEIGHT,
    chrome::{mark_selected, render_placeholder, table_cell},
    line,
};

// Column offsets for the Providers table, named so the header and the body cannot drift
// apart — the same reason Accounts and Episodes have theirs.
pub(super) const COL_PROVIDER: u16 = 2;
const COL_PROVIDER_KIND: u16 = 22;
const COL_PROVIDER_STATE: u16 = 34;
const COL_PROVIDER_LATENCY: u16 = 50;

/// Provider health. This screen exists because sources die and the user has to be able to
/// see *which* one and why, rather than facing an unexplained empty list.
pub(super) fn render_providers(buf: &mut Buffer, app: &App, area: Rect) {
    // Eyebrow, gap, column headers, hairline, and a row: five rows of chrome before any content,
    // plus the per-provider error line below the table. Drawing into less than that wrote past the
    // end of the buffer — a panic at 24×8, which a resize passes through on the way down. Found by
    // the size-matrix test, not by looking.
    if area.height < MIN_TABLE_HEIGHT + 3 {
        render_placeholder(buf, app, area, &t::sources_title(), &t::sources_too_small());
        return;
    }
    buf.set_string(
        area.left(),
        area.top(),
        t::sources_title(),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );

    if app.providers.is_empty() {
        buf.set_string(
            area.left(),
            area.top() + 2,
            t::sources_none_available(),
            app.palette.style(Role::Alert),
        );
        // The actual reason, when there is one. "You configured nothing" and "your VPN
        // guard failed verification" call for completely different responses.
        let note = app.provider_note.clone().unwrap_or_else(|| {
            t::sources_torrents_off("providers.torrent.enabled") // i18n-skip
        });
        let mut y = area.top() + 4;
        for line in wrap(&note, area.width.saturating_sub(2) as usize, 4) {
            buf.set_string(area.left(), y, line, app.palette.style(Role::TextDim));
            y += 1;
        }
        // With nothing to play from, a store is the likeliest way to get something.
        let y = render_store_summary(buf, app, area, y + 1);
        // The metadata ladder is not a streaming source, but "is it dead?" is this
        // screen's question — and it has an answer even with no sources configured.
        render_meta_sources(buf, app, area, y);
        return;
    }

    let head = area.top() + 2;
    // A column is shown whole or not at all: started wherever it fell and cut by the screen's
    // edge, it left half words — `STAT`, `read` — at 40 columns.
    let shows = |x: u16, label: &str| x + label.cells() as u16 <= area.width;
    let (source_head, kind_head, state_head, latency_head) = (
        t::sources_column_source(),
        t::sources_column_kind(),
        t::sources_column_state(),
        t::sources_column_latency(),
    );
    for (label, x) in [
        (&source_head, COL_PROVIDER),
        (&kind_head, COL_PROVIDER_KIND),
        (&state_head, COL_PROVIDER_STATE),
        (&latency_head, COL_PROVIDER_LATENCY),
    ] {
        if shows(x, label) {
            buf.set_string(area.left() + x, head, label, app.palette.style(Role::TextDim));
        }
    }
    Hairline::new(&app.palette).render(Rect { y: head + 1, height: 1, ..area }, buf);

    let mut y = head + 2;
    for (i, provider) in app.providers.iter().enumerate() {
        if y >= area.bottom() {
            break;
        }
        // The obi marks the row `K` and `J` move, as it marks the selected row everywhere.
        // Whether a source will be used is the state column's job, and the weight of its name,
        // which drops for one that will not.
        let selected = i == app.cursors.providers.selected;
        if selected {
            mark_selected(buf, app, area.left(), y, app.nav.focus() == Focus::Stage);
        }
        let name = app.palette.style(if provider.usable { Role::Text } else { Role::TextDim });
        table_cell(
            buf,
            area,
            COL_PROVIDER,
            COL_PROVIDER_KIND - COL_PROVIDER - 1,
            y,
            &provider.id,
            if selected { name.add_modifier(Modifier::BOLD) } else { name },
        );
        if shows(COL_PROVIDER_KIND, &kind_head) {
            table_cell(
                buf,
                area,
                COL_PROVIDER_KIND,
                COL_PROVIDER_STATE - COL_PROVIDER_KIND - 1,
                y,
                &kind_text(&provider.kind),
                app.palette.style(Role::TextDim),
            );
        }

        // Held back is not unhealthy: local policy is withholding a working source.
        let state_role = match (provider.held_back, provider.usable) {
            (true, _) => Role::State,
            (false, false) => Role::Alert,
            (false, true) => Role::State,
        };
        if shows(COL_PROVIDER_STATE, &state_head) {
            table_cell(
                buf,
                area,
                COL_PROVIDER_STATE,
                COL_PROVIDER_LATENCY - COL_PROVIDER_STATE - 1,
                y,
                &state_text(&provider.state),
                app.palette.style(state_role),
            );
        }

        if let Some(ms) = provider.latency_ms
            && shows(COL_PROVIDER_LATENCY, &latency_head)
        {
            table_cell(
                buf,
                area,
                COL_PROVIDER_LATENCY,
                u16::MAX,
                y,
                &t::sources_latency(ms as i64),
                app.palette.style(Role::TextDim),
            );
        }
        y += 1;
    }

    // The most recent error, spelled out under a hairline.
    if let Some(problem) = app.providers.iter().find(|p| p.last_error.is_some())
        && y + 2 < area.bottom()
    {
        Hairline::new(&app.palette).render(Rect { y: y + 1, height: 1, ..area }, buf);
        let text = t::sources_last_error(
            &problem.id,
            problem.last_error.as_deref().unwrap_or_default(),
        );
        buf.set_string(
            area.left() + 2,
            y + 2,
            truncate(&text, area.width.saturating_sub(4) as usize),
            app.palette.style(Role::Alert),
        );
        y += 2;
    }

    let y = render_store_summary(buf, app, area, y + 2);
    render_meta_sources(buf, app, area, y);
}

/// The plugin stores line on Providers: how many are joined, what is waiting, and the key in.
///
/// Returns the row after it. Drawn even with nothing joined, because this line is the only way
/// anyone finds out stores exist.
fn render_store_summary(buf: &mut Buffer, app: &App, area: Rect, top: u16) -> u16 {
    if top + 2 >= area.bottom() {
        return top;
    }
    line(
        buf,
        area.left(),
        top,
        t::sources_stores_title(),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    let updates: usize = app.stores.iter().map(StoreRow::updates).sum();
    let removed = app.stores.iter().filter(|s| s.state == StoreState::Revoked).count();
    let summary = if !app.stores_loaded || app.stores.is_empty() {
        t::sources_stores_invite()
    } else {
        let joined = t::sources_stores_joined(app.stores.len() as i64);
        let updates = match updates {
            0 => String::new(),
            n => t::sources_stores_updates(n as i64),
        };
        let removed = match removed {
            0 => String::new(),
            n => t::sources_stores_removed(n as i64),
        };
        glyph::dotted(&[&joined, &updates, &removed])
    };
    let hint = t::sources_stores_open();
    let room = area.width.saturating_sub(hint.cells() as u16 + 4) as usize;
    line(
        buf,
        area.left() + COL_PROVIDER,
        top + 1,
        truncate(&summary, room),
        app.palette.style(if updates > 0 { Role::State } else { Role::TextDim }),
    );
    let hint_x = area.right().saturating_sub(hint.cells() as u16 + 1);
    if hint_x > area.left() + COL_PROVIDER {
        line(buf, hint_x, top + 1, &hint, app.palette.style(Role::TextDim));
    }
    top + 3
}

/// The metadata ladder's rungs: which one answered last, and what is wrong with the rest.
///
/// Lives on the Providers screen because "has AniList died?" is the same question as
/// "has my source died?" — where the answer is, not what kind of thing failed.
fn render_meta_sources(buf: &mut Buffer, app: &App, area: Rect, top: u16) {
    if app.meta_status.is_empty() || top + 2 >= area.bottom() {
        return;
    }
    buf.set_string(
        area.left(),
        top,
        t::sources_metadata_title(),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );

    for (y, row) in (top + 2..).zip(app.meta_status.iter()) {
        if y >= area.bottom() {
            break;
        }
        // Which rung answered last is in its name's weight and its state, not an obi: that
        // marks the selected row, and nothing here is selectable.
        table_cell(
            buf,
            area,
            COL_PROVIDER,
            COL_PROVIDER_KIND - COL_PROVIDER - 1,
            y,
            &row.source,
            app.palette.style(if row.active { Role::Text } else { Role::TextDim }),
        );
        table_cell(
            buf,
            area,
            COL_PROVIDER_KIND,
            COL_PROVIDER_STATE - COL_PROVIDER_KIND - 1,
            y,
            &meta_state_text(&row.state),
            app.palette.style(if row.healthy { Role::State } else { Role::Alert }),
        );
        if let Some(detail) = meta_detail(row, &app.clock)
            && COL_PROVIDER_STATE < area.width
        {
            buf.set_string(
                area.left() + COL_PROVIDER_STATE,
                y,
                truncate(&detail, area.width.saturating_sub(COL_PROVIDER_STATE + 2) as usize),
                app.palette.style(Role::TextDim),
            );
        }
    }
}

/// A rung's detail, with when a resting one is next tried said at draw time.
///
/// A clock time rather than a countdown: the idle screen is redrawn on change and a slow
/// heartbeat, so "next try in 39s" stood still for as long as nothing happened, and a clock
/// time is right whenever it is drawn. Past the deadline the source is not asked until something
/// needs it, so that is what the row says rather than a time already gone.
fn meta_detail(row: &anistream_model::sources::MetaSourceRow, clock: &Clock) -> Option<String> {
    let Some(at) = row.retry_at else { return row.detail.clone() };
    let cause = row.detail.clone().unwrap_or_else(t::sources_meta_cause_failing);
    Some(match clock.local(at) {
        Some(when) if at > clock.now_epoch() => {
            t::sources_meta_skipped_until(&when.format("%H:%M").to_string(), &cause)
        }
        _ => t::sources_meta_next_request(&cause),
    })
}

/// A source's kind as the table says it. A kind the catalogue does not know stays as sent.
fn kind_text(kind: &str) -> String {
    match kind {
        "native" => t::sources_kind_native(),
        "plugin" => t::sources_kind_plugin(),
        "remote" => t::sources_kind_remote(),
        "torrent" => t::sources_kind_torrent(),
        "licensed" => t::sources_kind_licensed(),
        other => other.to_owned(),
    }
}

/// A source's state as the table says it. A state the catalogue does not know stays as sent.
fn state_text(state: &str) -> String {
    match state {
        "ready" => t::sources_state_ready(),
        "degraded" => t::sources_state_degraded(),
        "down" => t::sources_state_down(),
        "unchecked" => t::sources_state_unchecked(),
        "held back" => t::sources_state_held_back(),
        other => other.to_owned(),
    }
}

/// A metadata service's state as the table says it. One the catalogue does not know stays as
/// sent.
fn meta_state_text(state: &str) -> String {
    match state {
        "live" => t::sources_meta_state_live(),
        "unreachable" => t::sources_meta_state_unreachable(),
        "backing off" => t::sources_meta_state_backing_off(),
        "off" => t::sources_meta_state_off(),
        "serving" => t::sources_meta_state_serving(),
        "standby" => t::sources_meta_state_standby(),
        "failing" => t::sources_meta_state_failing(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::{catalog::ContentList, event::Event};

    use crate::{
        nav::Section,
        screens::{
            fixtures::{NOON_MONDAY, app_with, lines_of, provider, text_of},
            render_to_buffer,
        },
    };

    /// The backoff line was a countdown formatted when the status was sent, and the idle screen
    /// is not redrawn every second, so it read "next try in 39s" for minutes.
    #[test]
    fn a_resting_meta_source_says_until_when_it_is_skipped() {
        // Noon UTC, read nine hours east: the retry two minutes on is a clock time there.
        let mut clock = Clock::fixed(NOON_MONDAY, crate::clock::Zone::hours_east(9).unwrap());
        let mut row = anistream_model::sources::MetaSourceRow {
            source: "anilist".into(),
            state: "backing off".into(),
            healthy: false,
            active: false,
            detail: Some("network: timed out".into()),
            retry_at: Some(NOON_MONDAY + 120),
        };
        assert_eq!(
            meta_detail(&row, &clock).as_deref(),
            Some("Skipped until 21:02 (network: timed out)")
        );
        // Once the deadline is behind it, a time already gone would read as stale.
        clock.advance(std::time::Duration::from_secs(121));
        assert_eq!(
            meta_detail(&row, &clock).as_deref(),
            Some("Tried again on the next request (network: timed out)")
        );
        row.retry_at = None;
        assert_eq!(meta_detail(&row, &clock).as_deref(), Some("network: timed out"));
    }

    #[test]
    fn the_providers_screen_names_what_broke_and_why() {
        // This screen exists because sources die; an unexplained empty list is the failure
        // mode the whole design avoids.
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Providers);
        app.apply(Event::Providers(vec![
            provider("torrent", "ready", true, false),
            anistream_model::sources::ProviderRow {
                last_error: Some("cloudflare challenge (403)".into()),
                ..provider("webprov", "down", false, false)
            },
        ]));

        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        assert!(text.contains("torrent"));
        assert!(text.contains("310 ms"), "latency helps judge a slow source");
        assert!(text.contains("cloudflare challenge (403)"), "the reason must be spelled out");
    }

    #[test]
    fn a_held_back_provider_is_visually_distinct_from_a_broken_one() {
        // Local policy withholding a working source is not the same as the source failing,
        // and the screen must not conflate them.
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Providers);
        app.apply(Event::Providers(vec![provider("torrent", "held back", false, true)]));

        let buf = render_to_buffer(&mut app, 110, 20);
        let text = text_of(&buf);
        assert!(text.contains("Held back"));

        let row = text.lines().position(|l| l.contains("Held back")).unwrap() as u16;
        let x = text.lines().nth(row as usize).unwrap().find("Held").unwrap() as u16;
        assert_eq!(
            buf[(x, row)].fg,
            app.palette.color(Role::State),
            "held back must not use the alert colour reserved for real failures"
        );
    }

    #[test]
    fn an_empty_provider_list_says_how_to_fix_it() {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Providers);
        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        assert!(text.contains("No sources available"));
        assert!(text.contains("VPN mode"), "must say what to actually do");
    }

    #[test]
    fn a_failing_guard_is_reported_instead_of_the_generic_advice() {
        // "You configured nothing" and "your VPN guard failed verification" call for
        // completely different responses, so the screen must not guess between them.
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Providers);
        app.apply(Event::ProviderNote(anistream_model::Msg::verbatim(
            "vpn guard: expected one of [Mullvad], but traffic is leaving via Comcast",
        )));

        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        assert!(text.contains("Comcast"), "the real reason must reach the screen");
        assert!(
            !text.contains("providers.torrent.enabled"),
            "generic setup advice is wrong here and would mislead"
        );
    }

    #[test]
    fn the_providers_table_marks_only_the_row_the_order_keys_move() {
        // One row, the selected one, and the mark goes with it when `J` moves it down the chain.
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Providers);
        app.nav.focus_stage();
        app.apply(Event::Providers(vec![
            provider("one", "ready", true, false),
            provider("two", "ready", true, false),
        ]));
        let marked = |app: &mut App| -> Vec<String> {
            lines_of(app, 120, 30)
                .into_iter()
                .filter(|l| l.contains("▌ one") || l.contains("▌ two"))
                .collect()
        };
        let lines = marked(&mut app);
        assert!(lines.len() == 1 && lines[0].contains("▌ one"), "{lines:#?}");

        app.settings.sources.order = vec!["one".into(), "two".into()];
        app.handle(crate::keymap::Action::LowerSource, 20);
        let lines = marked(&mut app);
        assert!(lines.len() == 1 && lines[0].contains("▌ one"), "{lines:#?}");
        let lines = lines_of(&mut app, 120, 30);
        let two = lines.iter().position(|l| l.contains(" two ")).unwrap();
        let one = lines.iter().position(|l| l.contains("▌ one")).unwrap();
        assert!(two < one, "{lines:#?}");
    }

    #[test]
    fn a_narrow_providers_table_drops_columns_rather_than_halving_words() {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Providers);
        app.apply(Event::Providers(vec![provider("someplugin", "ready", true, false)]));
        let text = text_of(&render_to_buffer(&mut app, 40, 15));
        assert!(
            !text.contains("Stat\u{20}")
                && !text.lines().any(|l| l.trim_end().ends_with("Stat"))
        );
        assert!(!text.lines().any(|l| l.trim_end().ends_with("Read")), "{text}");
    }
}
