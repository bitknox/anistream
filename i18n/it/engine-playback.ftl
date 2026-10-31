notice-reason = { $reason }
cause-no-sources = Non sono configurate { -source-pl }. Aggiungine una nella schermata { -source-cap-pl }.
notice-not-aired = L'episodio { $episode } va in onda { $when }.
notice-not-aired-yet = L'episodio { $episode } non è ancora in onda. L'episodio { $next } va in onda { $when }.
cause-airs-today = oggi alle { $time }
cause-airs-tomorrow = domani alle { $time }
cause-airs-in-days = { $days ->
    [one] tra un giorno alle { $time }
    [many] tra { $days } giorni alle { $time }
   *[other] tra { $days } giorni alle { $time }
 }
cause-airs-on-date = il { $date } alle { $time }
cause-no-playable-stream = La { -source } ha risposto, ma non c'è niente da riprodurre. Provane un'altra.
cause-title-no-match = Impossibile abbinare questo titolo: { $detail }
cause-source-gone = La { -source } { $source } non è più disponibile. Scegline un'altra.
cause-release-failed = Impossibile avviare quella { -release }: { $detail }
cause-sources-timed-out = Le { -source-pl } hanno impiegato troppo tempo a rispondere. Riprova, oppure scegline un'altra.
cause-no-source-has-episode = Nessuna { -source } ha l'episodio { $episode }: { $detail }
cause-source-cannot-download = Questa { -source } offre episodi in streaming, ma non permette di scaricarli.
cause-no-source-could-list-show = Nessuna { -source } è riuscita a elencare questa serie, quindi non può essere scaricata.
cause-subtitles-merged = { $tracks ->
    [one] È stata unita al video una traccia di sottotitoli.
    [many] Sono state unite al video { $tracks } tracce di sottotitoli.
   *[other] Sono state unite al video { $tracks } tracce di sottotitoli.
 }
cause-subtitles-left-apart = I sottotitoli restano come file separati. { $reason }
cause-already-in-downloads = { $count ->
    [one] Un episodio è già nei download.
    [many] { $count } episodi sono già nei download.
   *[other] { $count } episodi sono già nei download.
 }
