# The terminal's frame, its overlays and the notices it raises itself. Platform file: only the
# terminal shows these.

# Header chip while the metadata sources are degraded. The mark is a status glyph. Shown beside
# the { -source } chips, which are short.
# @arg mark: text
# @max 20
chrome-meta-chip = Metadata { $mark }

# Activity row: how many more running jobs there are than fit. It follows the last job shown.
# @arg count: int
# @max 8
chrome-activity-more = +{ $count }

# Activity row: a running job that is held up (a rate-limit pause), with the wait counting down.
# The note says what it waits on; the countdown is a clock such as 0:12.
# @arg note: text
# @arg countdown: text
chrome-job-held = { $note }, { $countdown }

# Heading of an overlay that lists the lines of a long table: which lines are in view.
# @arg from: int
# @arg to: int
# @arg total: int
# @max 24
overlays-window-position = { $from }–{ $to } of { $total }

# Heading of the help overlay when its table does not fit on one page.
# @arg page: int
# @arg pages: int
# @max 24
overlays-help-page = Page { $page } of { $pages }

# The log overlay, when nothing has been logged this session.
overlays-logs-empty = Nothing logged yet

# Prompt hint under the manual search heading. Enter and Esc are the fixed keys of every prompt.
overlays-hint-manual-query = Type what to search for. An empty Enter resets to the automatic match.

# Prompt hint for the first step of signing in to Kitsu.
overlays-hint-kitsu-username = Email or user name. Enter continues, Esc cancels.

# Prompt hint for the second step of signing in to Kitsu. The password is shown as dots.
overlays-hint-kitsu-password = Your password is exchanged for a token and not stored. Enter signs in.

# Prompt hint for the download range. The examples are episode numbers and stay as written.
overlays-hint-download-range = 4 for one episode, 1-12 for a run, 7- for everything from there.

# Prompt hint for editing a setting. The setting is named by its label on the Settings screen.
# @arg setting: text
overlays-hint-edit-setting = { $setting }. Enter saves, Esc leaves it as it is.

# Prompt hint for editing a setting, when the setting is not known.
overlays-hint-edit-setting-bare = Enter saves, Esc leaves it as it is.

# Prompt hint for joining a { -plugin-store }.
overlays-hint-join-store = Paste the invite or { -store } link you were given. Enter checks it, Esc cancels.

# Prompt hint for naming the person an invite link is for.
overlays-hint-invite-name = Their name, as the members list will show it. Enter makes the link.

# Overlay heading after the title: the plugin's version and the { -plugin-store } it comes from.
# The plugin's name leads the heading.
# @arg name: text
# @arg version: text
# @arg store: text
overlays-heading-plugin = { $name }   { $version }  ·  from { $store }

# Overlay heading, after the title "Invite link": whom the link is for.
# @arg name: text
overlays-heading-invite-for = For { $name }

# Title of the invite link overlay when the link is not for a named person.
overlays-title-link-device = Link another { -device }

# Overlay heading, after the title of the store's link: which { -plugin-store } it opens.
# @arg store: text
overlays-heading-open-link-of = { $store }

# Overlay heading, after the title of the publish list: where the plugin goes.
# @arg store: text
overlays-heading-publish-to = To { $store }

# Overlay heading, after the title "Which one": the title being matched.
# @arg name: text
overlays-heading-disambiguate = Match for { $name }. Enter pins it.

# Same, when no title is known.
overlays-heading-disambiguate-bare = Enter pins it.

# Overlay heading, after the title "Watch order": the title the order is built around.
# @arg name: text
overlays-heading-watch-order = Around { $name }. Enter opens.

# Same, when no title is known.
overlays-heading-watch-order-bare = Enter opens.

# Overlay heading, after the title "Recommended": the title the list came from.
# @arg name: text
overlays-heading-recommendations = Because you liked { $name }. Enter opens.

# Same, when no title is known.
overlays-heading-recommendations-bare = Enter opens.

# Overlay heading, after the title of the release list.
overlays-heading-release = Enter pins it. The other { -source-pl } fill what it lacks.

# Overlay heading, after the title of the { -source } list: the choice is kept for this title.
# @arg name: text
overlays-heading-source = { $name }. Enter pins it.

# Same, when no title is known.
overlays-heading-source-bare = Enter pins it.

# Overlay heading, after the title "Filters": the keys of the filter editor.
overlays-heading-filters = ←/→ pick. Enter searches. Esc keeps them.

# Overlay heading, after the title of the releases list: the episode it is for.
# @arg episode: text
overlays-heading-releases = Episode { $episode }. Enter plays your pick.

# Same, when no episode is known.
overlays-heading-releases-bare = Enter plays your pick.

# Toast: a download that is finished cannot be cancelled, only deleted. The key is the delete key.
# @arg delete: text
toast-download-done = Already downloaded. Press { $delete } to delete it and its file.

# Toast: a command needs a title and none is selected.
toast-nothing-selected = Nothing is selected. Pick a title first.

# Toast: the download-range key was pressed away from the episode table.
toast-ranges-elsewhere = Ranges are queued from the episode table.

# Toast: the filter key was pressed on a screen without filters.
toast-filters-elsewhere = Filters live in search and the episode table.

# Toast: a watched-mark key was pressed away from the episode table.
toast-watched-elsewhere = Watched marks live in the episode table.

# Toast: Back was pressed at the top of a section. The key is the quit key.
# @arg quit: text
toast-at-top = You're at the top. Press { $quit } to quit.

# Toast: an action was run that does nothing on this screen. The action is its label, such as
# "Mark all previous watched"; the place says where it works, such as "in the episode table".
# @arg action: text
# @arg place: text
toast-works-elsewhere = { $action } works { $place }.

# Toast: the bug report is being opened.
toast-bug-report-opening = Opening a bug report in your browser. The link is in the log.

# Log line holding the whole bug report link, for a terminal that cannot open it.
# @arg url: text
tui-log-bug-report-link = Bug report: { $url }

# Toast: a key binding in the config file was refused. The problem says which and why.
# @arg problem: text
toast-keybinding-not-applied = Key binding not applied: { $problem }. Check the keys section of the config file.

# Standing notice: the config file did not load. The problem is the parser's own text.
# @arg file: text
# @arg what: text
tui-config-problem = { $file }: { $what }. Using the defaults until it is fixed.

# Same, when the problem is on a known line.
# @arg file: text
# @arg line: int
# @arg what: text
tui-config-problem-at-line = { $file } line { $line }: { $what }. Using the defaults until it is fixed.

# Log line for the same problem. The path is the whole path of the config file.
# @arg path: text
# @arg problem: text
tui-config-problem-logged = { $path }: { $problem }. Using the defaults.

# The status line at the foot of the screen when nothing is being said: the source in use, the audio and the quality.
# @arg source: text
# @arg audio: text
# @arg quality: int
chrome-status-state = { $source } · { $audio } · { $quality }p
