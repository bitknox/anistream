## The title page

# Title of the state when a title page could not load.
title-load-failed = Couldn't load this title

# Line under the load failure when AniList did not answer.
title-no-answer = AniList didn't answer. Try again.

# Short line while a title page loads.
title-loading = Loading this title

# Button on the title page that plays the trailer.
title-trailer = Trailer

# Menu item that plays the trailer.
title-play-trailer = Play trailer

# Name and tooltip of the button beside a playing trailer that turns its sound on or off.
# @max 24
title-trailer-sound = Trailer sound

# Menu item that opens the title on the AniList website.
title-open-anilist = Open on AniList

# Menu item that opens the title page of the show a row belongs to.
title-open = Open title

# Heading of the list of seasons.
title-seasons = Seasons

# Name of one season of a show, such as "Season 2".
# @arg number: int
season-number = Season { $number }

# Heading of the list of episodes.
title-episodes = Episodes

# Heading of the list of related titles.
title-related = Related

# Heading of the block of facts about a title.
title-details = Details

# Label of the studio in the details.
title-studio = Studio

# Heading of the block of tags.
title-tags = Tags

# Button that opens the full page of facts about a show.
title-more-about = More about this show

# Accessibility hint of the button that expands a synopsis.
title-synopsis-hint = Shows the whole synopsis

# Label under a season: which episodes are dubbed so far.
# @arg episode: int
title-dubbed-to = { -dubbed-cap } to episode { $episode }

# State of a title with no episodes listed yet.
title-no-episodes = No episodes yet

# Line when the episodes could not be listed.
title-episodes-load-failed = Couldn't list episodes

# Line under the episode list when it comes from AniList and no source can play them.
title-episodes-no-source = Episodes come from AniList. Add a { -source } to play them.

# Button that collapses the watched episodes of a season.
title-hide-watched = Hide watched episodes

# Button that shows the watched episodes of a season again.
title-show-watched = Show watched episodes

# Accessibility state of a section that is open.
state-expanded = Expanded

# Accessibility state of a section that is closed.
state-collapsed = Collapsed

# Row that stands for several watched episodes in a row, such as "Episodes 1 to 5 watched".
# @arg first: int
# @arg last: int
title-watched-range = Episodes { $first } to { $last } watched

## Episodes

# Name of an episode, such as "Episode 7".
# @arg number: int
episode-number = Episode { $number }

# Short name of an episode, for tight spaces.
# @arg number: int
# @max 12
episode-number-short = E{ $number }

# An episode and its name.
# @arg number: int
# @arg title: text
episode-with-title = Episode { $number } · { $title }

# An episode, dubbed, and its name.
# @arg number: int
# @arg title: text
episode-with-title-dubbed = Episode { $number } · { -dubbed-cap } · { $title }

# Badge on an episode the person has watched.
episode-watched = Watched

# Badge on an episode that is filler.
episode-filler = Filler

# Row note for an episode that has not aired.
episode-row-not-out = Not out yet

# Row note for an episode that has not aired, with when it does.
# @arg when: text
episode-row-airs = Airs { $when }

# Accessibility label of an episode's still while spoiler protection hides it.
episode-spoiler-hidden = Still hidden: spoiler protection

# Accessibility label of the button that opens an episode's menu.
episode-options = Episode options

# Menu item that marks an episode as watched.
episode-mark-watched = Mark watched

# Menu item that marks an episode as not watched.
episode-mark-unwatched = Mark unwatched

# Menu item that marks every episode before this one as watched.
episode-mark-previous-watched = Mark all previous watched

# Menu item that marks every episode up to a given one as watched.
# @arg episode: int
episode-mark-up-to = Mark watched up to episode { $episode }

# Menu item that marks one numbered episode as watched.
# @arg episode: int
episode-mark-n-watched = Mark episode { $episode } watched

# Menu item that marks one numbered episode as not watched.
# @arg episode: int
episode-mark-n-unwatched = Mark episode { $episode } unwatched

# Notice after an episode was marked watched.
# @arg episode: int
episode-marked-watched = Marked episode { $episode } watched.

# Menu item that starts an episode from the beginning.
episode-play-from-beginning = Play from the beginning

# Menu item that downloads one numbered episode.
# @arg episode: int
episode-download-n = Download episode { $episode }

# Menu item that downloads every episode not yet watched.
episode-download-unwatched = Download unwatched episodes

## More about this show

# Heading of the cast list.
more-cast = Cast

# Note under the cast heading: whose voices are listed.
more-cast-voices = Japanese voices

# Role of a character in the story.
more-role-main = Main

# Role of a character in the story.
more-role-supporting = Supporting

# Role of a character in the story.
more-role-background = Background

# Heading of the staff list.
more-staff = Staff

# Heading of the list of recommended titles.
more-recommendations = Recommendations

# Heading of the list of rankings.
more-rankings = Rankings

# Line when AniList has no cast, staff or recommendations for a show.
more-empty = AniList has no cast, staff or recommendations for this show yet.

# Badge on a tag that gives away the story.
more-spoiler = Spoiler

# Button that hides the spoiler tags.
more-hide-spoiler-tags = Hide spoiler tags

# Button that shows the spoiler tags.
# @arg count: int
more-show-spoiler-tags = { $count ->
    [one] Show one spoiler tag
   *[other] Show { $count } spoiler tags
 }
