## The glossary (~/anistream-notes/specs/glossary.md) as Fluent terms. A message never types one of
## these words: it says { -source } or { -source-cap-pl }, and the catalogue lint fails when it
## does. A translation of a term fixes the word everywhere. A form a sentence needs is a term of
## its own: -x-cap (starts a sentence), -x-pl (plural), -x-cap-pl. (Fluent allows a term's
## attributes only as selectors, so they cannot carry the forms.)

# The product name. It is never translated.
-app-name = anistream

# A site or feed anistream reads episodes from. Never "provider".
-source = source
-source-cap = Source
-source-pl = sources
-source-cap-pl = Sources

# A WebAssembly add-on that supplies a source.
-plugin = plugin
-plugin-cap = Plugin
-plugin-pl = plugins
-plugin-cap-pl = Plugins

# The invite-based list plugins are installed from.
-plugin-store = plugin store
-plugin-store-cap = Plugin store
-plugin-store-pl = plugin stores
-plugin-store-cap-pl = Plugin stores

# Shows you have saved, with status.
-library = library
-library-cap = Library

# AniList, Kitsu and similar accounts anistream updates.
-tracker = tracker
-tracker-cap = Tracker
-tracker-pl = trackers
-tracker-cap-pl = Trackers

# Watching in sync with other people. Never "watch together".
-watch-party = watch party
-watch-party-cap = Watch party
-watch-party-pl = watch parties
-watch-party-cap-pl = Watch parties

# Where releases are looked up.
-torrent-index = torrent index
-torrent-index-cap = Torrent index

# One downloadable file in a torrent index. Not a place episodes come from.
-release = release
-release-cap = Release
-release-pl = releases
-release-cap-pl = Releases

# One release that holds many episodes.
-batch = batch
-batch-cap = Batch

# A phone, tablet, TV or computer running anistream.
-device = device
-device-cap = Device
-device-pl = devices
-device-cap-pl = Devices

# The button that resumes the show.
-continue = continue
-continue-cap = Continue

# The episode that plays after this one.
-up-next = up next
-up-next-cap = Up next

# The audio setting's two values.
-subbed = subbed
-subbed-cap = Subbed
-dubbed = dubbed
-dubbed-cap = Dubbed
