# Home: the first section of every app. Provisional until the final design board.

# Name of the Home section in the sidebar, the tab bar and the window title.
# @max 16
home-title = Home

# Count beside the { -continue-cap } heading on Home.
# @arg count: int
# @max 16
home-continue-count = { $count ->
    [one] 1 show
   *[other] { $count } shows
}

# Link at the end of a shelf that opens the full list.
# @max 16
home-view-all = View all

# Link at the end of This week that opens the calendar.
# @max 20
home-full-calendar = Full calendar

# Line under a This week card for an episode still to air. The time is worded already, such as 22:30.
# @arg episode: int
# @arg time: text
# @max 32
home-week-airs = Episode { $episode } · { $time }

# The closing line at the end of Home.
home-end = That's everything for now. Seasonal and Calendar have the full lists.

# The closing line at the end of Home when no { -tracker } is signed in.
home-end-signed-out = Sign in to AniList to bring in your list and Planning.

# Line on a title that no installed { -source } has. The main action becomes Add to Planning.
# @max 36
home-not-found = Not found in your { -source-pl }

# Notice after Not interested on Home. It has an Undo button.
# @arg title: text
home-hidden = { $title } is hidden from { -up-next-cap } for you

# Notice when part of Home could not be loaded. It has a Try again button.
home-failed = Couldn't load suggestions

# Activity line while Home's suggestions take longer than a few seconds.
home-loading = Loading suggestions

# Line at the top of Home when it is offline and shows what it last loaded. The time is worded already, such as 14:20.
# @arg time: text
home-offline = Offline. Showing Home from { $time }.

# Settings row that lists the titles hidden with Not interested.
# @max 28
settings-hidden-titles = Hidden titles

# Settings, Hidden titles, when nothing is hidden.
settings-hidden-titles-empty = Nothing is hidden. Titles you mark Not interested on Home show here.

# Line under a hidden title that comes back by itself once it starts airing.
# @max 40
settings-hidden-until-airs = Hidden until it airs

# Button beside a hidden title that brings it back to Home.
# @max 16
settings-hidden-restore = Restore
