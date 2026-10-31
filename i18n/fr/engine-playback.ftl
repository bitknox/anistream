notice-reason = { $reason }
cause-no-sources = Aucune { -source-pl } n'est configurée. Ajoutez-en une dans l'écran { -source-cap-pl }.
notice-not-aired = L'épisode { $episode } est diffusé { $when }.
notice-not-aired-yet = L'épisode { $episode } n'est pas encore diffusé. L'épisode { $next } est diffusé { $when }.
cause-airs-today = aujourd'hui à { $time }
cause-airs-tomorrow = demain à { $time }
cause-airs-in-days = { $days ->
    [one] dans un jour à { $time }
    [many] dans { $days } jours à { $time }
   *[other] dans { $days } jours à { $time }
 }
cause-airs-on-date = le { $date } à { $time }
cause-no-playable-stream = La { -source } a répondu, mais sans rien à lire. Essayez une autre { -source }.
cause-title-no-match = Impossible de faire correspondre ce titre : { $detail }
cause-source-gone = La { -source } { $source } n'est plus disponible. Choisissez-en une autre.
cause-release-failed = Impossible de lancer cette { -release } : { $detail }
cause-sources-timed-out = Les { -source-pl } ont mis trop de temps à répondre. Réessayez, ou choisissez une autre { -source }.
cause-no-source-has-episode = Aucune { -source } n'a l'épisode { $episode } : { $detail }
cause-source-cannot-download = Cette { -source } permet de regarder des épisodes, mais pas de les télécharger.
cause-no-source-could-list-show = Aucune { -source } n'a pu répertorier cette série, elle ne peut donc pas être téléchargée.
cause-subtitles-merged = { $tracks ->
    [one] Une piste de sous-titres a été intégrée.
    [many] { $tracks } pistes de sous-titres ont été intégrées.
   *[other] { $tracks } pistes de sous-titres ont été intégrées.
 }
cause-subtitles-left-apart = Les sous-titres restent dans des fichiers séparés. { $reason }
cause-already-in-downloads = { $count ->
    [one] Un épisode est déjà dans les téléchargements.
    [many] { $count } épisodes sont déjà dans les téléchargements.
   *[other] { $count } épisodes sont déjà dans les téléchargements.
 }
