lists-terminal-too-small = Agrandissez le terminal
downloads-screen-title = Téléchargements
downloads-column-title = Titre
downloads-column-state = État
downloads-column-progress = Progression
downloads-empty-heading = Rien en file d'attente
downloads-empty-hint = Appuyez sur { $key } sur un titre ou un épisode pour le télécharger.
downloads-row-episode = E{ $episode }
downloads-row-episode-ahead = E{ $episode } · en avance
downloads-note-sizes = { $downloaded } / { $total }
downloads-note-waiting-metadata = En attente des métadonnées
downloads-note-torrents-off = Torrents désactivés
downloads-note-waiting-start = En attente de démarrage
downloads-state-queued = En attente
downloads-state-active = Téléchargement
downloads-state-paused = En pause
downloads-state-done = Téléchargé
downloads-state-failed = Échec
downloads-detail-ahead = Téléchargé en avance, supprimé un jour après le visionnage
downloads-detail-with-ahead = { $detail } · téléchargé en avance, supprimé un jour après le visionnage
downloads-detail-paused-at = En pause à { $downloaded } sur { $total }, il reprendra à partir de là
downloads-detail-paused-early = En pause avant le démarrage
downloads-detail-torrents-off = Les téléchargements nécessitent des torrents, désactivés (appuyez sur { $key } pour ouvrir les paramètres)
downloads-detail-queued = En attente, il démarre quand un emplacement de téléchargement se libère
downloads-detail-waiting-metadata = En attente des métadonnées du torrent
downloads-seeding-heading = Partage après visionnage
downloads-seeding-detail = { $ratio } sur { $goal } · { $left } restant · { $uploaded } envoyés
downloads-failed-no-reason = Le téléchargement a échoué, et aucune raison n'a été enregistrée.
downloads-not-finished = Ce téléchargement n'est pas encore terminé.
accounts-screen-title = Comptes
accounts-badge-sign-in = { $tracker } { $mark } reconnexion
accounts-needs-sign-in = { $tracker } demande une nouvelle connexion. Appuyez sur { $key } pour ouvrir Comptes.
accounts-conflicts-found = { $count ->
    [one] 1 différence de synchronisation trouvée. Appuyez sur { $key } pour la consulter.
    [many] { $count } différences de synchronisation trouvées. Appuyez sur { $key } pour les consulter.
   *[other] { $count } différences de synchronisation trouvées. Appuyez sur { $key } pour les consulter.
 }
accounts-conflicts-none = Aucune différence de synchronisation à consulter.
accounts-no-tracker = Aucun { -tracker } n'est configuré. Appuyez sur { $key } pour ouvrir Comptes.
accounts-finish-signing-in = Terminez la connexion à { $tracker }
accounts-step-open = 1. Ouvrez   { $url }
accounts-step-enter = 2. Saisissez
accounts-code-expires-seconds = Expire dans { $seconds } s
accounts-code-expires-minutes = Expire dans { $minutes } min
accounts-code-expired = Code expiré
accounts-column-state = État
accounts-column-queued = En file
accounts-column-token = Jeton
accounts-empty-heading = Aucun { -tracker-pl } activé
accounts-empty-hint = Ajoutez-en un dans { $setting } du fichier config.toml. Votre historique de visionnage fonctionne sans.
accounts-state-sign-in-again = À reconnecter
accounts-state-connected = Connecté
accounts-state-not-connected = Non connecté
accounts-pull-failed-today = La récupération de votre liste a échoué à { $time } : { $error }
accounts-pull-failed-dated = La récupération de votre liste a échoué le { $date } à { $time } : { $error }
accounts-pull-failed = La récupération de votre liste a échoué : { $error }
accounts-token-files = Les jetons sont stockés dans des fichiers aux droits 0600, car le trousseau du système est indisponible.
accounts-enter-sign-out = ↵ Se déconnecter
accounts-enter-sign-in = ↵ Se connecter
accounts-action-sign-in-again = Se reconnecter
accounts-action-sign-out = Se déconnecter
accounts-action-sign-in = Se connecter
accounts-rows-empty = Aucun { -tracker-pl } activé. Ajoutez-en un dans { $setting } du fichier config.toml.
accounts-row-signed-in-as = Connecté en tant que { $user }
accounts-row-connected = Connecté
accounts-row-not-connected = Non connecté
accounts-row-queued = { $count ->
    [one] { $count } en file
    [many] { $count } en file
   *[other] { $count } en file
 }
