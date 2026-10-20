//! Drawing.
//!
//! Read-only over [`App`]: every function here takes `&App` and paints. Nothing mutates, so
//! a render can never change behaviour, and screens can be exercised against a `TestBackend`
//! without a running event loop. What only a frame can find out — how an overlay laid out at
//! this size — it reports, as a [`FrameReport`] for whatever drew it to hand back to the App.

mod accounts;
mod chrome;
mod downloads;
#[cfg(test)]
mod fixtures;
mod lists;
mod playback;
mod providers;
mod settings;
mod stores;
mod title;

pub use downloads::download_state_label;
pub use playback::skip_label;

use anistream_model::episodes::EpisodeKind;
use ratatui::{
    Frame as TFrame, buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget,
};

use crate::{
    app::{App, Field, FrameReport},
    layout::{self, Frame},
    nav::{Focus, Overlay, Section, StageView},
    theme::{
        Palette, Role,
        glyph::{self, OBI},
    },
    widgets::{Divider, Hairline, Rail, truncate, wrap},
};

use accounts::{accounts_rows, conflict_rows, render_accounts};
use chrome::{mark_selected, render_activity, render_header, render_status, render_toasts};
use downloads::render_downloads;
use lists::{plate_row, render_library, render_list, render_loading};
use playback::{render_eyecatch, render_now_playing};
use providers::render_providers;
use settings::render_settings;
use stores::{
    invite_link_chunks, invite_link_rows, link_width, plugin_action_rows, publish_rows,
    render_store, render_store_members, render_stores, store_confirm_heading,
    store_confirm_rows,
};
use title::{
    rate_rows, recommendation_rows, render_title, source_provider_rows, status_rows,
    watch_order_rows,
};

/// Write a line, or nothing at all if it would land outside the buffer.
///
/// Every screen here is laid out by adding row offsets to a top, and a terminal can always be
/// shorter than the layout wants — a tmux split, a drag of the window edge, a 24×6 pane someone
/// keeps in a corner. `Buffer::set_string` panics on a position outside the buffer, so a screen
/// that forgets one bounds check takes the whole app down at a size nobody tested.
///
/// Guarding here rather than at each call site is deliberate: the checks were already present in
/// about half the places that needed them, and the half that were missing is not a list anyone
/// can keep correct by hand.
fn line(buf: &mut Buffer, x: u16, y: u16, text: impl AsRef<str>, style: ratatui::style::Style) {
    let bounds = buf.area();
    if y >= bounds.bottom() || x >= bounds.right() {
        return;
    }
    buf.set_string(x, y, text, style);
}

/// Draw a progress meter with the empty track at hairline weight.
///
/// The track is never painted in the fill's role: an empty meter in fill colour reads as a
/// solid bar, and a column of them (26 unwatched episodes) stacks into a slab. At `Rule`
/// weight the track recedes the way every other structural element here does.
fn set_meter(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    palette: &Palette,
    fraction: f64,
    width: usize,
    fill: Role,
) {
    let (filled, empty) = glyph::meter_parts(fraction, width);
    let filled_width = filled.chars().count() as u16;
    buf.set_string(x, y, &filled, palette.style(fill));
    buf.set_string(x + filled_width, y, &empty, palette.style(Role::Rule));
}

/// Paint a whole frame, and say what it found out that only a frame can: see [`FrameReport`].
pub fn render(frame: &mut TFrame<'_>, app: &App) -> FrameReport {
    let area = frame.area();
    let now = app.clock.now_instant();
    let activity = app.activity(now);
    let geometry = layout::compute(area, app.nav.rail_width());
    let buf = frame.buffer_mut();

    // Immersive mode paints its own ground; adaptive inherits the terminal's, which is what
    // lets anistream sit alongside the user's other tools.
    if let Some(ground) = app.palette.ground() {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_bg(ground.to_ratatui());
            }
        }
    }

    render_header(buf, app, &geometry);
    Hairline::new(&app.palette).render(geometry.header_rule, buf);

    if geometry.has_rail() {
        let counts = app.rail_counts();
        Rail::new(&app.palette, app.nav.rail_width(), app.nav.section(), app.nav.focus())
            .counts(&counts)
            .render(geometry.rail, buf);
        Divider::new(&app.palette).render(geometry.divider, buf);
    }

    render_stage(buf, app, geometry.stage);

    Hairline::new(&app.palette).render(geometry.status_rule, buf);
    let caption_end = render_activity(buf, app, &activity, geometry.status_rule, now);
    let report = render_overlay(buf, app, area, &geometry);
    // After the overlay, so it is not dimmed with the rest: while one is open the status line
    // carries that overlay's own keys, which are no use faded into the scrim — and the overlay
    // has to be laid out before its hints can say whether it pages, which is this frame's word
    // where it drew the help table and the last one's where it did not.
    render_status(buf, app, &geometry, report.help_pages.unwrap_or(app.help_pages()));
    // After the overlay for the same reason: a message answering a key pressed in a picker
    // is read while the picker is still up.
    render_toasts(buf, app, geometry.status_rule, caption_end);
    // Last, so it occludes: an eyecatch that toasts could bleed through would not be covering
    // anything.
    render_eyecatch(buf, app, geometry.stage);
    report
}

fn render_stage(buf: &mut Buffer, app: &App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let inner = layout::inset(area, 2, 0);

    match app.nav.current() {
        StageView::Title(_) => render_title(buf, app, inner),
        StageView::Episodes(_) => render_episodes(buf, app, inner),
        StageView::NowPlaying => render_now_playing(buf, app, inner),
        StageView::Stores => render_stores(buf, app, inner),
        StageView::Store(id) => render_store(buf, app, inner, id),
        StageView::StoreMembers(id) => render_store_members(buf, app, inner, id),
        StageView::Section(section) => match section {
            Section::Home | Section::Seasonal | Section::Search | Section::Calendar => {
                render_list(buf, app, inner, *section)
            }
            Section::Downloads => render_downloads(buf, app, inner),
            Section::Providers => render_providers(buf, app, inner),
            Section::Accounts => render_accounts(buf, app, inner),
            Section::Library => render_library(buf, app, inner),
            Section::Settings => render_settings(buf, app, inner),
        },
    }
}

