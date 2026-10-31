episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Épisode { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
    [one] Un épisode en pause
    [many] { $count } épisodes en pause
   *[other] { $count } épisodes en pause
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = { $size } restants
downloads-live-progress = { $done } sur { $total }
downloads-live-paused-one = En pause
downloads-live-waiting = En attente
settings-seed-ratio-until = Jusqu’à un ratio de { $ratio }
settings-upload-rate = { $size }/s
settings-coming-up-dubbed = { $count ->
    [one] { $names } prévient quand le doublage sort.
    [many] { $names } préviennent quand leurs doublages sortent.
   *[other] { $names } préviennent quand leurs doublages sortent.
 }
settings-coming-up-switched-off = { $count ->
    [one] { $names } est désactivé dans ses propres réglages.
    [many] { $names } sont désactivés dans leurs propres réglages.
   *[other] { $names } sont désactivés dans leurs propres réglages.
 }
settings-coming-up-opens = Une série ouvre ses réglages.
torrents-check-note-warn = { -app-name } vérifie d’où sort le trafic des torrents quand un torrent démarre, puis chaque minute pendant qu’il tourne, et vous prévient si la vérification échoue. Votre VPN ou votre proxy est ce qui vous protège.
torrents-check-note-stop = { -app-name } vérifie d’où sort le trafic des torrents quand un torrent démarre, puis chaque minute pendant qu’il tourne, et arrête les torrents si la vérification échoue. Votre VPN ou votre proxy est ce qui vous protège.
torrents-protect-proxy-name = Proxy SOCKS5
torrents-protect-vpn-name = VPN sur cet { -device }
torrents-warn-body = { -app-name } vous préviendra au lieu de mettre en pause. Si votre VPN ou votre proxy est coupé, d’autres personnes peuvent voir votre adresse IP.
torrents-proxy-as = avec le compte { $user }
title-facts-aired = Diffusion { $dates }
title-facts-airing = En cours depuis le { $date }
title-facts-airs = Diffusion à partir du { $date }
media-dates-range = du { $from } au { $to }
title-also-on-more = { $more ->
    [one] { $first } et un autre
    [many] { $first } et { $more } autres
   *[other] { $first } et { $more } autres
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } utilisés · { $free } libres
downloads-storage-used = { $used } utilisés
downloads-time-left = { $time } restant
downloads-rate = { $size }/s
downloads-uploaded = { $size } envoyés
downloads-ratio-of = { $ratio } sur { $goal }
downloads-group-summary = { $count ->
    [one] Un épisode · { $size }
    [many] { $count } épisodes · { $size }
   *[other] { $count } épisodes · { $size }
 }
episode-menu-download = Télécharger
episode-menu-download-paused = Téléchargement en pause
reason-timeout-anilist = AniList a mis trop de temps à répondre.
reason-timeout-server = Le serveur a mis trop de temps à répondre.
reason-busy-anilist = AniList est occupé. Réessayez dans une minute.
reason-busy-server = Le serveur est occupé. Réessayez dans une minute.
reason-problem-anilist = AniList a rencontré un problème.
reason-problem-server = Le serveur a rencontré un problème.
reason-silent-anilist = AniList n’a pas répondu.
reason-silent-server = Le serveur n’a pas répondu.
updates-installing-plain = Installation
updates-closes-as-installs = { -app-name } se ferme pendant l’installation
updates-open-release = Ouvrir la page
country-korea = Corée du Sud
search-genres-picked = { $count ->
    [one] Un genre
    [many] { $count } genres
   *[other] { $count } genres
 }
search-formats-picked = { $count ->
    [one] Un format
    [many] { $count } formats
   *[other] { $count } formats
 }
search-statuses-picked = { $count ->
    [one] Un statut
    [many] { $count } statuts
   *[other] { $count } statuts
 }
filter-score-any = Aucun
filter-years-from = À partir de { $year }
filter-years-between = De { $from } à { $to }
filter-tag-rank-title = Rang minimal du tag
filter-tag = Tag
search-filter-by = Filtrer par { $name }
search-loading-genres = Chargement des genres
search-paused-rate-limit = La recherche pendant la saisie est suspendue un moment, pour rester dans les limites d’AniList. Appuyez sur la touche de recherche pour chercher maintenant.
action-clear-all = Tout effacer
filter-source-material = Basé sur
search-detail-episode = { $status } · E{ $episode }
sources-use = Utiliser { $name }
sources-move = Déplacer { $name }
sources-reorder = Réorganiser
sources-version-available = Version { $version } disponible
sources-update-from = Version { $version } de { $store }.
sources-detail-off = Désactivée. Elle garde sa place et n’est pas essayée avant d’être réactivée.
sources-fallback-title = { -source-cap }
reason-source-timeout = Elle a mis trop de temps à répondre.
reason-source-busy = Elle est occupée. Réessayez dans une minute.
reason-source-problem = Elle a rencontré un problème.
reason-source-turned-away = Elle a refusé la demande.
reason-source-no-address = Son adresse est introuvable.
reason-source-unreachable = Impossible de la joindre.
stores-updates-waiting = { $count ->
    [one] Une mise à jour
    [many] { $count } mises à jour
   *[other] { $count } mises à jour
 }
stores-preview-key-link = La clé correspond au lien
stores-preview-key-invite = La clé correspond à l’invitation
stores-key-verified = Vérifiée
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = la { -plugin-store }
stores-link-sheet-detail = Ouvrez ce lien sur votre autre { -device } pour rejoindre la { -plugin-store } là-bas. Il ne fonctionne qu’une fois.
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Ouvre les comptes
accounts-card-description-signed-out = Non connecté. Ouvre les comptes
accounts-signed-in-plain = Connecté
settings-summary-notify-on = Nouveaux épisodes activés
settings-summary-notify-off = Nouveaux épisodes désactivés
settings-summary-sources = { $count ->
    [one] Une { -source }
    [many] { $count } { -source-pl }
   *[other] { $count } { -source-pl }
 }
settings-summary-no-sources = Aucune { -source-pl } pour le moment
settings-summary-torrents-on = torrents activés
settings-summary-torrents-off = torrents désactivés
library-storage-on-device = { $size } sur cet { -device }
library-storage-none = Rien sur cet { -device }
library-detail-of = E{ $next } sur { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Dernier visionnage
library-sort-current = Actif
settings-notify-permission-note = { -app-name } demande l’autorisation la première fois que vous activez ceci.
settings-notify-refused = Désactivé : Android n’autorise pas les notifications de { -app-name }
settings-notify-system-title = Réglages des notifications
settings-notify-system-detail = Son, vibration et écran verrouillé
settings-titles-line-signed-out = En anglais tant que vous n’êtes pas connecté à AniList
settings-titles-line-account-language = { $language }, selon votre compte AniList
settings-titles-line-follows = Suit votre compte AniList
settings-titles-line-english = Romaji quand il n’existe pas de titre anglais
settings-titles-line-romaji = Titres japonais en lettres latines
settings-titles-note-account = Suit votre compte AniList, réglé sur { $language }. Tant que vous n’êtes pas connecté à AniList, les titres s’affichent en anglais.
settings-titles-note-follows = Suit votre compte AniList. Tant que vous n’êtes pas connecté à AniList, les titres s’affichent en anglais.
settings-titles-note-english-missing = { $title } n’a pas de titre anglais sur AniList, il garde donc son titre en romaji.
settings-titles-note-english = Titres anglais tels qu’AniList les donne, et romaji quand il n’y en a pas.
settings-titles-note-romaji = Titres japonais en lettres latines, tels qu’AniList les donne.
settings-index-none = Aucune définie
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Partage au mieux sur cet { -device } : il ne fonctionne que pendant que { -app-name } est ouvert.
settings-downloads-count = { $count ->
    [one] Un épisode
    [many] { $count } épisodes
   *[other] { $count } épisodes
 }
settings-anilist-footer = La progression et les changements de liste se synchronisent avec AniList. La { -library } affiche vos listes AniList.
accounts-sign-in-again-button = Se reconnecter
accounts-review-differences = { $count ->
    [one] Vérifier une différence
    [many] Vérifier { $count } différences
   *[other] Vérifier { $count } différences
 }
settings-licence-title = Licence
settings-licence-value = MIT, open source et non commerciale
settings-typefaces-value = Roboto Flex et Zen Kaku Gothic New, sous licence SIL Open Font License 1.1
settings-libraries-value = JNA et AndroidX, sous licence Apache 2.0
settings-report-bug-value = Ouvre GitHub dans votre navigateur
downloads-storage-share-description = Les téléchargements occupent { $size }
settings-aside-notifications = Une notification quand un épisode d’une série que vous suivez sort : le titre de la série et le numéro de l’épisode, jamais son nom. Une série peut désactiver les siennes dans ses réglages.
settings-aside-downloads = Les téléchargements restent dans l’espace propre à { -app-name } sur cet { -device }, que les autres applications ne peuvent pas lire. Supprimer l’application les efface.
settings-aside-connection-check = Vérification de la connexion
settings-aside-sources = Les { -source-pl } sont essayées dans l’ordre, de haut en bas. Une série peut garder sa propre { -source }, choisie depuis sa page.
settings-upscaling-description = La même image fixe, la moitié gauche à l’original 480p, la moitié droite avec { $mode }
settings-upscaling-source = Original 480p
settings-upscaling-note-off-original = L’original 480p, mis à l’échelle sans traitement.
torrents-sheet-turn-on-title = Activer les torrents
torrents-sheet-protect-title = Protéger le trafic des torrents
torrents-sheet-turn-on-button = Activer
torrents-sheet-host = Hôte
torrents-sheet-port = Port
torrents-sheet-proxy-auth-note = Nom d’utilisateur et mot de passe seulement si le proxy les demande.
torrents-sheet-edit-proxy-title = Modifier le proxy
title-facts-also-on = Aussi sur
title-fetch-failed-detail = AniList n’a pas répondu.
continue-resize-queue = Redimensionner la file d’attente
continue-go-to-title = Aller à la série
continue-short-left = { $episode } · { $left }
nav-accounts-and-settings-user = { $user }, comptes et réglages
episode-unaired-label = Épisode { $number }, { $title }. Pas encore diffusé
episode-unaired-label-plain = Épisode { $number }. Pas encore diffusé
episode-unaired-airs-label = Épisode { $number }, { $title }. Diffusion { $airs }, pas encore sorti
episode-unaired-airs-label-plain = Épisode { $number }. Diffusion { $airs }, pas encore sorti
tv-sheet-on-network = Sur ce réseau
tv-sheet-not-listed = Pas dans la liste
tv-sheet-scan = Scanner le code sur la TV
tv-sheet-code-on = Code sur { $name }
tv-sheet-pairing-button = Vérification
tv-sheet-connecting = Connexion à la TV
tv-sheet-type-code = Saisissez le code
tv-sheet-volume = Volume de la TV
tv-sheet-volume-down = Baisser le volume
tv-sheet-volume-up = Monter le volume
tv-sheet-volume-value = { $value } sur 100
tv-sheet-nothing-playing = Rien en cours de lecture
tv-sheet-position = Position sur la TV
tv-sheet-position-state = { $at } sur { $total }
tv-sheet-request-titled = { $title }, épisode { $episode }
tv-sheet-request-plain = Épisode { $episode }
tv-sheet-footer = Une TV affiche cet { -device } avec le code { $code }. Oublier une TV ici l’oublie aussi pour cet { -device }.
tv-sheet-title-fallback = TV
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, aujourd’hui
calendar-day-label-airing = { $day }, des épisodes sortent
calendar-day-label-today-airing = { $day }, aujourd’hui, des épisodes sortent
continue-show-airing-in-calendar = Voir les sorties du jour dans le calendrier
continue-show-season-in-seasonal = Voir cette saison dans Saisonnier
search-searching-filters = Recherche de { $filters }
episode-number-plain = Épisode { $number }
episode-number-title-text = Épisode { $number } · { $title }
player-position-state = { $at } sur { $total }
player-sheet-finding = Recherche des { -source-pl }…
player-sheet-none = Aucune { -source-pl } trouvée pour cet épisode.
player-sheet-episodes-loading = La liste des épisodes arrive.
player-dual-audio = { -subbed-cap } et { -dubbed }
notif-new-episodes = { $count ->
    [one] Un nouvel épisode
    [many] { $count } nouveaux épisodes
   *[other] { $count } nouveaux épisodes
 }
notif-hidden-text = Nouvel épisode
notif-episode-out = L’épisode { $episode } est sorti.
notif-episode-out-dubbed = L’épisode { $episode } est sorti, { -dubbed }.
notif-episode-out-next = L’épisode { $episode } est sorti. Prochain pour vous : épisode { $next }.
notif-episode-out-dubbed-next = L’épisode { $episode } est sorti, { -dubbed }. Prochain pour vous : épisode { $next }.
device-name-phone = Téléphone Android
device-name-tablet = Tablette Android
tv-session-starting = Démarrage sur { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · { $minutes } min restantes
widget-line-of = E{ $episode } sur { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = À jour
widget-caught-up = Vous êtes à jour.
widget-caught-up-next = { $title } E{ $episode } sort { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Lire l’épisode { $episode }, { -dubbed }, de { $title }
tv-action-resume = { -continue-cap } l’épisode { $episode }
tv-action-resume-dubbed = { -continue-cap } l’épisode { $episode } · { -dubbed-cap }
tv-action-out = Lire l’épisode { $episode } (sortie { $day })
tv-action-not-out = Lire l’épisode { $episode } (pas encore sorti)
tv-out-today = aujourd’hui
tv-hero-resume = { -continue-cap } { $episode }
tv-hero-start = Lire { $episode }
