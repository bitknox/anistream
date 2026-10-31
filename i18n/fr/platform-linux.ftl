source-picker-seeders-linux = { $count ->
    [one] Un partageur
    [many] { $count } partageurs
   *[other] { $count } partageurs
 }
sidebar-show = Afficher le menu
activity-continue = Chargement de ce que vous regardez
activity-calendar = Chargement du calendrier
activity-episodes = Chargement des épisodes
activity-sources = Vérification des { -source-pl }
activity-downloads = Chargement des téléchargements
activity-browse = Chargement des titres
size-per-second = { $size }/s
time-now = Maintenant
time-just-now = À l’instant
spoken-hours = { $count ->
    [one] { $count } heure
    [many] { $count } heures
   *[other] { $count } heures
 }
spoken-minutes = { $count ->
    [one] { $count } minute
    [many] { $count } minutes
   *[other] { $count } minutes
 }
spoken-seconds = { $count ->
    [one] { $count } seconde
    [many] { $count } secondes
   *[other] { $count } secondes
 }
continue-row-caught-up-airs = À jour · E{ $episode } sort { $when }
continue-row-caught-up-not-out = À jour · E{ $episode } n’est pas encore sorti
language-name-english = Anglais
language-name-japanese = Japonais
language-name-spanish = Espagnol
language-name-portuguese = Portugais
language-name-french = Français
language-name-german = Allemand
language-name-italian = Italien
language-name-russian = Russe
language-name-arabic = Arabe
language-name-indonesian = Indonésien
language-name-korean = Coréen
language-name-chinese = Chinois
seasonal-format-all = Tous
seasonal-format-ona-ova = ONA et OVA
title-open-named = Ouvrir { $title }
continue-row-spoken = { $title }, { $detail }
continue-preview = Aperçu
up-next-airs-at = Sort à { $time }