/// The timing-sheet episode table.
///
/// Modelled on an animation production timing sheet: a fixed-width numeral column, hairline
/// headers, and dense rows. Episodes genuinely are a sequence, so the numbering carries
/// information rather than decorating — which is what earns the zero-padded field.
fn render_episodes(buf: &mut Buffer, app: &App, area: Rect) {
    let title = app.detail.as_ref().map_or("EPISODES", |e| e.title.as_str());
    buf.set_string(
        area.left(),
        area.top(),
        format!("{}  {}", glyph::BACK, truncate(title, area.width.saturating_sub(4) as usize)),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );

    let mut table = Rect { y: area.top() + 2, height: area.height.saturating_sub(2), ..area };
    if table.height < 3 {
        return;
    }

    // A still panel for the selected episode, and only when there is a reason for it. Coverage
    // is uneven — the listings come from licensed services, so an older title often has none at
    // all — and reserving a column of empty plate for every such show would be a permanent gap
    // pretending to be a feature. The table simply keeps the whole width instead.
    let still = still_panel(app, table);
    if let Some(panel) = still {
        table.width = table.width.saturating_sub(panel.width + STILL_GAP);
    }

    // Loading and empty are different answers and need different messages. This screen used to
    // show the previous title's rows while it waited, then correct itself.
    if app.episodes_loading {
        render_loading(buf, app, table);
        return;
    }

    if app.episodes.is_empty() {
        let (headline, hint) = empty_episodes(app);
        line(
            buf,
            table.left(),
            table.top(),
            glyph::eyebrow(headline),
            app.palette.style(Role::TextDim),
        );
        let room = table.bottom().saturating_sub(table.top() + 2) as usize;
        for (i, text) in wrap(&hint, table.width as usize, room.min(3)).into_iter().enumerate()
        {
            line(
                buf,
                table.left(),
                table.top() + 2 + i as u16,
                text,
                app.palette.style(Role::TextDim),
            );
        }
        return;
    }

    if let Some(panel) = still {
        // The panel is shared: the still on top, the episode's synopsis under it. When no
        // episode in this list has a still at all, the plate would be an empty rectangle
        // pretending to be an image — the words start at the top instead.
        let selected = app.episodes.get(app.cursors.episodes.selected);
        let mut text_top = panel.y;
        if app.episodes.iter().any(|row| row.thumbnail.is_some()) {
            draw_artwork(buf, app, selected.and_then(|e| e.thumbnail.as_deref()), panel);
            text_top = panel.bottom() + 1;
        }
        if let Some(description) = selected.and_then(|e| e.description.as_deref()) {
            let room = table.bottom().saturating_sub(text_top) as usize;
            let lines = wrap(description, panel.width as usize, room);
            for (i, line) in lines.into_iter().enumerate() {
                buf.set_string(
                    panel.x,
                    text_top + i as u16,
                    line,
                    app.palette.style(Role::TextDim),
                );
            }
        }
    }

    // Column headers, in caps above a hairline.
    let cols = episode_columns(table.width);
    buf.set_string(
        table.left() + cols.number,
        table.top(),
        "#",
        app.palette.style(Role::TextDim),
    );
    buf.set_string(
        table.left() + cols.title,
        table.top(),
        glyph::eyebrow("title"),
        app.palette.style(Role::TextDim),
    );
    buf.set_string(
        table.left() + cols.runtime,
        table.top(),
        glyph::eyebrow("run"),
        app.palette.style(Role::TextDim),
    );
    buf.set_string(
        table.left() + cols.state,
        table.top(),
        glyph::eyebrow("watched"),
        app.palette.style(Role::TextDim),
    );
    // The active filter sits at the right edge of the header row — always visible while
    // it is narrowing the table, invisible when it is not.
    if app.episode_filter != crate::app::EpisodeFilter::All {
        let marker = glyph::eyebrow(app.episode_filter.label());
        let width = marker.chars().count() as u16;
        if table.left() + cols.state + 10 + width < table.right() {
            buf.set_string(
                table.right().saturating_sub(width),
                table.top(),
                marker,
                app.palette.style(Role::State),
            );
        }
    }
    Hairline::new(&app.palette).render(Rect { y: table.top() + 1, height: 1, ..table }, buf);

    let body_top = table.top() + 2;
    let rows = table.bottom().saturating_sub(body_top) as usize;
    // Keep the selection on screen without a separate scroll offset: the table is short
    // enough that anchoring the window to the selection is sufficient.
    let first = app.cursors.episodes.selected.saturating_sub(rows.saturating_sub(1));

    for (i, episode) in app.episodes.iter().enumerate().skip(first).take(rows) {
        let y = body_top + (i - first) as u16;
        let selected = i == app.cursors.episodes.selected;

        if selected {
            mark_selected(buf, app, table.left(), y, app.nav.focus() == Focus::Stage);
        }

        let number_style = if selected {
            app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
        } else {
            app.palette.style(Role::TextDim)
        };
        buf.set_string(
            table.left() + cols.number,
            y,
            anistream_core::media::EpisodeNumber::new(episode.number.clone()).padded(),
            number_style,
        );

        let title_room = (cols.runtime - cols.title).saturating_sub(2) as usize;
        buf.set_string(
            table.left() + cols.title,
            y,
            truncate(episode.title.as_deref().unwrap_or("—"), title_room),
            if selected {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else {
                app.palette.style(Role::TextDim)
            },
        );

        // An unknown runtime drops to hairline weight: 26 rows of `--:--` at text weight
        // form a column of dash noise that competes with the titles.
        let runtime_role =
            if episode.duration_secs.is_some() { Role::TextDim } else { Role::Rule };
        buf.set_string(
            table.left() + cols.runtime,
            y,
            episode.runtime(),
            app.palette.style(runtime_role),
        );

        let meter_width = (table.width.saturating_sub(cols.state + 6)).min(10) as usize;
        let meter_role = if episode.completed { Role::State } else { Role::Obi };
        set_meter(
            buf,
            table.left() + cols.state,
            y,
            &app.palette,
            episode.watched,
            meter_width,
            meter_role,
        );

        // `done` and the filler mark share the column after the meter: an episode is rarely both
        // finished and worth flagging, and two columns for one word each would be noise.
        let tail_x = table.left() + cols.state + meter_width as u16 + 2;
        if episode.completed {
            buf.set_string(tail_x, y, "done", app.palette.style(Role::TextDim));
        } else if let Some(kind) = episode.kind {
            // Only skippable filler earns the alert role. `mixed` is labelled but not coloured,
            // because it is information rather than a suggestion — skipping it loses story.
            let role = if episode.skippable { Role::Alert } else { Role::TextDim };
            buf.set_string(tail_x, y, episode_kind_label(kind), app.palette.style(role));
        }
    }
}

/// What the episode table calls a kind of episode.
pub fn episode_kind_label(kind: EpisodeKind) -> &'static str {
    match kind {
        EpisodeKind::Canon => "canon",
        EpisodeKind::Mixed => "mixed",
        EpisodeKind::Filler => "filler",
        EpisodeKind::AnimeCanon => "anime canon",
    }
}

