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
sources-detail-disabled = Off. It keeps its place and isn't tried.

# Status line: the source is gone.
sources-detail-gone = This { -source } is no longer here.

# Status line: the site did not answer.
sources-detail-site = The { -source } didn't answer.

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
sources-contacts-none = Lists no hosts to contact

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

# Over the { -source } list while the { -source-pl } are set up again after a change.
sources-rebuilding = { -source-cap-pl } are being set up again


# Value of a plugin's response time, in milliseconds.
# @arg ms: int
sources-response-ms = { $ms } ms

## Fixing a source match: the sheet that chooses which of a source's titles a show is, from the
## title page when the source couldn't choose, or from "Fix this match" when its choice is wrong.

# Menu, palette and source picker entry that opens the sheet.
# @max 28
match-fix = Fix this match

# The title page's main action while the source couldn't choose, and the sheet's title.
# @max 24
match-choose = Choose the match

# The line on the title page while the match is open, under its main action. The count leaves out
# titles that were ruled out.
# @arg count: int
# @arg source: text
match-uncertain = { $count ->
    [one] { $source } lists a title that could be this one. Choose it to play its episodes.
   *[other] { $source } lists { $count } titles that could be this one. Choose one to play its episodes.
 }

# The line on a title page when the source listed no title that fits, so the person searches.
# @arg source: text
match-uncertain-none = { $source } has no title that fits this one. Choose one anyway, or search.

# The last row of the source picker, which opens the match sheet.
# @max 36
match-source-picker-row = Wrong title? Fix this match

# Under the show's name in the sheet: what AniList says of it, such as "TV · 2026 · 24 episodes".
# @arg facts: text
match-anilist-facts = On AniList: { $facts }

# Heading over the titles a source offers.
# @arg source: text
match-section-source = On { $source }

# Heading over the title the show is matched to now.
# @max 28
match-now = Matched now

# Beside the title matched now: the person chose it.
# @max 24
match-chosen-by-you = Chosen by you

# Beside the title matched now: anistream chose it.
# @max 28
match-chosen-automatically = Chosen automatically

# How closely a source's title matches the show, at the end of its row.
# @arg similarity: percent
# @max 14
match-similarity = { $similarity } match

# Placeholder of the search field in the sheet.
# @arg source: text
match-search-field = Search { $source }

# A TV row that opens the search screen from the sheet's panel.
# @only android
# @max 32
match-search-another = Search for another title

# While a search of the source runs.
# @arg source: text
# @arg query: text
match-searching = Searching { $source } for “{ $query }”

# A search of the source that found nothing.
# @arg source: text
# @arg query: text
match-search-empty = Nothing on { $source } for “{ $query }”.

# Under match-search-empty: what to try.
match-search-empty-hint = Try fewer words, or the title in another language.

# A search of the source that failed. The reason follows on its own line.
# @arg source: text
match-search-failed = Couldn't search { $source }.

# On the row picked, while the source lists its episodes.
# @max 16
match-picking = Matching

# A pick that failed. The reason follows on its own line.
match-fix-failed = Couldn't use that match.

# Notice once a picked title's episodes arrive.
# @arg title: text
# @arg source: text
match-picked = Matched to { $title } on { $source }.

# Button that forgets the person's choice, shown only while the match is one they chose.
# @max 28
match-automatic = Match automatically

# Under "Match automatically", on iPhone, iPad and Android.
match-automatic-detail = Forget your choice and let { -app-name } match this title again.

# Notice after "Match automatically".
match-cleared = Matching automatically again.

# Screen reader label for one row of the sheet. The closeness is match-similarity or a reason the
# title was ruled out.
# @arg title: text
# @arg facts: text
# @arg closeness: text
match-row-label = { $title }, { $facts }, { $closeness }

# Name of the built-in demo source in every list of sources.
sources-demo-name = Demo

# Line under the demo source in Your sources: the work, its licence and where it streams from.
sources-demo-line = こうしす！ · CC BY 4.0 · from Wikimedia Commons

# Title of the page that describes the demo.
sources-demo-about = About the demo

# The credit block shown wherever the demo work shows as a title. The licence asks for this wording.
sources-demo-credit = こうしす！ by OPAP-JP contributors. Licensed CC BY 4.0. Streamed from Wikimedia Commons. Sponsor logos in the episodes are not covered by the licence.

# Link to the licence text, in the credit block.
sources-demo-link-licence = Licence

# Link to the work's file page on Wikimedia Commons, in the credit block.
sources-demo-link-commons = Wikimedia Commons

# Link to the makers' site, in the credit block.
sources-demo-link-maker = OPAP-JP

# Line on the demo page that names the only two hosts the demo reaches.
sources-demo-hosts = Reaches Wikimedia Commons only: upload.wikimedia.org and commons.wikimedia.org.

# Line on the demo page about accounts and trackers.
sources-demo-private = The demo never signs in anywhere and never writes to AniList.

# Button on the demo page that removes the demo from the sources.
# @max 24
sources-demo-remove = Remove demo

# Button that adds the demo back to the sources.
# @max 24
sources-demo-add = Add the demo

# Closing line of Home while the demo is the only source. Plugin stores is a link.
sources-demo-only = Only the demo is on. Join a { -plugin-store } to play from your own { -source-pl }.

# Line under the search field while the demo is the only source.
search-demo-only = Only the demo is on. Other titles play once you join a { -plugin-store }.

# Line under the demo title when it is a search result.
search-demo-plays = Demo · Plays now

# Main button on the demo title. The part is a number.
# @arg part: text
# @max 24
title-play-part = Play part { $part }

# Line on the player's starting screen for a part of the demo.
# @arg part: text
player-starting-part = Starting part { $part }

# Credit line under the starting line on the player's starting screen for the demo.
player-demo-credit = こうしす！ by OPAP-JP contributors · CC BY 4.0
