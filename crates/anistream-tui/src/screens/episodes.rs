//! The episode table, laid out as a timing sheet, with the selected episode's still beside it
//! when it earns the room, and why the table is empty when it is; and the rows of the overlays
//! that pick a title's match and an episode's release.

use anistream_i18n::t;
use anistream_model::episodes::EpisodeKind;
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, countdown_text, push_detail},
    nav::{Focus, Section},
    theme::{Role, glyph},
    widgets::{Hairline, truncate, wrap},
};

use super::{chrome::mark_selected, draw_artwork, line, lists::render_loading, set_meter};

/// Columns between the title and the dub line beside it.
const DUB_GAP: usize = 3;
/// The least of the title the dub line is shown beside: narrower, the title has the row.
const MIN_TITLE: usize = 16;

/// The timing-sheet episode table.
///
/// Modelled on an animation production timing sheet: a fixed-width numeral column, hairline
/// headers, and dense rows. Episodes genuinely are a sequence, so the numbering carries
/// information rather than decorating — which is what earns the zero-padded field.
pub(super) fn render_episodes(buf: &mut Buffer, app: &App, area: Rect) {
    let heading = t::episodes_heading();
    let title = app.detail.as_ref().map_or(heading.as_str(), |e| e.title.as_str());
    // How far the dub goes sits at the right of the title's row, when the row has room for both:
    // it is the source's, as the table is.
    let room = area.width.saturating_sub(4) as usize;
    let dub = app
        .detail
        .as_ref()
        .and_then(|e| app.dub_line(e.id))
        .filter(|dub| room >= dub.chars().count() + DUB_GAP + MIN_TITLE);
    let title_room = dub.as_ref().map_or(room, |dub| room - dub.chars().count() - DUB_GAP);
    buf.set_string(
        area.left(),
        area.top(),
        format!("{}  {}", glyph::BACK, truncate(title, title_room)),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    if let Some(dub) = dub {
        let width = dub.chars().count() as u16;
        buf.set_string(
            area.right().saturating_sub(width),
            area.top(),
            dub,
            app.palette.style(Role::TextDim),
        );
    }

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
        line(buf, table.left(), table.top(), headline, app.palette.style(Role::TextDim));
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
        t::episodes_column_title(),
        app.palette.style(Role::TextDim),
    );
    buf.set_string(
        table.left() + cols.runtime,
        table.top(),
        t::episodes_column_run(),
        app.palette.style(Role::TextDim),
    );
    buf.set_string(
        table.left() + cols.state,
        table.top(),
        t::episodes_column_watched(),
        app.palette.style(Role::TextDim),
    );
    // The active filter sits at the right edge of the header row — always visible while
    // it is narrowing the table, invisible when it is not.
    if app.episode_filter != crate::app::EpisodeFilter::All {
        let marker = app.episode_filter.label();
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

        // The next episode, from AniList's schedule, before it is out: when it airs, in place
        // of a meter there is nothing to fill yet.
        let now = app.clock.now_epoch();
        if let Some(at) = episode.airs_at.filter(|at| *at > now) {
            let airs = t::episodes_airs_in(&countdown_text(at - now));
            buf.set_string(
                table.left() + cols.state,
                y,
                airs,
                app.palette.style(Role::TextDim),
            );
            continue;
        }

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
            buf.set_string(tail_x, y, t::episodes_done(), app.palette.style(Role::TextDim));
        } else if let Some(kind) = episode.kind {
            // Only skippable filler earns the alert role. `mixed` is labelled but not coloured,
            // because it is information rather than a suggestion — skipping it loses story.
            let role = if episode.skippable { Role::Alert } else { Role::TextDim };
            buf.set_string(tail_x, y, episode_kind_label(kind), app.palette.style(role));
        }
    }
}

/// What the episode table calls a kind of episode.
pub fn episode_kind_label(kind: EpisodeKind) -> String {
    match kind {
        EpisodeKind::Canon => t::episodes_kind_canon(),
        EpisodeKind::Mixed => t::episodes_kind_mixed(),
        EpisodeKind::Filler => t::episodes_kind_filler(),
        EpisodeKind::AnimeCanon => t::episodes_kind_anime_canon(),
    }
}

