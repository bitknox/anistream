episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Episodio { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
    [one] Un episodio en pausa
    [many] { $count } episodios en pausa
   *[other] { $count } episodios en pausa
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = { $size } restantes
downloads-live-progress = { $done } de { $total }
downloads-live-paused-one = En pausa
downloads-live-waiting = En espera
settings-seed-ratio-until = Hasta una proporción de { $ratio }
settings-upload-rate = { $size }/s
settings-coming-up-dubbed = { $count ->
    [one] { $names } avisa cuando salga su doblaje.
    [many] { $names } avisan cuando salga su doblaje.
   *[other] { $names } avisan cuando salga su doblaje.
 }
settings-coming-up-switched-off = { $count ->
    [one] { $names } está desactivada en sus propios ajustes.
    [many] { $names } están desactivadas en sus propios ajustes.
   *[other] { $names } están desactivadas en sus propios ajustes.
 }
settings-coming-up-opens = Una serie abre sus ajustes.
torrents-check-note-warn = { -app-name } revisa por dónde sale el tráfico de torrents cuando empieza un torrent y cada minuto mientras se ejecuta, y te avisa si esa revisión falla. Tu VPN o proxy es lo que te protege.
torrents-check-note-stop = { -app-name } revisa por dónde sale el tráfico de torrents cuando empieza un torrent y cada minuto mientras se ejecuta, y detiene los torrents si esa revisión falla. Tu VPN o proxy es lo que te protege.
torrents-protect-proxy-name = Proxy SOCKS5
torrents-protect-vpn-name = VPN en este { -device }
torrents-warn-body = { -app-name } te avisará en lugar de pausar. Si tu VPN o proxy no funciona, otras personas pueden ver tu dirección IP.
torrents-proxy-as = como { $user }
title-facts-aired = Emitida { $dates }
title-facts-airing = En emisión desde { $date }
title-facts-airs = Se emite desde { $date }
media-dates-range = { $from } a { $to }
title-also-on-more = { $more ->
    [one] { $first } y uno más
    [many] { $first } y { $more } más
   *[other] { $first } y { $more } más
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } usados · { $free } libres
downloads-storage-used = { $used } usados
downloads-time-left = { $time } restantes
downloads-rate = { $size }/s
downloads-uploaded = { $size } subidos
downloads-ratio-of = { $ratio } de { $goal }
downloads-group-summary = { $count ->
    [one] Un episodio · { $size }
    [many] { $count } episodios · { $size }
   *[other] { $count } episodios · { $size }
 }
episode-menu-download = Descargar
episode-menu-download-paused = Descarga en pausa
reason-timeout-anilist = AniList tardó demasiado en responder.
reason-timeout-server = El servidor tardó demasiado en responder.
reason-busy-anilist = AniList está ocupado. Inténtalo de nuevo en un minuto.
reason-busy-server = El servidor está ocupado. Inténtalo de nuevo en un minuto.
reason-problem-anilist = AniList tuvo un problema.
reason-problem-server = El servidor tuvo un problema.
reason-silent-anilist = AniList no respondió.
reason-silent-server = El servidor no respondió.
updates-installing-plain = Instalando
updates-closes-as-installs = { -app-name } se cierra mientras se instala
updates-open-release = Abrir página
country-korea = Corea del Sur
search-genres-picked = { $count ->
    [one] Un género
    [many] { $count } géneros
   *[other] { $count } géneros
 }
search-formats-picked = { $count ->
    [one] Un formato
    [many] { $count } formatos
   *[other] { $count } formatos
 }
search-statuses-picked = { $count ->
    [one] Un estado
    [many] { $count } estados
   *[other] { $count } estados
 }
filter-score-any = Cualquiera
filter-years-from = Desde { $year }
filter-years-between = { $from } a { $to }
filter-tag-rank-title = Peso mínimo de la etiqueta
filter-tag = Etiqueta
search-filter-by = Filtrar por { $name }
search-loading-genres = Cargando géneros
search-paused-rate-limit = La búsqueda mientras escribes se pausa un momento, para no superar los límites de AniList. Presiona la tecla de búsqueda para buscar ahora.
action-clear-all = Borrar todo
filter-source-material = Basado en
search-detail-episode = { $status } · E{ $episode }
sources-use = Usar { $name }
sources-move = Mover { $name }
sources-reorder = Reordenar
sources-version-available = { $version } disponible
sources-update-from = Versión { $version } de { $store }.
sources-detail-off = Desactivada. Conserva su lugar y no se prueba hasta que vuelva a activarse.
sources-fallback-title = { -source-cap }
reason-source-timeout = Tardó demasiado en responder.
reason-source-busy = Está ocupada. Inténtalo de nuevo en un minuto.
reason-source-problem = Tuvo un problema.
reason-source-turned-away = Rechazó la solicitud.
reason-source-no-address = No se encontró su dirección.
reason-source-unreachable = No se pudo contactar.
stores-updates-waiting = { $count ->
    [one] Una actualización
    [many] { $count } actualizaciones
   *[other] { $count } actualizaciones
 }
stores-checked-ago = revisada { $ago }
stores-preview-key-link = La clave coincide con el enlace
stores-preview-key-invite = La clave coincide con la invitación
stores-key-verified = Verificada
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = la { -store }
stores-link-sheet-detail = Abre este enlace en tu otro { -device } para unirte a la { -plugin-store } ahí. Funciona una sola vez.
accounts-synced-ago = Sincronizado { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Abre Cuentas
accounts-card-description-signed-out = Sin sesión iniciada. Abre Cuentas
accounts-signed-in-plain = Sesión iniciada
settings-summary-notify-on = Nuevos episodios activados
settings-summary-notify-off = Nuevos episodios desactivados
settings-summary-sources = { $count ->
    [one] Una { -source }
    [many] { $count } { -source-pl }
   *[other] { $count } { -source-pl }
 }
settings-summary-no-sources = Sin { -source-pl } todavía
settings-summary-torrents-on = torrents activados
settings-summary-torrents-off = torrents desactivados
library-storage-on-device = { $size } en este { -device }
library-storage-none = Nada en este { -device }
library-detail-of = E{ $next } de { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Última vez vista
library-sort-current = Actual
settings-notify-permission-note = { -app-name } pide permiso la primera vez que lo activas.
settings-notify-refused = Desactivado: Android no permite las notificaciones de { -app-name }
settings-notify-system-title = Ajustes de notificaciones
settings-notify-system-detail = Sonido, vibración y pantalla de bloqueo
settings-titles-line-signed-out = En inglés mientras no hayas iniciado sesión en AniList
settings-titles-line-account-language = { $language }, de tu cuenta de AniList
settings-titles-line-follows = Sigue tu cuenta de AniList
settings-titles-line-english = Romaji donde no hay título en inglés
settings-titles-line-romaji = Títulos japoneses con letras latinas
settings-titles-note-account = Sigue tu cuenta de AniList, que está en { $language }. Mientras no hayas iniciado sesión en AniList, los títulos se muestran en inglés.
settings-titles-note-follows = Sigue tu cuenta de AniList. Mientras no hayas iniciado sesión en AniList, los títulos se muestran en inglés.
settings-titles-note-english-missing = { $title } no tiene título en inglés en AniList, así que conserva su título en romaji.
settings-titles-note-english = Títulos en inglés como los lista AniList, y romaji cuando no hay.
settings-titles-note-romaji = Títulos japoneses escritos con letras latinas, como los lista AniList.
settings-index-none = Sin configurar
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Compartir funciona a mejor esfuerzo en este { -device }: solo mientras { -app-name } está abierta.
settings-downloads-count = { $count ->
    [one] Un episodio
    [many] { $count } episodios
   *[other] { $count } episodios
 }
settings-anilist-footer = Los avances y los cambios de lista se sincronizan con AniList. { -library-cap } muestra tus listas de AniList.
accounts-sign-in-again-button = Volver a iniciar sesión
accounts-review-differences = { $count ->
    [one] Revisa una diferencia
    [many] Revisa { $count } diferencias
   *[other] Revisa { $count } diferencias
 }
settings-licence-title = Licencia
settings-licence-value = MIT, código abierto y no comercial
settings-typefaces-value = Roboto Flex y Zen Kaku Gothic New, bajo la SIL Open Font License 1.1
settings-libraries-value = JNA y AndroidX, bajo la Apache License 2.0
settings-report-bug-value = Abre GitHub en tu navegador
downloads-storage-share-description = Las descargas ocupan { $size }
settings-aside-notifications = Una notificación cuando sale un episodio de una serie que sigues: el nombre de la serie y el número del episodio, nunca el título del episodio. Una serie puede desactivar sus propios avisos en sus ajustes.
settings-aside-downloads = Las descargas se guardan en el espacio propio de { -app-name } en este { -device }, al que otras apps no pueden acceder. Al quitar la app, se borran.
settings-aside-connection-check = Revisión de conexión
settings-aside-sources = { -source-cap-pl } se prueban en orden, de arriba abajo. Una serie puede conservar su propia { -source }, elegida desde su página.
settings-upscaling-description = La misma imagen: la mitad izquierda es el 480p original, la derecha con { $mode }
settings-upscaling-source = 480p original
settings-upscaling-note-off-original = El 480p original, escalado para ajustarse sin procesar.
torrents-sheet-turn-on-title = Activar torrents
torrents-sheet-protect-title = Proteger el tráfico de torrents
torrents-sheet-turn-on-button = Activar
torrents-sheet-host = Host
torrents-sheet-port = Puerto
torrents-sheet-proxy-auth-note = Usuario y contraseña solo si el proxy los pide.
torrents-sheet-edit-proxy-title = Editar proxy
title-facts-also-on = También en
title-fetch-failed-detail = AniList no respondió.
continue-resize-queue = Cambiar el tamaño de la cola
continue-go-to-title = Ir al título
continue-short-left = { $episode } · { $left }
continue-airing-out = E{ $episode } · { $time } · salió { $ago }
continue-airing-in = E{ $episode } · { $time } · en { $countdown }
calendar-line-out = { $episode } salió { $ago }
calendar-line-in = { $episode } en { $countdown }
nav-accounts-and-settings-user = { $user }, cuentas y ajustes
episode-unaired-label = Episodio { $number }, { $title }. Aún no sale
episode-unaired-label-plain = Episodio { $number }. Aún no sale
episode-unaired-airs-label = Episodio { $number }, { $title }. Sale { $airs }, todavía no disponible
episode-unaired-airs-label-plain = Episodio { $number }. Sale { $airs }, todavía no disponible
tv-sheet-on-network = En esta red
tv-sheet-not-listed = No aparece
tv-sheet-scan = Escanear el código en la TV
tv-sheet-code-on = Código en { $name }
tv-sheet-pairing-button = Vinculando
tv-sheet-connecting = Conectando con la TV
tv-sheet-type-code = Escribe el código
tv-sheet-volume = Volumen de la TV
tv-sheet-volume-down = Bajar volumen
tv-sheet-volume-up = Subir volumen
tv-sheet-volume-value = { $value } de 100
tv-sheet-nothing-playing = Nada en reproducción
tv-sheet-position = Posición en la TV
tv-sheet-position-state = { $at } de { $total }
tv-sheet-request-titled = { $title }, episodio { $episode }
tv-sheet-request-plain = Episodio { $episode }
tv-sheet-footer = Una TV muestra este { -device } con el código { $code }. Si olvidas una TV aquí, también olvida este { -device }.
tv-sheet-title-fallback = TV
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, hoy
calendar-day-label-airing = { $day }, hay episodios en emisión
calendar-day-label-today-airing = { $day }, hoy, hay episodios en emisión
continue-show-airing-in-calendar = Ver en Calendario las series que salen hoy
continue-show-season-in-seasonal = Ver esta temporada en Temporada
search-searching-filters = Buscando { $filters }
episode-number-plain = Episodio { $number }
episode-number-title-text = Episodio { $number } · { $title }
player-position-state = { $at } de { $total }
player-sheet-finding = Buscando { -source-pl }…
player-sheet-none = No se encontró ninguna { -source } para este episodio.
player-sheet-episodes-loading = La lista de episodios está en camino.
player-dual-audio = { -subbed-cap } y { -dubbed }
notif-new-episodes = { $count ->
    [one] Un episodio nuevo
    [many] { $count } episodios nuevos
   *[other] { $count } episodios nuevos
 }
notif-hidden-text = Episodio nuevo
notif-episode-out = Ya salió el episodio { $episode }.
notif-episode-out-dubbed = Ya salió el episodio { $episode }, { -dubbed }.
notif-episode-out-next = Ya salió el episodio { $episode }. Sigue para ti: episodio { $next }.
notif-episode-out-dubbed-next = Ya salió el episodio { $episode }, { -dubbed }. Sigue para ti: episodio { $next }.
device-name-phone = Teléfono Android
device-name-tablet = Tableta Android
tv-session-starting = Iniciando en { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · faltan { $minutes } min
widget-line-of = E{ $episode } de { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = Al día
widget-caught-up = Estás al día.
widget-caught-up-next = { $title } E{ $episode } sale { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Reproducir el episodio { $episode }, { -dubbed }, de { $title }
tv-action-resume = Reanudar episodio { $episode }
tv-action-resume-dubbed = Reanudar episodio { $episode } · { -dubbed-cap }
tv-action-out = Reproducir episodio { $episode } (sale { $day })
tv-action-not-out = Reproducir episodio { $episode } (aún no sale)
tv-out-today = hoy
tv-hero-resume = Reanudar { $episode }
tv-hero-start = Reproducir { $episode }