accounts-row-queued-held = { $queued } en file, { $held } en attente de correspondance
accounts-conflict-keep-mine = Garder ici
accounts-conflict-keep-theirs = Garder le leur
accounts-conflict-row = { $title }  ·  { $field } : ici { $local } / là-bas { $remote }
lists-library-title = { -library-cap }
lists-remove-wrong-list = Vous ne pouvez masquer des titres que dans la liste { -continue-cap }.
lists-continue-removed = { $title } masqué de la liste { -continue-cap }. Le regarder à nouveau le fait réapparaître.
lists-nothing-selected = Aucun titre sélectionné.
lists-search-prompt = Rechercher { $query }
lists-load-failed = Impossible de charger cette liste
lists-loading = Chargement…
lists-empty-press-enter = Appuyez sur ↵ pour rechercher
lists-empty-type-to-search = Saisissez pour rechercher
lists-empty-browse-filtered = Appuyez sur ↵ pour parcourir avec ces filtres.
lists-empty-browse-popular = Appuyez sur ↵ pour parcourir les plus populaires.
lists-empty-no-matches = Aucun résultat
lists-empty-fewer-filters = Essayez avec moins de filtres.
lists-empty-shorter-query = Essayez une recherche plus courte, ou le titre en romaji.
lists-empty-nothing-watched = Rien de visionné pour l'instant
lists-empty-ways-on = Appuyez sur { $seasonal } pour la saison en cours, ou sur { $search } pour rechercher.
lists-empty-nothing-to-continue = Rien à { -continue }
lists-empty-hidden-come-back = Les titres masqués réapparaissent quand vous les regardez. Appuyez sur { $seasonal } pour la saison en cours, ou sur { $search } pour rechercher.
lists-empty-nothing-here = Rien ici pour l'instant
lists-library-no-account = Aucun compte connecté
lists-library-works-without = Votre historique de visionnage fonctionne sans. La { -library } reflète la liste d'un { -tracker }.
lists-library-enable-tracker = Ajoutez-en un dans { $setting } du fichier config.toml, puis appuyez sur { $key } pour ouvrir Comptes.
lists-library-sign-in = Appuyez sur { $key } pour ouvrir Comptes et vous connecter.
lists-segment-watching = En cours
lists-segment-planning = À voir
lists-segment-completed = Terminé
lists-segment-paused = En pause
lists-segment-dropped = Abandonné
lists-episode = E{ $episode }
lists-episode-dubbed = E{ $episode } · { -dubbed-cap }
lists-score = { $score } %
lists-row-resume-percent = { $episode } · { $percent } %
lists-row-resume-clock = { $episode } · { $clock }
lists-episode-out = { $episode } disponible
lists-episode-in = { $episode } dans { $countdown }
lists-next-in = Prochain dans { $countdown }
lists-episode-out-ago = { $episode } sorti il y a { $countdown }
lists-episode-out-just-now = { $episode } sorti à l'instant
lists-time-in = Dans { $countdown }
lists-time-ago = Il y a { $countdown }
lists-time-just-now = À l'instant
lists-time-approximate = ~{ $time }
lists-day-today = Aujourd'hui
lists-day-yesterday = Hier
lists-day-tomorrow = Demain
lists-preview-continue-percent = { -continue-cap } { $episode } à { $clock } ({ $percent } %)
lists-preview-continue = { -continue-cap } { $episode } à { $clock }
lists-progress-aired = { $total ->
    [one] { $done } sur { $total } épisode diffusé
    [many] { $done } sur { $total } épisodes diffusés
   *[other] { $done } sur { $total } épisodes diffusés
 }
lists-progress-watched = { $count ->
    [one] 1 épisode visionné
    [many] { $count } épisodes visionnés
   *[other] { $count } épisodes visionnés
 }
lists-filter-genre = Genre
lists-filter-year = Année
lists-filter-season = Saison
lists-filter-format = Format
lists-filter-status = Statut
lists-filter-sort = Tri
lists-filter-any = Tous
lists-sort-relevance = Pertinence
lists-season-winter = Hiver
lists-season-spring = Printemps
lists-season-summer = Été
lists-season-fall = Automne
lists-format-tv = TV
lists-format-tv-short = TV courte
lists-format-movie = Film
lists-format-ova = OVA
lists-format-ona = ONA
lists-format-special = Spécial
lists-format-music = Musique
lists-status-airing = En diffusion
lists-status-finished = Terminée
lists-status-upcoming = À venir
lists-sort-popularity = Popularité
lists-sort-score = Score
lists-sort-trending = Tendances
lists-sort-newest = Plus récent
