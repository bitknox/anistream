## Apple only: the words of the search screen and its filter panel that the other apps word another way.

# Line while results wait for the rate limit to recover, on a phone where a Search key runs the search.
search-rate-limit-search = Results wait for Search while AniList’s rate limit recovers

# Line while results wait for the rate limit to recover, on a Mac where Return runs the search.
search-rate-limit-return = Results wait for Return while AniList’s rate limit recovers

# Line while a browse is slow to answer. The headline names what is browsed, such as "Trending now".
# @arg what: text
search-searching-browse = Searching for { $what }

# Title of the genre filter when more than one genre is chosen.
# @arg count: int
search-genres-count = { $count ->
    [one] { $count } genre
   *[other] { $count } genres
 }

# Title of the format filter when more than two formats are chosen.
# @arg count: int
search-formats-count = { $count ->
    [one] { $count } format
   *[other] { $count } formats
 }

# Title of the status filter when more than one status is chosen.
# @arg count: int
search-statuses-count = { $count ->
    [one] { $count } status
   *[other] { $count } statuses
 }

# Submenu and context menu item that leaves a genre or tag out of the results.
search-leave-out = Leave out

# Context menu item that includes a tag or genre in the results.
search-include = Include

# Tooltip of the All filters button.
search-filters-help = Tags, length, score, origin and where to watch

# VoiceOver value of the All filters button: how many advanced filters are set.
# @arg count: int
search-filters-set = { $count } set

# Title of the sort picker in the filter bar.
search-sort-by = Sort by

# Button that clears every filter chip at once.
search-clear-all = Clear all

# Title of the empty state when the filters match nothing.
search-no-results = No results

# Heading of the placeholder rail shown while the search page loads.
search-trending-now = Trending now

# Prompt of the lower bound of a range filter.
filter-range-from = From

# Prompt of the upper bound of a range filter.
filter-range-to = To

# Word between the two bounds of a range filter, small and lower case.
filter-range-between = to

# Value of the score slider when no minimum is set.
filter-any = Any

# Unit shown after the bounds of the episode length filter.
filter-minutes-unit = min

# Label of the years range in the Released section.
filter-years = Years

# VoiceOver hint of a filter group that is open.
filter-group-closes = Closes the group

# VoiceOver hint of a filter group that is closed.
filter-group-opens = Opens the group
