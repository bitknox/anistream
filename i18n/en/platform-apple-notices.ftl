## Apple only: the notifications the system shows for new episodes, and their buttons.
## Never an episode's name: a notification can show on a lock screen.

# Body of a new-episode notification when the viewer is caught up.
# @arg episode: int
notice-out = Episode { $episode } is out.

# Body of a new-episode notification for the dub the viewer watches the show in.
# @arg episode: int
notice-out-dubbed = Episode { $episode } is out, { -dubbed }.

# Body of a new-episode notification when the viewer is behind: the episode out and the one to watch next.
# @arg episode: int
# @arg next: int
notice-out-behind = Episode { $episode } is out. Next for you: episode { $next }.

# Body of a new-episode notification for a dub when the viewer is behind.
# @arg episode: int
# @arg next: int
notice-out-dubbed-behind = Episode { $episode } is out, { -dubbed }. Next for you: episode { $next }.

# What the system shows instead of the body when previews are hidden.
notice-hidden-preview = New episode

# What the system shows under a group of new-episode notifications. The system fills in the count at %u.
notice-summary = %u more new episodes

# Button of a notice that opens the plugin stores page.
notice-open-stores = Open { -plugin-store-pl }

# VoiceOver action of a notice that dismisses it.
notice-dismiss = Dismiss
