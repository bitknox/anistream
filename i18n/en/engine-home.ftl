# Messages for Home: the line over the spotlight's title, the section headings and the line under
# each card. The engine picks which one applies; each app draws it.

# Line over the spotlight's title when it is a show the person is part way through.
# @max 32
home-spotlight-continuing = { -continue-cap } watching

# Line over the spotlight's title when it is a new season of a show the person finished. The title is the season they finished.
# @arg title: text
# @max 60
home-spotlight-new-season = New season · You finished { $title }

# Line over the spotlight's title when it is on the person's Planning list.
# @max 32
home-spotlight-planning = On your Planning list

# Line over the spotlight's title when people who liked a show the person finished rated it highly. The title is that show; only it truncates.
# @arg title: text
# @max 60
home-spotlight-because = Because you finished { $title }

# Line over the spotlight's title when it is recommended from what the person watched, and the show it is like is not known.
# @max 32
home-spotlight-recommended = Recommended for you

# Line over the spotlight's title when nothing of the person's own is waiting, so it is this season's most popular show they have not started.
# @max 32
home-spotlight-popular = Popular this season

# Line under the spotlight's title, and under a { -continue-cap } card, for an episode part way through: the episode and the minutes left in it.
# @arg episode: int
# @arg minutes: int
# @max 40
home-time-left = Episode { $episode } · { $minutes } min left

# Line under the spotlight's title for a show part way through with its next episode waiting.
# @arg episode: int
# @max 32
home-episode-out = Episode { $episode } is out

# Line under a { -continue-cap } card whose next episode came out today.
# @arg episode: int
# @max 32
home-episode-out-today = Episode { $episode } · out today

# Line under a poster on This season: the newest episode out.
# @arg episode: int
# @max 28
home-episode-out-short = Episode { $episode } out

# Line under a This week or Airing today card whose episode is already out today. The time is worded as a span, such as 2h 5m.
# @arg ago: seconds
# @max 24
home-out-ago = Out { $ago } ago

# Short reason under a poster: a new season of a show the person finished.
# @max 28
home-reason-new-season = New season

# Short reason under a poster: the title is on the person's Planning list.
# @max 28
home-reason-planning = On your Planning list

# Short reason under a poster: recommended from what the person watched. The hover card gives the full reason.
# @max 28
home-reason-recommended = Recommended for you

# Heading over the shows the person is part way through.
# @max 24
home-continue = { -continue-cap }

# Heading over next season's titles in the two weeks before it starts.
# @max 32
home-starting-winter = Starting in Winter

# Heading over next season's titles in the two weeks before it starts.
# @max 32
home-starting-spring = Starting in Spring

# Heading over next season's titles in the two weeks before it starts.
# @max 32
home-starting-summer = Starting in Summer

# Heading over next season's titles in the two weeks before it starts.
# @max 32
home-starting-autumn = Starting in Autumn

# Heading over this week's episodes of the shows the person follows.
# @max 24
home-this-week = This week

# Heading over what airs today, for someone with nothing watched and no { -tracker } signed in.
# @max 24
home-airing-today = Airing today

# Heading over the suggestions made from what the person watched and their lists.
# @max 32
home-up-next = { -up-next-cap } for you

# Heading over this season's most popular titles.
# @max 24
home-this-season = This season

# Heading over recommendations from one show the person finished and rated well. The title is that show.
# @arg title: text
# @max 60
home-because = Because you finished { $title }

# Heading over popular titles of the genre the person finished most of. The genre is lower case, such as "mecha".
# @arg genre: text
# @max 40
home-more-genre = More { $genre }

# Line over the spotlight's title when it is the built-in demo.
# @max 32
home-spotlight-demo = Demo, free to share
