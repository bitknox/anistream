lists-terminal-too-small = Serve un terminale più grande
downloads-screen-title = Download
downloads-column-title = Titolo
downloads-column-state = Stato
downloads-column-progress = Avanzamento
downloads-empty-heading = Nessun download in coda
downloads-empty-hint = Premi { $key } su un titolo o un episodio per metterlo in coda.
downloads-row-episode = E{ $episode }
downloads-row-episode-ahead = E{ $episode } · in anticipo
downloads-note-sizes = { $downloaded } / { $total }
downloads-note-waiting-metadata = In attesa dei metadati
downloads-note-torrents-off = Torrent disattivati
downloads-note-waiting-start = In attesa di avvio
downloads-state-queued = In coda
downloads-state-active = Scaricamento
downloads-state-paused = In pausa
downloads-state-done = Scaricato
downloads-state-failed = Errore
downloads-detail-ahead = Scaricato in anticipo, verrà rimosso un giorno dopo la visione
downloads-detail-with-ahead = { $detail } · scaricato in anticipo, verrà rimosso un giorno dopo la visione
downloads-detail-paused-at = In pausa a { $downloaded } di { $total } (riprende da lì)
downloads-detail-paused-early = In pausa prima dell'avvio
downloads-detail-torrents-off = Per i download servono i torrent, che sono disattivati (premi { $key } per le Impostazioni)
downloads-detail-queued = In coda (parte quando si libera uno slot di download)
downloads-detail-waiting-metadata = In attesa dei metadati del torrent
downloads-seeding-heading = In seeding dopo la visione
downloads-seeding-detail = { $ratio } su { $goal } · mancano { $left } · caricati { $uploaded }
downloads-failed-no-reason = Il download non è riuscito e non è stato registrato alcun motivo.
downloads-not-finished = Questo download non è ancora terminato.
accounts-screen-title = Account
accounts-badge-sign-in = { $tracker } { $mark } accedi
accounts-needs-sign-in = { $tracker } richiede un nuovo accesso. Premi { $key } per aprire Account.
accounts-conflicts-found = { $count ->
    [one] 1 differenza di sincronizzazione trovata. Premi { $key } per esaminarla.
    [many] { $count } differenze di sincronizzazione trovate. Premi { $key } per esaminarle.
   *[other] { $count } differenze di sincronizzazione trovate. Premi { $key } per esaminarle.
 }
accounts-conflicts-none = Non ci sono differenze di sincronizzazione da esaminare.
accounts-no-tracker = Nessun { -tracker } è collegato. Premi { $key } per aprire Account.
accounts-finish-signing-in = Completa l'accesso a { $tracker }
accounts-step-open = 1. Apri   { $url }
accounts-step-enter = 2. Inserisci
accounts-code-expires-seconds = Scade tra { $seconds } s
accounts-code-expires-minutes = Scade tra { $minutes } min
accounts-code-expired = Codice scaduto
accounts-column-tracker = { -tracker-cap }
accounts-column-state = Stato
accounts-column-queued = In coda
accounts-column-token = Token
accounts-empty-heading = Nessun { -tracker-pl } attivo
accounts-empty-hint = Aggiungine uno a { $setting } in config.toml. La cronologia di visione funziona anche senza.
accounts-state-sign-in-again = Accedi di nuovo
accounts-state-connected = Collegato
accounts-state-not-connected = Non collegato
accounts-pull-failed-today = Il recupero della lista non è riuscito alle { $time }: { $error }
accounts-pull-failed-dated = Il recupero della lista non è riuscito il { $date } alle { $time }: { $error }
accounts-pull-failed = Il recupero della lista non è riuscito: { $error }
accounts-token-files = I token sono salvati in file con permessi 0600 perché il portachiavi di sistema non è disponibile.
accounts-enter-sign-out = ↵ Disconnetti
accounts-enter-sign-in = ↵ Accedi
accounts-action-sign-in-again = Accedi di nuovo
accounts-action-sign-out = Disconnetti
accounts-action-sign-in = Accedi
accounts-rows-empty = Nessun { -tracker-pl } attivo. Aggiungine uno a { $setting } in config.toml.
accounts-row-signed-in-as = Accesso come { $user }
accounts-row-connected = Collegato
accounts-row-not-connected = Non collegato
accounts-row-queued = { $count ->
    [one] { $count } in coda
    [many] { $count } in coda
   *[other] { $count } in coda
 }