status-looking-up-title = Cerco il titolo…
status-loading-sources = Carico le { -source-pl }…
status-searching-sources = Cerco nelle { -source-pl } l'episodio { $episode }…
notice-player-failed-to-start = Impossibile avviare mpv. Verifica che si avvii da solo. ({ $reason })
notice-stream-no-video = Nessun video da questo flusso. Provo il flusso { $next } di { $total }.
notice-player-could-not-play = mpv non è riuscito a riprodurre questo file. { $reason }
notice-player-exited-silently = mpv si è chiuso senza riprodurre nulla e senza spiegare il motivo. Prova un'altra { -source }.
notice-player-not-playing = mpv non ha iniziato a riprodurre dopo { $seconds } s. La { -source } potrebbe non avere dati. Verifica che mpv riproduca un file da solo.
notice-no-player = Questa edizione di { -app-name } non può riprodurre video.
notice-mpv-missing = { $binary } non è stato trovato. Installa mpv per riprodurre gli episodi.
notice-mpv-old = mpv { $version } può riprodurre, ma il player di { -app-name } richiede mpv 0.36 o più recente per il pulsante di salto, la scheda { -up-next-cap }, i menu e le miniature.
notice-subtitles-unavailable = Non è stato possibile caricare i sottotitoli di questa { -source }. Scegline un'altra per riprovare.
notice-opened-in-browser = Aperto nel tuo browser.
notice-could-not-open = Impossibile aprire questo link. { $reason }
notice-trailer-playing = Trailer in riproduzione in mpv.
notice-trailer-opened = Trailer aperto nel browser.
notice-trailer-failed = Impossibile aprire il trailer. { $reason }
notice-filler-skipped = Filler saltati. { -up-next-cap } è l'episodio { $episode }.
notice-audio-now-subbed = Ora l'audio è { -subbed }.
notice-audio-now-dubbed = Ora l'audio è { -dubbed }.
notice-already-first-episode = Sei già al primo episodio.
notice-last-episode = Non c'è un episodio successivo. L'episodio { $episode } è l'ultimo.
notice-no-next-episode = Non c'è un episodio successivo.
notice-no-previous-episode = Non c'è un episodio precedente.
notice-match-reset-failed = Impossibile reimpostare l'abbinamento. { $reason }
notice-match-save-failed = Impossibile salvare questo abbinamento. { $reason }
notice-match-did-not-stick = Questo abbinamento non è stato applicato. La { -source } potrebbe essere cambiata. Prova a sceglierlo di nuovo.
notice-source-search-empty = Nessun risultato per “{ $query }”. Prova una ricerca diversa.
notice-source-pinned = Per questo titolo si userà { $source }.
notice-source-automatic = Per questo titolo la { -source } viene scelta di nuovo automaticamente.
notice-source-choice-failed = Impossibile salvare la scelta. { $reason }
notice-no-source-could-list = Nessuna { -source } è riuscita a elencare questa serie: { $reason }
notice-episodes-failed = Impossibile caricare gli episodi. { $reason }
notice-match-uncertain = { $source } ha trovato più serie che potrebbero essere questa, quindi per ora gli episodi sono quelli di AniList.
notice-continue-hide-failed = Impossibile rimuovere il titolo da { -continue-cap }. { $reason }
notice-download-queued = Aggiunto alla coda l'episodio { $episode } di { $title }.
notice-download-already-queued = L'episodio { $episode } di { $title } è già nei download.
notice-download-starting-now = Avvio ora l'episodio { $episode } di { $title }.
notice-download-cancelled = Download annullato.
notice-download-file-deleted = Download e file eliminati.
notice-download-removed-file-gone = Download rimosso. Il file era già stato eliminato.
notice-download-removed-file-kept = Download rimosso, ma non è stato possibile eliminare il file. { $reason }
notice-download-removed = Download rimosso.
notice-downloads-cleared = { $count ->
    [one] Cancellato un download completato.
    [many] Cancellati { $count } download completati.
   *[other] Cancellati { $count } download completati.
 }
notice-download-finished = Episodio { $episode } di { $title } scaricato.
notice-download-finished-note = Episodio { $episode } di { $title } scaricato. { $note }
notice-download-failed = Impossibile scaricare l'episodio { $episode } di { $title }. { $reason }
notice-downloads-none-queued = Niente in coda.
notice-downloads-none-queued-stopped = Niente in coda. { $reason }
notice-downloads-none-new = Niente di nuovo in coda. { $existing }
notice-downloads-none-new-stopped = Niente di nuovo in coda. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] Aggiunto in coda un episodio. { $existing }
    [many] Aggiunti in coda { $count } episodi. { $existing }
   *[other] Aggiunti in coda { $count } episodi. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] Aggiunto in coda un episodio, poi l'operazione si è fermata. { $reason }
    [many] Aggiunti in coda { $count } episodi, poi l'operazione si è fermata. { $reason }
   *[other] Aggiunti in coda { $count } episodi, poi l'operazione si è fermata. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] Aggiunto in coda un episodio, poi l'operazione si è fermata. { $existing } { $reason }
    [many] Aggiunti in coda { $count } episodi, poi l'operazione si è fermata. { $existing } { $reason }
   *[other] Aggiunti in coda { $count } episodi, poi l'operazione si è fermata. { $existing } { $reason }
 }
notice-keep-next-full = Spazio libero insufficiente per tenere scaricati i prossimi episodi. Libera un po' di spazio per scaricarne altri.
party-needs-server = Scegli prima un server per il { -watch-party }, come syncplay.server. { $server } è gestito da { $operator }.
party-choose-server = Scegli prima un server per il { -watch-party }. { $server } è gestito da { $operator }.
notice-no-party = I { -watch-party-cap-pl } non sono disponibili in questa edizione di { -app-name }.
party-needs-room = Per il { -watch-party } serve una stanza in cui incontrarsi. Scegline prima una.
party-server-lost = Persa la connessione al server del { -watch-party }. Riprovo.
party-rejoined = Di nuovo nel { -watch-party }.
party-left = Uscito dal { -watch-party }.
