//! The Accounts screen: one row per tracker with everything that decides whether sync works, the
//! sign-in prompt while a device code is pending, and the rows of the overlays that show the
//! accounts and the divergences a merge refused to guess at.

use anistream_i18n::t;
use anistream_model::{present::tracker_name, sync::PullError};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::App,
    clock::Clock,
    theme::{
        Role,
        glyph::{self, OBI},
    },
    widgets::{Hairline, truncate, wrap},
};

use super::{
    MIN_TABLE_HEIGHT,
    chrome::{mark_selected, render_placeholder},
};

/// Accounts column offsets. Named because every field has to be truncated to its own column, and
/// three bare numbers in four places is how `not connecte0` happened.
const COL_STATE: u16 = 16;
const COL_QUEUED: u16 = 34;
const COL_TOKEN: u16 = 44;

/// The Accounts screen: one row per tracker, with everything that decides whether sync works.
///
/// Promoted from an overlay. One account fits in a modal; two do not — sync state, outbox depth,
/// where the token is stored and the last error are things you *compare* across trackers, and a
/// dialog you must dismiss to look at anything else is the wrong container for that.
pub(super) fn render_accounts(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height < MIN_TABLE_HEIGHT || area.width < 40 {
        render_placeholder(
            buf,
            app,
            area,
            &t::accounts_screen_title(),
            &t::lists_terminal_too_small(),
        );
        return;
    }
    let mut y = area.top();

    // A pending device code comes first and gets the loudest treatment on the screen. It is the one
    // thing in the whole app the user has to transcribe by hand, it stays valid for fifteen minutes,
    // and it used to live in the status line — dim, and liable to be overwritten by any background
    // task that set a status. Reported as never displayed at all, which it effectively was.
    if let Some(prompt) = &app.device_code {
        buf[(area.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
        buf.set_string(
            area.left() + 2,
            y,
            truncate(
                &t::accounts_finish_signing_in(&tracker_name(&prompt.tracker)),
                area.width.saturating_sub(3) as usize,
            ),
            app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
        );
        y += 2;
        buf.set_string(
            area.left() + 2,
            y,
            truncate(
                &t::accounts_step_open(&prompt.url),
                area.width.saturating_sub(3) as usize,
            ),
            app.palette.style(Role::TextDim),
        );
        y += 1;
        // Letterspaced, and this is the one place that treatment is right: a code being copied
        // character by character wants the characters separated. Everywhere else it was noise.
        let spaced: String =
            prompt.code.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
        buf.set_string(
            area.left() + 2,
            y,
            truncate(&t::accounts_step_enter(), area.width.saturating_sub(3) as usize),
            app.palette.style(Role::TextDim),
        );
        buf.set_string(
            area.left() + 12,
            y,
            truncate(&spaced, area.width.saturating_sub(13) as usize),
            app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
        );
        // How long the code has left, under it; once it expires, Enter on the row asks again.
        if let Some(at) = prompt.expires_at {
            y += 1;
            buf.set_string(
                area.left() + 12,
                y,
                truncate(
                    &code_expiry(at - app.clock.now_epoch()),
                    area.width.saturating_sub(13) as usize,
                ),
                app.palette.style(Role::TextDim),
            );
        }
        y += 2;
        Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
        y += 2;
        if y >= area.bottom() {
            return;
        }
    }

    buf.set_string(
        area.left(),
        y,
        t::accounts_column_tracker(),
        app.palette.style(Role::TextDim),
    );
    for (label, x) in [
        (t::accounts_column_state(), COL_STATE),
        (t::accounts_column_queued(), COL_QUEUED),
        (t::accounts_column_token(), COL_TOKEN),
    ] {
        if area.left() + x < area.right() {
            buf.set_string(area.left() + x, y, label, app.palette.style(Role::TextDim));
        }
    }
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
    y += 1;

    if app.sync.is_empty() {
        buf.set_string(
            area.left(),
            y,
            t::accounts_empty_heading(),
            app.palette.style(Role::TextDim),
        );
        buf.set_string(
            area.left(),
            y + 2,
            truncate(&t::accounts_empty_hint(TRACKERS_SETTING), area.width as usize),
            app.palette.style(Role::TextDim),
        );
        return;
    }

    let focused = app.nav.focus() == crate::nav::Focus::Stage;
    for (i, state) in app.sync.iter().enumerate() {
        if y >= area.bottom().saturating_sub(2) {
            break;
        }
        let selected = i == app.cursors.accounts.selected;
        if selected {
            mark_selected(buf, app, area.left(), y, focused);
        }
        buf.set_string(
            area.left() + 2,
            y,
            truncate(&tracker_name(&state.tracker), (COL_STATE - 3) as usize),
            if selected {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else {
                app.palette.style(Role::Text)
            },
        );

        // Needing re-authorisation is not the same as being signed out, and conflating them would
        // hide the one condition the user has to act on.
        // The same marks as the header chip for this tracker, so the two never disagree.
        let (mark, label, role) = match (state.connected, state.needs_reauth) {
            (_, true) => (glyph::STATE_DOWN, t::accounts_state_sign_in_again(), Role::Alert),
            (true, _) => (glyph::STATE_READY, t::accounts_state_connected(), Role::State),
            (false, _) => (glyph::STATE_OFF, t::accounts_state_not_connected(), Role::TextDim),
        };
        buf.set_string(
            area.left() + 16,
            y,
            format!("{} {}", mark, label),
            app.palette.style(role),
        );

        if area.left() + 30 < area.right() {
            // Queue depth belongs beside the account: ops accumulate against a tracker whether or
            // not it is signed in, and that is the number that says whether anything is stuck.
            let queued = state.outbox.to_string();
            buf.set_string(
                area.left() + COL_QUEUED,
                y,
                truncate(&queued, (COL_TOKEN - COL_QUEUED - 1) as usize),
                app.palette.style(if state.outbox > 0 { Role::State } else { Role::TextDim }),
            );
        }
        if let Some(storage) = &state.storage
            && area.left() + COL_TOKEN < area.right()
        {
            buf.set_string(
                area.left() + COL_TOKEN,
                y,
                truncate(
                    storage,
                    area.right().saturating_sub(area.left() + COL_TOKEN + 1) as usize,
                ),
                app.palette.style(Role::TextDim),
            );
        }
        y += 1;
        // A failed pull says so under its tracker until the next one succeeds: "connected" on
        // its own read as everything working while the library had stopped updating.
        if let Some(PullError { reason: error, at }) = &state.pull_error
            && y < area.bottom().saturating_sub(2)
        {
            // Wrapped, two lines at most: the reason is the part worth reading, and it comes last.
            let width = area.right().saturating_sub(area.left() + 17) as usize;
            for text in wrap(&pull_failed(&app.clock, *at, error), width, 2) {
                if y >= area.bottom().saturating_sub(2) {
                    break;
                }
                buf.set_string(
                    area.left() + 16,
                    y,
                    truncate(&text, width),
                    app.palette.style(Role::Alert),
                );
                y += 1;
            }
        }
    }

    // File-stored tokens are worth knowing about on a shared machine, but they are a working
    // state, not an error — so the fact is stated once, quietly, rather than a salmon cell
    // per row that reads as three failures.
    if app.sync.iter().any(|s| s.storage_degraded) && y + 1 < area.bottom() {
        y += 1;
        let width = area.width.saturating_sub(4) as usize;
        for text in wrap(&t::accounts_token_files(), width, 2) {
            if y >= area.bottom() {
                break;
            }
            buf.set_string(
                area.left() + 2,
                y,
                truncate(&text, width),
                app.palette.style(Role::TextDim),
            );
            y += 1;
        }
    }

    // What Enter will do to the selected row, spelled out — signing out is destructive enough that
    // it should never be a surprise.
    let action = app.sync.get(app.cursors.accounts.selected).map(|s| {
        if s.connected && !s.needs_reauth {
            t::accounts_enter_sign_out()
        } else {
            t::accounts_enter_sign_in()
        }
    });
    if let Some(action) = action
        && y + 1 < area.bottom()
    {
        Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
        buf.set_string(
            area.left() + 2,
            y + 1,
            truncate(&action, area.width.saturating_sub(4) as usize),
            app.palette.style(Role::TextDim),
        );
    }
}

/// Tracker accounts and what sync is doing.
pub(super) fn accounts_rows(app: &App) -> Vec<(String, String)> {
    if app.sync.is_empty() {
        return vec![(String::new(), t::accounts_rows_empty(TRACKERS_SETTING))];
    }
    app.sync
        .iter()
        .map(|state| {
            let action = if state.needs_reauth {
                t::accounts_action_sign_in_again()
            } else if state.connected {
                t::accounts_action_sign_out()
            } else {
                t::accounts_action_sign_in()
            };
            let mut detail = vec![tracker_name(&state.tracker)];
            match (&state.user, state.connected) {
                (Some(user), _) => detail.push(t::accounts_row_signed_in_as(user)),
                (None, true) => detail.push(t::accounts_row_connected()),
                (None, false) => detail.push(t::accounts_row_not_connected()),
            }
            match (state.outbox, state.deferred) {
                (0, _) => {}
                (queued, 0) => detail.push(t::accounts_row_queued(i64::from(queued))),
                (queued, held) => {
                    detail.push(t::accounts_row_queued_held(i64::from(queued), i64::from(held)))
                }
            }
            // Surfaced rather than hidden: someone on a shared machine should know their token
            // is in a file rather than a keychain.
            if state.storage_degraded
                && let Some(storage) = &state.storage
            {
                detail.push(storage.clone());
            }
            if let Some(last) = &state.last {
                detail.push(last.clone());
            }
            // Last, so a narrow row truncates the reason rather than the fact.
            if let Some(PullError { reason: error, at }) = &state.pull_error {
                detail.push(pull_failed(&app.clock, *at, error));
            }
            (action, detail.join("  ·  "))
        })
        .collect()
}

/// That a pull failed and why, with the local clock time of the failure — and the date once it
/// is not today, because "14:32" from last week reads as an outage still going on.
fn pull_failed(clock: &Clock, at: i64, error: &str) -> String {
    match clock.local(at) {
        Some(when) if when.date_naive() == clock.today() => {
            t::accounts_pull_failed_today(&when.format("%H:%M").to_string(), error)
        }
        Some(when) => t::accounts_pull_failed_dated(
            &when.format("%-d %b").to_string(),
            &when.format("%H:%M").to_string(),
            error,
        ),
        None => t::accounts_pull_failed(error),
    }
}

/// How long a device code has left: seconds in the last minute, whole minutes before that.
fn code_expiry(seconds_left: i64) -> String {
    match seconds_left {
        ..=0 => t::accounts_code_expired(),
        1..60 => t::accounts_code_expires_seconds(seconds_left),
        _ => t::accounts_code_expires_minutes((seconds_left + 59) / 60),
    }
}

/// The config setting that lists the enabled trackers, as the config file spells it.
pub(super) const TRACKERS_SETTING: &str = "trackers.enabled"; // i18n-skip

/// Divergences the merge refused to guess at, two rows each: keep this device's value, or the
/// tracker's. The second row only names the choice, so the pair reads as one difference.
pub(super) fn conflict_rows(app: &App) -> Vec<(String, String)> {
    app.conflicts
        .iter()
        .flat_map(|row| {
            [
                (
                    t::accounts_conflict_keep_mine(),
                    t::accounts_conflict_row(
                        &truncate(&row.title, 34),
                        &row.field,
                        &row.local,
                        &row.remote,
                    ),
                ),
                (t::accounts_conflict_keep_theirs(), String::new()),
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::{catalog::ContentList, event::Event};

    use crate::{
        nav::Section,
        screens::{
            fixtures::{NOON_MONDAY, app_with, text_of},
            render_to_buffer,
        },
    };

    /// Ops held for an id mapping are still queued, and the row says why they have not gone.
    #[test]
    fn an_accounts_row_says_how_much_of_the_queue_waits_for_a_mapping() {
        let mut app = app_with(ContentList::Entries(Vec::new()));
        app.apply(Event::Sync(Box::new(anistream_model::sync::SyncState {
            connected: true,
            outbox: 3,
            deferred: 2,
            ..anistream_model::sync::SyncState::new("mal")
        })));
        let (_, detail) = &accounts_rows(&app)[0];
        assert!(detail.contains("3 queued, 2 waiting for an id mapping"), "{detail}");

        app.apply(Event::Sync(Box::new(anistream_model::sync::SyncState {
            connected: true,
            outbox: 3,
            ..anistream_model::sync::SyncState::new("mal")
        })));
        let (_, detail) = &accounts_rows(&app)[0];
        assert!(detail.contains("3 queued") && !detail.contains("mapping"), "{detail}");
    }

    /// A failed pull used to leave the row saying "connected · pulled 40 titles" from the last
    /// success, for as long as the outage lasted.
    #[test]
    fn an_accounts_row_says_when_and_why_the_last_pull_failed() {
        let mut app = app_with(ContentList::Entries(Vec::new()));
        app.clock = Clock::fixed(NOON_MONDAY, crate::clock::Zone::utc());
        let at = NOON_MONDAY - 60;
        app.apply(Event::Sync(Box::new(anistream_model::sync::SyncState {
            connected: true,
            pull_error: Some(PullError { reason: "network: timed out".into(), at }),
            ..anistream_model::sync::SyncState::new("anilist")
        })));
        let (_, detail) = &accounts_rows(&app)[0];
        assert!(
            detail.contains("Fetching your list failed at 11:59: network: timed out"),
            "today's failure reads as a clock time: {detail}"
        );
        assert!(!detail.contains("pulled"), "{detail}");

        // An older failure carries its date.
        assert_eq!(
            pull_failed(&app.clock, at - 10 * 86_400, "timed out"),
            "Fetching your list failed on 9 Oct at 11:59: timed out"
        );
    }

    #[test]
    fn the_accounts_screen_shows_a_failed_pull_under_its_tracker() {
        // The reason used to reach only a list nothing on the screen drew from, so the row kept
        // saying "connected" while the library had stopped updating.
        let mut app = app_with(ContentList::Entries(Vec::new()));
        app.apply(Event::Sync(Box::new(anistream_model::sync::SyncState {
            connected: true,
            pull_error: Some(PullError {
                reason: "network: timed out".into(),
                at: app.clock.now_epoch(),
            }),
            ..anistream_model::sync::SyncState::new("anilist")
        })));
        app.go_to_section(Section::Accounts);
        let text = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(text.contains("Fetching your list failed at "), "{text}");
        assert!(text.contains("network: timed out"), "{text}");
    }

    #[test]
    fn a_pending_device_code_is_displayed_prominently() {
        // Reported from real use: "the pin is obviously never displayed to me". It was being sent to
        // the status line — the dimmest role in the palette, and overwritten by any background task
        // that set a status during the fifteen-minute wait.
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Accounts);
        app.apply(anistream_model::event::Event::Sync(Box::new(
            anistream_model::sync::SyncState::new("simkl"),
        )));
        app.apply(anistream_model::event::Event::DeviceCode(Some(
            anistream_model::sync::DeviceCodePrompt {
                tracker: "simkl".into(),
                code: "FA433".into(),
                url: "https://simkl.com/pin".into(),
                expires_at: None,
            },
        )));

        let buf = render_to_buffer(&mut app, 110, 20);
        let text = text_of(&buf);
        assert!(text.contains("Finish signing in to Simkl"), "got {text:?}");
        assert!(text.contains("https://simkl.com/pin"), "the URL has to be there too");
        // Letterspaced *here specifically*: a code copied character by character wants them
        // separated. This is the one place that treatment earns its keep.
        assert!(text.contains("F A 4 3 3"), "the code must be legible: {text:?}");
        // And the account rows are still visible underneath, not pushed off.
        assert!(text.contains("Simkl"), "the table must survive the prompt");
    }

    #[test]
    fn the_device_code_is_drawn_bold_in_the_text_role_not_the_dim_one() {
        // The specific defect: it was rendered in `TextDim`, the least visible role there is, for
        // the one string in the app a user has to transcribe by hand.
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Accounts);
        app.apply(anistream_model::event::Event::DeviceCode(Some(
            anistream_model::sync::DeviceCodePrompt {
                tracker: "simkl".into(),
                code: "FA433".into(),
                url: "https://simkl.com/pin".into(),
                expires_at: None,
            },
        )));
        let buf = render_to_buffer(&mut app, 110, 20);

        // Bold text rather than amber, which is the obi's alone.
        let text = app.palette.style(Role::Text).fg;
        let found = (0..20).any(|y| {
            (0..110).any(|x| {
                let cell = &buf[(x, y)];
                cell.symbol() == "F"
                    && cell.style().fg == text
                    && cell.style().add_modifier.contains(Modifier::BOLD)
            })
        });
        assert!(found, "the code should be drawn bold in the text role");
    }

    #[test]
    fn the_prompt_disappears_once_the_flow_ends() {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Accounts);
        app.apply(anistream_model::event::Event::DeviceCode(Some(
            anistream_model::sync::DeviceCodePrompt {
                tracker: "simkl".into(),
                code: "FA433".into(),
                url: "https://simkl.com/pin".into(),
                expires_at: None,
            },
        )));
        assert!(text_of(&render_to_buffer(&mut app, 110, 20)).contains("F A 4 3 3"));

        // A prompt left up after approval asks for a code that no longer does anything.
        app.apply(anistream_model::event::Event::DeviceCode(None));
        assert!(!text_of(&render_to_buffer(&mut app, 110, 20)).contains("F A 4 3 3"));
    }
}
