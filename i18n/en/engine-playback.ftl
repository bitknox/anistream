## Playback, downloads, title matching, catalogue and watch party notices from the engine.

# A reason on its own, as a toast or a failed play's explanation. The reason is one of the
# engine's own reasons, so it is translated.
# @arg reason: cause
notice-reason = { $reason }

# Reason: no source is set up, so nothing can be played or downloaded. Shown in a toast.
cause-no-sources = No { -source-pl } are set up. Add one on the { -source-cap-pl } screen.

# A play's explanation: the episode's broadcast is still ahead. The "when" says today, tomorrow or a date.
# @arg episode: int
# @arg when: cause
notice-not-aired = Episode { $episode } airs { $when }.

# A play's explanation: the episode is after the next one to air. The "when" says when that one airs.
# @arg episode: int
# @arg next: int
# @arg when: cause
notice-not-aired-yet = Episode { $episode } hasn't aired yet. Episode { $next } airs { $when }.

# Part of an air time: the broadcast is later today. The time is the viewer's local time.
# @arg time: text
cause-airs-today = today at { $time }

# Part of an air time: the broadcast is tomorrow. The time is the viewer's local time.
# @arg time: text
cause-airs-tomorrow = tomorrow at { $time }

# Part of an air time: the broadcast is within the next week.
# @arg days: int
# @arg time: text
cause-airs-in-days = { $days ->
    [one] in one day at { $time }
   *[other] in { $days } days at { $time }
 }

# Part of an air time: the broadcast is more than a week away. The date is year-month-day.
# @arg date: text
# @arg time: text
cause-airs-on-date = on { $date } at { $time }

# Reason: the source answered but gave nothing to play.
cause-no-playable-stream = The { -source } answered, but with nothing to play. Try another.

# Reason: no source could be matched to the title. The detail is what each source said.
# @arg detail: text
cause-title-no-match = Couldn't match this title: { $detail }

# Reason: the source the person picked is no longer installed or enabled.
# @arg source: text
cause-source-gone = The { -source } { $source } is no longer available. Choose another.

# Reason: the release the person picked would not start. The detail comes from the source.
# @arg detail: text
cause-release-failed = Couldn't start that { -release }: { $detail }

# Reason: every source matched the title but none has the episode. The detail is what each said.
# @arg episode: text
# @arg detail: text
cause-no-source-has-episode = No { -source } has episode { $episode }: { $detail }

# Reason: the source streams episodes but only torrents can be downloaded.
cause-source-cannot-download = This { -source } streams episodes but can't download them.

# Reason: the title was never listed by a source, so it can't be downloaded.
cause-no-source-could-list-show = No { -source } could list this show, so it can't be downloaded.

# Part of a finished download's toast: subtitle files were merged into the video.
# @arg tracks: int
cause-subtitles-merged = { $tracks ->
    [one] One subtitle track was merged in.
   *[other] { $tracks } subtitle tracks were merged in.
 }

# Part of a finished download's toast: merging failed, so the subtitles stay as files beside it.
# @arg reason: text
cause-subtitles-left-apart = The subtitles stay as separate files. { $reason }

# Part of a download toast: how many episodes of the range were in the queue already.
# @arg count: int
cause-already-in-downloads = { $count ->
    [one] One episode is already in downloads.
   *[other] { $count } episodes are already in downloads.
 }

# Job note while a play looks the title up.
status-looking-up-title = Looking up the title…

# Job note while a play waits for the sources to arrive.
status-loading-sources = Loading { -source-pl }…

# Job note while the sources are asked for an episode. The episode is its label, often a number.
# @arg episode: text
status-searching-sources = Searching { -source-pl } for episode { $episode }…

# Status while the player opens.
status-starting-player = Starting mpv…

# Status line while an episode plays.
# @arg title: text
# @arg episode: text
status-playing = Playing { $title }, episode { $episode }

# Toast when the player cannot be started. The reason comes from the system.
# @arg reason: text
notice-player-failed-to-start = Couldn't start mpv. Check that it runs on its own. ({ $reason })

# Toast when a stream never showed a frame and the next one is tried.
# @arg next: int
# @arg total: int
notice-stream-no-video = No video from that stream. Trying { $next } of { $total }.