status-looking-up-title = Recherche du titre…
status-loading-sources = Chargement des { -source-pl }…
status-searching-sources = Recherche dans les { -source-pl } de l'épisode { $episode }…
notice-player-failed-to-start = Impossible de lancer mpv. Vérifiez qu'il démarre seul. ({ $reason })
notice-stream-no-video = Aucune image sur ce flux. Essai { $next } sur { $total }.
notice-player-could-not-play = mpv n'a pas pu lire ce fichier. { $reason }
notice-player-exited-silently = mpv s'est fermé sans rien lire et sans donner de raison. Essayez une autre { -source }.
notice-player-not-playing = mpv n'a pas commencé la lecture après { $seconds } s. La { -source } n'a peut-être pas de données. Vérifiez que mpv lit un fichier tout seul.
notice-no-player = Cette version de { -app-name } ne peut pas lire de vidéo.
notice-mpv-missing = { $binary } est introuvable. Installez mpv pour lire les épisodes.
notice-mpv-old = mpv { $version } peut lire des vidéos, mais le lecteur de { -app-name } demande mpv 0.36 ou plus récent pour son bouton Passer, la carte { -up-next-cap }, les menus et les miniatures.
notice-subtitles-unavailable = Les sous-titres de cette { -source } n'ont pas pu être chargés. Choisissez une autre { -source } pour réessayer.
notice-opened-in-browser = Ouvert dans votre navigateur.
notice-could-not-open = Impossible d'ouvrir ce lien. { $reason }
notice-trailer-playing = Bande-annonce en cours de lecture dans mpv.
notice-trailer-opened = Bande-annonce ouverte dans votre navigateur.
notice-trailer-failed = Impossible d'ouvrir la bande-annonce. { $reason }
notice-filler-skipped = Hors-série ignoré. { -up-next-cap } est l'épisode { $episode }.
notice-audio-now-subbed = L'audio est maintenant { -subbed }.
notice-audio-now-dubbed = L'audio est maintenant { -dubbed }.
notice-already-first-episode = Déjà au premier épisode.
notice-last-episode = Il n'y a pas d'épisode suivant. L'épisode { $episode } est le dernier.
notice-no-next-episode = Il n'y a pas d'épisode suivant.
notice-no-previous-episode = Il n'y a pas d'épisode précédent.
notice-match-reset-failed = Impossible de réinitialiser la correspondance. { $reason }
notice-match-save-failed = Impossible d'enregistrer cette correspondance. { $reason }
notice-match-did-not-stick = Cette correspondance n'a pas été retenue. La { -source } a peut-être changé. Essayez de la choisir à nouveau.
notice-source-search-empty = Aucun résultat pour « { $query } ». Essayez une autre recherche.
notice-source-pinned = Pour ce titre, la { -source } choisie est { $source }.
notice-source-automatic = Retour à la sélection automatique de la { -source }.
notice-source-choice-failed = Impossible d'enregistrer ce choix. { $reason }
notice-no-source-could-list = Aucune { -source } n'a pu répertorier cette série : { $reason }
notice-episodes-failed = Impossible de charger les épisodes. { $reason }
notice-match-uncertain = { $source } a trouvé plusieurs séries qui pourraient être celle-ci. Les épisodes affichés viennent donc d'AniList pour l'instant.
notice-continue-hide-failed = Impossible de retirer ce titre de { -continue-cap }. { $reason }
notice-download-queued = Épisode { $episode } de { $title } ajouté à la file.
notice-download-already-queued = L'épisode { $episode } de { $title } est déjà dans les téléchargements.
notice-download-starting-now = Lancement immédiat de l'épisode { $episode } de { $title }.
notice-download-cancelled = Téléchargement annulé.
notice-download-file-deleted = Téléchargement et fichier supprimés.
notice-download-removed-file-gone = Téléchargement retiré. Son fichier n'existait déjà plus.
notice-download-removed-file-kept = Téléchargement retiré, mais son fichier n'a pas pu être supprimé. { $reason }
notice-download-removed = Téléchargement retiré.
notice-downloads-cleared = { $count ->
    [one] Un téléchargement terminé a été retiré de la liste.
    [many] { $count } téléchargements terminés ont été retirés de la liste.
   *[other] { $count } téléchargements terminés ont été retirés de la liste.
 }
notice-download-finished = Épisode { $episode } de { $title } téléchargé.
notice-download-finished-note = Épisode { $episode } de { $title } téléchargé. { $note }
notice-download-failed = Impossible de télécharger l'épisode { $episode } de { $title }. { $reason }
notice-downloads-none-queued = Rien ajouté à la file.
notice-downloads-none-queued-stopped = Rien ajouté à la file. { $reason }
notice-downloads-none-new = Rien de nouveau ajouté à la file. { $existing }
notice-downloads-none-new-stopped = Rien de nouveau ajouté à la file. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] Un épisode ajouté à la file. { $existing }
    [many] { $count } épisodes ajoutés à la file. { $existing }
   *[other] { $count } épisodes ajoutés à la file. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] Un épisode ajouté à la file, puis l'ajout s'est arrêté. { $reason }
    [many] { $count } épisodes ajoutés à la file, puis l'ajout s'est arrêté. { $reason }
   *[other] { $count } épisodes ajoutés à la file, puis l'ajout s'est arrêté. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] Un épisode ajouté à la file, puis l'ajout s'est arrêté. { $existing } { $reason }
    [many] { $count } épisodes ajoutés à la file, puis l'ajout s'est arrêté. { $existing } { $reason }
   *[other] { $count } épisodes ajoutés à la file, puis l'ajout s'est arrêté. { $existing } { $reason }
 }
notice-keep-next-full = Espace libre insuffisant pour garder les épisodes suivants téléchargés. Libérez de l'espace pour en télécharger davantage.
party-needs-server = Choisissez d'abord un serveur pour le { -watch-party }, dans syncplay.server. { $server } est géré par { $operator }.
party-choose-server = Choisissez d'abord un serveur pour le { -watch-party }. { $server } est géré par { $operator }.
notice-no-party = Les { -watch-party-pl } ne sont pas disponibles dans cette version de { -app-name }.
party-needs-room = Un { -watch-party } a besoin d'un salon où se retrouver. Choisissez-en un d'abord.
party-server-lost = Connexion au serveur du { -watch-party } perdue. Nouvelle tentative en cours.
party-rejoined = De retour dans le { -watch-party }.
party-left = Vous avez quitté le { -watch-party }.
