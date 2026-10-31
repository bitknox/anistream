queue-episodes-left = { $count ->
    [one] Queda un episodio de { $title }
    [many] Quedan { $count } episodios de { $title }
   *[other] Quedan { $count } episodios de { $title }
 }
episodes-count = { $count ->
    [one] { $count } episodio
    [many] { $count } episodios
   *[other] { $count } episodios
 }
progress-episodes = { $total ->
    [one] { $watched } de { $total } episodio
    [many] { $watched } de { $total } episodios
   *[other] { $watched } de { $total } episodios
 }
duration-days-hours = { $days } d { $hours } h
group-in-progress = En curso
group-new-this-week = Nuevos esta semana
continue-empty-title = Nada visto todavía
continue-empty-detail = Elige algo de Temporada o busca un título.
continue-empty-action = Explorar Temporada
continue-load-failed = No se pudo cargar { -continue-cap }
continue-heading = { -continue-cap }
continue-airing-today = Se emiten hoy
continue-this-season = Esta temporada
continue-hide = Ocultar de { -continue-cap }
continue-hidden = Oculto de { -continue-cap }.
continue-drop = Pasar a Abandonado
continue-more-for = Más opciones para { $title }
continue-row-resume = Reanudar E{ $episode } · { $clock }
continue-row-resume-dubbed = Reanudar E{ $episode } { -dubbed } · { $clock }
continue-row-start = Reproducir E{ $episode }
continue-row-start-dubbed = Reproducir E{ $episode } { -dubbed }
continue-row-caught-up = Al día · E{ $episode } { $when }
continue-row-caught-up-plain = Al día
continue-widget-connect = Conecta AniList para verlo aquí.
continue-caught-up = Estás al día.
title-button-resume = Reanudar episodio { $episode } · { $time }
title-button-play = Reproducir episodio { $episode }
title-button-add-source = Añade una { -source } para reproducir
library-status-watching = Viendo
library-status-planning = Pendiente
library-status-completed = Completado
library-status-paused = En pausa
library-status-dropped = Abandonado
library-sort-updated = Última actualización
library-sort-title = Título
library-sort-score = Puntuación
library-sort-button = Orden: { $sort }
library-sort-label = Ordenar por { $sort }
library-signed-out-title = Tus listas vienen de AniList
library-signed-out-detail = Inicia sesión para sincronizar lo que ves y dónde te quedaste.
library-signed-out-action = Iniciar sesión en AniList
library-list-empty = No hay nada en tu lista de { $list }
library-list-empty-detail = Los títulos que añadas a esta lista en AniList aparecen aquí.
library-list-load-failed = No se pudo cargar tu lista de { $list }
library-loading = Cargando tus listas
library-add-to-list = Añadir a lista
library-add-planning = Añadir a Pendiente
library-added-planning = Añadido a Pendiente.
library-on-your-list = En tu lista
library-in-your-list = En tu lista
progress-episodes-short = E{ $done } de { $total }
progress-episode-short = E{ $done }
up-next-heading = { -up-next-cap } para ti
up-next-starts = Empieza { $date }
up-next-reason-new-season = Nueva temporada
up-next-reason-planning = En tu lista Pendiente
up-next-reason-recommended = Porque terminaste { $title }
up-next-reason-recommended-plain = Recomendado para ti
up-next-because-detail = Recomendado en AniList por personas a quienes les gustó { $title }
up-next-reason-airing-today = Se emite hoy
up-next-add-planning = Añadir a Pendiente
up-next-not-interested = No me interesa
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { -up-next-cap } para ti: se ocultó { $title }
up-next-hidden-until-airs = Se ocultará { $title } hasta que se emita.
up-next-broadcast = Episodio { $episode } · { $when }
