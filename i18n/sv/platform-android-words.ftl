episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Avsnitt { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
    [one] Ett avsnitt pausat
   *[other] { $count } avsnitt pausade
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = { $size } kvar
downloads-live-progress = { $done } av { $total }
downloads-live-paused-one = Pausad
downloads-live-waiting = Väntar
settings-seed-ratio-until = Till ratio { $ratio }
settings-upload-rate = { $size }/s
settings-coming-up-dubbed = { $count ->
    [one] { $names } meddelar när dubbningen är klar.
   *[other] { $names } meddelar när dubbningarna är klara.
 }
settings-coming-up-switched-off = { $count ->
    [one] { $names } är avstängd i sina egna inställningar.
   *[other] { $names } är avstängda i sina egna inställningar.
 }
settings-coming-up-opens = En serie öppnar sina inställningar.
torrents-check-note-warn = { -app-name } kontrollerar var torrenttrafiken lämnar när en torrent startar och varje minut medan den körs. Om kontrollen misslyckas varnar appen dig. Din VPN eller proxy är det som skyddar dig.
torrents-check-note-stop = { -app-name } kontrollerar var torrenttrafiken lämnar när en torrent startar och varje minut medan den körs. Om kontrollen misslyckas stoppas torrenterna. Din VPN eller proxy är det som skyddar dig.
torrents-protect-proxy-name = SOCKS5-proxy
torrents-protect-vpn-name = VPN på den här { -device }en
torrents-warn-body = { -app-name } varnar dig i stället för att pausa. Om din VPN eller proxy är nere kan andra se din IP-adress.
torrents-proxy-as = som { $user }
title-facts-aired = Sändes { $dates }
title-facts-airing = Sänds sedan { $date }
title-facts-airs = Sänds från { $date }
media-dates-range = { $from } till { $to }
title-also-on-more = { $more ->
    [one] { $first } och en till
   *[other] { $first } och { $more } till
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } använt · { $free } ledigt
downloads-storage-used = { $used } använt
downloads-time-left = { $time } kvar
downloads-rate = { $size }/s
downloads-uploaded = { $size } uppladdat
downloads-ratio-of = { $ratio } av { $goal }
downloads-group-summary = { $count ->
    [one] Ett avsnitt · { $size }
   *[other] { $count } avsnitt · { $size }
 }
episode-menu-download = Ladda ner
episode-menu-download-paused = Nedladdning pausad
reason-timeout-anilist = AniList svarade inte i tid.
reason-timeout-server = Servern svarade inte i tid.
reason-busy-anilist = AniList är upptaget. Försök igen om en minut.
reason-busy-server = Servern är upptagen. Försök igen om en minut.
reason-problem-anilist = AniList hade ett problem.
reason-problem-server = Servern hade ett problem.
reason-silent-anilist = AniList svarade inte.
reason-silent-server = Servern svarade inte.
updates-installing-plain = Installerar
updates-closes-as-installs = { -app-name } stängs under installationen.
updates-open-release = Öppna sidan
country-korea = Sydkorea
search-genres-picked = { $count ->
    [one] En genre
   *[other] { $count } genrer
 }
search-formats-picked = { $count ->
    [one] Ett format
   *[other] { $count } format
 }
search-statuses-picked = { $count ->
    [one] En status
   *[other] { $count } statusar
 }
filter-score-any = Valfri
filter-years-from = { $year } och framåt
filter-years-between = { $from } till { $to }
filter-tag-rank-title = Lägsta taggranking
filter-tag = Tagg
search-filter-by = Filtrera på { $name }
search-loading-genres = Hämtar genrer
search-paused-rate-limit = Sökning medan du skriver är pausad en stund för att hålla sig inom AniLists gränser. Tryck på sökknappen för att söka nu.
action-clear-all = Rensa alla
filter-source-material = Baserad på
search-detail-episode = { $status } · E{ $episode }
sources-use = Använd { $name }
sources-move = Flytta { $name }
sources-reorder = Ändra ordning
sources-version-available = { $version } finns
sources-update-from = Version { $version } från { $store }.
sources-detail-off = Av. Den behåller sin plats och testas inte förrän den slås på igen.
sources-fallback-title = { -source-cap }
reason-source-timeout = Källan svarade inte i tid.
reason-source-busy = Källan är upptagen. Försök igen om en minut.
reason-source-problem = Källan hade ett problem.
reason-source-turned-away = Källan avvisade begäran.
reason-source-no-address = Källans adress hittades inte.
reason-source-unreachable = Det gick inte att nå källan.
stores-updates-waiting = { $count ->
    [one] En uppdatering
   *[other] { $count } uppdateringar
 }
stores-checked-ago = kontrollerades { $ago }
stores-preview-key-link = Nyckeln stämmer med länken
stores-preview-key-invite = Nyckeln stämmer med inbjudan
stores-key-verified = Verifierad
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = { -plugin-store }en
stores-link-sheet-detail = Öppna den här länken på din andra { -device } för att gå med i { -plugin-store }en där. Den fungerar en gång.
accounts-synced-ago = Synkad { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Öppnar Konton
accounts-card-description-signed-out = Inte inloggad. Öppnar Konton
accounts-signed-in-plain = Inloggad
settings-summary-notify-on = Nya avsnitt på
settings-summary-notify-off = Nya avsnitt av
settings-summary-sources = { $count ->
    [one] En { -source }
   *[other] { $count } { -source-pl }
 }
settings-summary-no-sources = Inga { -source-pl } än
settings-summary-torrents-on = torrenter på
settings-summary-torrents-off = torrenter av
library-storage-on-device = { $size } på den här { -device }en
library-storage-none = Inget på den här { -device }en
library-detail-of = E{ $next } av { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Senast tittad
library-sort-current = Aktuell
settings-notify-permission-note = { -app-name } frågar efter tillstånd första gången du slår på det här.
settings-notify-refused = Av: Android tillåter inte { -app-name }s aviseringar
settings-notify-system-title = Aviseringsinställningar
settings-notify-system-detail = Ljud, vibration och låsskärm
settings-titles-line-signed-out = Engelska när du är utloggad från AniList
settings-titles-line-account-language = { $language }, från ditt AniList-konto
settings-titles-line-follows = Följer ditt AniList-konto
settings-titles-line-english = Romaji där det inte finns någon engelsk titel
settings-titles-line-romaji = Japanska titlar med latinska bokstäver
settings-titles-note-account = Följer ditt AniList-konto, som är satt till { $language }. Medan du är utloggad från AniList visas titlar på engelska.
settings-titles-note-follows = Följer ditt AniList-konto. Medan du är utloggad från AniList visas titlar på engelska.
settings-titles-note-english-missing = { $title } har ingen engelsk titel på AniList, så den behåller sin romaji-titel.
settings-titles-note-english = Engelska titlar som AniList anger, och romaji där det inte finns någon.
settings-titles-note-romaji = Japanska titlar skrivna med latinska bokstäver, som AniList anger dem.
settings-index-none = Inte angiven
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Bästa möjliga på den här { -device }en: seeding körs bara medan { -app-name } är öppen.
settings-downloads-count = { $count ->
    [one] Ett avsnitt
   *[other] { $count } avsnitt
 }
settings-anilist-footer = Framsteg och liständringar synkas till AniList. { -library-cap } visar dina AniList-listor.
accounts-sign-in-again-button = Logga in igen
accounts-review-differences = { $count ->
    [one] Granska en skillnad
   *[other] Granska { $count } skillnader
 }
settings-licence-title = Licens
settings-licence-value = MIT, öppen källkod och icke-kommersiell
settings-typefaces-value = Roboto Flex och Zen Kaku Gothic New, under SIL Open Font License 1.1
settings-libraries-value = JNA och AndroidX, under Apache License 2.0
settings-report-bug-value = Öppnar GitHub i din webbläsare
downloads-storage-share-description = Nedladdningarna tar { $size }
settings-aside-notifications = En avisering när ett avsnitt av en serie du följer kommer ut, med serien och avsnittsnumret men aldrig avsnittets titel. En serie kan stänga av sina egna i sina inställningar.
settings-aside-downloads = Nedladdningar ligger i { -app-name }s eget utrymme på den här { -device }en, som andra appar inte kan läsa. Tar du bort appen tas de bort.
settings-aside-connection-check = Anslutningskontroll
settings-aside-sources = { -source-cap-pl } testas i ordning, uppifrån och ned. En serie kan ha sin egen { -source }, vald från dess titelsida.
settings-upscaling-description = Samma bild: vänstra halvan är originalet i 480p, högra halvan med { $mode }
settings-upscaling-source = Original 480p
settings-upscaling-note-off-original = Originalet i 480p, skalat för att passa utan bearbetning.
torrents-sheet-turn-on-title = Slå på torrenter
torrents-sheet-protect-title = Skydda torrenttrafik
torrents-sheet-turn-on-button = Slå på
torrents-sheet-host = Värd
torrents-sheet-port = Port
torrents-sheet-proxy-auth-note = Användarnamn och lösenord endast om proxyn kräver dem.
torrents-sheet-edit-proxy-title = Redigera proxy
title-facts-also-on = Finns också på
title-fetch-failed-detail = AniList svarade inte.
continue-resize-queue = Ändra storlek på kön
continue-go-to-title = Gå till titel
continue-short-left = { $episode } · { $left }
continue-airing-out = E{ $episode } · { $time } · ute { $ago } sedan
continue-airing-in = E{ $episode } · { $time } · om { $countdown }
calendar-line-out = { $episode } ute { $ago } sedan
calendar-line-in = { $episode } om { $countdown }
nav-accounts-and-settings-user = { $user }, konton och inställningar
episode-unaired-label = Avsnitt { $number }, { $title }. Inte ute än
episode-unaired-label-plain = Avsnitt { $number }. Inte ute än
episode-unaired-airs-label = Avsnitt { $number }, { $title }. Sänds { $airs }, inte ute än
episode-unaired-airs-label-plain = Avsnitt { $number }. Sänds { $airs }, inte ute än
tv-sheet-on-network = På nätverket
tv-sheet-not-listed = Visas inte
tv-sheet-scan = Skanna koden på TV:n
tv-sheet-code-on = Kod på { $name }
tv-sheet-pairing-button = Parkopplar
tv-sheet-connecting = Ansluter till TV:n
tv-sheet-type-code = Skriv koden
tv-sheet-volume = TV:ns volym
tv-sheet-volume-down = Sänk volymen
tv-sheet-volume-up = Höj volymen
tv-sheet-volume-value = { $value } av 100
tv-sheet-nothing-playing = Spelar inget
tv-sheet-position = Position på TV:n
tv-sheet-position-state = { $at } av { $total }
tv-sheet-request-titled = { $title }, avsnitt { $episode }
tv-sheet-request-plain = Avsnitt { $episode }
tv-sheet-footer = En TV listar den här { -device }en med koden { $code }. Tar du bort en TV här glömmer den också den här { -device }en.
tv-sheet-title-fallback = TV
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, idag
calendar-day-label-airing = { $day }, avsnitt sänds
calendar-day-label-today-airing = { $day }, idag, avsnitt sänds
continue-show-airing-in-calendar = Serier som sänds idag i Kalender
continue-show-season-in-seasonal = Serier den här säsongen i Säsong
search-searching-filters = Söker efter { $filters }
episode-number-plain = Avsnitt { $number }
episode-number-title-text = Avsnitt { $number } · { $title }
player-position-state = { $at } av { $total }
player-sheet-finding = Hittar { -source-pl }…
player-sheet-none = Inga { -source-pl } hittades för det här avsnittet.
player-sheet-episodes-loading = Avsnittslistan är på väg.
player-dual-audio = { -subbed-cap } och { -dubbed }
notif-new-episodes = { $count ->
    [one] Ett nytt avsnitt
   *[other] { $count } nya avsnitt
 }
notif-hidden-text = Nytt avsnitt
notif-episode-out = Avsnitt { $episode } är ute.
notif-episode-out-dubbed = Avsnitt { $episode } är ute, { -dubbed }.
notif-episode-out-next = Avsnitt { $episode } är ute. { -up-next } för dig: avsnitt { $next }.
notif-episode-out-dubbed-next = Avsnitt { $episode } är ute, { -dubbed }. { -up-next } för dig: avsnitt { $next }.
device-name-phone = Android-telefon
device-name-tablet = Android-surfplatta
tv-session-starting = Startar på { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · { $minutes } min kvar
widget-line-of = E{ $episode } av { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = Ikapp
widget-caught-up = Du är ikapp.
widget-caught-up-next = { $title } E{ $episode } sänds { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Spela avsnitt { $episode }, { -dubbed }, av { $title }
tv-action-resume = Fortsätt avsnitt { $episode }
tv-action-resume-dubbed = Fortsätt avsnitt { $episode } · { -dubbed-cap }
tv-action-out = Spela avsnitt { $episode } (ute { $day })
tv-action-not-out = Spela avsnitt { $episode } (inte ute än)
tv-out-today = idag
tv-hero-resume = Fortsätt { $episode }
tv-hero-start = Spela { $episode }