# Toast when mpv gave up on a file. The reason is mpv's own line.
# @arg reason: text
notice-player-could-not-play = mpv couldn't play this. { $reason }

# Toast when mpv closed without playing and without saying why.
notice-player-exited-silently = mpv exited without playing anything and gave no reason. Try another { -source }.

# Toast when no picture has appeared a while after the player opened.
# @arg seconds: int
notice-player-not-playing = mpv hasn't started playing after { $seconds } s. The { -source } may have no data. Check that mpv plays a file on its own.

# Toast when a play is asked for in a build with no player.
notice-no-player = This version of { -app-name } can't play video.

# Toast at launch when the mpv program is missing. The binary is the name the settings give it.
# @arg binary: text
notice-mpv-missing = { $binary } wasn't found. Install mpv to play episodes.

# Toast at launch when mpv is too old for anistream's own player.
# @arg version: text
notice-mpv-old = mpv { $version } can play, but { -app-name }'s player needs mpv 0.36 or newer for its skip button, { -up-next-cap } card, menus and thumbnails.

# Toast after a link was handed to the browser.
notice-opened-in-browser = Opened in your browser.

# Toast when a link could not be opened. The reason comes from the system.
# @arg reason: text
notice-could-not-open = Couldn't open that. { $reason }

# Toast when a trailer plays in mpv.
notice-trailer-playing = Trailer playing in mpv.

# Toast when a trailer opens in the browser.
notice-trailer-opened = Trailer opened in your browser.

# Toast when a trailer could not be opened. The reason comes from the system.
# @arg reason: text
notice-trailer-failed = Couldn't open the trailer. { $reason }

# Toast when the next episode is not the one after this because filler was skipped.
# @arg episode: text
notice-filler-skipped = Skipped filler. { -up-next-cap } is episode { $episode }.

# Toast after switching the audio while playing.
notice-audio-now-subbed = Audio is now { -subbed }.

# Toast after switching the audio while playing.
notice-audio-now-dubbed = Audio is now { -dubbed }.

# Toast when the previous-episode control is used on the first episode.
notice-already-first-episode = Already at the first episode.

# Toast when the next-episode control is used on the last known episode.
# @arg episode: text
notice-last-episode = There's no next episode. Episode { $episode } is the last.

# Toast when the next-episode control is used and the next episode is not known.
notice-no-next-episode = There's no next episode.

# Toast when the previous-episode control finds no previous episode.
notice-no-previous-episode = There's no previous episode.

# Toast when the title's episode match could not be cleared. The reason comes from the system.
# @arg reason: text
notice-match-reset-failed = Couldn't reset the match. { $reason }

# Toast when the chosen match could not be saved. The reason comes from the system.
# @arg reason: text
notice-match-save-failed = Couldn't save that match. { $reason }

# Toast when a chosen match still left the title undecided.
notice-match-did-not-stick = That match didn't stick. The { -source } may have changed. Try choosing it again.

# Toast when a search of the sources for a title found nothing.
# @arg query: text
notice-source-search-empty = Nothing found for “{ $query }”. Try a different search.

# Toast after the person picked a source for one title.
# @arg source: text
notice-source-pinned = { $source } will be used for this title.

# Toast after the person went back to automatic source selection for one title.
notice-source-automatic = Back to automatic { -source } selection.

# Toast when the source choice for a title could not be saved. The reason comes from the system.
# @arg reason: text
notice-source-choice-failed = Couldn't save that choice. { $reason }

# Toast when no source could list a show's episodes. The reason is what the sources said.
# @arg reason: text
notice-no-source-could-list = No { -source } could list this show: { $reason }

# Toast when a title cannot be taken off the Continue row. The reason comes from the system.
# @arg reason: text
notice-continue-hide-failed = Couldn't remove that from { -continue-cap }. { $reason }

# Toast after an episode was added to the download queue.
# @arg title: text
# @arg episode: text
notice-download-queued = Queued episode { $episode } of { $title }.

# Toast when an episode asked for is in downloads already.
# @arg title: text
# @arg episode: text
notice-download-already-queued = Episode { $episode } of { $title } is already in downloads.

