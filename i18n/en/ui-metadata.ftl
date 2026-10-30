## Words from the show data AniList sends. The apps show them in filters, badges and the details
## of a title.

## Formats

# Format of a show: broadcast on television.
format-tv = TV

# Format of a show: a short series on television.
format-tv-short = TV short

# Format of a show: a film.
format-movie = Movie

# Format of a show: an extra episode that is not part of a series.
format-special = Special

# Format of a show: released straight to video.
format-ova = OVA

# Format of a show: released straight to the web.
format-ona = ONA

# Format of a show: a music video.
format-music = Music

## Seasons of the year

# A season of the year, for when a show airs.
season-winter = Winter

# A season of the year, for when a show airs.
season-spring = Spring

# A season of the year, for when a show airs.
season-summer = Summer

# A season of the year, for when a show airs. British English.
season-autumn = Autumn

# A season and its year, such as "Autumn 2026". Both are already worded.
# @arg season: text
# @arg year: text
season-with-year = { $season } { $year }

## Airing status

# Status of a show that has finished airing.
media-status-aired = Aired

# Status of a show that is airing now.
media-status-airing = Airing

# Status of a show that has not started airing.
media-status-upcoming = Not yet released

# Status of a show that was cancelled.
media-status-cancelled = Cancelled

# Status of a show that is on a break.
media-status-hiatus = On hiatus

# Date line of a show that is airing: when it started.
# @arg date: text
media-airing-since = Since { $date }

# Date line of a show that has not started: when it will.
# @arg date: text
media-airing-from = From { $date }

# Date line of a show that has finished: when it began and when it ended. Both are already worded dates.
# @arg from: text
# @arg to: text
media-aired-range = Aired { $from } to { $to }

# Date line of a show that has finished and ran on one day, such as a film.
# @arg date: text
media-aired-on = Aired { $date }

# Date line of a show on a break: the month of the last episode that aired. The date is already worded, as a month and a year.
# @arg date: text
media-hiatus-since = On hiatus since { $date }

# Date line of a show that was cancelled: the last episode that aired.
# @arg episode: int
media-cancelled-after = Cancelled after episode { $episode }

## Where a show can be watched officially

# Heading of the list of official services that stream a show, in the sheet of more about it.
media-streaming-on = Streaming on

# Note under the list of official services: AniList's links are not checked against where the viewer is.
media-streaming-region-note = Availability depends on your region.

# Note beside a link to an official service whose page is in another language, such as "Site in Japanese".
# @arg language: text
media-link-language = Site in { $language }

## How long a film runs

# Length of a film or a single episode, in its metadata line: hours and the minutes past them, such as "1 h 52 min".
# @arg hours: int
# @arg minutes: int
film-length-hours-minutes = { $hours } h { $minutes } min

# Length of a film or a single episode that is a whole number of hours, such as "2 h".
# @arg hours: int
film-length-hours = { $hours } h

# Length of a film or a single episode under an hour, such as "46 min".
# @arg minutes: int
film-length-minutes = { $minutes } min

## Where a show was made

# Country name: the origin filter and, after "Made in", the title facts.
# @max 20
country-japan = Japan

# Country name: the origin filter and, after "Made in", the title facts.
# @max 20
country-china = China

# Country name, after "Made in".
country-south-korea = South Korea

# Country name: the origin filter and, after "Made in", the title facts.
# @max 20
country-taiwan = Taiwan

# Fact on a show made outside Japan, with the country already named.
# @arg country: text
media-made-in = Made in { $country }

# Fact on a show made outside Japan, for a country this app has no name for.
media-made-elsewhere = Made outside Japan

## What a show is based on

# Value of "Based on": a story written for the screen.
material-original = Original story

# Value of "Based on".
material-manga = Manga

# Value of "Based on".
material-light-novel = Light novel

# Value of "Based on".
material-web-novel = Web novel

# Value of "Based on".
material-visual-novel = Visual novel

# Value of "Based on".
material-video-game = Video game

# Value of "Based on".
material-novel = Novel

# Value of "Based on".
material-doujinshi = Doujinshi

# Value of "Based on": another anime.
material-anime = Anime

# Value of "Based on".
material-live-action = Live action

# Value of "Based on".
material-game = Game

# Value of "Based on".
material-comic = Comic

# Value of "Based on".
material-multimedia = Multimedia project

# Value of "Based on".
material-picture-book = Picture book

## How one show relates to another

# Relation of a related title: it comes before this one.
relation-prequel = Prequel

# Relation of a related title: it comes after this one.
relation-sequel = Sequel

# Relation of a related title: the story this one branches from.
relation-parent = Parent story

# Relation of a related title: a story set beside this one.
relation-side-story = Side story

# Relation of a related title: a show that grew out of this one.
relation-spin-off = Spin-off

# Relation of a related title: a retelling.
relation-alternative = Alternative

# Relation of a related title: a recap.
relation-summary = Summary

# Relation of a related title: a compilation of this one.
relation-compilation = Compilation

# Relation of a related title: it contains this one.
relation-contains = Contains

# Relation of a related title: set in the same universe.
relation-same-universe = Same universe

# Relation of a related title: it shares characters with this one.
relation-shares-characters = Shares characters

# Relation of a related title: an adaptation of this one.
relation-adaptation = Adaptation

# Relation of a related title: the work this show is based on.
relation-original-work = Original work

# Relation of a related title that fits none of the others.
relation-other = Related

# A related title's relation, its format and its year.
# @arg relation: text
# @arg format: text
# @arg year: text
relation-line = { $relation } · { $format } · { $year }

## Rankings

# Ranking line, such as "#4 Highest rated, Autumn 2026". The label is already worded.
# @arg rank: int
# @arg label: text
ranking-line = #{ $rank } { $label }

# Label of a ranking by score within a period.
# @arg scope: text
ranking-highest-rated = Highest rated, { $scope }

# Label of a ranking by popularity within a period.
# @arg scope: text
ranking-most-popular = Most popular, { $scope }

# The period of a ranking that covers everything.
ranking-all-time = all time

## Genres

# Genre name.
genre-action = Action

# Genre name.
genre-adventure = Adventure

# Genre name.
genre-comedy = Comedy

# Genre name.
genre-drama = Drama

# Genre name.
genre-ecchi = Ecchi

# Genre name.
genre-fantasy = Fantasy

# Genre name.
genre-horror = Horror

# Genre name.
genre-mahou-shoujo = Mahou shoujo

# Genre name.
genre-mecha = Mecha

# Genre name.
genre-music = Music

# Genre name.
genre-mystery = Mystery

# Genre name.
genre-psychological = Psychological

# Genre name.
genre-romance = Romance

# Genre name.
genre-sci-fi = Sci-fi

# Genre name.
genre-slice-of-life = Slice of life

# Genre name.
genre-sports = Sports

# Genre name.
genre-supernatural = Supernatural

# Genre name.
genre-thriller = Thriller
