## Seasonal

# Button that shows the season before.
seasonal-previous = Previous season

# Button that shows the season after.
seasonal-next = Next season

# Short line while a season loads.
seasonal-loading = Loading this season

# Title of the state when a season could not load.
seasonal-load-failed = Couldn't load this season

# Title of the state when a season lists no titles. The season is already worded.
# @arg season: text
seasonal-empty-title = Nothing listed for { $season } yet

# Line under the empty season title.
seasonal-empty-detail = AniList adds a season's titles as they're announced.

# Title of the state when the chosen format has no titles this season.
seasonal-empty-format = No titles in this format

# Line under the empty format title.
seasonal-empty-format-detail = Try all formats.

# Button that opens the season in Search, where filters narrow it.
seasonal-filter-in-search = Filter in Search

# Tooltip of the Filter in Search button.
seasonal-filter-help = This season in Search, where every filter narrows it

# Sort option on Seasonal and Search: most popular first.
sort-popularity = Popularity

# Sort option on Seasonal and Search: highest score first.
sort-score = Score

# Sort option on Seasonal: by the day of the week the show airs.
sort-air-day = Air day

# Sort option on Search: best match for the words typed.
sort-relevance = Relevance

# Button showing the current sort order.
# @arg sort: text
sort-button = Sort: { $sort }

# Accessibility label of the sort button.
# @arg sort: text
sort-label = Sort by { $sort }

## Search

# Placeholder of the search field.
search-placeholder = Search titles

# Accessibility label of the button that empties the search field.
search-clear = Clear search

# Row under the field while a search runs. The query is what the person typed.
# @arg query: text
search-searching = Searching for “{ $query }”

# Heading over the results of a search.
# @arg query: text
search-results-for = Results for “{ $query }”

# Heading over the titles shown when nothing was typed.
search-browse = Browse

# Title of the state when a search finds nothing.
# @arg query: text
search-nothing-found = Nothing found for “{ $query }”

# Line under the title when a search finds nothing.
search-nothing-found-detail = Check the spelling, or try the English or Japanese title.

# Title of the state when filters leave nothing.
search-nothing-with-filters = Nothing found with these filters

# Title of the state when a search and its filters leave nothing.
# @arg query: text
search-nothing-found-with-filters = Nothing found for “{ $query }” with these filters

# Line under a state that filters emptied.
search-try-fewer-filters = Try fewer filters.

# Button that removes every filter.
search-clear-filters = Clear filters

# Title of the state when a search failed.
search-failed = Couldn't search

# Title of the state when search needs a connection and there is none.
search-needs-connection = Search needs a connection

# Line when the search service does not answer.
search-unreachable = Search isn't reachable right now. Try again in a minute.

# Heading over the searches a person made lately.
search-recent = Recent searches

# Accessibility label of the button that forgets one recent search.
# @arg query: text
search-recent-remove = Remove { $query }

# Button that shows every title in a group.
search-see-all = See all

# Heading over the genre shortcuts.
search-genres = Genres

# Line while more results load at the end of a list.
search-loading-more = Loading more titles

## Filters

# Title of the filters sheet and panel.
filters-title = Filters

# Button that opens the filters, with how many are on.
# @arg count: int
filters-button-count = All filters · { $count }

# Button that opens the filters when none is on.
filters-button = All filters

# Filter name.
filter-genre = Genre

# Filter name, and the heading over the genre choices.
filter-genres = Genres

# Filter name.
filter-year = Year

# Filter name.
filter-season = Season

# Filter name.
filter-format = Format

# Filter name.
filter-status = Status

# Filter name: how long a show is, in episodes.
filter-length = Length

# Filter name: the number of episodes.
filter-episodes = Episodes

# Filter name: the length of one episode.
filter-episode-length = Episode length

# Filter group: how well a show was received.
filter-reception = Reception

# Filter name: a show's minimum score.
filter-score-at-least = Score at least

# Filter name: hides titles few people rated.
filter-hide-obscure = Hide obscure titles

# Filter group: where a show comes from.
filter-origin = Origin

# Filter name: the country a show was made in.
filter-country = Country

# Filter name: when a show came out.
filter-released = Released

# Filter group: where a show can be watched.
filter-where-to-watch = Where to watch

# Filter name: only shows with a legal licence.
filter-licensed-only = Licensed only

# Filter name: how titles already on the person's list are treated.
filter-my-list = Titles on my list

# Value of a three-way filter: include the titles.
filter-show = Show

# Value of a three-way filter: leave the titles out.
filter-hide = Hide

# Value of a three-way filter: only the titles.
filter-only = Only

# Note under the filters that tells results change as the person picks.
filter-live-note = Results update as you change these.

# Filter group: tags.
filter-tags = Tags

# Placeholder of the field that finds a tag.
filter-tag-find = Find a tag

# Line when no tag matches what was typed.
filter-tag-none = No tag has that in its name

# Filter name: how closely a tag must fit a show.
# @arg rank: text
filter-tag-rank = Tag rank at least { $rank }

# Switch that shows tags that give away the story.
filter-spoiler-tags = Show spoiler tags

# Choice meaning no genre filter.
filter-any-genre = Any genre

# Choice meaning no year filter.
filter-any-year = Any year

# Choice meaning no season filter.
filter-any-season = Any season

# Choice meaning no format filter.
filter-any-format = Any format

# Choice meaning no status filter.
filter-any-status = Any status

# Choice meaning every format on Seasonal.
filter-all-formats = All formats

# Note on the season filter while no year is chosen.
filter-season-pick-year = Choose a year first

# Choice in the year range: up to a year.
# @arg year: text
filter-years-up-to = Up to { $year }

# State of a genre or tag in a filter: it must be there.
filter-included = Included

# State of a genre or tag in a filter: it must not be there.
filter-left-out = Left out

# Hint on a genre or tag on a touch screen.
filter-hint-touch = Tap once to include, twice to leave out.

# Hint on a genre or tag on the TV, where a remote presses. Only the Android TV app shows it.
filter-hint-remote = Press once to include, twice to leave out.

# Accessibility label of the button that removes one filter.
# @arg filter: text
filter-remove = Remove { $filter }

# Search: sort by how many people marked the title a favourite.
# @max 20
search-sort-favourites = Favourites

# Search: sort by release date, newest first.
# @max 20
search-sort-newest = Newest

# Search: sort by release date, oldest first.
# @max 20
search-sort-oldest = Oldest

# Search: sort by title, A to Z.
# @max 20
search-sort-title = Title

# Search: sort by what is popular right now.
# @max 20
search-sort-trending = Trending
