# Episode discussions: AniList's forum thread for an episode, in a sheet, a pane or a window.

# Menu item that opens an episode's discussion.
# @max 24
discussion-open = Discussion

# Title of the discussion window or sheet when the thread has no title yet.
# @arg episode: int
# @max 40
discussion-title-episode = Episode { $episode } discussion

# Quiet line under an episode that opens its discussion.
# @arg count: int
# @max 40
discussion-row = { $count ->
    [one] Discussion · one comment
   *[other] Discussion · { $count } comments
 }

# Count of comments in the header of the discussion.
# @arg count: int
# @max 24
discussion-count = { $count ->
    [one] One comment
   *[other] { $count } comments
 }

# Button that likes a comment, with its count after it. Also its accessible name.
# @max 16
discussion-like = Like

# Button that replies to a comment.
# @max 16
discussion-reply = Reply

# Line under a reply that has more replies folded in it. The line opens them.
# @arg count: int
# @max 28
discussion-more-replies = { $count ->
    [one] One more reply
   *[other] { $count } more replies
 }

# Muted bar in place of a spoiler in a comment, until it is opened.
# @max 16
discussion-spoiler = Spoiler

# Line in place of an image in a comment. It opens the image in the browser.
# @max 16
discussion-image = Image

# Line in place of a video in a comment. It opens the video in the browser.
# @max 16
discussion-video = Video

# Placeholder in the field for a new comment.
# @max 32
discussion-add-comment = Add a comment

# Placeholder in the field while editing a comment.
# @max 32
discussion-edit-comment = Edit your comment

# Button that sends the comment in the field.
# @max 16
discussion-post = Post

# Line above the field while answering a comment.
# @arg name: text
# @max 40
discussion-replying-to = Replying to { $name }

# Accessible name of the button that stops replying or editing.
# @max 24
discussion-cancel-reply = Cancel reply

# Line in place of the field when nobody is signed in to AniList. It opens the sign-in.
# @max 40
discussion-sign-in = Sign in with AniList to reply

# Line in place of the field when the viewer is signed in but offline.
# @max 48
discussion-offline-post = Posting waits for the network

# Line under a comment AniList did not accept. The button beside it sends it again.
# @max 32
discussion-post-failed = Didn't post. Try again

# Title of the question asked before deleting the viewer's own comment.
# @max 32
discussion-delete-title = Delete this comment?

# Body of the question asked before deleting the viewer's own comment.
# @max 60
discussion-delete-body = It is removed from AniList too.

# Line while the comments are still loading after a few seconds.
# @max 40
discussion-still-loading = Still loading from AniList

# Line while AniList's request budget is spent. The engine waits, then goes on by itself.
# @arg time: text
# @max 40
discussion-rate-limited = Waiting for AniList · { $time }

# Title of the state when the episode has no thread on AniList.
# @max 36
discussion-no-thread = No discussion yet

# Line under the no-thread title.
# @max 80
discussion-no-thread-detail = Nobody has opened a thread for this episode on AniList.

# Title of the state when a thread has no comments.
# @max 28
discussion-no-comments = No comments yet

# Title of the state when the discussion could not be read.
# @max 40
discussion-failed = Couldn't load the discussion

# Line over comments that are kept from an earlier visit while offline. The age reads "2h ago".
# @arg age: text
# @max 56
discussion-offline-cached = Offline. Showing comments from { $age }.

# Heading of the Settings group for discussion rows.
# @max 24
discussion-settings-group = Discussions

# Age of a comment that is on its way to AniList.
# @max 16
discussion-sending = Sending

# Button under a comment that is cut off after ten lines.
# @max 24
discussion-show-more = Show more

# Comment menu item that copies the comment's text.
# @max 24
discussion-copy-text = Copy text

# Line for a picture or video in a comment: what it is and the site it is on.
# @arg kind: text
# @arg host: text
# @only linux
# @max 40
discussion-media-line = { $kind } · { $host }
