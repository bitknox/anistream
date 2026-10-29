# Text only the Android phone and tablet apps show on the browse screens: the title page, search,
# Continue, library, calendar, downloads and the frame around them.

# Accessibility label of the account button in the top bar when someone is signed in.
# @only android
# @arg name: text
account-button-named = { $name }, accounts and settings

# Button on a notice that opens the plugin stores, where its cause can be fixed.
# @only android
# @max 24
notice-open-stores = Open { -plugin-store-pl }

# Settings row: whether an episode's AniList discussion can be opened.
# @only android
# @max 40
settings-discussions-label = Episode discussions

# Note under the Episode discussions setting.
# @only android
# @max 120
settings-discussions-note = Reads the comments on AniList's forum thread for episodes you have watched.

# Settings row: whether images in comments load inline.
# @only android
# @max 40
settings-discussion-images-label = Load images in comments

# Note under the Load images in comments setting.
# @only android
# @max 120
settings-discussion-images-note = Each image loads from the site that hosts it, which sees your IP address.

# Word on a comment that was just sent and has not been answered yet.
# @only android
# @max 16
discussion-sending = Sending

# Line in place of the comments while AniList asks the app to wait. The time is a countdown such as 20 s.
# @only android
# @max 40
# @arg time: text
discussion-waiting = Waiting for AniList · { $time }

# Button that unfolds a comment cut at ten lines.
# @only android
# @max 24
discussion-show-more = Show more

# Menu item that copies a comment's text.
# @only android
# @max 24
discussion-copy-text = Copy text

# Line for an image in a comment when images do not load inline. The host is where it lives.
# @only android
# @max 60
# @arg host: text
discussion-image-host = Image · { $host }

# Line for a video in a comment. The host is where it lives.
# @only android
# @max 60
# @arg host: text
discussion-video-host = Video · { $host }
