episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Episodio { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
    [one] Un episodio in pausa
    [many] { $count } episodi in pausa
   *[other] { $count } episodi in pausa
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = { $size } rimanenti
downloads-live-progress = { $done } di { $total }
downloads-live-paused-one = In pausa
downloads-live-waiting = In attesa
settings-seed-ratio-until = Fino al rapporto { $ratio }
settings-upload-rate = { $size }/s
settings-coming-up-dubbed = { $count ->
    [one] { $names } avvisa quando esce il doppiaggio.
    [many] { $names } avvisano quando escono i doppiaggi.
   *[other] { $names } avvisano quando escono i doppiaggi.
 }
settings-coming-up-switched-off = { $count ->
    [one] { $names } è disattivato nelle sue impostazioni.
    [many] { $names } sono disattivati nelle loro impostazioni.
   *[other] { $names } sono disattivati nelle loro impostazioni.
 }
settings-coming-up-opens = Una serie apre le sue impostazioni.
torrents-check-note-warn = { -app-name } controlla da dove esce il traffico dei torrent quando un torrent parte e ogni minuto mentre è in corso, e ti avvisa se il controllo non riesce. La VPN o il proxy è ciò che ti protegge.
torrents-check-note-stop = { -app-name } controlla da dove esce il traffico dei torrent quando un torrent parte e ogni minuto mentre è in corso, e interrompe i torrent se il controllo non riesce. La VPN o il proxy è ciò che ti protegge.
torrents-protect-proxy-name = Proxy SOCKS5
torrents-protect-vpn-name = VPN su questo { -device }
torrents-warn-body = { -app-name } ti avviserà invece di mettere in pausa. Se la VPN o il proxy non funziona, altre persone possono vedere il tuo indirizzo IP.
torrents-proxy-as = come { $user }
title-facts-aired = Trasmesso { $dates }
title-facts-airing = In onda dal { $date }
title-facts-airs = Inizia il { $date }
media-dates-range = Dal { $from } al { $to }
title-also-on-more = { $more ->
    [one] { $first } e un altro
    [many] { $first } e altri { $more }
   *[other] { $first } e altri { $more }
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } usati · { $free } liberi
downloads-storage-used = { $used } usati
downloads-time-left = { $time } rimanenti
downloads-rate = { $size }/s
downloads-uploaded = { $size } inviati
downloads-ratio-of = { $ratio } su { $goal }
downloads-group-summary = { $count ->
    [one] Un episodio · { $size }
    [many] { $count } episodi · { $size }
   *[other] { $count } episodi · { $size }
 }
episode-menu-download = Scarica
episode-menu-download-paused = Scaricamento in pausa
reason-timeout-anilist = AniList ha impiegato troppo tempo a rispondere.
reason-timeout-server = Il server ha impiegato troppo tempo a rispondere.
reason-busy-anilist = AniList è occupato. Riprova tra un minuto.
reason-busy-server = Il server è occupato. Riprova tra un minuto.
reason-problem-anilist = AniList ha avuto un problema.
reason-problem-server = Il server ha avuto un problema.
reason-silent-anilist = AniList non ha risposto.
reason-silent-server = Il server non ha risposto.
updates-installing-plain = Installazione in corso
updates-closes-as-installs = { -app-name } si chiude durante l'installazione
updates-open-release = Apri pagina
country-korea = Corea del Sud
search-genres-picked = { $count ->
    [one] Un genere
    [many] { $count } generi
   *[other] { $count } generi
 }
search-formats-picked = { $count ->
    [one] Un formato
    [many] { $count } formati
   *[other] { $count } formati
 }
search-statuses-picked = { $count ->
    [one] Uno stato
    [many] { $count } stati
   *[other] { $count } stati
 }
filter-score-any = Qualsiasi
filter-years-from = Dal { $year }
filter-years-between = Dal { $from } al { $to }
filter-tag-rank-title = Rango minimo del tag
filter-tag = Tag
search-filter-by = Filtra per { $name }
search-loading-genres = Caricamento generi
search-paused-rate-limit = La ricerca mentre scrivi è in pausa per un momento, per restare entro i limiti di AniList. Premi il tasto di ricerca per cercare subito.
action-clear-all = Rimuovi tutto
filter-source-material = Basato su
search-detail-episode = { $status } · E{ $episode }
sources-use = Usa { $name }
sources-move = Sposta { $name }
sources-reorder = Riordina
sources-version-available = { $version } disponibile
sources-update-from = { -release-cap } { $version } da { $store }.
sources-detail-off = Disattivata. Mantiene il suo posto e non viene provata finché non la riattivi.
sources-fallback-title = { -source-cap }
reason-source-timeout = Ha impiegato troppo tempo a rispondere.
reason-source-busy = È occupata. Riprova tra un minuto.
reason-source-problem = Ha avuto un problema.
reason-source-turned-away = Ha rifiutato la richiesta.
reason-source-no-address = Non è stato possibile trovare il suo indirizzo.
reason-source-unreachable = Non è stato possibile raggiungerla.
stores-updates-waiting = { $count ->
    [one] Un aggiornamento
    [many] { $count } aggiornamenti
   *[other] { $count } aggiornamenti
 }
stores-checked-ago = controllato { $ago }
stores-preview-key-link = La chiave corrisponde al link
stores-preview-key-invite = La chiave corrisponde all'invito
stores-key-verified = Verificata
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = il { -plugin-store }
stores-link-sheet-detail = Apri questo link sull'altro { -device } per entrare nel { -plugin-store } lì. Funziona una volta sola.
accounts-synced-ago = Sincronizzato { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Apre Account
accounts-card-description-signed-out = Non hai effettuato l'accesso. Apre Account
accounts-signed-in-plain = Accesso effettuato
settings-summary-notify-on = Nuovi episodi attivi
settings-summary-notify-off = Nuovi episodi disattivati
settings-summary-sources = { $count ->
    [one] Una { -source }
    [many] { $count } { -source-pl }
   *[other] { $count } { -source-pl }
 }
settings-summary-no-sources = Nessuna { -source-pl } ancora
settings-summary-torrents-on = torrent attivi
settings-summary-torrents-off = torrent disattivati
library-storage-on-device = { $size } su questo { -device }
library-storage-none = Niente su questo { -device }
library-detail-of = E{ $next } di { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Ultima visione
library-sort-current = Attuale
settings-notify-permission-note = { -app-name } chiede il permesso la prima volta che lo attivi.
settings-notify-refused = Disattivo: Android non consente le notifiche di { -app-name }.
settings-notify-system-title = Impostazioni notifiche
settings-notify-system-detail = Suono, vibrazione e schermata di blocco
settings-titles-line-signed-out = Inglese se non hai effettuato l'accesso ad AniList
settings-titles-line-account-language = { $language }, dal tuo account AniList
settings-titles-line-follows = Segue il tuo account AniList
settings-titles-line-english = Romaji dove manca il titolo inglese
settings-titles-line-romaji = Titoli giapponesi in caratteri latini
settings-titles-note-account = Segue il tuo account AniList, impostato su { $language }. Mentre non hai effettuato l'accesso ad AniList, i titoli appaiono in inglese.
settings-titles-note-follows = Segue il tuo account AniList. Mentre non hai effettuato l'accesso ad AniList, i titoli appaiono in inglese.
settings-titles-note-english-missing = { $title } non ha un titolo inglese su AniList, quindi mantiene quello romaji.
settings-titles-note-english = Titoli inglesi come li elenca AniList, e romaji dove manca.
settings-titles-note-romaji = Titoli giapponesi in caratteri latini, come li elenca AniList.
settings-index-none = Nessuno impostato
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Nei limiti del possibile su questo { -device }: la condivisione procede solo mentre { -app-name } è in esecuzione.
settings-downloads-count = { $count ->
    [one] Un episodio
    [many] { $count } episodi
   *[other] { $count } episodi
 }
settings-anilist-footer = Avanzamento e modifiche alle liste si sincronizzano con AniList. { -library-cap } mostra le tue liste AniList.
accounts-sign-in-again-button = Accedi di nuovo
accounts-review-differences = { $count ->
    [one] Controlla una differenza
    [many] Controlla { $count } differenze
   *[other] Controlla { $count } differenze
 }
settings-licence-title = Licenza
settings-licence-value = MIT, open source e non commerciale
settings-typefaces-value = Roboto Flex e Zen Kaku Gothic New, con licenza SIL Open Font License 1.1
settings-libraries-value = JNA e AndroidX, con licenza Apache License 2.0
settings-report-bug-value = Apre GitHub nel browser
downloads-storage-share-description = I download occupano { $size }
settings-aside-notifications = Una notifica quando esce un episodio di una serie che segui: il titolo della serie e il numero dell'episodio, mai il suo nome. Una serie può disattivare le sue notifiche nelle proprie impostazioni.
settings-aside-downloads = I download restano nello spazio riservato a { -app-name } su questo { -device }, che le altre app non possono leggere. Rimuovere l'app li elimina.
settings-aside-connection-check = Controllo della connessione
settings-aside-sources = Le { -source-pl } vengono provate in ordine, dall'alto in basso. Una serie può mantenere una { -source } propria, scelta dalla sua pagina.
settings-upscaling-description = La stessa immagine: a sinistra il 480p originale, a destra con { $mode }
settings-upscaling-source = 480p originale
settings-upscaling-note-off-original = Il 480p originale, scalato per adattarsi senza elaborazione.
torrents-sheet-turn-on-title = Attiva i torrent
torrents-sheet-protect-title = Proteggi il traffico torrent
torrents-sheet-turn-on-button = Attiva
torrents-sheet-host = Host
torrents-sheet-port = Porta
torrents-sheet-proxy-auth-note = Nome utente e password solo se il proxy li richiede.
torrents-sheet-edit-proxy-title = Modifica proxy
title-facts-also-on = Anche su
title-fetch-failed-detail = AniList non ha risposto.
continue-resize-queue = Ridimensiona la coda
continue-go-to-title = Vai al titolo
continue-short-left = { $episode } · { $left }
continue-airing-out = E{ $episode } · { $time } · uscito { $ago }
continue-airing-in = E{ $episode } · { $time } · tra { $countdown }
calendar-line-out = { $episode } uscito { $ago }
calendar-line-in = { $episode } tra { $countdown }
nav-accounts-and-settings-user = { $user }, account e impostazioni
episode-unaired-label = Episodio { $number }, { $title }. Non ancora uscito
episode-unaired-label-plain = Episodio { $number }. Non ancora uscito
episode-unaired-airs-label = Episodio { $number }, { $title }. Non ancora uscito, esce { $airs }
episode-unaired-airs-label-plain = Episodio { $number }. Non ancora uscito, esce { $airs }
tv-sheet-on-network = Su questa rete
tv-sheet-not-listed = Non in elenco
tv-sheet-scan = Scansiona il codice sulla TV
tv-sheet-code-on = Codice su { $name }
tv-sheet-pairing-button = Abbinamento in corso
tv-sheet-connecting = Connessione alla TV
tv-sheet-type-code = Digita il codice
tv-sheet-volume = Volume della TV
tv-sheet-volume-down = Abbassa volume
tv-sheet-volume-up = Alza volume
tv-sheet-volume-value = { $value } su 100
tv-sheet-nothing-playing = Niente in riproduzione
tv-sheet-position = Posizione sulla TV
tv-sheet-position-state = { $at } di { $total }
tv-sheet-request-titled = { $title }, episodio { $episode }
tv-sheet-request-plain = Episodio { $episode }
tv-sheet-footer = Una TV elenca questo { -device } con il codice { $code }. Se rimuovi una TV da qui, rimuovi anche questo { -device }.
tv-sheet-title-fallback = TV
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, oggi
calendar-day-label-airing = { $day }, episodi in onda
calendar-day-label-today-airing = { $day }, oggi, episodi in onda
continue-show-airing-in-calendar = Mostra in Calendario le serie in onda oggi
continue-show-season-in-seasonal = Mostra questa stagione in Stagionale
search-searching-filters = Ricerca di { $filters }
episode-number-plain = Episodio { $number }
episode-number-title-text = Episodio { $number } · { $title }
player-position-state = { $at } di { $total }
player-sheet-finding = Ricerca delle { -source-pl }…
player-sheet-none = Nessuna { -source-pl } trovata per questo episodio.
player-sheet-episodes-loading = L'elenco degli episodi sta arrivando.
player-dual-audio = { -subbed-cap } e { -dubbed }
notif-new-episodes = { $count ->
    [one] Un nuovo episodio
    [many] { $count } nuovi episodi
   *[other] { $count } nuovi episodi
 }
notif-hidden-text = Nuovo episodio
notif-episode-out = L'episodio { $episode } è uscito.
notif-episode-out-dubbed = L'episodio { $episode } è uscito, { -dubbed }.
notif-episode-out-next = L'episodio { $episode } è uscito. Il { -up-next } per te è l'episodio { $next }.
notif-episode-out-dubbed-next = L'episodio { $episode } è uscito, { -dubbed }. Il { -up-next } per te è l'episodio { $next }.
device-name-phone = Telefono Android
device-name-tablet = Tablet Android
tv-session-starting = Avvio su { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · { $minutes } min rimanenti
widget-line-of = E{ $episode } di { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = In pari
widget-caught-up = Sei in pari.
widget-caught-up-next = { $title } E{ $episode } esce { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Riproduci l'episodio { $episode }, { -dubbed }, di { $title }
tv-action-resume = Riprendi l'episodio { $episode }
tv-action-resume-dubbed = Riprendi l'episodio { $episode } · { -dubbed-cap }
tv-action-out = Riproduci l'episodio { $episode } (esce { $day })
tv-action-not-out = Riproduci l'episodio { $episode } (non ancora uscito)
tv-out-today = oggi
tv-hero-resume = Riprendi { $episode }
tv-hero-start = Riproduci { $episode }
