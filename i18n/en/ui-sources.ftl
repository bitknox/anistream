# Title of the screen or section that lists sources.
sources-title = { -source-cap-pl }

# Shown on the sources screen when none is installed.
sources-empty = No { -source-pl } yet. Join a { -plugin-store } to add some.

# Warning at install and in onboarding, in the same words in both.
plugin-risk-warning = { -plugin-cap-pl } are written by other people and run on your { -device }. Install only from people you trust.

## The source picker for one title: the command palette on Mac, Windows and Linux, the title's
## "..." menu on iPhone, iPad, Android and the TV.

# Menu and palette entry that opens the picker, and the picker's title.
# @max 28
source-picker-title = { -source-cap } for this title

# The first choice: let anistream pick per episode. Paired with source-picker-automatic-detail.
# @max 20
source-picker-automatic = Automatic

# Under "Automatic": what it does.
# @max 64
source-picker-automatic-detail = Each episode from the first { -source } that has it

# Under a source: how many of the title's released episodes it lists.
# @arg covered: int
# @arg of: int
# @max 32
source-picker-coverage = { $covered } of { $of ->
    [one] { $of } episode
   *[other] { $of } episodes
 }

# Under a source that is switched off in Settings.
# @max 24
source-note-off = Switched off

# Under a source that failed a few times in a row. It is tried again shortly.
# @max 44
source-note-resting = Not responding, trying again soon

# Under a source picked for this title that is no longer installed.
# @max 24
source-note-gone = Not installed

# Under a source that found more than one show by this name and could not tell which is this one.
# @max 36
source-note-ambiguous = Couldn't match this title

# Under a source held back by the VPN check in Settings.
# @max 36
source-note-held-back = Held back by the VPN check

# Heading over one source's releases, for a source that offers several (a batch, single episodes).
# @max 20
source-picker-releases = { -release-cap-pl }

# Accessibility label and badge for the chosen row.
# @max 16
source-picker-chosen = Chosen

# Notice when the source picked for a title is off, removed or failing. Shown once per visit.
# @arg name: text
source-pin-unavailable = { $name } isn't available right now, so this title's episodes come from your other { -source-pl }.

# Under an episode that is out but that no source lists. The row is dimmed and can't be played.
# @max 36
episode-unsourced = No { -source } has this episode
