queue-episodes-left = { $count ->
    [one] Un épisode restant dans { $title }
    [many] { $count } épisodes restants dans { $title }
   *[other] { $count } épisodes restants dans { $title }
 }
episodes-count = { $count ->
    [one] { $count } épisode
    [many] { $count } épisodes
   *[other] { $count } épisodes
 }
progress-episodes = { $total ->
    [one] { $watched } sur { $total } épisode
    [many] { $watched } sur { $total } épisodes
   *[other] { $watched } sur { $total } épisodes
 }
duration-days-hours = { $days } j { $hours } h
group-in-progress = En cours
group-new-this-week = Nouveautés cette semaine
continue-empty-title = Rien de vu pour l’instant
continue-empty-detail = Choisissez un titre dans Saisonnier, ou recherchez-en un.
continue-empty-action = Parcourir Saisonnier
continue-load-failed = Impossible de charger { -continue-cap }
continue-heading = { -continue-cap } le visionnage
continue-airing-today = Diffusés aujourd’hui
continue-this-season = Cette saison
continue-hide = Retirer de la section { -continue-cap }
continue-hidden = Masquée de la section { -continue-cap }.
continue-drop = Déplacer dans Abandonné
continue-more-for = Plus pour { $title }
continue-row-resume = { -continue-cap } E{ $episode } · { $clock }
continue-row-resume-dubbed = { -continue-cap } E{ $episode } { -dubbed } · { $clock }
continue-row-start = Lire E{ $episode }
continue-row-start-dubbed = Lire E{ $episode } { -dubbed }
continue-row-start-aired = Lire E{ $episode } · sorti { $ago }
continue-row-caught-up = À jour · E{ $episode } { $when }
continue-row-caught-up-plain = À jour
continue-widget-connect = Connectez AniList pour le voir ici.
continue-caught-up = Vous êtes à jour.
title-button-resume = { -continue-cap } l’épisode { $episode } · { $time }
title-button-play = Lire l’épisode { $episode }
title-button-add-source = Ajouter une { -source } pour lire
library-status-watching = En cours
library-status-planning = À voir
library-status-completed = Terminé
library-status-paused = En pause
library-status-dropped = Abandonné
library-sort-updated = Dernière mise à jour
library-sort-title = Titre
library-sort-score = Note
library-sort-button = Tri : { $sort }
library-sort-label = Trier par { $sort }
library-signed-out-title = Vos listes viennent d’AniList
library-signed-out-detail = Connectez-vous pour synchroniser ce que vous regardez et où vous vous êtes arrêté.
library-signed-out-action = Se connecter à AniList
library-list-empty = Rien dans votre liste { $list }
library-list-empty-detail = Les titres que vous ajoutez à cette liste sur AniList apparaissent ici.
library-list-load-failed = Impossible de charger votre liste { $list }
library-loading = Chargement de vos listes
library-add-to-list = Ajouter à une liste
library-add-planning = Ajouter à la liste À voir
library-added-planning = Ajouté à la liste À voir.
library-on-your-list = Dans votre liste
library-in-your-list = Dans votre liste
progress-episodes-short = E{ $done } sur { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Lire E{ $episode } { -dubbed } · sorti { $ago }
up-next-heading = { -up-next-cap } pour vous
up-next-starts = Commence le { $date }
up-next-reason-new-season = Nouvelle saison
up-next-reason-planning = Dans votre liste À voir
up-next-reason-recommended = Parce que vous avez terminé { $title }
up-next-reason-recommended-plain = Recommandé pour vous
up-next-because-detail = Recommandé sur AniList par des personnes qui ont aimé { $title }
up-next-reason-airing-today = Diffusé aujourd’hui
up-next-add-planning = Ajouter à la liste À voir
up-next-not-interested = Pas intéressé
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } n’est plus affichée dans la section { -up-next-cap }.
up-next-hidden-until-airs = { $title } est masquée jusqu’à sa diffusion.
up-next-broadcast = Épisode { $episode } · { $when }
up-next-out = sorti { $ago }
