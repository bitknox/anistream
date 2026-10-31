notice-reason = { $reason }
cause-no-sources = No hay { -source-pl } configuradas. Añade una en la pantalla { -source-cap-pl }.
notice-not-aired = El episodio { $episode } se emite { $when }.
notice-not-aired-yet = El episodio { $episode } aún no se ha emitido. El episodio { $next } se emite { $when }.
cause-airs-today = hoy a las { $time }
cause-airs-tomorrow = mañana a las { $time }
cause-airs-in-days = { $days ->
    [one] dentro de un día a las { $time }
    [many] dentro de { $days } días a las { $time }
   *[other] dentro de { $days } días a las { $time }
 }
cause-airs-on-date = el { $date } a las { $time }
cause-no-playable-stream = La { -source } respondió, pero no tiene nada que reproducir. Prueba con otra.
cause-title-no-match = No hay coincidencias para este título: { $detail }
cause-source-gone = La { -source } { $source } ya no está disponible. Elige otra.
cause-release-failed = No se pudo iniciar ese { -release }: { $detail }
cause-sources-timed-out = Las { -source-pl } tardaron demasiado en responder. Inténtalo de nuevo o elige otra { -source }.
cause-no-source-has-episode = Ninguna { -source } tiene el episodio { $episode }: { $detail }
cause-source-cannot-download = Esta { -source } transmite episodios, pero no permite descargarlos.
cause-no-source-could-list-show = Ninguna { -source } pudo listar esta serie, así que no se puede descargar.
cause-subtitles-merged = { $tracks ->
    [one] Se añadió una pista de subtítulos al video.
    [many] Se añadieron { $tracks } pistas de subtítulos al video.
   *[other] Se añadieron { $tracks } pistas de subtítulos al video.
 }
cause-subtitles-left-apart = Los subtítulos quedan como archivos separados. { $reason }
cause-already-in-downloads = { $count ->
    [one] Un episodio ya está en descargas.
    [many] { $count } episodios ya están en descargas.
   *[other] { $count } episodios ya están en descargas.
 }
