## The terminal's title page, its episode table, and the pickers opened from them.

# Heading of the title page when no title is highlighted.
# @only tui
title-heading = Title

# Under the heading of the title page and the episode table when no title is highlighted.
# @only tui
title-nothing-selected-note = Nothing selected

# Toast when a title action is asked for and no title is highlighted.
# @only tui
title-nothing-selected = Nothing is selected. Highlight a title and try again.

# Toast when something that needs a title's full page is asked from a list.
# @only tui
title-trailer-off-page = Open the title's page to find its trailer.

# Toast when something that needs a title's full page is asked from a list.
# @only tui
title-recommendations-off-page = Open the title's page to see its recommendations.

# Toast when something that needs a title's full page is asked from a list.
# @only tui
title-seasons-off-page = Open the title's page to see its connected seasons.

# Toast when a title has no trailer on record.
# @only tui
title-no-trailer = No trailer is on record for this title.

# Toast when a title has no recommendations.
# @only tui
title-no-recommendations = No recommendations are known for this title.

# Toast when a title has no connected seasons.
# @only tui
title-no-seasons = No connected seasons are known for this title.

# Toast when the { -source } picker is asked for from a list.
# @only tui
title-pin-off-page = Open the title's page to pick a { -source } for it.

# Toast when the { -source } picker is asked for and no { -source } is registered. They register a moment after launch.
# @only tui
title-no-sources-yet = No { -source-pl } are registered yet. Try again in a moment.

# Toast when a list status or rating is asked for and no { -tracker } is signed in. The command is typed as it is.
# @only tui
title-status-needs-tracker = Your list status is kept on a { -tracker }, and none is signed in. Use :accounts to sign in to one.

# Toast when a rating is asked for and no { -tracker } is signed in. The command is typed as it is.
# @only tui
title-rating-needs-tracker = Ratings are kept on a { -tracker }, and none is signed in. Use :accounts to sign in to one.

# Toast after a list status is picked. The status is one of the status names; the trackers are a comma separated list.
# @arg status: text
# @arg trackers: text
# @only tui
title-status-set = Status set to { $status }. Queued for { $trackers }.

# Toast after a rating is picked.
# @arg score: int
# @arg trackers: text
# @only tui
title-rated = Rated { $score }/10. Queued for { $trackers }.

# List status in the status picker and in the toast after picking one.
# @only tui
title-status-watching = Watching

# List status in the status picker and in the toast after picking one.
# @only tui
title-status-planning = Planning

# List status in the status picker and in the toast after picking one.
# @only tui
title-status-completed = Completed

# List status in the status picker and in the toast after picking one.
# @only tui
title-status-paused = Paused

# List status in the status picker and in the toast after picking one.
# @only tui
title-status-dropped = Dropped

# Beside the Completed status in the status picker, when the title has progress: what choosing it does.
# @only tui
title-status-completed-hint = Sets progress to the last episode

# A rating in the rating picker, beside the number 10.
# @only tui
title-rating-10 = Masterpiece

# A rating in the rating picker, beside the number 9.
# @only tui
title-rating-9 = Great

# A rating in the rating picker, beside the number 8.
# @only tui
title-rating-8 = Very good

# A rating in the rating picker, beside the number 7.
# @only tui
title-rating-7 = Good

# A rating in the rating picker, beside the number 6.
# @only tui
title-rating-6 = Fine

# A rating in the rating picker, beside the number 5.
# @only tui
title-rating-5 = Average

# A rating in the rating picker, beside the number 4.
# @only tui
title-rating-4 = Poor

# A rating in the rating picker, beside the number 3.
# @only tui
title-rating-3 = Bad

# A rating in the rating picker, beside the number 2.
# @only tui
title-rating-2 = Awful

# A rating in the rating picker, beside the number 1.
# @only tui
title-rating-1 = Appalling

# Place of a title in the watch order: the nth season of its franchise. Sits in a narrow column.
# @arg number: int
# @max 16
# @only tui
title-season = Season { $number }

# Place of a title in the watch order: it comes before this one.
# @max 12
# @only tui
title-relation-prequel = Prequel

# Place of a title in the watch order: it is the story this one comes from.
# @max 12
# @only tui
title-relation-parent = Parent

# Place of a title in the watch order: it comes after this one.
# @max 12
# @only tui
title-relation-sequel = Sequel

# First choice in the { -source } picker: let anistream pick.
# @only tui
title-source-automatic = Automatic (best available { -source })

# Beside a { -source } in the picker when it is failing and so not used.
# @only tui
title-source-failing = Failing

# First choice in the { -release } picker: the { -source } decides.
# @only tui
title-release-first = Whichever { -release } the { -source } ranks first

# The services a title is available on, under its metadata. The list is already joined.
# @arg services: text
# @only tui
title-available-on = Available on { $services }

# Beside the progress meter on the title page: the episode that follows what has been watched.
# @arg next: int
# @only tui
title-up-next = { -up-next-cap }: episode { $next }

# Beside the progress meter when the watched count is against the episodes that have aired so far.
# @arg done: int
# @arg aired: int
# @only tui
title-progress-aired = { $done } of { $aired } aired

# Beside the progress meter when there is nothing to count against.
# @arg done: int
# @only tui
title-progress-watched = { $done } watched

## The episode table.

# Heading of the episode table when the title is not known.
# @only tui
episodes-heading = Episodes

# Column heading over the episode titles.
# @max 12
# @only tui
episodes-column-title = Title

# Column heading over the running time of each episode.
# @max 8
# @only tui
episodes-column-run = Run

