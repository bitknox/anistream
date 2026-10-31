party-status = { -watch-party-cap } : état
party-members = Membres
party-choose-a-source = Choisir une { -source }
party-sources-none-listed = Aucune { -source } ne liste l’épisode { $episode }. Les { -source-pl } viennent des { -plugin-pl }, qu’une { -plugin-store } ajoute.
party-sources-found-line = { $count ->
    [one] Une { -source } l’a. Celle que vous choisissez se lit avec le { -watch-party }.
    [many] { $count } { -source-pl } l’ont. Celle que vous choisissez se lit avec le { -watch-party }.
   *[other] { $count } { -source-pl } l’ont. Celle que vous choisissez se lit avec le { -watch-party }.
 }
player-could-not-start = Le lecteur n’a pas pu démarrer. Réessayez, ou choisissez une autre { -source }.
player-file-unreadable = Impossible de lire ce fichier. Essayez-en un autre.
player-inhibit-reason = Lecture d’un épisode
player-time-left = Reste { $time }
player-note-waited = { $note } · { $seconds } s
player-starting-plain = Démarrage
player-undo-tooltip = Annuler (U)
player-time-opening = { $clock } · Intro
player-time-ending = { $clock } · Générique
player-channels-mono = Mono
player-channels-stereo = Stéréo
player-channels = { $count ->
    [one] Un canal
    [many] { $count } canaux
   *[other] { $count } canaux
 }
player-translation-label = { -subbed-cap } ou { -dubbed }
