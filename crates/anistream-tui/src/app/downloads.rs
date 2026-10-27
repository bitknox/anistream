//! The download queue's screen.

use anistream_i18n::t;
use anistream_model::{
    command::Command,
    downloads::{DownloadRow, DownloadState},
};

use super::{App, alert, info};
use crate::nav::{Focus, Section, StageView};

impl App {
    /// Whether the download queue has focus.
    pub(super) fn in_downloads_stage(&self) -> bool {
        self.nav.section() == Section::Downloads
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Downloads))
    }

    /// The download queue as it stands.
    pub(super) fn apply_downloads(&mut self, rows: Vec<DownloadRow>) {
        self.downloads = rows;
        // The cursor has to stay in range: a completed row being cleared shortens the list
        // under it. The screen's own, so it needs no asking whether that is on screen.
        let last = self.downloads.len().saturating_sub(1);
        self.cursors.downloads.selected = self.cursors.downloads.selected.min(last);
    }

    /// Enter on a download: played from disk once it has finished, started now, ahead of the
    /// queue, while it waits, or why it cannot be.
    pub(super) fn open_download(&mut self) -> Option<Command> {
        let row = self.downloads.get(self.cursors.downloads.selected)?.clone();
        match (&row.path, row.state) {
            (Some(path), DownloadState::Done) => {
                // First-class playback: same staging, history and sync as a stream.
                self.raise_now_playing_titled(row.anilist_id, row.title.clone(), &row.episode);
                return Some(Command::PlayLocal {
                    id: row.anilist_id,
                    episode: row.episode,
                    title: row.title,
                    path: path.clone(),
                });
            }
            (_, DownloadState::Failed) => {
                // The reason is on the row; surfacing it is more use than a dead keypress.
                let reason = row.error.clone().unwrap_or_else(t::downloads_failed_no_reason);
                self.push_toast(alert(reason));
            }
            (_, DownloadState::Queued | DownloadState::Paused) => {
                return Some(Command::DownloadStartNow { id: row.id });
            }
            _ => self.push_toast(info(t::downloads_not_finished())),
        }
        None
    }

    /// The highlighted download deleted, and its file with it.
    pub(super) fn delete_download(&mut self) -> Option<Command> {
        let row = self.downloads.get(self.cursors.downloads.selected)?;
        Some(Command::DownloadDelete { id: row.id })
    }

    /// An episode queued for download: the highlighted one in the table, or the next unwatched
    /// one from a title.
    pub(super) fn download_episode(&mut self) -> Option<Command> {
        // From the episode table it is the highlighted episode; from a title it is the next
        // one you have not watched, which matches what Enter would have played.
        let (id, episode) = match self.nav.current() {
            StageView::Episodes(id) => {
                (*id, self.episodes.get(self.cursors.episodes.selected)?.number.clone())
            }
            _ => {
                let entry = self.target()?;
                let episode = entry.progress.map_or(1, |p| p.next).to_string();
                (entry.id, episode)
            }
        };
        // Listed from AniList alone: no source here has it to download.
        if self.unlisted(id) {
            let sources = self.section_key(Section::Providers);
            self.push_toast(info(t::downloads_needs_source(&sources)));
            return None;
        }
        Some(Command::DownloadEpisode { id, episode })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn space_and_x_pause_and_cancel_the_highlighted_download() {
        // Idle they arrived as play-next and remove-from-Continue, so nothing in the queue
        // could be paused or cancelled; playing, they paused the player instead.
        let download = |id: i64| DownloadRow {
            id,
            anilist_id: AnilistId::new(1),
            title: "Title 0".into(),
            episode: id.to_string(),
            state: DownloadState::Active,
            fraction: 0.5,
            downloaded: 1,
            total: 2,
            error: None,
            path: None,
            ahead: false,
        };
        let mut a = app();
        a.handle(Action::JumpSection(6), 5);
        a.apply(Event::Downloads(vec![download(7), download(8)]));
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::PlayNext, 5), Some(Command::DownloadPause { id: 8 }));
        assert_eq!(
            a.handle(Action::HideFromContinue, 5),
            Some(Command::DownloadCancel { id: 8 })
        );

        a.playing = Some(NowPlaying::default());
        assert_eq!(a.handle(Action::PlayPause, 5), Some(Command::DownloadPause { id: 8 }));
        assert_eq!(a.handle(Action::StopPlayback, 5), Some(Command::DownloadCancel { id: 8 }));
    }

    #[test]
    fn enter_on_a_waiting_download_starts_it_now() {
        let row = |id: i64, state: DownloadState| DownloadRow {
            id,
            anilist_id: AnilistId::new(1),
            title: "Title 0".into(),
            episode: id.to_string(),
            state,
            fraction: 0.0,
            downloaded: 0,
            total: 0,
            error: None,
            path: None,
            ahead: false,
        };
        let mut a = app();
        a.handle(Action::JumpSection(6), 5);
        a.apply(Event::Downloads(vec![
            row(1, DownloadState::Active),
            row(2, DownloadState::Queued),
            row(3, DownloadState::Paused),
        ]));
        a.nav.focus_stage();
        assert_eq!(a.handle(Action::Open, 5), None, "already fetching");
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::DownloadStartNow { id: 2 }));
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::DownloadStartNow { id: 3 }));
    }

    #[test]
    fn x_on_a_finished_download_keeps_it() {
        // Cancelling a finished row forgot it and left its file behind, unreachable from here.
        let mut a = app();
        a.handle(Action::JumpSection(6), 5);
        a.apply(Event::Downloads(vec![DownloadRow {
            id: 3,
            anilist_id: AnilistId::new(1),
            title: "Title 0".into(),
            episode: "1".into(),
            state: DownloadState::Done,
            fraction: 1.0,
            downloaded: 2,
            total: 2,
            error: None,
            path: Some("/tmp/title-0-1.mkv".into()),
            ahead: false,
        }]));
        assert_eq!(a.handle(Action::HideFromContinue, 5), None);
        assert!(
            a.toasts
                .iter()
                .any(|t| t.notice.text == anistream_i18n::t::toast_download_done("X")),
            "{:?}",
            a.toasts
        );
    }
}
