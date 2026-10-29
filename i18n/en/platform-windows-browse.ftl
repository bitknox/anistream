## Windows: Continue, Calendar, Seasonal, Library, Search and the title page.

# Context menu item on a Continue row: plays the episode from where it stopped.
# @only windows
continue-menu-resume = { -continue-cap }

# Busy line while Continue loads.
# @only windows
continue-loading = Loading what you're watching

# Line under Continue's heading: changes made offline that AniList has not had yet.
# @only windows
# @arg count: int
continue-sync-waiting = { $count ->
    [one] One change waiting to sync
   *[other] { $count } changes waiting to sync
 }

# Calendar: a day column of the week view with nothing airing that day.
# @only windows
calendar-day-empty = Nothing airs

# Calendar: the week shown, from its first day to its last, as dates.
# @only windows
# @arg start: text
# @arg end: text
calendar-week-range = { $start } to { $end }

# Calendar: an air time that is only an estimate, as shown in a row. The tilde means "about".
# @only windows
# @arg time: text
calendar-time-estimate = ~{ $time }

# Calendar: an estimated air time as the screen reader says it.
# @only windows
# @arg time: text
calendar-time-about = about { $time }

# Calendar: an episode that airs dubbed, with no time.
# @only windows
# @arg episode: int
calendar-episode-dubbed = E{ $episode } { -dubbed }

# Calendar: the one quiet line of an episode already watched: the show, then the episode.
# @only windows
# @arg title: text
# @arg episode: text
calendar-watched-line = { $title } · { $episode }

# Calendar: what the screen reader says for an episode already watched.
# @only windows
# @arg title: text
# @arg episode: text
calendar-watched-spoken = { $title }, { $episode }, watched

# Library: a list's tab with how many titles are on it, such as "Watching 12".
# @only windows
# @arg list: text
# @arg count: int
library-list-count = { $list } { $count }

# Search: the Sort drop-down's name before a sort is picked.
# @only windows
search-sort-name = Sort

# Search: a filter drop-down with two values picked, such as two genres.
# @only windows
# @arg first: text
# @arg second: text
filter-picked-two = { $first }, { $second }

# Search: a filter drop-down with three or more values picked: the first, then how many more.
# @only windows
# @arg first: text
# @arg more: int
filter-picked-more = { $first } +{ $more }

# Title page: a season in the season picker with when it aired, such as "Season 2 · Spring 2025".
# @only windows
# @arg season: text
# @arg when: text
title-season-when = { $season } · { $when }

# Title page: an episode row's download button, for an episode numbered other than as a whole number, such as "12.5".
# @only windows
# @arg episode: text
episode-download-text = Download episode { $episode }

# Title page: an episode row's download button after a failure, for an episode numbered other than as a whole number.
# @only windows
# @arg episode: text
episode-download-failed-text = Couldn't download episode { $episode }. Download again.

# Title page: an episode row with no name of its own, numbered other than as a whole number, such as "Episode 12.5".
# @only windows
# @arg episode: text
episode-number-text = Episode { $episode }

# Title page: an episode row's heading with a name, for an episode numbered other than as a whole number.
# @only windows
# @arg episode: text
# @arg title: text
episode-text-with-title = Episode { $episode } · { $title }

# Title page: what the screen reader says for an episode row: its heading, then its runtime or what is left.
# @only windows
# @arg heading: text
# @arg line: text
episode-row-spoken = { $heading }, { $line }

# Title page: the line of an episode that only AniList lists, so nothing can play it yet.
# @only windows
episode-listed-by-anilist = Listed by AniList

# Title page details: the studio that made the show.
# @only windows
# @arg studio: text
title-studio-line = Studio: { $studio }

# Title page details: what the show is adapted from, such as "Based on: Manga".
# @only windows
# @arg material: text
title-based-on-line = Based on: { $material }

# Title page: a related title's line, its relation and its format, such as "Sequel · TV".
# @only windows
# @arg relation: text
# @arg format: text
title-relation-format = { $relation } · { $format }

# Title page details: the latest episode and how long ago it came out. The span is already worded, such as "2d ago".
# @only windows
# @arg episode: int
# @arg ago: text
title-episode-out-ago = E{ $episode } out { $ago }

# Title page details: when the next episode airs, when its number is not known. The time is already worded, such as "in 4d 20h".
# @only windows
# @arg when: text
title-next-airs = Next episode airs { $when }

# Title page: the menu item that downloads one episode, and the download dialog's button.
# @only windows
title-episode-download = Download

# Title page: the More menu item that opens the download dialog.
# @only windows
title-download-menu = Download…

# Title page: the More menu item that opens this show's settings.
# @only windows
title-show-settings-menu = Show settings…

# Title page: the tooltip of Play on TV.
# @only windows
title-play-on-tv-tooltip = Play this episode on a TV with { -app-name }

# Title page: the screen reader's name for the button with the show's list status.
# @only windows
title-list-status = List status

# Download dialog: its title.
# @only windows
download-range-title = Download episodes

# Download dialog: the first episode of the range.
# @only windows
download-range-from = From

# Download dialog: the last episode of the range.
# @only windows
download-range-to = To

# Download dialog: the note under the range.
# @only windows
download-range-quality-note = Downloads use the quality in Settings › Downloads.

# Calendar: tooltip of the button that returns to the current week, with its shortcut.
# @only windows
# @arg keys: text
calendar-today-tooltip = Today ({ $keys })