status-looking-up-title = Buscando el título…
status-loading-sources = Cargando { -source-pl }…
status-searching-sources = Buscando { -source-pl } del episodio { $episode }…
notice-player-failed-to-start = No se pudo iniciar mpv. Comprueba que mpv se abra por sí solo. ({ $reason })
notice-stream-no-video = Esa transmisión no muestra video. Probando la transmisión { $next } de { $total }.
notice-player-could-not-play = mpv no pudo reproducir esto. { $reason }
notice-player-exited-silently = mpv se cerró sin reproducir nada ni dar un motivo. Prueba con otra { -source }.
notice-player-not-playing = mpv no empezó a reproducir después de { $seconds } s. La { -source } puede no tener datos. Comprueba que mpv reproduzca un archivo por sí solo.
notice-no-player = Esta versión de { -app-name } no puede reproducir video.
notice-mpv-missing = No se encontró { $binary }. Instala mpv para reproducir episodios.
notice-mpv-old = mpv { $version } puede reproducir, pero el reproductor de { -app-name } necesita mpv 0.36 o más reciente para su botón de saltar, la tarjeta { -up-next-cap }, los menús y las miniaturas.
notice-subtitles-unavailable = No se pudieron cargar los subtítulos de esta { -source }. Elige otra { -source } para intentarlo de nuevo.
notice-opened-in-browser = Abierto en tu navegador.
notice-could-not-open = No se pudo abrir eso. { $reason }
notice-trailer-playing = El tráiler se reproduce en mpv.
notice-trailer-opened = El tráiler se abrió en tu navegador.
notice-trailer-failed = No se pudo abrir el tráiler. { $reason }
notice-filler-skipped = Se omitieron episodios de relleno. { -up-next-cap } es el episodio { $episode }.
notice-audio-now-subbed = El audio ahora está { -subbed }.
notice-audio-now-dubbed = El audio ahora está { -dubbed }.
notice-already-first-episode = Ya estás en el primer episodio.
notice-last-episode = No hay más episodios. El episodio { $episode } es el último.
notice-no-next-episode = No hay más episodios.
notice-no-previous-episode = No hay episodio anterior.
notice-match-reset-failed = No se pudo restablecer la coincidencia. { $reason }
notice-match-save-failed = No se pudo guardar esa coincidencia. { $reason }
notice-match-did-not-stick = Esa coincidencia no se mantuvo. Puede que la { -source } haya cambiado. Intenta elegirla otra vez.
notice-source-search-empty = No se encontró nada para “{ $query }”. Prueba con otra búsqueda.
notice-source-pinned = { $source } se usará para este título.
notice-source-automatic = Se volvió a la selección automática de { -source-pl }.
notice-source-choice-failed = No se pudo guardar esa elección. { $reason }
notice-no-source-could-list = Ninguna { -source } pudo listar esta serie: { $reason }
notice-episodes-failed = No se pudieron cargar los episodios. { $reason }
notice-match-uncertain = { $source } encontró más de una serie que podría ser esta, así que por ahora los episodios vienen de AniList.
notice-continue-hide-failed = No se pudo quitar eso de { -continue-cap }. { $reason }
notice-download-queued = Se añadió el episodio { $episode } de { $title } a la cola.
notice-download-already-queued = El episodio { $episode } de { $title } ya está en descargas.
notice-download-starting-now = Iniciando ahora mismo el episodio { $episode } de { $title }.
notice-download-cancelled = Descarga cancelada.
notice-download-file-deleted = Descarga y archivo eliminados.
notice-download-removed-file-gone = Descarga quitada. Su archivo ya no existía.
notice-download-removed-file-kept = Se quitó la descarga, pero no se pudo borrar su archivo. { $reason }
notice-download-removed = Descarga quitada.
notice-downloads-cleared = { $count ->
    [one] Se quitó una descarga terminada.
    [many] Se quitaron { $count } descargas terminadas.
   *[other] Se quitaron { $count } descargas terminadas.
 }
notice-download-finished = Se descargó el episodio { $episode } de { $title }.
notice-download-finished-note = Se descargó el episodio { $episode } de { $title }. { $note }
notice-download-failed = No se pudo descargar el episodio { $episode } de { $title }. { $reason }
notice-downloads-none-queued = No se añadió nada a la cola.
notice-downloads-none-queued-stopped = No se añadió nada a la cola. { $reason }
notice-downloads-none-new = No se añadió nada nuevo a la cola. { $existing }
notice-downloads-none-new-stopped = No se añadió nada nuevo a la cola. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] Se añadió un episodio a la cola. { $existing }
    [many] Se añadieron { $count } episodios a la cola. { $existing }
   *[other] Se añadieron { $count } episodios a la cola. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] Se añadió un episodio a la cola y luego se detuvo. { $reason }
    [many] Se añadieron { $count } episodios a la cola y luego se detuvo. { $reason }
   *[other] Se añadieron { $count } episodios a la cola y luego se detuvo. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] Se añadió un episodio a la cola y luego se detuvo. { $existing } { $reason }
    [many] Se añadieron { $count } episodios a la cola y luego se detuvo. { $existing } { $reason }
   *[other] Se añadieron { $count } episodios a la cola y luego se detuvo. { $existing } { $reason }
 }
notice-keep-next-full = No hay suficiente espacio libre para mantener descargados los siguientes episodios. Libera espacio para descargar más.
party-needs-server = Primero elige un servidor para la { -watch-party }, en el ajuste syncplay.server. { $server } es operado por { $operator }.
party-choose-server = Primero elige un servidor para la { -watch-party }. { $server } es operado por { $operator }.
notice-no-party = Las { -watch-party-pl } no están disponibles en esta versión de { -app-name }.
party-needs-room = La { -watch-party } necesita una sala para reunirse. Elige una primero.
party-server-lost = Se perdió el servidor de la { -watch-party }. Intentando de nuevo.
party-rejoined = De vuelta en la { -watch-party }.
party-left = Saliste de la { -watch-party }.
