# Home: the first section of every app. The engine sends the headings and card lines; these are
# what each app draws around them.

# Name of the Home section in the sidebar, the tab bar and the window title.
# @max 16
home-title = Home

# Main action for an episode part way through.
# @arg episode: int
# @max 28
home-resume-episode = Resume episode { $episode }

# Main action for an episode not started.
# @arg episode: int
# @max 28
home-play-episode = Play episode { $episode }

# Shown in place of Add to Planning while it saves, after a moment.
# @max 20
home-adding = Adding

# Second action that opens the title's page.
# @max 16
home-details = Details

# Accessible name of the button that opens more actions.
# @max 16
home-more = More

# Action in the spotlight's More menu for a show part way through.
# @arg episode: int
# @max 40
home-mark-watched = Mark episode { $episode } as watched

# Notice after the action above. It has an Undo button.
# @arg episode: int
home-marked-watched = Episode { $episode } marked as watched

# Action on a { -continue-cap } card and in the spotlight's More menu.
# @max 32
home-remove-continue = Remove from { -continue-cap }

# Count beside the { -continue-cap } heading when there are more than twelve shows.
# @arg count: int
# @max 16
home-continue-count = { $count ->
    [one] 1 show
   *[other] { $count } shows
}

# Link beside a heading that opens the full list.
# @max 16
home-view-all = View all

# Link beside This week and Airing today that opens the calendar.
# @max 20
home-full-calendar = Full calendar

# The first day of This week.
# @max 16
home-today = Today

# A day of This week with nothing on it.
# @max 20
home-nothing-airing = Nothing airing

# A day of This week with more titles than fit.
# @arg count: int
# @max 16
home-day-more = { $count } more

# Link beside This season and Starting in that opens Seasonal.
# @max 16
home-seasonal = Seasonal

# Link beside a genre shelf that opens Search with that genre.
# @max 16
home-search = Search

# The closing line at the end of Home. Seasonal and Calendar are links.
home-end = That's everything for now. Seasonal and Calendar have the full lists.

# The closing line at the end of Home when no { -tracker } is signed in.
home-end-signed-out = Sign in to AniList to bring in your list and Planning.

# Button in the closing line when no { -tracker } is signed in.
# @max 28
home-sign-in = Sign in to AniList

# The closing line at the end of Home when no { -source } is installed. Plugin stores is a link.
home-end-no-sources = Add a { -source } to play from Home. { -plugin-store-cap-pl } has some to choose from.

# Line on a title that no installed { -source } has. The main action becomes Add to Planning.
# @max 36
home-not-found = Not found in your { -source-pl }

# Notice when part of Home could not be loaded. It has a Try again button.
home-failed = Couldn't load suggestions

# Button in the notice above.
# @max 16
home-try-again = Try again

# Activity line while Home's suggestions take longer than a few seconds.
home-loading = Loading suggestions

# Line at the top of Home when it is offline and shows what it last loaded. The time is worded already, such as 14:20.
# @arg time: text
home-offline = Offline. Showing Home from { $time }.

# Heading when Home is offline and has nothing saved.
# @max 28
home-offline-empty-title = You're offline

# Text under the heading above.
home-offline-empty-body = Home needs a connection the first time it opens. Downloads still play.

# Button under the text above.
# @max 24
home-open-downloads = Open Downloads

# Settings row and page that lists the titles hidden with Not interested.
# @max 28
settings-hidden-titles = Hidden titles

# Text under the list of hidden titles.
settings-hidden-titles-body = Titles you marked Not interested. They stay out of Home and { -up-next-cap } for you until you restore them.

# Line under a hidden title: when it was hidden. The date is worded already, such as "today" or "3 October".
# @arg date: text
# @max 32
settings-hidden-on = Hidden { $date }

# Line under a hidden title that comes back by itself once it starts airing.
# @max 40
settings-hidden-until-airs = Comes back when it airs

# Button beside a hidden title that brings it back to Home.
# @max 16
settings-restore = Restore

# Settings, Hidden titles, when nothing is hidden.
settings-hidden-empty = Nothing hidden. Titles you mark Not interested appear here.

# Accessible name of the arrow that pages a shelf back.
# @max 16
home-previous = Previous

# Accessible name of the arrow that pages a shelf on.
# @max 16
home-next = Next

# Line in the header capsule while Home is up to date with the signed-in tracker.
# @max 24
home-synced = AniList synced
