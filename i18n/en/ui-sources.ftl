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

## Sources screen and a plugin's page

# Heading of the order list.
sources-order = { -source-cap } order

# Line under the heading.
sources-order-detail = { -source-cap-pl } are tried top to bottom. Drag one by its handle to change the order.

# Line under the heading on a phone, where a show can keep its own.
sources-order-detail-show = { -source-cap-pl } are tried in order, top to bottom. A show can keep its own { -source }, chosen from its title page.

# Accessibility hint of the drag handle.
sources-drag-hint = Drag to change the order

# Menu item that moves a source up.
sources-move-up = Move up

# Menu item that moves a source down.
sources-move-down = Move down

# Title when the list could not be read.
sources-load-failed = Couldn't load { -source-pl }

# Title when no source is installed.
sources-empty-title = No { -source-pl } yet

# Line under it.
sources-empty-detail = { -source-cap-pl } come from { -plugin-store-pl }. Join one with the invite link its owner sends you.

# Row for the built-in torrent source.
sources-torrents = Torrents

# Label of a source that is not a plugin and controls this device.
sources-remote = Remote

# Origin: set in the config file.
sources-origin-config = Set in the config file

# Origin: built into the app.
sources-origin-built-in = Built in

# Origin: from a store the user left.
sources-origin-left-store = From a { -plugin-store } you left

# Origin: added by hand.
sources-origin-by-hand = Added by hand

# Origin: not from a store.
sources-origin-no-store = Not from a { -plugin-store }

# Origin: from a store.
# @arg name: text
sources-origin-from = From { $name }

# Origin: from a store, with the place in the list.
# @arg place: text
sources-origin-in = In { $place }

# Status: the source does not answer.
sources-status-not-answering = Not answering

# Status: held back.
sources-status-held-back = Held back

# Status: slow.
sources-status-slow = Slow

# Status: an update exists.
sources-status-update = Update available

# Status line: no answer, with a reason.
# @arg why: cause
sources-detail-not-answering = Not answering. { $why }

# Status line: no answer.
sources-detail-not-answering-plain = Not answering.

# Status line: held back by the check.
sources-detail-held-back = Held back until the connection check passes.

# Status line: disabled.
sources-detail-disabled = Disabled. It keeps its place and is not tried.

# Status line: the source is gone.
sources-detail-gone = This { -source } is no longer here.

# Status line: the site did not answer.
sources-detail-site = The site did not answer.

# Row label: version.
sources-version = Version

# Row label: response time.
sources-response-time = Response time

# Value: no answer to measure.
sources-no-answer = No answer

# Value: not checked yet.
sources-not-checked = Not checked yet

# Row label: the hosts a plugin contacts.
sources-contacts-label = Contacts

# Value: how the plugin is listed.
sources-listed-by-store = Listed by its { -plugin-store }

# Line: the plugin contacts one host.
# @arg first: text
sources-contacts-one = Contacts { $first }

# Line: the plugin contacts several hosts. The count is the hosts after the first.
# @arg first: text
# @arg more: int
sources-contacts-line = { $more ->
    [one] Contacts { $first } and one more
   *[other] Contacts { $first } and { $more } more
 }

# Line: the plugin contacts no host.
sources-contacts-none = Contacts no hosts

# Value when no host is listed.
sources-no-hosts = No hosts

# Line: an update adds one host.
# @arg first: text
sources-update-contacts-one = This update also contacts { $first }.

# Line: an update adds several hosts. The count is the hosts after the first.
# @arg first: text
# @arg more: int
sources-update-contacts = { $more ->
    [one] This update also contacts { $first } and one more.
   *[other] This update also contacts { $first } and { $more } more.
 }

# Title of the update question.
# @arg name: text
sources-update-title = Update { $name }?

# Title of the replace question.
# @arg name: text
sources-replace-title = Replace { $name }?

# Line under the update question.
# @arg version: text
# @arg store: text
# @arg from: text
sources-update-detail = Version { $version } from { $store } replaces { $from }.

# Line under the replace question.
# @arg version: text
# @arg store: text
# @arg detail: text
sources-replace-detail = Version { $version } from { $store } replaces the copy added by hand. { $detail }.

# Button that opens the settings for sources.
sources-open-settings = Open { -source } settings

# Heading of the torrent group in the settings.
sources-torrent-source = Torrent { -source }

# Row: where the plugin's page stands.
sources-source-order-row = { -source-cap } order

# Note under the facts on the Torrents page in Sources. Torrents are built in, not a plugin.
sources-torrents-note = Torrents are part of { -app-name }, not a { -plugin }. What protects their traffic, the connection check and seeding are set in Settings, under { -source-cap-pl }.
