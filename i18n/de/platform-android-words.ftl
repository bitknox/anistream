episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Episode { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
    [one] Eine Episode pausiert
   *[other] { $count } Episoden pausiert
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = { $size } übrig
downloads-live-progress = { $done } von { $total }
downloads-live-paused-one = Pausiert
downloads-live-waiting = Wartet
settings-seed-ratio-until = Bis Verhältnis { $ratio }
settings-upload-rate = { $size }/s
settings-coming-up-dubbed = { $count ->
    [one] { $names } meldet sich, sobald die Synchronisation da ist.
   *[other] { $names } melden sich, sobald die Synchronisationen da sind.
 }
settings-coming-up-switched-off = { $count ->
    [one] { $names } ist in den eigenen Einstellungen ausgeschaltet.
   *[other] { $names } sind in den eigenen Einstellungen ausgeschaltet.
 }
settings-coming-up-opens = Eine Serie öffnet ihre Einstellungen.
torrents-check-note-warn = { -app-name } prüft beim Start eines Torrents und danach jede Minute, wo der Torrent-Verkehr austritt, und warnt dich, wenn diese Prüfung fehlschlägt. Dein VPN oder dein Proxy schützt dich.
torrents-check-note-stop = { -app-name } prüft beim Start eines Torrents und danach jede Minute, wo der Torrent-Verkehr austritt, und stoppt Torrents, wenn diese Prüfung fehlschlägt. Dein VPN oder dein Proxy schützt dich.
torrents-protect-proxy-name = SOCKS5-Proxy
torrents-protect-vpn-name = VPN auf diesem { -device }
torrents-warn-body = { -app-name } warnt dich, statt zu pausieren. Wenn dein VPN oder Proxy ausgefallen ist, können andere Leute deine IP-Adresse sehen.
torrents-proxy-as = als { $user }
title-facts-aired = Ausgestrahlt { $dates }
title-facts-airing = Läuft seit { $date }
title-facts-airs = Startet am { $date }
media-dates-range = { $from } bis { $to }
title-also-on-more = { $more ->
    [one] { $first } und eine weitere
   *[other] { $first } und { $more } weitere
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } belegt · { $free } frei
downloads-storage-used = { $used } belegt
downloads-time-left = { $time } übrig
downloads-rate = { $size }/s
downloads-uploaded = { $size } hochgeladen
downloads-ratio-of = { $ratio } von { $goal }
downloads-group-summary = { $count ->
    [one] Eine Episode · { $size }
   *[other] { $count } Episoden · { $size }
 }
episode-menu-download = Herunterladen
episode-menu-download-paused = Download pausiert
reason-timeout-anilist = AniList brauchte zu lange für eine Antwort.
reason-timeout-server = Der Server brauchte zu lange für eine Antwort.
reason-busy-anilist = AniList ist ausgelastet. Versuch es in einer Minute erneut.
reason-busy-server = Der Server ist ausgelastet. Versuch es in einer Minute erneut.
reason-problem-anilist = AniList hat ein Problem.
reason-problem-server = Der Server hat ein Problem.
reason-silent-anilist = AniList hat nicht geantwortet.
reason-silent-server = Der Server hat nicht geantwortet.
updates-installing-plain = Wird installiert
updates-closes-as-installs = { -app-name } schließt sich während der Installation.
updates-open-release = Seite öffnen
country-korea = Südkorea
search-genres-picked = { $count ->
    [one] Ein Genre
   *[other] { $count } Genres
 }
search-formats-picked = { $count ->
    [one] Ein Format
   *[other] { $count } Formate
 }
search-statuses-picked = { $count ->
    [one] Ein Status
   *[other] { $count } Status
 }
filter-score-any = Beliebig
filter-years-from = ab { $year }
filter-years-between = { $from } bis { $to }
filter-tag-rank-title = Mindestrang für Tags
filter-tag = Tag
search-filter-by = Nach { $name } filtern
search-loading-genres = Genres werden geladen
search-paused-rate-limit = Die Suche beim Tippen ist kurz pausiert, um die Grenzen von AniList einzuhalten. Drück die Suchtaste, um jetzt zu suchen.
action-clear-all = Alle entfernen
filter-source-material = Basiert auf
search-detail-episode = { $status } · E{ $episode }
sources-use = { $name } verwenden
sources-move = { $name } verschieben
sources-reorder = Neu ordnen
sources-version-available = { $version } verfügbar
sources-update-from = Version { $version } von { $store }.
sources-detail-off = Aus. Sie behält ihren Platz und wird erst wieder genutzt, wenn du sie einschaltest.
sources-fallback-title = { -source-cap }
reason-source-timeout = Die Antwort hat zu lange gedauert.
reason-source-busy = Sie ist ausgelastet. Versuch es in einer Minute erneut.
reason-source-problem = Sie hat ein Problem.
reason-source-turned-away = Sie hat die Anfrage abgewiesen.
reason-source-no-address = Ihre Adresse wurde nicht gefunden.
reason-source-unreachable = Sie war nicht erreichbar.
stores-updates-waiting = { $count ->
    [one] Ein Update wartet
   *[other] { $count } Updates warten
 }
stores-checked-ago = geprüft { $ago }
stores-preview-key-link = Schlüssel passt zum Link
stores-preview-key-invite = Schlüssel passt zur Einladung
stores-key-verified = Geprüft
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = der { -plugin-store }
stores-link-sheet-detail = Öffne diesen Link auf deinem anderen { -device }, um dort dem { -plugin-store } beizutreten. Er funktioniert nur einmal.
accounts-synced-ago = Abgeglichen { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Öffnet Konten
accounts-card-description-signed-out = Nicht angemeldet. Öffnet Konten
accounts-signed-in-plain = Angemeldet
settings-summary-notify-on = Neue Episoden an
settings-summary-notify-off = Neue Episoden aus
settings-summary-sources = { $count ->
    [one] Eine { -source }
   *[other] { $count } { -source-pl }
 }
settings-summary-no-sources = Noch keine { -source-pl }
settings-summary-torrents-on = Torrents an
settings-summary-torrents-off = Torrents aus
library-storage-on-device = { $size } auf diesem { -device }
library-storage-none = Nichts auf diesem { -device }
library-detail-of = E{ $next } von { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Zuletzt gesehen
library-sort-current = Aktuell
settings-notify-permission-note = { -app-name } fragt einmal nach der Berechtigung, wenn du dies zum ersten Mal einschaltest.
settings-notify-refused = Aus: Android erlaubt die Benachrichtigungen von { -app-name } nicht
settings-notify-system-title = Benachrichtigungseinstellungen
settings-notify-system-detail = Ton, Vibration und Sperrbildschirm
settings-titles-line-signed-out = Englisch, solange du nicht bei AniList angemeldet bist
settings-titles-line-account-language = { $language }, aus deinem AniList-Konto
settings-titles-line-follows = Folgt deinem AniList-Konto
settings-titles-line-english = Romaji, wenn es keinen englischen Titel gibt
settings-titles-line-romaji = Japanische Titel in lateinischen Buchstaben
settings-titles-note-account = Folgt deinem AniList-Konto, das auf { $language } eingestellt ist. Solange du bei AniList nicht angemeldet bist, werden Titel auf Englisch angezeigt.
settings-titles-note-follows = Folgt deinem AniList-Konto. Solange du bei AniList nicht angemeldet bist, werden Titel auf Englisch angezeigt.
settings-titles-note-english-missing = { $title } hat bei AniList keinen englischen Titel, deshalb bleibt der Romaji-Titel.
settings-titles-note-english = Englische Titel, wie AniList sie führt, und Romaji, wenn es keinen gibt.
settings-titles-note-romaji = Japanische Titel in lateinischen Buchstaben, wie AniList sie führt.
settings-index-none = Nicht festgelegt
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Nach Möglichkeit auf diesem { -device }: Seeding läuft nur, solange { -app-name } geöffnet ist.
settings-downloads-count = { $count ->
    [one] Eine Episode
   *[other] { $count } Episoden
 }
settings-anilist-footer = Fortschritt und Listenänderungen werden mit AniList abgeglichen. { -library-cap } zeigt deine AniList-Listen.
accounts-sign-in-again-button = Erneut anmelden
accounts-review-differences = { $count ->
    [one] Einen Unterschied prüfen
   *[other] { $count } Unterschiede prüfen
 }
settings-licence-title = Lizenz
settings-licence-value = MIT, Open Source und nicht kommerziell
settings-typefaces-value = Roboto Flex und Zen Kaku Gothic New, unter der SIL Open Font License 1.1
settings-libraries-value = JNA und AndroidX, unter der Apache License 2.0
settings-report-bug-value = Öffnet GitHub im Browser
downloads-storage-share-description = Downloads belegen { $size }
settings-aside-notifications = Eine Benachrichtigung, wenn eine Episode einer Serie erscheint, der du folgst: die Serie und die Nummer der Episode, nie ihr Name. Eine Serie kann ihre eigenen in den Einstellungen abschalten.
settings-aside-downloads = Downloads bleiben im eigenen Bereich von { -app-name } auf diesem { -device }, den andere Apps nicht lesen können. Wenn du die App entfernst, werden sie gelöscht.
settings-aside-connection-check = Verbindungsprüfung
settings-aside-sources = { -source-cap-pl } werden der Reihe nach von oben nach unten probiert. Eine Serie kann ihre eigene { -source } behalten, die auf ihrer Titelseite gewählt wird.
settings-upscaling-description = Dasselbe Standbild: die linke Hälfte im Original 480p, die rechte mit { $mode }
settings-upscaling-source = Original 480p
settings-upscaling-note-off-original = Das Original 480p, auf die Größe skaliert, ohne Verarbeitung.
torrents-sheet-turn-on-title = Torrents einschalten
torrents-sheet-protect-title = Torrent-Verkehr schützen
torrents-sheet-turn-on-button = Einschalten
torrents-sheet-host = Host
torrents-sheet-port = Port
torrents-sheet-proxy-auth-note = Benutzername und Passwort nur, wenn der Proxy sie verlangt.
torrents-sheet-edit-proxy-title = Proxy bearbeiten
title-facts-also-on = Auch auf
title-fetch-failed-detail = AniList hat nicht geantwortet.
continue-resize-queue = Größe der Warteschlange ändern
continue-go-to-title = Zum Titel
continue-short-left = { $episode } · { $left } übrig
continue-airing-out = E{ $episode } · { $time } · erschienen { $ago }
continue-airing-in = E{ $episode } · { $time } · in { $countdown }
calendar-line-out = { $episode } erschienen { $ago }
calendar-line-in = { $episode } in { $countdown }
nav-accounts-and-settings-user = { $user }, Konten und Einstellungen
episode-unaired-label = Episode { $number }, { $title }. Noch nicht erschienen
episode-unaired-label-plain = Episode { $number }. Noch nicht erschienen
episode-unaired-airs-label = Episode { $number }, { $title }. Erscheint { $airs }, noch nicht erschienen
episode-unaired-airs-label-plain = Episode { $number }. Erscheint { $airs }, noch nicht erschienen
tv-sheet-on-network = Im Netzwerk
tv-sheet-not-listed = Nicht gefunden
tv-sheet-scan = Code auf dem Fernseher scannen
tv-sheet-code-on = Code auf { $name }
tv-sheet-pairing-button = Koppeln
tv-sheet-connecting = Verbindung zum Fernseher wird hergestellt
tv-sheet-type-code = Code eingeben
tv-sheet-volume = Lautstärke des Fernsehers
tv-sheet-volume-down = Leiser
tv-sheet-volume-up = Lauter
tv-sheet-volume-value = { $value } von 100
tv-sheet-nothing-playing = Nichts wird abgespielt
tv-sheet-position = Position auf dem Fernseher
tv-sheet-position-state = { $at } von { $total }
tv-sheet-request-titled = { $title }, Episode { $episode }
tv-sheet-request-plain = Episode { $episode }
tv-sheet-footer = Ein Fernseher listet dieses { -device } mit dem Code { $code } auf. Wenn du hier einen Fernseher vergisst, vergisst er auch dieses { -device }.
tv-sheet-title-fallback = Fernseher
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, heute
calendar-day-label-airing = { $day }, Episoden erscheinen
calendar-day-label-today-airing = { $day }, heute, Episoden erscheinen
continue-show-airing-in-calendar = Heute laufende Serien im Kalender zeigen
search-searching-filters = Suche nach { $filters }
episode-number-plain = Episode { $number }
episode-number-title-text = Episode { $number } · { $title }
player-position-state = { $at } von { $total }
player-sheet-finding = { -source-pl } werden gesucht…
player-sheet-none = Für diese Episode wurden keine { -source-pl } gefunden.
player-sheet-episodes-loading = Die Episodenliste ist unterwegs.
player-dual-audio = { -subbed-cap } und { -dubbed }
notif-new-episodes = { $count ->
    [one] Eine neue Episode
   *[other] { $count } neue Episoden
 }
notif-hidden-text = Neue Episode
notif-episode-out = Episode { $episode } ist da.
notif-episode-out-dubbed = Episode { $episode } ist da, { -dubbed }.
notif-episode-out-next = Episode { $episode } ist da. { -up-next-cap } für dich: Episode { $next }.
notif-episode-out-dubbed-next = Episode { $episode } ist da, { -dubbed }. { -up-next-cap } für dich: Episode { $next }.
device-name-phone = Android-Handy
device-name-tablet = Android-Tablet
tv-session-starting = Startet auf { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · noch { $minutes } Min.
widget-line-of = E{ $episode } von { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = Auf dem neuesten Stand
widget-caught-up = Du bist auf dem neuesten Stand.
widget-caught-up-next = { $title } E{ $episode } erscheint { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Episode { $episode } abspielen, { -dubbed }, von { $title }
tv-action-resume = Episode { $episode } fortsetzen
tv-action-resume-dubbed = Episode { $episode } fortsetzen · { -dubbed-cap }
tv-action-out = Episode { $episode } abspielen (ab { $day })
tv-action-not-out = Episode { $episode } abspielen (noch nicht erschienen)
tv-out-today = heute
tv-hero-resume = { $episode } fortsetzen
tv-hero-start = { $episode } abspielen
