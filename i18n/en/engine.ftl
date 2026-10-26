# The legal line. It exists once, so no translation can paraphrase it.
legal-hosts-no-content = { -app-name } hosts no content.

# Toast after a setting is stored. The label is the setting's name.
# @arg label: text
notice-saved = Saved { $label }.

# Toast when a setting could not be stored. The reason comes from the system.
# @arg reason: text
notice-not-saved = Couldn't save that setting. { $reason }

# Toast when a title's details did not load.
notice-title-load-failed = Couldn't load this title. Check your connection and try again.

# Toast when playback stops with an error.
notice-playback-failed = Playback failed. Try another { -source }.

# Toast after episodes are added to the download queue.
# @arg count: int
notice-episodes-queued = { $count ->
    [one] Queued one episode.
   *[other] Queued { $count } episodes.
 }