# Column heading over how much of each episode is watched.
# @max 12
# @only tui
episodes-column-watched = Watched

# Beside a finished episode, in the last column.
# @max 8
# @only tui
episodes-done = Done

# In the last column for an episode that is not out yet. The span is short, such as 2d 3h.
# @arg span: text
# @max 12
# @only tui
episodes-airs-in = In { $span }

# Episode kind in the last column: the story episodes.
# @max 12
# @only tui
episodes-kind-canon = Canon

# Episode kind in the last column: part story, part filler.
# @max 12
# @only tui
episodes-kind-mixed = Mixed

# Episode kind in the last column: filler, which can be skipped.
# @max 12
# @only tui
episodes-kind-filler = Filler

# Episode kind in the last column: made for the anime and not in the book it comes from.
# @max 16
# @only tui
episodes-kind-anime-canon = Anime canon

# The episode filter shown when nothing is filtered, and named at the top right of the table.
# @only tui
episodes-filter-all = All episodes

# The episode filter that hides the episodes already watched.
# @only tui
episodes-filter-unwatched = Unwatched

# The episode filter that hides filler.
# @only tui
episodes-filter-no-filler = No filler

# Toast after the filter changes.
# @only tui
episodes-showing-all = Showing all episodes.

# Toast after the filter changes.
# @only tui
episodes-showing-unwatched = Showing unwatched episodes.

# Toast after the filter changes.
# @only tui
episodes-showing-no-filler = Showing episodes without filler.

# Top right of the episode table: how far the dub goes, for a dub that stops short of what has aired.
# @arg through: int
# @only tui
episodes-dub-through = Dub to episode { $through }

# Top right of the episode table: the dub has every episode.
# @only tui
episodes-dub-complete = { -dubbed-cap }

# Heading of the empty episode table when the filter hides every episode.
# @only tui
episodes-empty-filtered = Nothing matches the filter

# Under the heading when the filter hides every episode. The key is the filter key.
# @arg key: text
# @only tui
episodes-empty-filtered-hint = The filter hides every episode. Press { $key } to change it.

# Heading of the empty episode table when no { -source } is ready.
# @only tui
episodes-empty-no-source = No episodes yet

# Under the heading when no { -source } is ready. The key opens the { -source-pl } screen.
# @arg key: text
# @only tui
episodes-empty-no-source-hint = AniList lists no episodes for this title yet, and no { -source } is ready. Press { $key } to open the { -source-cap-pl } screen.

# Heading of the empty episode table when the title has not aired yet.
# @only tui
episodes-empty-unaired = Nothing has aired yet

# Under the heading when the title has not aired yet. The span is short, such as 2d 3h.
# @arg episode: int
# @arg span: text
# @only tui
episodes-empty-unaired-hint = Episode { $episode } airs in { $span }.

# Heading of the empty episode table when a { -source } matched the title and listed nothing.
# @only tui
episodes-empty-none-listed = No episodes listed

# Under the heading when a { -source } listed no episodes. The keys search by hand and pick another { -source }.
# @arg search: text
# @arg pick: text
# @only tui
episodes-empty-none-listed-hint = The { -source } listed no episodes for this title. Press { $search } to search for the right match, or { $pick } to pick another { -source }.

# Toast when an episode has no { -release } to pick from.
# @only tui
episodes-no-releases = No { -release-pl } are available to pick from for this episode.

# Toast when a download is asked for from a table listed from AniList alone. The key opens the { -source-pl } screen.
# @arg key: text
# @only tui
episodes-add-source-to-download = Add a { -source } before downloading. Press { $key } to open the { -source-cap-pl } screen.

# Toast when marking the episodes before the highlighted one and the table is empty.
# @only tui
episodes-mark-none-listed = No episodes are listed to mark.

# Toast when marking the episodes before the first one.
# @only tui
episodes-mark-nothing-before = Nothing comes before the first episode.

# Toast when every episode before the highlighted one is already watched.
# @arg episode: text
# @only tui
episodes-mark-already-watched = Everything before episode { $episode } is already watched.

# Toast after the episodes before the highlighted one are marked watched.
# @arg count: int
# @only tui
episodes-marked-watched = { $count ->
    [one] Marked { $count } episode watched.
   *[other] Marked { $count } episodes watched.
 }

# Toast when the range typed for a download is not understood. The examples are typed as they are.
# @only tui
episodes-range-invalid = Couldn't read that range. Use one episode (4), a range (1-12) or everything from one on (7-).

# Toast when no episode falls in the range typed for a download.
# @only tui
episodes-range-empty = No episodes in that range. Try another range.

# Toast after a range is queued for download.
# @arg count: int
# @only tui
episodes-queueing = { $count ->
    [one] Queueing { $count } episode for download.
   *[other] Queueing { $count } episodes for download.
 }

# Toast when a match is fixed from a list.
# @only tui
episodes-match-off-page = Open the title's page to fix its match.

# Toast when a match is fixed and no title is highlighted.
# @only tui
episodes-match-nothing = Nothing is selected to re-match.

# Toast after a match is picked. The title is the candidate's own.
# @arg title: text
# @only tui
episodes-matched = Matched to { $title }.

# After a { -release } in the picker: it holds both the subbed and the dubbed audio.
# @only tui
episodes-release-dual = { -subbed-cap } and { -dubbed }

# After a { -release } in the picker: it is dubbed.
# @only tui
episodes-release-dub = { -dubbed-cap }

# After a { -release } in the picker: it is the one anistream would play on its own.
# @only tui
episodes-release-current-pick = Current pick
