# Messages for Home: the line over the spotlight's title, the section headings and the line under
# each card. The engine picks which one applies; each app draws it. Provisional until the final
# design board.

# Line over the spotlight's title when it is a show the person is part way through.
# @max 32
home-spotlight-continuing = { -continue-cap } watching

# Line over the spotlight's title when it is a new season of a show the person finished. The title is the season they finished.
# @arg title: text
# @max 60
home-spotlight-new-season = New season · You finished { $title }

# Line over the spotlight's title when it is on the person's Planning list.
# @max 32
home-spotlight-planning = On your Planning

# Line over the spotlight's title when people who liked a show the person finished rated it highly. The title is that show.
# @arg title: text
# @max 48
home-spotlight-recommended = Like { $title }

# Line over the spotlight's title when it is recommended from what the person watched, and the show it is like is not known.
# @max 32
home-spotlight-recommended-plain = Recommended for you

# Line over the spotlight's title when nothing of the person's own is waiting, so it is this season's most popular show they have not started.
# @max 32
home-spotlight-popular = Popular this season

# Heading over the shows the person is part way through.
# @max 24
home-continue = { -continue-cap }

# Heading over next season's shows, in the two weeks before it starts.
# @max 32
home-starting-winter = Starting in Winter

# Heading over next season's shows, in the two weeks before it starts.
# @max 32
home-starting-spring = Starting in Spring

# Heading over next season's shows, in the two weeks before it starts.
# @max 32
home-starting-summer = Starting in Summer

# Heading over next season's shows, in the two weeks before it starts.
# @max 32
home-starting-autumn = Starting in Autumn

# Heading over the week's broadcasts of the shows the person follows, from today.
# @max 24
home-this-week = This week

# Heading over today's broadcasts, shown to someone who has not watched anything yet.
# @max 24
home-airing-today = Airing today

# Heading over the suggestions from the person's own history and lists.
# @max 32
home-up-next = { -up-next-cap } for you

# Heading over this season's most popular shows.
# @max 24
home-this-season = This season

# Heading over the shows recommended from the one the person finished most recently. The title is that show.
# @arg title: text
# @max 60
home-because = Because you finished { $title }

# Heading over popular shows in the genre the person finishes most. The genre is AniList's name for it, such as Mystery.
# @arg genre: text
# @max 40
home-more-genre = More { $genre }

# Line under a card of a show part way through an episode: the episode and the minutes left in it.
# @arg episode: int
# @arg minutes: int
# @max 40
home-line-left = { $minutes ->
    [one] Episode { $episode } · 1 min left
   *[other] Episode { $episode } · { $minutes } min left
}

# Line under a card of a show part way through an episode when its length is not known.
# @arg episode: int
# @max 32
home-line-resume = Resume episode { $episode }

# Line under a card of a show whose next episode is out.
# @arg episode: int
# @max 32
home-line-next = Episode { $episode }

# Line under a card whose episode came out a short while ago. The span is how long ago, such as 2h.
# @arg episode: int
# @arg ago: seconds
# @max 36
home-line-out = Episode { $episode } · out { $ago } ago

# Line under a suggestion that is a new season of a show the person finished.
# @max 28
home-line-new-season = New season

# Line under a suggestion on the person's Planning list.
# @max 28
home-line-planning = On your Planning

# Line under a suggestion that people who liked a finished show rated highly. The title is that show.
# @arg title: text
# @max 36
home-line-like = Like { $title }