/// Why the episode table is empty, said as what is known.
///
/// The filter hiding everything is not a missing source, and telling someone to check
/// Providers over their own filter would be cruel. Nor is a title that has not aired yet, or a
/// source that matched and listed nothing: "no source is configured" was said over all of them,
/// with a ready plugin a screen away.
fn empty_episodes(app: &App) -> (&'static str, String) {
    use crate::keymap::Action;
    if app.episodes_all_filtered_out() {
        let filter = app.key_for(Action::Filter);
        return (
            "nothing matches the filter",
            format!("{filter} cycles it — showing none now"),
        );
    }
    if !app.providers.iter().any(|p| p.usable) {
        let providers = app.section_key(Section::Providers);
        return (
            "no episodes yet",
            format!("no source is ready yet — press {providers} for the Providers screen"),
        );
    }
    if let Some(detail) = &app.detail
        && detail.latest_aired().is_none()
        && let Some(secs) = detail.next_airing_in.filter(|s| *s > 0)
    {
        let episode = detail.next_episode.unwrap_or(1);
        return (
            "nothing has aired yet",
            format!("ep {episode} in {}", anistream_model::wording::countdown(secs)),
        );
    }
    (
        "no episodes listed",
        format!(
            "the source listed none for this title — {} searches for the right match, {} picks another source",
            app.key_for(Action::FixMapping),
            app.key_for(Action::PickProvider),
        ),
    )
}

fn search_filter_rows(app: &App) -> Vec<(String, String)> {
    use crate::app::{SEARCH_FILTER_ROWS, search_filter_value};
    SEARCH_FILTER_ROWS
        .iter()
        .enumerate()
        .map(|(row, name)| (search_filter_value(&app.search_filter, row), glyph::eyebrow(name)))
        .collect()
}

struct EpisodeColumns {
    number: u16,
    title: u16,
    runtime: u16,
    state: u16,
}

/// Column offsets, so the header and the body cannot drift apart.
fn episode_columns(width: u16) -> EpisodeColumns {
    let state = width.saturating_sub(16).max(20);
    let runtime = state.saturating_sub(8).max(14);
    EpisodeColumns { number: 2, title: 8, runtime, state }
}
/// How the help table breaks into columns, and the columns into pages, at one size.
struct HelpLayout {
    /// Row ranges, one per column, in reading order.
    columns: Vec<std::ops::Range<usize>>,
    /// Columns side by side on a page.
    across: usize,
    /// Rows in the tallest column.
    height: usize,
}

impl HelpLayout {
    fn pages(&self) -> usize {
        self.columns.len().div_ceil(self.across.max(1)).max(1)
    }
}

/// Lay the help table out in the fewest columns that hold it in `room` rows, or, when it does
/// not fit even at full width, in pages of columns `room` tall.
///
/// Pages rather than a scroll: columns that scroll together read column one's top, then column
/// two's top, with the rest of column one out of sight between them. A page keeps the reading
/// order — down the first column, then the next — and every binding is on some page, where a
/// clipped table hid half of them at 80×24 and all but GLOBAL at 60×20.
fn help_layout(rows: &[(String, String)], width: u16, room: usize) -> HelpLayout {
    let is_heading = |i: usize| rows[i].0.is_empty() && !rows[i].1.starts_with(' ');
    let max_across = layout::columns_for(width, HELP_COLUMN_WIDTH);
    for across in 1..=max_across {
        for height in rows.len().div_ceil(across).max(1)..=room {
            let columns = flow(rows.len(), height, is_heading);
            if columns.len() <= across {
                let height = columns.iter().map(ExactSizeIterator::len).max().unwrap_or(1);
                return HelpLayout { columns, across, height };
            }
        }
    }
    HelpLayout { columns: flow(rows.len(), room, is_heading), across: max_across, height: room }
}

/// Break `len` rows into columns of at most `height`, never ending a column on a heading: the
/// rows under it would start the next column and leave it orphaned at the bottom of this one.
fn flow(
    len: usize,
    height: usize,
    is_heading: impl Fn(usize) -> bool,
) -> Vec<std::ops::Range<usize>> {
    let height = height.max(1);
    let mut columns = Vec::new();
    let mut start = 0;
    while start < len {
        let mut end = (start + height).min(len);
        if end < len && end - start > 1 && is_heading(end - 1) {
            end -= 1;
        }
        columns.push(start..end);
        start = end;
    }
    columns
}

/// Narrowest column the help table reads well in.
///
/// Sized to the longest label ("Mark all previous watched", 24 cells) plus its key and the
/// gap between them. Wider would be prettier and would clip the table on a 30-row terminal,
/// which costs the user a whole category of bindings — so this is set by what has to fit.
const HELP_COLUMN_WIDTH: u16 = 38;

/// Rows a table-shaped screen needs before it can draw anything.
///
/// Eyebrow, hairline, a gap, and one row. Below this the screens that draw tables were writing
/// past the end of the buffer rather than degrading — a panic, not a cosmetic problem, and a
/// resize passes through every size on the way down.
const MIN_TABLE_HEIGHT: u16 = 4;

/// Modal overlays.
///
/// Consistent with the borderless rule: a ground shift, an obi bar and two hairlines. A
/// bordered dialog would break the one rule the whole look depends on.
/// Candidates for a title the ladder could not resolve, best first.
///
/// The similarity is shown rather than hidden: it is the only thing that distinguishes a genuine
/// coin-flip between two near-identical scores from a list where nothing really matched and the
/// answer is probably "none of these". A rejection reason, where the ladder recorded one, says
/// why a plausible-looking row was passed over.
fn candidate_rows(app: &App) -> Vec<(String, String)> {
    app.match_candidates
        .iter()
        .map(|candidate| {
            let similarity = format!("{:>3.0}%", candidate.similarity * 100.0);
            let title = match candidate.rejected {
                Some(reason) => format!("{}  ·  {reason}", candidate.title),
                None => candidate.title.clone(),
            };
            (similarity, title)
        })
        .collect()
}

