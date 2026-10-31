widget-episode-short = E{ $episode }
widget-episode-long = Episodio { $episode }
widget-day-tomorrow = domani
widget-next-line = Prossima uscita: { $when } · { $title } · Episodio { $episode }
widget-next-line-short = Prossima: { $when }, { $title } E{ $episode }
widget-next-time = Prossima { $time }
widget-airs-day = { $title } E{ $episode } va in onda { $day }.
widget-out-ago = Uscito { $time } fa
widget-in-time = Tra { $time }
widget-airing-title-small = Oggi
widget-nothing-airing-short = Niente in onda.
widget-open-to-update = Apri { -app-name } per aggiornarlo.
widget-nothing-more-today = Niente altro oggi
widget-tomorrow-none = Domani: niente in onda
widget-tomorrow-first = Domani: { $time } · { $title }
widget-tomorrow-more = Domani: { $time } · { $title } e altri { $count }
widget-continue-title-apple = { -continue-cap }
widget-continue-description = La prossima puntata di ciò che stai guardando.
widget-airing-description = Gli episodi di oggi, con le serie nella tua lista evidenziate.
widget-connect-detail-long = Collega { $tracker } o riproduci un episodio per vedere qui cosa stai guardando.
widget-connect-detail-short = Collega { $tracker } o riproduci un episodio per vederlo qui.
widget-connect-tracker = Collega { $tracker }
widget-caught-up-title-apple = Sei aggiornato
widget-caught-up-none = Non c'è altro da guardare.
widget-caught-up-next-apple = Dalla tua lista:
widget-line-left-apple = { $episode } · { $left }
widget-line-resume = Riprendi { $episode }
widget-action-resume = Riprendi { $episode }
widget-action-start = Riproduci { $episode }
widget-a11y-episode = episodio { $episode }
widget-a11y-episode-marked = { $episode }, { $mark }
widget-a11y-resume = { $minutes ->
    [one] { $episode }, manca un minuto
    [many] { $episode }, mancano { $minutes } minuti
   *[other] { $episode }, mancano { $minutes } minuti
 }
widget-a11y-start = inizia { $episode }
widget-a11y-row = { $title }, { $what }
widget-a11y-airing = { $title }, episodio { $episode }, { $time }
widget-a11y-then-watched = { $label }, già visto
widget-a11y-then-out = { $label }, uscito
widget-a11y-then-list = { $label }, nella tua lista
widget-out-now = Uscito ora
widget-airs-at = Va in onda alle { $time }
widget-aired-at = Andato in onda alle { $time }
widget-a11y-airs-at = { $title }, episodio { $episode }, va in onda alle { $time }
widget-a11y-out-now = { $title }, episodio { $episode }, uscito ora
widget-downloaded-line = Episodio { $episode } scaricato
widget-downloaded-ready = Pronto per la visione offline
widget-downloading-title = { $count ->
    [one] Download di un episodio in corso
    [many] Download di { $count } episodi in corso
   *[other] Download di { $count } episodi in corso
 }
widget-downloads-paused = In pausa
widget-downloads-then-more = { $title } E{ $episode }, e altri { $count }
widget-size-left = { $size } rimanenti
widget-time-left = { $time } rimanenti
widget-continues-when-open = Continua quando apri { -app-name }
widget-bytes-of = { $done } di { $total }
widget-speed = { $size }/s
widget-downloads-episode = Episodio { $episode }
widget-a11y-downloaded = { $title }, episodio { $episode }, scaricato
widget-a11y-downloading = { $title }, episodio { $episode }, { $percent }
widget-a11y-then-paused = { $label }, in pausa
widget-a11y-then-time-left = { $label }, { $time } rimanenti
widget-a11y-then-count = { $count ->
    [one] { $label }, un episodio in download
    [many] { $label }, { $count } episodi in download
   *[other] { $label }, { $count } episodi in download
 }
widget-play-episode-short = Riproduci E{ $episode }
widget-play-episode-long = Riproduci episodio { $episode }
widget-resume-downloads = Riprendi i download
widget-pause-downloads = Metti in pausa i download
widget-paused-line = In pausa · { $left } rimanenti
widget-a11y-playback-paused = { $minutes ->
    [one] { $title }, episodio { $episode }, in pausa, manca un minuto
    [many] { $title }, episodio { $episode }, in pausa, mancano { $minutes } minuti
   *[other] { $title }, episodio { $episode }, in pausa, mancano { $minutes } minuti
 }
widget-a11y-playback-playing = { $minutes ->
    [one] { $title }, episodio { $episode }, in riproduzione, manca un minuto
    [many] { $title }, episodio { $episode }, in riproduzione, mancano { $minutes } minuti
   *[other] { $title }, episodio { $episode }, in riproduzione, mancano { $minutes } minuti
 }
