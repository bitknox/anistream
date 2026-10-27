# Text only the Android apps show, built in the store models and shared by the phone, the TV, the
# widgets and the notifications: short times, the Continue row, the download notification, the
# settings values, the torrent notes and the facts on a title.

# A countdown that has run out.
# @only android
# @max 12
time-now = now

# How long ago, when it was under a minute.
# @only android
# @max 16
time-just-now = just now

# The episode in a short form for a title last watched dubbed.
# @only android
# @arg episode: int
# @max 24
episode-short-dubbed = E{ $episode } · { -dubbed-cap }

# The episode in its long form for a title last watched dubbed.
# @only android
# @arg episode: int
# @max 32
episode-long-dubbed = Episode { $episode } · { -dubbed-cap }

# Continue row: a new episode, watched dubbed, with how long ago it came out.
# @only android
# @arg episode: int
# @arg ago: text
continue-row-start-aired-dubbed = Play E{ $episode } { -dubbed } · out { $ago }

# Title page button: continue a dubbed episode part-way through.
# @only android
# @arg episode: int
# @arg time: text
title-button-resume-dubbed = Resume episode { $episode } · { -dubbed-cap } · { $time }

# Title page button: play the next dubbed episode.
# @only android
# @arg episode: int
title-button-play-dubbed = Play episode { $episode } · { -dubbed-cap }

# Download notification title when several episodes are paused.
# @only android
# @arg count: int
# @max 40
downloads-live-paused = { $count ->
    [one] One episode paused
   *[other] { $count } episodes paused
 }

# Download notification, several episodes: the one that runs, by show and number.
# @only android
# @arg title: text
# @arg episode: int
downloads-live-lead = { $title } E{ $episode }

# Download notification, several episodes: how much is left. The size is already formatted.
# @only android
# @arg size: text
downloads-live-left = { $size } left

# Download notification, one episode: how much is down. Both sizes are already formatted.
# @only android
# @arg done: text
# @arg total: text
downloads-live-progress = { $done } of { $total }

# Download notification, one episode: the download is paused.
# @only android
# @max 16
downloads-live-paused-one = Paused

# Download notification, one episode: it waits for a free slot.
# @only android
# @max 16
downloads-live-waiting = Waiting

# Setting value: the upscaler tuned for natural-looking pictures. ArtCNN is the model's name.
# @only android
# @max 28
settings-upscaling-natural = Natural · ArtCNN

# Setting value: the upscaler tuned for sharp lines. Anime4K is the shader's name.
# @only android
# @max 28
settings-upscaling-sharp = Sharp · Anime4K

# Setting value: seed until the upload ratio reaches a number such as 1.5.
# @only android
# @arg ratio: text
settings-seed-ratio-until = Until ratio { $ratio }

# Setting value: no upload limit.
# @only android
# @max 16
settings-upload-unlimited = Unlimited

# Setting value: an upload limit, in size per second. The size is already formatted.
# @only android
# @arg size: text
settings-upload-rate = { $size }/s

# Setting value: the standard download quality.
# @only android
# @max 16
settings-quality-standard = Standard

# Setting value: the higher download quality.
# @only android
# @max 16
settings-quality-higher = Higher

# Footer under the week's notifications: shows watched dubbed, which notify when the dub is out.
# The names are a list of titles.
# @only android
# @arg names: text
# @arg count: int
settings-coming-up-dubbed = { $count ->
    [one] { $names } notifies when its dub is out.
   *[other] { $names } notify when their dubs are out.
 }

# Footer under the week's notifications: shows with their own notification switch off.
# The names are a list of titles.
# @only android
# @arg names: text
# @arg count: int
settings-coming-up-switched-off = { $count ->
    [one] { $names } is off in its own settings.
   *[other] { $names } are off in their own settings.
 }

# Footer under the week's notifications: what a row does.
# @only android
settings-coming-up-opens = A show opens its settings.

# What the connection check does, said wherever torrents are on.
# @only android
torrents-check-note = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.

# The same note when a failed check only warns.
# @only android
torrents-check-note-warn = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and warns you if that check fails. Your VPN or proxy is what protects you.

# The same note when a failed check stops torrents.
# @only android
torrents-check-note-stop = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and stops torrents if that check fails. Your VPN or proxy is what protects you.

# Notice after the torrent protection was saved.
# @only android
torrents-protection-saved = Torrent protection saved

# Name of the proxy protection.
# @only android
# @max 28
torrents-protect-proxy-name = SOCKS5 proxy

# Name of the VPN protection.
# @only android
# @max 28
torrents-protect-vpn-name = VPN on this { -device }

# Body of the question asked before a failed check only warns.
# @only android
torrents-warn-body = { -app-name } will warn you instead of pausing. If your VPN or proxy is down, other people can see your IP address.

# Part of a proxy's line: the user it lets in.
# @only android
# @arg user: text
torrents-proxy-as = as { $user }

# Title facts: a finished title and when it aired. The dates are already formatted.
# @only android
# @arg dates: text
title-facts-aired = Aired { $dates }

# Title facts: a title that is airing, and since when.
# @only android
# @arg date: text
title-facts-airing = Airing since { $date }

# Title facts: a title that has not started, and when it does.
# @only android
# @arg date: text
title-facts-airs = Airs from { $date }

# Two dates as a range, such as the first and last broadcast.
# @only android
# @arg from: text
# @arg to: text
media-dates-range = { $from } to { $to }

# Title facts: where else the title is, by the first place and how many more.
# @only android
# @arg first: text
# @arg more: int
title-also-on-more = { $more ->
    [one] { $first } and one more
   *[other] { $first } and { $more } more
 }

# Mark on a title meant for adults.
# @only android
# @max 8
title-adult-mark = 18+