# Toast when a waiting or paused download was moved to the front and started.
# @arg title: text
# @arg episode: text
notice-download-starting-now = Starting episode { $episode } of { $title } now.

# Toast after a download was cancelled.
notice-download-cancelled = Download cancelled.

# Toast after a download and its file were deleted.
notice-download-file-deleted = Download and file deleted.

# Toast after a download was removed whose file was already gone.
notice-download-removed-file-gone = Download removed. Its file was already gone.

# Toast after a download was removed but its file could not be deleted. The reason comes from the system.
# @arg reason: text
notice-download-removed-file-kept = Removed the download, but couldn't delete its file. { $reason }

# Toast after a download with no file was removed.
notice-download-removed = Download removed.

# Toast after finished downloads were cleared from the list.
# @arg count: int
notice-downloads-cleared = { $count ->
    [one] Cleared one finished download.
   *[other] Cleared { $count } finished downloads.
 }

# Toast when an episode finished downloading.
# @arg title: text
# @arg episode: text
notice-download-finished = Downloaded episode { $episode } of { $title }.

# Toast when an episode finished downloading and the note says what happened to its subtitles.
# @arg title: text
# @arg episode: text
# @arg note: cause
notice-download-finished-note = Downloaded episode { $episode } of { $title }. { $note }

# Toast when a download could not start. The reason comes from the torrent client.
# @arg title: text
# @arg episode: text
# @arg reason: text
notice-download-failed = Couldn't download episode { $episode } of { $title }. { $reason }

# Toast after a range was asked for and nothing was queued.
notice-downloads-none-queued = Nothing queued.

# Toast after a range was asked for and nothing was queued, and the reason it stopped.
# @arg reason: cause
notice-downloads-none-queued-stopped = Nothing queued. { $reason }

# Toast after a range was asked for and every episode was in the queue already. `existing` is the sentence of `cause-already-in-downloads`.
# @arg existing: cause
notice-downloads-none-new = Nothing new queued. { $existing }

# Toast as above, with the reason the range stopped. `existing` is the sentence of `cause-already-in-downloads`; `reason` is a sentence saying why the range stopped.
# @arg existing: cause
# @arg reason: cause
notice-downloads-none-new-stopped = Nothing new queued. { $existing } { $reason }

# Toast after a range was asked for, some episodes were queued and some were there already. `existing` is the sentence of `cause-already-in-downloads`.
# @arg count: int
# @arg existing: cause
notice-episodes-queued-existing = { $count ->
    [one] Queued one episode. { $existing }
   *[other] Queued { $count } episodes. { $existing }
 }

# Toast as above, with the reason the range stopped.
# @arg count: int
# @arg reason: cause
notice-episodes-queued-stopped = { $count ->
    [one] Queued one episode, then it stopped. { $reason }
   *[other] Queued { $count } episodes, then it stopped. { $reason }
 }

# Toast as above, with some episodes in the queue already and the reason the range stopped. `existing` is the sentence of `cause-already-in-downloads`; `reason` is a sentence saying why the range stopped.
# @arg count: int
# @arg existing: cause
# @arg reason: cause
notice-episodes-queued-existing-stopped = { $count ->
    [one] Queued one episode, then it stopped. { $existing } { $reason }
   *[other] Queued { $count } episodes, then it stopped. { $existing } { $reason }
 }

# Toast when keep next found no room for another episode.
notice-keep-next-full = Not enough free space to keep the next episodes downloaded. Free up some space to download more.

# Message when a watch party was asked for with no server chosen. The names are a suggested server and who runs it.
# @arg server: text
# @arg operator: text
party-needs-server = Choose a server for the { -watch-party } first, as syncplay.server. { $server } is run by { $operator }.

# Toast when a watch party is asked for in a build without watch parties.
notice-no-party = { -watch-party-cap-pl } aren't available in this version of { -app-name }.

# Message when a watch party was asked for with no room named.
party-needs-room = A { -watch-party } needs a room to meet in. Choose one first.

# Message in a watch party when its server connection dropped and is retried.
party-server-lost = Lost the { -watch-party } server. Trying again.

# Message in a watch party when the connection came back.
party-rejoined = Back in the { -watch-party }.

# Message when the person left a watch party.
party-left = Left the { -watch-party }.
