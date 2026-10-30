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

# Toast when something went wrong inside the engine and it carried on.
notice-engine-fault = Something went wrong inside { -app-name }. It carried on, but if a screen looks stuck, quit and open it again.

# Toast at launch when the database was damaged, so a fresh one was started. The path is where
# the damaged file was moved, kept for recovery. Watch history and downloads start over; what the
# trackers hold comes back with the next sync.
# @arg path: text
notice-database-set-aside = The database was damaged, so { -app-name } started a new one. The old file is kept at { $path }. Your { -tracker-pl } bring back what they hold at the next sync.

# Message when anistream could not start because a folder could not be made. The path is the
# folder, the reason comes from the system.
# @arg path: text
# @arg reason: text
start-failed-folder = { -app-name } couldn't create the folder { $path }. Check that the disk has room and the folder can be written to, then try again. { $reason }

# Message when anistream could not start because the config file has a setting it can't use.
# The path is the file; the problem names the line where it can.
# @arg path: text
# @arg problem: text
start-failed-config = { $path } has a setting { -app-name } can't use: { $problem }. Fix or remove it, then try again.

# Message when anistream could not start because its database could not be opened. The path is
# the database file, the reason comes from the system.
# @arg path: text
# @arg reason: text
start-failed-database = { -app-name } couldn't open its database at { $path }. If another { -app-name } app is open on this folder, quit it and try again. { $reason }

# Message when anistream could not start its network connection from the network settings.
# @arg reason: text
start-failed-network = { -app-name } couldn't set up its network connection from the network settings in config.toml. { $reason }

# Message when anistream itself could not start. The reason comes from the system or the engine.
# @arg reason: text
start-failed = { -app-name } couldn't start. Quit and open it again. { $reason }

# Message when the TV remote could not start listening or could not forget a phone.
# @arg reason: text
start-failed-remote = The remote couldn't start. { $reason }

# Message when the built-in player could not start.
# @arg reason: text
start-failed-player = The player couldn't start. { $reason }

# Toast after episodes are added to the download queue.
# @arg count: int
notice-episodes-queued = { $count ->
    [one] Queued one episode.
   *[other] Queued { $count } episodes.
 }