accounts-row-queued-held = { $queued } in coda, { $held } in attesa di un abbinamento con l'ID del { -tracker }
accounts-conflict-keep-mine = Tieni il mio
accounts-conflict-keep-theirs = Tieni il suo
accounts-conflict-row = { $title }  ·  { $field }: mio { $local } / { -tracker-cap }: { $remote }
lists-library-title = { -library-cap }
lists-remove-wrong-list = Puoi nascondere i titoli solo nella lista { -continue-cap }.
lists-continue-removed = { $title } è stato nascosto da { -continue-cap }. Guardandolo di nuovo, tornerà in lista.
lists-nothing-selected = Nessun titolo selezionato.
lists-search-prompt = Cerca { $query }
lists-load-failed = Impossibile caricare questa lista
lists-loading = Caricamento…
lists-empty-press-enter = Premi ↵ per cercare
lists-empty-type-to-search = Scrivi per cercare
lists-empty-browse-filtered = Premi ↵ per sfogliare con questi filtri.
lists-empty-browse-popular = Premi ↵ per sfogliare i più popolari.
lists-empty-no-matches = Nessun risultato
lists-empty-fewer-filters = Prova con meno filtri.
lists-empty-shorter-query = Prova con una ricerca più breve o con il titolo romaji.
lists-empty-nothing-watched = Non hai ancora guardato nulla
lists-empty-ways-on = Premi { $seasonal } per questa stagione o { $search } per cercare.
lists-empty-nothing-to-continue = Niente da riprendere
lists-empty-hidden-come-back = I titoli che hai nascosto tornano quando li guardi di nuovo. Premi { $seasonal } per questa stagione o { $search } per cercare.
lists-empty-nothing-here = Ancora niente qui
lists-library-no-account = Nessun account collegato
lists-library-works-without = La cronologia funziona anche senza. La { -library } rispecchia la lista su un { -tracker }.
lists-library-enable-tracker = Aggiungine uno a { $setting } in config.toml, poi premi { $key } per aprire Account.
lists-library-sign-in = Premi { $key } per aprire Account e accedere.
lists-segment-watching = Guardando
lists-segment-planning = Da guardare
lists-segment-completed = Completati
lists-segment-paused = In pausa
lists-segment-dropped = Abbandonati
lists-episode = E{ $episode }
lists-episode-dubbed = E{ $episode } · { -dubbed-cap }
lists-score = { $score }%
lists-row-resume-percent = { $episode } · { $percent }%
lists-row-resume-clock = { $episode } · { $clock }
lists-episode-out = Episodio { $episode } uscito
lists-episode-in = Episodio { $episode } tra { $countdown }
lists-next-in = { -up-next-cap } tra { $countdown }
lists-episode-out-ago = Episodio { $episode } uscito { $countdown } fa
lists-episode-out-just-now = Episodio { $episode } uscito adesso
lists-time-in = Tra { $countdown }
lists-time-ago = { $countdown } fa
lists-time-just-now = Poco fa
lists-time-approximate = ~{ $time }
lists-day-today = Oggi
lists-day-yesterday = Ieri
lists-day-tomorrow = Domani
lists-preview-continue-percent = { -continue-cap } { $episode }, fermo a { $clock } ({ $percent }%)
lists-preview-continue = { -continue-cap } { $episode }, fermo a { $clock }
lists-progress-aired = { $total ->
    [one] { $done } di { $total } episodio uscito
    [many] { $done } di { $total } episodi usciti
   *[other] { $done } di { $total } episodi usciti
 }
lists-progress-watched = { $count ->
    [one] 1 episodio guardato
    [many] { $count } episodi guardati
   *[other] { $count } episodi guardati
 }
lists-filter-genre = Genere
lists-filter-year = Anno
lists-filter-season = Stagione
lists-filter-format = Formato
lists-filter-status = Stato
lists-filter-sort = Ordine
lists-filter-any = Qualsiasi
lists-sort-relevance = Pertinenza
lists-season-winter = Inverno
lists-season-spring = Primavera
lists-season-summer = Estate
lists-season-fall = Autunno
lists-format-tv = TV
lists-format-tv-short = TV breve
lists-format-movie = Film
lists-format-ova = OVA
lists-format-ona = ONA
lists-format-special = Speciale
lists-format-music = Musica
lists-status-airing = In corso
lists-status-finished = Concluso
lists-status-upcoming = In arrivo
lists-sort-popularity = Popolarità
lists-sort-score = Punteggio
lists-sort-trending = Di tendenza
lists-sort-newest = Più recenti