/// The selectable releases for an episode: seeders in the key column — the number that
/// decides whether a torrent will actually stream — with size and audio flags on the row.
fn source_rows(app: &App) -> Vec<(String, String)> {
    app.sources
        .iter()
        .map(|source| {
            // A healthy swarm stays quiet — this design decorates problems, not health.
            // Under ~20 seeders streaming gets chancy; under 5 it probably will not start.
            let seeders = match source.seeders {
                Some(n) if n < 5 => format!("{} {n:>3}", glyph::STATE_DOWN),
                Some(n) if n <= 20 => format!("{} {n:>3}", glyph::STATE_DEGRADED),
                Some(n) => format!("{n:>5}"),
                None => String::new(),
            };
            let mut label = source.title.clone();
            if let Some(size) = &source.size {
                label.push_str(&format!("  ·  {size}"));
            }
            if source.dual_audio {
                label.push_str("  ·  dual");
            } else if source.dubbed {
                label.push_str("  ·  dub");
            }
            if source.auto_pick {
                label.push_str("  ·  current pick");
            }
            (seeders, label)
        })
        .collect()
}

/// The overlay on top, if there is one, and how it was laid out: see [`FrameReport`].
fn render_overlay(buf: &mut Buffer, app: &App, area: Rect, geometry: &Frame) -> FrameReport {
    let mut report = FrameReport::default();
    let Some(overlay) = app.nav.overlay() else {
        return report;
    };

    // Full width, but only ever as tall as the stage. The header and status line carry state
    // the user still needs while a modal is open — provider health, the VPN badge, the sync
    // depth — so a long help table must clip rather than swallow them.
    let host = Rect {
        x: area.x,
        width: area.width,
        y: geometry.stage.y,
        height: geometry.stage.height,
    };

    let rows = match overlay {
        Overlay::Help => help_rows(app),
        Overlay::CommandPalette => palette_rows(app),
        Overlay::Accounts => accounts_rows(app),
        Overlay::Conflicts => conflict_rows(app),
        Overlay::ListStatus => status_rows(app),
        Overlay::Rate => rate_rows(),
        Overlay::SearchFilters => search_filter_rows(app),
        Overlay::Logs => log_rows(app),
        Overlay::Disambiguate => candidate_rows(app),
        Overlay::Sources => source_rows(app),
        Overlay::SourceProvider => source_provider_rows(app),
        Overlay::WatchOrder => watch_order_rows(app),
        Overlay::Recommendations => recommendation_rows(app),
        Overlay::ManualQuery => vec![(
            String::new(),
            "type what to search for — an empty enter resets to the automatic match".into(),
        )],
        Overlay::KitsuUsername => {
            vec![(String::new(), "email or username — enter continues, esc cancels".into())]
        }
        Overlay::KitsuPassword => vec![(
            String::new(),
            "exchanged for a token once and never stored — enter signs in".into(),
        )],
        Overlay::DownloadRange => vec![(
            String::new(),
            "4 for one episode, 1-12 for a run, 7- for everything from there".into(),
        )],
        Overlay::EditSetting => vec![(
            String::new(),
            match app.edit_target {
                Some(id) => format!("{} — enter saves, esc leaves it be", id.label()),
                None => "enter saves, esc leaves it be".into(),
            },
        )],
        Overlay::JoinStore => vec![(
            String::new(),
            "paste the invite link the store's owner sent you — enter checks it, esc cancels"
                .into(),
        )],
        Overlay::InviteName => vec![(
            String::new(),
            "their name, as the members list will show it — enter makes the link".into(),
        )],
        Overlay::StoreConfirm => store_confirm_rows(app),
        Overlay::PluginAction => plugin_action_rows(app).0,
        Overlay::InviteLink => invite_link_rows(app, link_width(area.width)),
        Overlay::PublishPick => publish_rows(app),
    };

    // Overlays that are a list of things you pick from need a focus marker. The palette was
    // excluded here on the theory that it is a preview rather than a picker — but it *is* a
    // picker, so it showed matches with no indication of which one Enter would run.
    let selected = match overlay {
        // The plugin overlay explains before it offers, so its choices start below the facts.
        Overlay::PluginAction => Some(plugin_action_rows(app).1 + app.cursors.overlay.selected),
        // "Nothing logged yet" is a note, not a line that can be picked.
        Overlay::Logs if app.logs.is_empty() => None,
        _ => matches!(
            overlay,
            Overlay::Accounts
                | Overlay::Conflicts
                | Overlay::ListStatus
                | Overlay::Rate
                | Overlay::SearchFilters
                | Overlay::CommandPalette
                | Overlay::Logs
                | Overlay::Disambiguate
                | Overlay::Sources
                | Overlay::SourceProvider
                | Overlay::WatchOrder
                | Overlay::Recommendations
                | Overlay::PublishPick
        )
        .then_some(app.cursors.overlay.selected),
    };
    // The invite link is the thing to copy, so it is drawn at full weight rather than dimmed
    // like the notes under it.
    let emphasised = match overlay {
        Overlay::InviteLink => invite_link_chunks(app, link_width(area.width)).len(),
        _ => 0,
    };

    // Only the help overlay is a reference table worth columnising. A picker reads as a
    // single ranked list; splitting it would scatter the best match away from the top.
    let room = host.height.saturating_sub(4).max(1) as usize;
    let (columns, column_width, per_column, position) = if matches!(overlay, Overlay::Help) {
        let available = area.width.saturating_sub(4);
        let help = help_layout(&rows, available, room);
        let pages = help.pages();
        report.help_pages = Some(pages);
        let page = app.help_page.min(pages - 1);
        let shown: Vec<std::ops::Range<usize>> =
            help.columns.iter().skip(page * help.across).take(help.across).cloned().collect();
        // Each column only as wide as its longest row needs. Stretched to share a wide band,
        // the keys sat a hundred cells from the labels they belong to.
        let natural = rows
            .iter()
            .map(|(key, label)| label.chars().count() + key.chars().count() + 3)
            .max()
            .unwrap_or(0) as u16;
        let width = (available / help.across.max(1) as u16).min(natural + 2);
        let position = (pages > 1).then(|| format!("page {} of {pages}", page + 1));
        (shown, width, help.height, position)
    } else {
        // One column, windowed around the selection so a long list scrolls with its highlight
        // rather than letting it walk off the bottom of the band.
        let visible = rows.len().min(room);
        let offset = match selected {
            Some(at) => app
                .overlay_offset()
                .min(at)
                .max((at + 1).saturating_sub(visible))
                .min(rows.len() - visible),
            None => 0,
        };
        report.overlay_offset = Some(offset);
        let position = (rows.len() > visible)
            .then(|| format!("{}–{} of {}", offset + 1, offset + visible, rows.len()));
        let shown = std::iter::once(offset..offset + visible).collect();
        (shown, area.width.saturating_sub(4), visible.max(1), position)
    };

    let height = (per_column as u16 + 4).min(host.height);
    let band = layout::overlay_band(host, height);

    // Dim everything behind the band. Without a scrim a full-width band cuts through the
    // rail and reads as a rendering fault; with one it reads as what it is — a layer on
    // top — and the interruption becomes deliberate.
    let dim = app.palette.style(Role::Rule);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_style(dim);
        }
    }

    // Then clear the band itself so content underneath does not show through it.
    let ground = app.palette.ground().unwrap_or(app.palette.ground_ref());
    for y in band.top()..band.bottom() {
        for x in band.left()..band.right() {
            buf[(x, y)].set_char(' ').set_bg(ground.to_ratatui());
        }
    }

    Hairline::new(&app.palette).render(Rect { height: 1, ..band }, buf);
    // The band is clamped to the stage, which on a very short terminal is one or two rows —
    // so the title row and the rule under it can both fall outside the buffer. An overlay is
    // the one thing reachable from every screen, `?` included, so an unguarded write here
    // crashed the app at any height a pane can be dragged to.
    let title_y = band.top() + 1;
    if title_y >= band.bottom() {
        return report;
    }
    buf[(band.left(), title_y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
    let heading = match overlay {
        Overlay::CommandPalette => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::Palette)
            )
        }
        Overlay::ManualQuery => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::ManualQuery)
            )
        }
        Overlay::DownloadRange => {
            format!("{}   {}▏", glyph::eyebrow(overlay.title()), field_text(app, Field::Range))
        }
        Overlay::EditSetting => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::Setting)
            )
        }
        Overlay::KitsuUsername => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::KitsuUser)
            )
        }
        // A link is longer than the band, and the end is where the cursor is, so the end is
        // what shows.
        Overlay::JoinStore => {
            let title = glyph::eyebrow(overlay.title());
            let room = (band.width as usize).saturating_sub(title.chars().count() + 8);
            format!("{title}   {}▏", tail(&field_text(app, Field::JoinLink), room))
        }
        Overlay::InviteName => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::InviteName)
            )
        }
        Overlay::StoreConfirm => store_confirm_heading(app),
        Overlay::PluginAction => match app.plugin_in_action() {
            Some((store, plugin)) => format!(
                "{}   {}  ·  from {}",
                glyph::eyebrow(&plugin.name),
                plugin.version,
                store.name
            ),
            None => glyph::eyebrow(overlay.title()),
        },
        Overlay::InviteLink => match app.invite_link.as_ref().and_then(|l| l.for_whom.as_ref())
        {
            Some(name) => format!("{}   for {name}", glyph::eyebrow(overlay.title())),
            None => glyph::eyebrow("link another device"),
        },
        Overlay::PublishPick => match app.publish_for.as_deref().and_then(|id| app.store(id)) {
            Some(store) => format!("{}   to {}", glyph::eyebrow(overlay.title()), store.name),
            None => glyph::eyebrow(overlay.title()),
        },
        // Masked, one dot per character: length feedback without the letters. This screen
        // can be shared, streamed, or screenshotted — a password never renders in it.
        Overlay::KitsuPassword => format!(
            "{}   {}▏",
            glyph::eyebrow(overlay.title()),
            field_text(app, Field::KitsuPassword)
        ),
        // Say what is being asked, or a bare "WHICH ONE" over a list of near-identical release
        // names is a riddle rather than a question. Not why: the same list answers a search
        // typed after `m`, where "no confident match" sat above a 100% candidate.
        Overlay::Disambiguate => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.detail.as_ref() {
                Some(detail) => format!("is {} — enter pins it", detail.title),
                None => "enter pins it".into(),
            }
        ),
        // Name the title the order is anchored on — a list of sequels floats without it.
        Overlay::WatchOrder => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.target() {
                Some(detail) => format!("around {} — enter opens", detail.title),
                None => "enter opens".into(),
            }
        ),
        // Anchor on the title the recommendations came from.
        Overlay::Recommendations => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.target() {
                Some(detail) => format!("if you liked {} — enter opens", detail.title),
                None => "enter opens".into(),
            }
        ),
        // Name the title, since the choice is remembered against it rather than globally.
        Overlay::SourceProvider => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.detail.as_ref() {
                Some(detail) => format!("{} — enter pins it", detail.title),
                None => "enter pins it".into(),
            }
        ),
        // The grammar is not self-evident for a value editor, so the heading states it.
        Overlay::SearchFilters => format!(
            "{}   ←/→ pick · enter searches · esc keeps them",
            glyph::eyebrow(overlay.title())
        ),
        // Name the episode the slate is for, or a wall of release names has no anchor.
        Overlay::Sources => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.source_context.as_ref() {
                Some((_, episode)) => format!("ep {episode} — enter plays your pick"),
                None => "enter plays your pick".into(),
            }
        ),
        other => glyph::eyebrow(other.title()),
    };
    // Where in a table too long for the band the window is, so the rest is known to exist.
    let position = position.filter(|p| p.chars().count() + 8 < band.width as usize);
    let position_room = position.as_ref().map_or(0, |p| p.chars().count() as u16 + 2);
    line(
        buf,
        band.left() + 2,
        title_y,
        truncate(&heading, band.width.saturating_sub(3 + position_room) as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    if let Some(position) = &position {
        let x = band.right().saturating_sub(position.chars().count() as u16 + 2);
        buf.set_string(x, title_y, position, app.palette.style(Role::TextDim));
    }
    if title_y + 1 < band.bottom() {
        Hairline::new(&app.palette).render(Rect { y: title_y + 1, height: 1, ..band }, buf);
    }

    let placed = columns.iter().enumerate().flat_map(|(column, range)| {
        range.clone().enumerate().map(move |(row, i)| (column, row, i))
    });
    for (column, row, i) in placed {
        let Some((key, label)) = rows.get(i) else { continue };
        let y = title_y + 2 + row as u16;
        if y >= band.bottom() {
            continue;
        }
        let x = band.left() + 2 + column as u16 * column_width;
        let key_width = key.chars().count() as u16;
        let label_room = column_width.saturating_sub(key_width + 3) as usize;

        // The obi again, in the one place a modal has a focused row.
        let is_selected = selected == Some(i);
        if is_selected {
            buf[(band.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
        }
        // Help's scope headings are the rows with no key and no indent; at dim weight
        // they sat flush among the bindings and vanished. Full text weight sets the
        // group apart without spending bold, which stays reserved for the selection.
        let is_scope_heading =
            matches!(overlay, Overlay::Help) && key.is_empty() && !label.starts_with(' ');
        buf.set_string(
            x,
            y,
            truncate(label, label_room),
            if is_selected || i < emphasised {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else if is_scope_heading {
                app.palette.style(Role::Text)
            } else {
                app.palette.style(Role::TextDim)
            },
        );
        if !key.is_empty() {
            let key_x = x + column_width.saturating_sub(key_width + 2);
            if key_x + key_width <= band.right() {
                buf.set_string(key_x, y, key, app.palette.style(Role::Text));
            }
        }
    }
    report
}

/// What a text field shows: what was typed, or a dot for each character of a masked one.
fn field_text(app: &App, field: Field) -> String {
    let line = app.field(field);
    if field.masked() { "•".repeat(line.len()) } else { line.as_str().to_owned() }
}

/// The last `room` characters of `text`, with an ellipsis when that is not all of it.
fn tail(text: &str, room: usize) -> String {
    let len = text.chars().count();
    if len <= room || room < 2 {
        return text.to_owned();
    }
    let kept: String = text.chars().skip(len - (room - 1)).collect();
    format!("…{kept}")
}

fn help_rows(app: &App) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    for (scope, entries) in app.keymap.help() {
        rows.push((String::new(), glyph::eyebrow(scope.heading())));
        for (key, label) in entries {
            rows.push((key, format!("  {label}")));
        }
    }
    rows
}

fn palette_rows(app: &App) -> Vec<(String, String)> {
    // Through `palette_matches` so what is drawn and what Enter runs come from one list. Two
    // independent `take(12)` calls is exactly how a palette ends up launching the wrong entry.
    app.palette_matches()
        .into_iter()
        .map(|(action, key)| (key, action.label().to_string()))
        .collect()
}

/// Recent errors and notices, newest first.
///
/// Newest first rather than chronological: the band clips at whatever height the stage has, and
/// a log viewer that clips the *newest* lines is answering a question nobody asked.
fn log_rows(app: &App) -> Vec<(String, String)> {
    if app.logs.is_empty() {
        return vec![(String::new(), "nothing logged yet".into())];
    }
    app.logs
        .iter()
        .rev()
        .map(|row| {
            // The marker carries severity, since this table's styling is driven by selection.
            let marker = match row.kind {
                anistream_model::notice::NoticeKind::Alert => glyph::STATE_DOWN,
                anistream_model::notice::NoticeKind::Info => glyph::STATE_UNKNOWN,
            };
            // A repeat shows as a count where a key would go, not as more lines.
            let count = if row.count > 1 { format!("×{}", row.count) } else { String::new() };
            (count, format!("{marker}  {}", row.text))
        })
        .collect()
}

/// Draw artwork into a reserved area, falling back to the plate.
///
/// The fallback is the point: a cover that has not arrived, failed to decode, or was never
/// offered must still leave the layout intact. The plate is reserved space, not a gap.
fn draw_artwork(buf: &mut Buffer, app: &App, url: Option<&str>, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if let Some(url) = url
        && app.images.render_into(url, area, buf)
    {
        return;
    }
    let row = plate_row(area.width as usize);
    for offset in 0..area.height {
        line(buf, area.left(), area.top() + offset, &row, app.palette.style(Role::Rule));
    }
}

/// Cells of breathing room between the episode table and the still beside it.
const STILL_GAP: u16 = 2;

/// Where the selected episode's still goes, or `None` when it would not earn its space.
///
/// Three conditions, all of them about not lying to the reader: something in this list has a
/// still at all, the overlay is wide enough that the table does not get squeezed to fit it, and
/// it is tall enough for the image to be an image rather than a stripe.
fn still_panel(app: &App, table: Rect) -> Option<Rect> {
    // A synopsis earns the panel the same way a still does — either is a reason; a list
    // with neither keeps the whole width for the table.
    if !app.episodes.iter().any(|row| row.thumbnail.is_some() || row.description.is_some()) {
        return None;
    }
    let height = 7.min(table.height.saturating_sub(2));
    if height < 4 {
        return None;
    }
    let width = still_plate_width(height);
    // The table is the point of this screen; the still is an accompaniment.
    if table.width < width + STILL_GAP + MIN_EPISODE_TABLE_WIDTH {
        return None;
    }
    Some(Rect { x: table.right() - width, y: table.top() + 2, width, height })
}

/// Columns the episode table needs before a still may take any width from it.
///
/// Enough for a title column of about thirty cells beside the number, runtime and meter. At
/// 44 the still kept its width at 80 columns and left the titles ten cells — a column of
/// `The Heroe…` with a picture beside it. The panel yields instead.
const MIN_EPISODE_TABLE_WIDTH: u16 = 64;

/// Width in cells for a 16:9 still of a given height.
///
/// Stills are widescreen where covers are portrait, so this is the same cell-aspect correction
/// as [`cover_plate_width`] applied to a different ratio — without it the reserved space is the
/// wrong shape and the real image will not fill it.
fn still_plate_width(height: u16) -> u16 {
    ((f32::from(height) * (16.0 / 9.0) * 2.0).round() as u16).max(6)
}

/// Number of content rows the stage can show, for paging.
pub fn visible_rows(area: Rect, section: Section) -> usize {
    let geometry = layout::compute(area, crate::nav::RailWidth::Expanded);
    let offset = match section {
        // The prompt and the gap under it.
        Section::Search => 2,
        // The season's heading, held at the top of the list.
        Section::Seasonal => 1,
        // Day rulings and their spacer rows share the window with the entries. Five is
        // an estimate — a screenful rarely spans more than three days — and estimating
        // low only scrolls the selection a couple of lines early, never hides it.
        Section::Calendar => 5,
        // The segment strip, its hairline and a gap sit above the list.
        Section::Library => 3,
        _ => 0,
    };
    geometry.stage.height.saturating_sub(offset) as usize
}

/// Render into a fresh off-screen buffer.
///
/// Used by the layout tests and by the screen goldens in `tests/goldens.rs`, which are how a
/// composition gets looked at rather than imagined. Not gated to `cfg(test)`, because those are
/// integration tests and see the crate only as a library.
///
/// Drawn as the event loop draws a frame: what it reports goes back to the app, so the next key
/// pages and scrolls the overlay as the frame laid it out.
pub fn render_to_buffer(app: &mut App, width: u16, height: u16) -> Buffer {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    let mut report = FrameReport::default();
    terminal.draw(|f| report = render(f, app)).expect("draw");
    app.took_frame(report);
    terminal.backend().buffer().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Field;
    use crate::screens::fixtures::{
        app_with, display_width, entry, episode, provider, text_of,
    };
    use crate::screens::lists::cover_plate_width;
    use anistream_core::ids::AnilistId;
    use anistream_model::{
        catalog::{Content, Entry},
        event::Event,
    };

    #[test]
    fn the_episode_table_width_is_never_stolen_by_the_rail() {
        // Chrome yields to content: at 80 columns the rail must be a strip, not a menu.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let geometry = layout::compute(Rect::new(0, 0, 80, 24), app.nav.rail_width());
        assert!(geometry.rail.width <= 3);
        assert!(geometry.stage.width >= 70);
    }

    /// A frame says how it laid the overlay out rather than writing it into the app: the help
    /// table's pages where it drew Help, the list's window where it drew a list overlay, and
    /// nothing where it drew neither, for the app to keep what an earlier frame said.
    #[test]
    fn a_frame_reports_how_it_laid_the_overlay_out() {
        use ratatui::{Terminal, backend::TestBackend};
        let draw = |app: &App| {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
            let mut report = FrameReport::default();
            terminal.draw(|f| report = render(f, app)).expect("draw");
            report
        };
        let mut app = app_with(Content::Empty);
        assert_eq!(draw(&app), FrameReport::default());
        app.nav.open_overlay(Overlay::Help);
        let help = draw(&app);
        assert!(help.help_pages.is_some_and(|pages| pages > 1), "{help:?}");
        assert_eq!(help.overlay_offset, None);
        app.nav.close_overlay();
        app.nav.open_overlay(Overlay::Logs);
        assert_eq!(draw(&app), FrameReport { overlay_offset: Some(0), help_pages: None });
    }

    #[test]
    fn the_help_overlay_lists_real_bindings() {
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        assert!(text.contains("KEYS"));
        assert!(text.contains("Episodes"), "help should list actions");
        assert!(text.contains("GLOBAL"), "help should be grouped by scope");
    }

    #[test]
    fn overlays_use_hairlines_and_an_obi_rather_than_a_border() {
        // A bordered dialog would break the one rule the whole look depends on.
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        for corner in ['┌', '┐', '└', '┘', '├', '┤'] {
            assert!(!text.contains(corner), "overlay drew a box corner {corner:?}");
        }
    }

    #[test]
    fn the_command_palette_shows_the_query_and_filters() {
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::CommandPalette);
        app.set_text(Field::Palette, "episode".into());
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        assert!(text.contains("RUN"));
        assert!(text.contains("episode"), "query should be visible");
        assert!(text.contains("Episodes"));
    }

    #[test]
    fn an_overlay_spans_the_full_width_so_nothing_bleeds_alongside_it() {
        // A partial-width panel leaves live content visible either side of it, which reads
        // as a rendering fault rather than a modal.
        let mut app = app_with(Content::Entries(
            (1..30).map(|i| entry(i, "A Distinctive Background Title")).collect(),
        ));
        app.nav.open_overlay(Overlay::Help);
        let buf = render_to_buffer(&mut app, 120, 30);

        // Find a row inside the band and confirm no background content survives on it.
        let text = text_of(&buf);
        let band_row =
            text.lines().position(|l| l.contains("Show keys")).expect("help content missing");
        assert!(
            !text.lines().nth(band_row).unwrap().contains("Distinctive"),
            "background content bled through the overlay"
        );
    }

    #[test]
    fn the_episode_table_reads_as_a_timing_sheet() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.handle(crate::keymap::Action::Open, 20);
        // As entering episodes does: the table is a view of the open title.
        app.detail = app.selected_entry().cloned();
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(app.episodes_here(vec![
            episode("9", "Aura the Guillotine", 1.0, true),
            episode("10", "Frieren the Slayer", 0.62, false),
            episode("11", "A Real Hero", 0.0, false),
        ]));

        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        // Zero-padded numerals in a fixed field: episodes are a real sequence, so the
        // numbering carries information rather than decorating.
        assert!(text.contains("009"), "numerals should be padded");
        assert!(text.contains("010"));
        assert!(text.contains("Aura the Guillotine"));
        assert!(text.contains("24:00"), "runtime as mm:ss");
        assert!(text.contains("done"), "completed episodes are marked");
        assert!(text.contains("TITLE"), "caps column headers");
        // A naive string sort would put 10 before 9; the caller supplies order, but the
        // padding must not reorder anything.
        let nine = text.find("009").unwrap();
        let ten = text.find("010").unwrap();
        assert!(nine < ten);
    }

    #[test]
    fn an_episode_table_with_no_source_explains_itself() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        assert!(text.contains("NO EPISODES YET"));
        assert!(text.contains("Providers screen"), "must point somewhere actionable");
    }

    #[test]
    fn the_episode_table_gets_the_full_stage_width() {
        // Chrome yields to content: this is the table the collapse rule exists for.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let geometry = layout::compute(Rect::new(0, 0, 110, 20), app.nav.rail_width());
        assert!(geometry.rail.width <= 3);
        assert!(geometry.stage.width >= 100);
    }

    /// An app sitting on the episodes overlay, the way the table tests set it up.
    fn app_at_episodes() -> App {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.handle(crate::keymap::Action::Open, 20);
        // As entering episodes does: the table is a view of the open title.
        app.detail = app.selected_entry().cloned();
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(app.episodes_here(vec![
            episode("9", "Aura the Guillotine", 1.0, true),
            episode("10", "Frieren the Slayer", 0.62, false),
            episode("11", "A Real Hero", 0.0, false),
        ]));
        app
    }

    #[test]
    fn the_match_picker_shows_how_close_each_candidate_was() {
        // The score is the whole point: two rows at 62% and 61% is a real coin-flip, while
        // 88% against 30% means something else went wrong and the list is worth distrusting.
        let mut app = app_at_episodes();
        app.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: vec![
                anistream_model::sources::MatchCandidate {
                    title: "Frieren S1".into(),
                    key: anistream_core::ids::ProviderKey::new("a"),
                    similarity: 0.62,
                    rejected: None,
                },
                anistream_model::sources::MatchCandidate {
                    title: "Frieren S2".into(),
                    key: anistream_core::ids::ProviderKey::new("b"),
                    similarity: 0.61,
                    rejected: Some("episode count"),
                },
            ],
        });

        let text = text_of(&render_to_buffer(&mut app, 110, 24));
        assert!(text.contains("WHICH ONE"), "the overlay names what it is asking");
        assert!(text.contains("62%") && text.contains("61%"), "similarity has to be visible");
        assert!(text.contains("Frieren S1") && text.contains("Frieren S2"));
        // A rejection reason explains why a plausible row was passed over.
        assert!(text.contains("episode count"));
    }

    #[test]
    fn the_episode_still_only_appears_when_there_is_one_to_show() {
        // Coverage is uneven, and a permanent empty plate would be a gap pretending to be a
        // feature. The table takes the whole width instead.
        let table = Rect::new(0, 0, 100, 20);

        let mut app = app_at_episodes();
        assert!(app.episodes.iter().all(|e| e.thumbnail.is_none()));
        assert!(still_panel(&app, table).is_none(), "nothing to show, nothing reserved");

        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());
        let panel = still_panel(&app, table).expect("a still was published");
        assert!(panel.width >= 6 && panel.height >= 4);
        assert!(panel.right() <= table.right(), "the panel stays inside the overlay");
    }

    #[test]
    fn a_narrow_or_short_overlay_keeps_the_table_and_drops_the_still() {
        // The table is the point of this screen; the still is an accompaniment.
        let mut app = app_at_episodes();
        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());

        assert!(still_panel(&app, Rect::new(0, 0, 60, 20)).is_none(), "too narrow");
        assert!(still_panel(&app, Rect::new(0, 0, 100, 5)).is_none(), "too short");
        assert!(still_panel(&app, Rect::new(0, 0, 100, 20)).is_some(), "room for both");
    }

    #[test]
    fn a_still_is_shaped_for_widescreen_not_for_a_cover() {
        // Cells are about twice as tall as they are wide; without that correction the
        // reserved space is the wrong shape and the image will not fill it.
        let height = 9;
        assert!(
            still_plate_width(height) > cover_plate_width(height),
            "a 16:9 still is wider than a 2:3 cover at the same height"
        );
    }

    #[test]
    fn the_episodes_screen_survives_every_size_with_a_still_present() {
        // The size matrix is where the out-of-bounds writes turn up.
        let mut app = app_at_episodes();
        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());
        for width in [20, 40, 60, 80, 100, 140] {
            for height in [6, 8, 12, 20, 30] {
                let _ = render_to_buffer(&mut app, width, height);
            }
        }
    }

    #[test]
    fn overlays_do_not_overflow_a_narrow_terminal_either() {
        // The help table is the widest thing in the app and the most likely to spill.
        for (width, height) in [(80, 24), (60, 20), (40, 15)] {
            for overlay in
                [Overlay::Help, Overlay::CommandPalette, Overlay::Accounts, Overlay::ListStatus]
            {
                let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
                app.nav.open_overlay(overlay.clone());
                let buf = render_to_buffer(&mut app, width, height);
                for (row, line) in text_of(&buf).lines().enumerate() {
                    assert!(
                        display_width(line) <= width as usize,
                        "{overlay:?} at {width}x{height}: row {row} overflows:\n{line}"
                    );
                }
            }
        }
    }

    #[test]
    fn an_overlay_dims_the_content_behind_it() {
        // Without a scrim a full-width band cuts through the rail and reads as a
        // rendering fault. Dimming makes it read as a layer on top.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        let before = render_to_buffer(&mut app, 120, 30);

        app.nav.open_overlay(Overlay::Help);
        let after = render_to_buffer(&mut app, 120, 30);

        // The header sits above the band and must survive, but dimmed.
        let header_x = 0;
        assert_eq!(before[(header_x, 0)].symbol(), after[(header_x, 0)].symbol());
        assert_ne!(
            before[(header_x, 0)].fg,
            after[(header_x, 0)].fg,
            "content behind the overlay was not dimmed"
        );
        assert_eq!(after[(header_x, 0)].fg, app.palette.color(Role::Rule));
    }

    #[test]
    fn a_picker_overlay_stays_a_single_ranked_list() {
        // Splitting a picker into columns would scatter the best match away from the top.
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::CommandPalette);
        app.set_text(Field::Palette, "episode".into());
        let text = text_of(&render_to_buffer(&mut app, 160, 24));

        let rows: Vec<&str> =
            text.lines().filter(|l| l.contains("episode") || l.contains("Episodes")).collect();
        for row in rows {
            // One entry per line: two would mean it had been columnised.
            assert!(
                row.matches("pisode").count() <= 1,
                "picker was laid out in columns: {row:?}"
            );
        }
    }

    #[test]
    fn a_wide_terminal_lays_the_keymap_out_in_columns() {
        // Forty-odd bindings do not fit one column; clipping would hide whole categories.
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 140, 32));
        assert!(text.contains("Show keys"), "first scope missing");
        assert!(text.contains("Stop playback"), "last scope missing — the list was clipped");
    }

    // ---- Wording and display consistency ----

    fn episodes_open_on(detail: Entry) -> App {
        let mut app = app_with(Content::Entries(vec![detail.clone()]));
        app.nav.push(StageView::Episodes(detail.id));
        app.detail = Some(detail);
        app.apply(app.episodes_here(Vec::new()));
        app
    }

    #[test]
    fn an_empty_episode_table_says_what_is_known() {
        // A source is ready: "no source is configured" was false here.
        let mut app = episodes_open_on(entry(1, "Paper Kite"));
        app.apply(Event::Providers(vec![provider("someplugin", "ready", true, false)]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NO EPISODES LISTED"), "{text}");
        assert!(text.contains("m searches for the right match"), "{text}");
        assert!(!text.contains("no source is configured"), "{text}");

        // Not out yet: the wait, not a fault.
        let unaired = Entry {
            next_episode: Some(1),
            next_airing_in: Some(36 * 3600),
            ..entry(2, "Lantern Season")
        };
        let mut app = episodes_open_on(unaired);
        app.apply(Event::Providers(vec![provider("someplugin", "ready", true, false)]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NOTHING HAS AIRED YET"), "{text}");
        assert!(text.contains("ep 1 in 1d 12h"), "{text}");
    }

    #[test]
    fn the_match_picker_heading_does_not_claim_the_match_was_unsure() {
        let mut app = app_at_episodes();
        app.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "someplugin".into(),
            candidates: vec![anistream_model::sources::MatchCandidate {
                title: "Exactly This".into(),
                key: anistream_core::ids::ProviderKey::new("a"),
                similarity: 1.0,
                rejected: None,
            }],
        });
        let text = text_of(&render_to_buffer(&mut app, 110, 24));
        assert!(text.contains("100%"));
        assert!(!text.contains("no confident match"), "{text}");
        assert!(text.contains("enter pins it"), "{text}");
    }

    #[test]
    fn at_eighty_columns_the_episode_titles_keep_their_room() {
        let mut app = app_at_episodes();
        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());
        app.episodes[1].title = Some("A Title That Needs Most of Thirty Cells".into());
        let text = text_of(&render_to_buffer(&mut app, 80, 24));
        assert!(text.contains("A Title That Needs Most of Thi"), "{text}");
    }

    #[test]
    fn list_statuses_read_as_lowercase_values_in_the_picker() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Kite")]));
        app.nav.open_overlay(Overlay::ListStatus);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("watching"), "{text}");
        assert!(!text.contains("WATCHING"), "{text}");
    }
}
