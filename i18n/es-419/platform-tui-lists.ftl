lists-terminal-too-small = Se necesita una terminal más grande
downloads-screen-title = Descargas
downloads-column-title = Título
downloads-column-state = Estado
downloads-column-progress = Progreso
downloads-empty-heading = Nada en cola
downloads-empty-hint = Pulsa { $key } en un título o episodio para descargarlo.
downloads-row-episode = E{ $episode }
downloads-row-episode-ahead = E{ $episode } · adelantado
downloads-note-sizes = { $downloaded } / { $total }
downloads-note-waiting-metadata = Esperando metadatos
downloads-note-torrents-off = Torrents desactivados
downloads-note-waiting-start = Esperando para empezar
downloads-state-queued = En cola
downloads-state-active = Descargando
downloads-state-paused = En pausa
downloads-state-done = Descargado
downloads-state-failed = Error
downloads-detail-ahead = Descargado con antelación, se elimina un día después de verlo
downloads-detail-with-ahead = { $detail } · descargado con antelación, se elimina un día después de verlo
downloads-detail-paused-at = En pausa en { $downloaded } de { $total } (se reanuda desde ahí)
downloads-detail-paused-early = En pausa antes de empezar
downloads-detail-torrents-off = Las descargas necesitan torrents, que están desactivados (pulsa { $key } para abrir Ajustes)
downloads-detail-queued = En cola (empieza cuando haya un espacio libre para descargar)
downloads-detail-waiting-metadata = Esperando los metadatos del torrent
downloads-seeding-heading = Compartiendo después de verlo
downloads-seeding-detail = { $ratio } de { $goal } · quedan { $left } · { $uploaded } enviados
downloads-failed-no-reason = La descarga falló y no se registró ningún motivo.
downloads-not-finished = Esa descarga todavía no ha terminado.
accounts-screen-title = Cuentas
accounts-badge-sign-in = { $tracker } { $mark } iniciar sesión
accounts-needs-sign-in = { $tracker } necesita volver a iniciar sesión. Pulsa { $key } para abrir Cuentas.
accounts-conflicts-found = { $count ->
    [one] Se encontró 1 diferencia de sincronización. Pulsa { $key } para revisarla.
    [many] Se encontraron { $count } diferencias de sincronización. Pulsa { $key } para revisarlas.
   *[other] Se encontraron { $count } diferencias de sincronización. Pulsa { $key } para revisarlas.
 }
accounts-conflicts-none = No hay diferencias de sincronización que revisar.
accounts-no-tracker = No hay ningún { -tracker } conectado. Pulsa { $key } para abrir Cuentas.
accounts-finish-signing-in = Termina de iniciar sesión en { $tracker }
accounts-step-open = 1. Abre   { $url }
accounts-step-enter = 2. Introduce
accounts-code-expires-seconds = Vence en { $seconds } s
accounts-code-expires-minutes = Vence en { $minutes } min
accounts-code-expired = Código vencido
accounts-column-state = Estado
accounts-column-queued = En cola
accounts-column-token = Token
accounts-empty-heading = No hay { -tracker-pl } activados
accounts-empty-hint = Añade uno en { $setting } en config.toml. Tu historial de reproducción funciona sin ninguno.
accounts-state-sign-in-again = Sesión caducada
accounts-state-connected = Conectado
accounts-state-not-connected = No conectado
accounts-pull-failed-today = Error al cargar tu lista a las { $time }: { $error }
accounts-pull-failed-dated = Error al cargar tu lista el { $date } a las { $time }: { $error }
accounts-pull-failed = Error al cargar tu lista: { $error }
accounts-token-files = Los tokens se guardan en archivos con permisos 0600 porque el llavero del sistema no está disponible.
accounts-enter-sign-out = ↵ Cerrar sesión
accounts-enter-sign-in = ↵ Iniciar sesión
accounts-action-sign-in-again = Volver a iniciar sesión
accounts-action-sign-out = Cerrar sesión
accounts-action-sign-in = Iniciar sesión
accounts-rows-empty = No hay { -tracker-pl } activados. Añade uno en { $setting } en config.toml.
accounts-row-signed-in-as = Sesión iniciada como { $user }
accounts-row-connected = Conectado
accounts-row-not-connected = No conectado
accounts-row-queued = { $count ->
    [one] { $count } en cola
    [many] { $count } en cola
   *[other] { $count } en cola
 }
