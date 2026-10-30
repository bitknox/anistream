# Under a release in the source picker: how many people share it right now.
# @only linux
# @arg count: int
# @max 24
source-picker-seeders-linux = { $count ->
    [one] One seeder
   *[other] { $count } seeders
 }

# Tooltip on the header bar button that shows the sidebar while the window is narrow.
# @only linux
# @max 24
sidebar-show = Show sidebar

# Faint line over Continue's placeholders while it loads.
# @only linux
# @max 40
activity-continue = Loading what you're watching

# Faint line over the calendar's placeholders while it loads.
# @only linux
# @max 40
activity-calendar = Loading the calendar

# Faint line over a title page's episode placeholders while they load.
# @only linux
# @max 40
activity-episodes = Loading episodes

# Faint line over the Sources page while the sources are checked.
# @only linux
# @max 40
activity-sources = Checking { -source-pl }

# Faint line over the Downloads page while it loads.
# @only linux
# @max 40
activity-downloads = Loading downloads

# Faint line over Search's placeholders while it lists titles with no query typed.
# @only linux
# @max 40
activity-browse = Loading titles

# A transfer rate: a size already worded, per second.
# @only linux
# @arg size: text
size-per-second = { $size }/s

# A wait that is over.
# @only linux
time-now = Now

# How long ago, when it was inside the last minute.
# @only linux
time-just-now = Just now

# A length of time said for a screen reader, in hours.
# @only linux
# @arg count: int
spoken-hours = { $count ->
    [one] { $count } hour
   *[other] { $count } hours
 }

# A length of time said for a screen reader, in minutes.
# @only linux
# @arg count: int
spoken-minutes = { $count ->
    [one] { $count } minute
   *[other] { $count } minutes
 }

# A length of time said for a screen reader, in seconds.
# @only linux
# @arg count: int
spoken-seconds = { $count ->
    [one] { $count } second
   *[other] { $count } seconds
 }

# Continue row: nothing new to watch, and the next episode airs at a known time. The time is already worded: "in 2d 3h".
# @only linux
# @arg episode: int
# @arg when: text
# @max 50
continue-row-caught-up-airs = Caught up · E{ $episode } airs { $when }

# Continue row: nothing new to watch, and the next episode has no date.
# @only linux
# @arg episode: int
# @max 50
continue-row-caught-up-not-out = Caught up · E{ $episode } is not out yet

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-english = English

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-japanese = Japanese

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-spanish = Spanish

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-portuguese = Portuguese

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-french = French

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-german = German

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-italian = Italian

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-russian = Russian

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-arabic = Arabic

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-indonesian = Indonesian

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-korean = Korean

# Name of a language, in a list of subtitle languages.
# @only linux
language-name-chinese = Chinese

# Seasonal's format toggle: every format.
# @only linux
# @max 12
seasonal-format-all = All

# Seasonal's format toggle: ONA, OVA and specials together.
# @only linux
# @max 20
seasonal-format-ona-ova = ONA and OVA

# Accessible name of a title's banner, which opens its page.
# @only linux
# @arg title: text
title-open-named = Open { $title }

# Accessible name of a card or row in Continue: the title, then what is said under it.
# @only linux
# @arg title: text
# @arg detail: text
continue-row-spoken = { $title }, { $detail }

# Accessible name of the pane beside Continue's queue that previews the selected title.
# @only linux
continue-preview = Preview

# Under a suggestion that airs later today when no episode number is known: the time it airs.
# @only linux
# @arg time: text
# @max 28
up-next-airs-at = Airs at { $time }
