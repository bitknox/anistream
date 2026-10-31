widget-episode-short = Ép. { $episode }
widget-episode-long = Épisode { $episode }
widget-day-tomorrow = demain
widget-next-line = { -up-next-cap } : { $when } · { $title } · Épisode { $episode }
widget-next-line-short = { -up-next-cap } : { $when }, { $title } Ép. { $episode }
widget-next-time = Prochain à { $time }
widget-airs-day = { $title } Ép. { $episode } sort { $day }.
widget-out-ago = Sorti il y a { $time }
widget-in-time = Dans { $time }
widget-airing-title-small = Aujourd’hui
widget-nothing-airing-short = Aucune sortie.
widget-open-to-update = Ouvrez { -app-name } pour la mettre à jour.
widget-nothing-more-today = Rien d’autre aujourd’hui
widget-tomorrow-none = Demain : aucune sortie
widget-tomorrow-first = Demain : { $time } · { $title }
widget-tomorrow-more = Demain : { $time } · { $title } et { $count } de plus
widget-continue-title-apple = { -continue-cap } le visionnage
widget-continue-description = L’épisode suivant de ce que vous regardez.
widget-airing-description = Les épisodes du jour, avec les séries de votre liste en évidence.
widget-connect-detail-long = Connectez { $tracker } ou lancez un épisode pour voir ce que vous regardez ici.
widget-connect-detail-short = Connectez { $tracker } ou lancez un épisode pour le voir ici.
widget-connect-tracker = Connecter { $tracker }
widget-caught-up-title-apple = Vous êtes à jour
widget-caught-up-none = Rien d’autre à { -continue }.
widget-caught-up-next-apple = Prochain de votre liste :
widget-line-left-apple = { $episode } · reste { $left }
widget-line-resume = { -continue-cap } { $episode }
widget-action-resume = { -continue-cap } { $episode }
widget-action-start = Lire { $episode }
widget-a11y-episode = épisode { $episode }
widget-a11y-episode-marked = { $episode }, { $mark }
widget-a11y-resume = { $minutes ->
    [one] { $episode }, il reste une minute
    [many] { $episode }, il reste { $minutes } minutes
   *[other] { $episode }, il reste { $minutes } minutes
 }
widget-a11y-start = lancer { $episode }
widget-a11y-row = { $title }, { $what }
widget-a11y-airing = { $title }, épisode { $episode }, { $time }
widget-a11y-then-watched = { $label }, vu
widget-a11y-then-out = { $label }, sorti
widget-a11y-then-list = { $label }, dans votre liste
widget-out-now = Sorti maintenant
widget-airs-at = Diffusion à { $time }
widget-aired-at = Diffusé à { $time }
widget-a11y-airs-at = { $title }, épisode { $episode }, diffusion à { $time }
widget-a11y-out-now = { $title }, épisode { $episode }, sorti maintenant
widget-downloaded-line = Épisode { $episode } téléchargé
widget-downloaded-ready = Prêt à regarder hors connexion
widget-downloading-title = { $count ->
    [one] Téléchargement d’un épisode
    [many] Téléchargement de { $count } épisodes
   *[other] Téléchargement de { $count } épisodes
 }
widget-downloads-paused = En pause
widget-downloads-then-more = { $title } Ép. { $episode }, et { $count } de plus
widget-size-left = { $size } restants
widget-time-left = { $time } restantes
widget-continues-when-open = Reprend à l’ouverture de { -app-name }
widget-bytes-of = { $done } sur { $total }
widget-speed = { $size }/s
widget-downloads-episode = Épisode { $episode }
widget-a11y-downloaded = { $title }, épisode { $episode }, téléchargé
widget-a11y-downloading = { $title }, épisode { $episode }, { $percent }
widget-a11y-then-paused = { $label }, en pause
widget-a11y-then-time-left = { $label }, { $time } restantes
widget-a11y-then-count = { $count ->
    [one] { $label }, un épisode en cours de téléchargement
    [many] { $label }, { $count } épisodes en cours de téléchargement
   *[other] { $label }, { $count } épisodes en cours de téléchargement
 }
widget-play-episode-short = Lire Ép. { $episode }
widget-play-episode-long = Lire l’épisode { $episode }
widget-resume-downloads = { -continue-cap } les téléchargements
widget-pause-downloads = Mettre les téléchargements en pause
widget-paused-line = En pause · reste { $left }
widget-a11y-playback-paused = { $minutes ->
    [one] { $title }, épisode { $episode }, en pause, il reste une minute
    [many] { $title }, épisode { $episode }, en pause, il reste { $minutes } minutes
   *[other] { $title }, épisode { $episode }, en pause, il reste { $minutes } minutes
 }
widget-a11y-playback-playing = { $minutes ->
    [one] { $title }, épisode { $episode }, en lecture, il reste une minute
    [many] { $title }, épisode { $episode }, en lecture, il reste { $minutes } minutes
   *[other] { $title }, épisode { $episode }, en lecture, il reste { $minutes } minutes
 }