accounts-row-queued-held = { $queued } en cola, { $held } esperando una asociación de ID
accounts-conflict-keep-mine = Usar el mío
accounts-conflict-keep-theirs = Usar el remoto
accounts-conflict-row = { $title }  ·  { $field }: mío { $local } / remoto { $remote }
lists-library-title = { -library-cap }
lists-remove-wrong-list = Solo puedes ocultar títulos de la lista { -continue-cap }.
lists-continue-removed = { $title } se ocultó de { -continue-cap }. Al volver a verlo, regresa.
lists-nothing-selected = No hay ningún título seleccionado.
lists-search-prompt = Buscar { $query }
lists-load-failed = No se pudo cargar esta lista
lists-loading = Cargando…
lists-empty-press-enter = Pulsa ↵ para buscar
lists-empty-type-to-search = Escribe para buscar
lists-empty-browse-filtered = Pulsa ↵ para explorar con estos filtros.
lists-empty-browse-popular = Pulsa ↵ para explorar los más populares.
lists-empty-no-matches = Sin resultados
lists-empty-fewer-filters = Prueba con menos filtros.
lists-empty-shorter-query = Prueba con una búsqueda más corta o con el título en romaji.
lists-empty-nothing-watched = Aún no has visto nada
lists-empty-ways-on = Pulsa { $seasonal } para esta temporada, o { $search } para buscar.
lists-empty-nothing-to-continue = Nada para { -continue }
lists-empty-hidden-come-back = Los títulos que quitaste vuelven cuando los veas. Pulsa { $seasonal } para esta temporada, o { $search } para buscar.
lists-empty-nothing-here = Todavía no hay nada aquí
lists-library-no-account = No hay ninguna cuenta conectada
lists-library-works-without = Tu historial funciona sin una. La { -library } refleja la lista de un { -tracker }.
lists-library-enable-tracker = Añade uno en { $setting } en config.toml y pulsa { $key } para abrir Cuentas.
lists-library-sign-in = Pulsa { $key } para abrir Cuentas e iniciar sesión.
lists-segment-watching = Viendo
lists-segment-planning = Planeado
lists-segment-completed = Completado
lists-segment-paused = Pausado
lists-segment-dropped = Abandonado
lists-episode = E{ $episode }
lists-episode-dubbed = E{ $episode } · { -dubbed-cap }
lists-score = { $score }%
lists-row-resume-percent = { $episode } · { $percent }%
lists-row-resume-clock = { $episode } · { $clock }
lists-episode-out = { $episode } ya salió
lists-episode-in = { $episode } en { $countdown }
lists-next-in = Próximo en { $countdown }
lists-episode-out-ago = { $episode } salió hace { $countdown }
lists-episode-out-just-now = { $episode } salió hace un momento
lists-time-in = En { $countdown }
lists-time-ago = Hace { $countdown }
lists-time-just-now = Ahora mismo
lists-time-approximate = ~{ $time }
lists-day-today = Hoy
lists-day-yesterday = Ayer
lists-day-tomorrow = Mañana
lists-preview-continue-percent = { -continue-cap } { $episode } en { $clock } ({ $percent }%)
lists-preview-continue = { -continue-cap } { $episode } en { $clock }
lists-progress-aired = { $total ->
    [one] { $done } de { $total } episodio emitido
    [many] { $done } de { $total } episodios emitidos
   *[other] { $done } de { $total } episodios emitidos
 }
lists-progress-watched = { $count ->
    [one] 1 episodio visto
    [many] { $count } episodios vistos
   *[other] { $count } episodios vistos
 }
lists-filter-genre = Género
lists-filter-year = Año
lists-filter-season = Temporada
lists-filter-format = Formato
lists-filter-status = Estado
lists-filter-sort = Orden
lists-filter-any = Cualquiera
lists-sort-relevance = Relevancia
lists-season-winter = Invierno
lists-season-spring = Primavera
lists-season-summer = Verano
lists-season-fall = Otoño
lists-format-tv = TV
lists-format-tv-short = TV corto
lists-format-movie = Película
lists-format-ova = OVA
lists-format-ona = ONA
lists-format-special = Especial
lists-format-music = Música
lists-status-airing = Emitiéndose
lists-status-finished = Finalizado
lists-status-upcoming = Próximamente
lists-sort-popularity = Popularidad
lists-sort-score = Puntuación
lists-sort-trending = Tendencia
lists-sort-newest = Más reciente
