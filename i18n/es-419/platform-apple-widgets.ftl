widget-episode-short = E{ $episode }
widget-episode-long = Episodio { $episode }
widget-day-tomorrow = mañana
widget-next-line = Próximo: { $when } · { $title } · Episodio { $episode }
widget-next-line-short = Próximo: { $when }, { $title } E{ $episode }
widget-next-time = Próximo: { $time }
widget-airs-day = { $title } E{ $episode } se emite { $day }.
widget-out-ago = Salió hace { $time }
widget-in-time = Sale en { $time }
widget-airing-title-small = Hoy
widget-nothing-airing-short = No se emite nada.
widget-open-to-update = Abre { -app-name } para actualizarlo.
widget-nothing-more-today = Nada más hoy
widget-tomorrow-none = Mañana: no se emite nada
widget-tomorrow-first = Mañana: { $time } · { $title }
widget-tomorrow-more = Mañana: { $time } · { $title } y { $count } más
widget-continue-title-apple = { -continue-cap }
widget-continue-description = El episodio que sigue de lo que estás viendo.
widget-airing-description = Los episodios de hoy, con las series de tu lista marcadas.
widget-connect-detail-long = Conecta { $tracker }, o reproduce un episodio, para ver aquí lo que estás viendo.
widget-connect-detail-short = Conecta { $tracker }, o reproduce un episodio, para verlo aquí.
widget-connect-tracker = Conectar { $tracker }
widget-caught-up-title-apple = Estás al día
widget-caught-up-none = Nada más para { -continue }.
widget-caught-up-next-apple = Próximo de tu lista:
widget-line-left-apple = { $episode } · { $left }
widget-line-resume = Retomar { $episode }
widget-action-resume = Retomar { $episode }
widget-action-start = Reproducir { $episode }
widget-a11y-episode = episodio { $episode }
widget-a11y-episode-marked = { $episode }, { $mark }
widget-a11y-resume = { $minutes ->
    [one] { $episode }, queda un minuto
    [many] { $episode }, quedan { $minutes } minutos
   *[other] { $episode }, quedan { $minutes } minutos
 }
widget-a11y-start = empezar { $episode }
widget-a11y-row = { $title }, { $what }
widget-a11y-airing = { $title }, episodio { $episode }, { $time }
widget-a11y-then-watched = { $label }, visto
widget-a11y-then-out = { $label }, ya disponible
widget-a11y-then-list = { $label }, en tu lista
widget-out-now = Ya disponible
widget-airs-at = Se emite a las { $time }
widget-aired-at = Se emitió a las { $time }
widget-a11y-airs-at = { $title }, episodio { $episode }, se emite a las { $time }
widget-a11y-out-now = { $title }, episodio { $episode }, ya disponible
widget-downloaded-line = Episodio { $episode } descargado
widget-downloaded-ready = Listo para ver sin conexión
widget-downloading-title = { $count ->
    [one] Descargando un episodio
    [many] Descargando { $count } episodios
   *[other] Descargando { $count } episodios
 }
widget-downloads-paused = En pausa
widget-downloads-then-more = { $title } E{ $episode }, y { $count } más
widget-size-left = Faltan { $size }
widget-time-left = Faltan { $time }
widget-continues-when-open = Continúa al abrir { -app-name }
widget-bytes-of = { $done } de { $total }
widget-speed = { $size }/s
widget-downloads-episode = Episodio { $episode }
widget-a11y-downloaded = { $title }, episodio { $episode }, descargado
widget-a11y-downloading = { $title }, episodio { $episode }, { $percent }
widget-a11y-then-paused = { $label }, en pausa
widget-a11y-then-time-left = { $label }, faltan { $time }
widget-a11y-then-count = { $count ->
    [one] { $label }, descargando un episodio
    [many] { $label }, descargando { $count } episodios
   *[other] { $label }, descargando { $count } episodios
 }
widget-play-episode-short = Reproducir E{ $episode }
widget-play-episode-long = Reproducir episodio { $episode }
widget-resume-downloads = Reanudar descargas
widget-pause-downloads = Pausar descargas
widget-paused-line = En pausa · faltan { $left }
widget-a11y-playback-paused = { $minutes ->
    [one] { $title }, episodio { $episode }, en pausa, queda un minuto
    [many] { $title }, episodio { $episode }, en pausa, quedan { $minutes } minutos
   *[other] { $title }, episodio { $episode }, en pausa, quedan { $minutes } minutos
 }
widget-a11y-playback-playing = { $minutes ->
    [one] { $title }, episodio { $episode }, reproduciendo, queda un minuto
    [many] { $title }, episodio { $episode }, reproduciendo, quedan { $minutes } minutos
   *[other] { $title }, episodio { $episode }, reproduciendo, quedan { $minutes } minutos
 }