/// Why the episode table is empty, said as what is known.
///
/// The filter hiding everything is not a missing source, and telling someone to check
/// Providers over their own filter would be cruel. Nor is a title that has not aired yet, or a
/// source that matched and listed nothing: "no source is configured" was said over all of them,
/// with a ready plugin a screen away.
fn empty_episodes(app: &App) -> (String, String) {
    use crate::keymap::Action;
    if app.episodes_all_filtered_out() {
        let filter = app.key_for(Action::Filter);
        return (t::episodes_empty_filtered(), t::episodes_empty_filtered_hint(&filter));
    }
    // AniList knows of none either: how many there are, and what has aired, is not known yet.
    if !app.providers.iter().any(|p| p.usable) {
        let providers = app.section_key(Section::Providers);
        return (t::episodes_empty_no_source(), t::episodes_empty_no_source_hint(&providers));
    }
    if let Some(detail) = &app.detail
        && detail.latest_aired().is_none()
        && let Some(secs) = detail.next_airing_in.filter(|s| *s > 0)
    {
        let episode = detail.next_episode.unwrap_or(1);
        return (
            t::episodes_empty_unaired(),
            t::episodes_empty_unaired_hint(i64::from(episode), &countdown_text(secs)),
        );
    }
    (
        t::episodes_empty_none_listed(),
        t::episodes_empty_none_listed_hint(
            &app.key_for(Action::FixMapping),
            &app.key_for(Action::PickProvider),
        ),
    )
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
pub(super) fn candidate_rows(app: &App) -> Vec<(String, String)> {
    app.match_candidates
        .iter()
        .map(|candidate| {
            let similarity = format!("{:>3.0}%", candidate.similarity * 100.0);
            let mut title = candidate.title.clone();
            if let Some(reason) = &candidate.rejected {
                push_detail(&mut title, &crate::app::say(reason));
            }
            (similarity, title)
        })
        .collect()
}

/// The selectable releases for an episode: seeders in the key column — the number that
/// decides whether a torrent will actually stream — with size and audio flags on the row.
pub(super) fn source_rows(app: &App) -> Vec<(String, String)> {
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
                push_detail(&mut label, size);
            }
            if source.dual_audio {
                push_detail(&mut label, &t::episodes_release_dual());
            } else if source.dubbed {
                push_detail(&mut label, &t::episodes_release_dub());
            }
            if source.auto_pick {
                push_detail(&mut label, &t::episodes_release_current_pick());
            }
            (seeders, label)
        })
        .collect()
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
/// as [`cover_plate_width`](super::lists::cover_plate_width) applied to a different ratio —
/// without it the reserved space is the wrong shape and the real image will not fill it.
fn still_plate_width(height: u16) -> u16 {
    ((f32::from(height) * (16.0 / 9.0) * 2.0).round() as u16).max(6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::ids::AnilistId;
    use anistream_model::{
        catalog::{ContentList, Entry},
        event::Event,
    };

    use crate::{
        layout,
        nav::StageView,
        screens::{
            fixtures::{app_with, entry, episode, provider, text_of},
            lists::cover_plate_width,
            render_to_buffer,
        },
    };

    #[test]
    fn the_episode_table_width_is_never_stolen_by_the_rail() {
        // Chrome yields to content: at 80 columns the rail must be a strip, not a menu.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let geometry = layout::compute(Rect::new(0, 0, 80, 24), app.nav.rail_width());
        assert!(geometry.rail.width <= 3);
        assert!(geometry.stage.width >= 70);
    }

    #[test]
    fn the_episode_table_reads_as_a_timing_sheet() {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.handle(crate::keymap::Action::Open, 20);
        // As entering episodes does: the table is a view of the open title.
        app.detail = app.selected_entry().cloned();
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(app.episodes_here(vec![
            episode("9", "The Last Lamplighter", 1.0, true),
            episode("10", "Where the Roads Cross", 0.62, false),
            episode("11", "The Sky Archive", 0.0, false),
        ]));

        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        // Zero-padded numerals in a fixed field: episodes are a real sequence, so the
        // numbering carries information rather than decorating.
        assert!(text.contains("009"), "numerals should be padded");
        assert!(text.contains("010"));
        assert!(text.contains("The Last Lamplighter"));
        assert!(text.contains("24:00"), "runtime as mm:ss");
        assert!(text.contains(&t::episodes_done()), "completed episodes are marked");
        assert!(text.contains(&t::episodes_column_title()), "column headers");
        // A naive string sort would put 10 before 9; the caller supplies order, but the
        // padding must not reorder anything.
        let nine = text.find("009").unwrap();
        let ten = text.find("010").unwrap();
        assert!(nine < ten);
    }

    #[test]
    fn how_far_the_dub_goes_sits_beside_the_title_for_that_title_alone() {
        use anistream_model::episodes::DubExtent;
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.handle(crate::keymap::Action::Open, 20);
        app.detail = app.selected_entry().cloned();
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(app.episodes_here(vec![episode("1", "The Lantern Road", 0.0, false)]));
        let said = |app: &mut App, id: u32, dub: Option<DubExtent>| {
            app.apply(Event::TitleDub { id: AnilistId::new(id), dub });
            text_of(&render_to_buffer(app, 110, 20))
        };

        let partial = Some(DubExtent { through: 6, complete: false });
        assert!(said(&mut app, 1, partial).contains("Dub to episode 6"));
        // Another title's dub is not this one's.
        let other = said(&mut app, 2, Some(DubExtent { through: 3, complete: false }));
        assert!(!other.contains("Dub to episode 3"));
        let whole = Some(DubExtent { through: 12, complete: true });
        assert!(said(&mut app, 1, whole).contains("Dubbed"));
        assert!(!said(&mut app, 1, None).contains("Dub"), "no dub, no line");
    }

    #[test]
    fn an_episode_table_with_no_source_explains_itself() {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        assert!(text.contains(&t::episodes_empty_no_source()));
        assert!(text.contains("Press 7"), "must point somewhere actionable: {text}");
    }

    #[test]
    fn the_episode_table_gets_the_full_stage_width() {
        // Chrome yields to content: this is the table the collapse rule exists for.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let geometry = layout::compute(Rect::new(0, 0, 110, 20), app.nav.rail_width());
        assert!(geometry.rail.width <= 3);
        assert!(geometry.stage.width >= 100);
    }

    /// An app sitting on the episodes overlay, the way the table tests set it up.
    fn app_at_episodes() -> App {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.handle(crate::keymap::Action::Open, 20);
        // As entering episodes does: the table is a view of the open title.
        app.detail = app.selected_entry().cloned();
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(app.episodes_here(vec![
            episode("9", "The Last Lamplighter", 1.0, true),
            episode("10", "Where the Roads Cross", 0.62, false),
            episode("11", "The Sky Archive", 0.0, false),
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
                    title: "Hoshikuzu S1".into(),
                    key: anistream_core::ids::ProviderKey::new("a"),
                    similarity: 0.62,
                    rejected: None,
                    year: None,
                    episodes: None,
                    format: None,
                },
                anistream_model::sources::MatchCandidate {
                    title: "Hoshikuzu S2".into(),
                    key: anistream_core::ids::ProviderKey::new("b"),
                    similarity: 0.61,
                    rejected: Some(anistream_model::Msg::MatchRuledOutEpisodes),
                    year: None,
                    episodes: None,
                    format: None,
                },
            ],
            answers: anistream_model::sources::MatchQuestion::Ladder,
            source: "Torrent".into(),
        });

        let text = text_of(&render_to_buffer(&mut app, 110, 24));
        assert!(text.contains("Which one"), "the overlay names what it is asking");
        assert!(text.contains("62%") && text.contains("61%"), "similarity has to be visible");
        assert!(text.contains("Hoshikuzu S1") && text.contains("Hoshikuzu S2"));
        // A rejection reason explains why a plausible row was passed over.
        assert!(text.contains("More episodes than this title"), "{text}");
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

    fn episodes_open_on(detail: Entry) -> App {
        let mut app = app_with(ContentList::Entries(vec![detail.clone()]));
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
        assert!(text.contains(&t::episodes_empty_none_listed()), "{text}");
        assert!(text.contains("Press m to search for the right match"), "{text}");
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
        assert!(text.contains(&t::episodes_empty_unaired()), "{text}");
        assert!(text.contains("Episode 1 airs in 1 d 12 h"), "{text}");
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
                year: None,
                episodes: None,
                format: None,
            }],
            answers: anistream_model::sources::MatchQuestion::Ladder,
            source: "Some plugin".into(),
        });
        let text = text_of(&render_to_buffer(&mut app, 110, 24));
        assert!(text.contains("100%"));
        assert!(!text.contains("no confident match"), "{text}");
        assert!(text.contains("Enter pins it"), "{text}");
    }

    #[test]
    fn at_eighty_columns_the_episode_titles_keep_their_room() {
        let mut app = app_at_episodes();
        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());
        app.episodes[1].title = Some("A Title That Needs Most of Thirty Cells".into());
        let text = text_of(&render_to_buffer(&mut app, 80, 24));
        assert!(text.contains("A Title That Needs Most of Thi"), "{text}");
    }
}
