## Apple: the discussion's own words, beside the shared ones in ui-discussion.ftl.

# Line under the header while AniList's request budget is spent. The placeholders stay.
# @arg time: text
# @max 40
discussion-apple-waiting = Waiting for AniList · { $time }

# Line in place of an image in a comment: the word, then the site that hosts it.
# @arg host: text
# @max 40
discussion-apple-image-host = Image · { $host }

# Line in place of a video in a comment: the word, then the site that hosts it.
# @arg host: text
# @max 40
discussion-apple-video-host = Video · { $host }

# Menu item that copies a comment's text.
# @max 24
discussion-apple-copy-text = Copy text

# Age of a comment that was sent and is not yet confirmed.
# @max 16
discussion-apple-sending = Sending

# Line above the field while the viewer edits their own comment.
# @max 40
discussion-apple-editing = Editing your comment

# Accessible name of the button that opens a comment's menu.
# @max 40
discussion-apple-comment-menu = More for this comment

# After the author's name on a reply to a reply, naming who it answers.
# @arg name: text
# @max 32
discussion-apple-answering = to { $name }

# Title of the state when the episode has no thread. Open on AniList goes to the show's page.
# @max 40
discussion-apple-no-thread = No discussion for this episode

# Title of the state when the device is offline and no comments were kept.
# @max 28
discussion-apple-offline = You're offline

# Menu item in the comment field's edit menu that hides the selected words as a spoiler.
# @max 16
discussion-apple-mark-spoiler = Spoiler

# Accessible name of a hidden spoiler that a tap shows.
# @max 32
discussion-apple-spoiler-hint = Shows the spoiler

# Accessible name of the button that closes the discussion.
# @max 24
discussion-apple-close = Close discussion

# Accessible name of a comment's heart when the viewer likes it.
# @max 24
discussion-apple-liked = Liked

# Row in the player's More menu that opens the episode's discussion, with the count and a note.
# @arg count: int
# @max 40
discussion-apple-player-note = { $count ->
    [one] One comment · pauses
   *[other] { $count } comments · pauses
 }

# Name of the Settings switch for episode discussions.
# @max 32
settings-apple-discussions = Episode discussions

# Note under the switch for episode discussions.
# @max 100
settings-apple-discussions-note = Reads the comments on AniList's forum thread for episodes you have watched.

# Name of the Settings switch that loads images in comments.
# @max 32
settings-apple-comment-images = Load images in comments

# Note under the switch that loads images in comments.
# @max 100
settings-apple-comment-images-note = Each image loads from the site that hosts it, which sees your IP address.

# Line under the comment field on the Mac: the keys that post and add a line.
# @max 48
discussion-apple-key-hint = ⌘Return posts. Return adds a line.

# Note beside Discussion in the player's menu when the comments are not counted yet.
# @max 32
discussion-apple-pauses = Pauses the video
