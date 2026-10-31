store-joined = Vous avez rejoint { $store } en tant que { $member }.
store-joined-unnamed = Vous avez rejoint { $store } sans nom.
store-left = Vous avez quitté { $store }. Ses { -plugin-pl } restent installés.
plugin-removed = { $plugin } supprimé.
store-member-removed = Membre retiré. Ses { -device-pl } ne fonctionnent plus avec cette { -plugin-store }.
store-invite-cancelled = Invitation annulée. Le lien ne fonctionne plus.
store-open-join-on = Toute personne disposant du lien de la { -plugin-store } peut désormais la rejoindre. Copiez-le depuis l’écran des membres.
store-open-join-off = Seules les personnes que vous invitez peuvent désormais rejoindre.
store-open-join-off-devices = { $devices ->
    [one] Seules les personnes que vous invitez peuvent désormais rejoindre. L’{ -device } qui a rejoint avec le lien reste actif jusqu’à ce que vous le retiriez.
    [many] Seules les personnes que vous invitez peuvent désormais rejoindre. Les { $devices } { -device-pl } qui ont rejoint avec le lien restent actifs jusqu’à ce que vous les retiriez.
   *[other] Seules les personnes que vous invitez peuvent désormais rejoindre. Les { $devices } { -device-pl } qui ont rejoint avec le lien restent actifs jusqu’à ce que vous les retiriez.
 }
store-device-removed = Cet { -device } ne fonctionne plus avec cette { -plugin-store }.
store-devices-removed = Tous les { -device-pl } ayant rejoint avec le lien ont été retirés.
plugin-published = { $plugin } { $version } publié.
plugin-unpublished = { $plugin } n’est plus proposé.
store-admin-opened = Page d’administration ouverte dans votre navigateur. Le lien ne fonctionne qu’une fois, pendant 10 minutes.
store-admin-open-failed = Impossible d’ouvrir un navigateur ({ $reason }). Sur la machine de la { -plugin-store }, lancez « { -app-name } store admin » pour afficher un lien.
plugin-installed-unused = { $plugin } { $version } installé, mais les { -plugin-pl } sont désactivés dans l’ordre des { -source-pl }, il n’est donc pas utilisé.
plugin-installed-name-clash = { $plugin } installé, mais { $id } est le nom d’une { -source } intégrée, il n’a donc pas été chargé.
plugin-installed = { $plugin } { $version } installé.
plugin-installed-not-loaded = { $plugin } installé, mais il n’a pas pu se charger. { $reason }
plugin-updates-ready = { $count ->
    [one] Mise à jour prête : { $names }.
    [many] Mises à jour prêtes : { $names }.
   *[other] Mises à jour prêtes : { $names }.
 }
plugins-load-failed = Impossible de charger les { -plugin-pl }. { $reason }
store-token-missing = Il n’y a pas de jeton pour { $store } sur cet { -device }. Rejoignez { $store } avec une nouvelle invitation.
store-not-joined = Vous n’avez plus rejoint cette { -plugin-store }.
store-key-damaged = La clé de cette { -plugin-store } sur cet { -device } est endommagée. Quittez la { -plugin-store } et rejoignez-la avec une nouvelle invitation.
store-device-refused = { $store } n’accepte plus cet { -device }. Rejoignez { $store } avec une nouvelle invitation.
store-unreachable = Impossible de joindre { $store }. { $reason }
store-moved = Vous avez rejoint { $store } à l’adresse { $current }, mais ce lien pointe vers { $target }. Si la { -plugin-store } a changé d’adresse, quittez-la d’abord, puis rejoignez-la.
store-join-would-unname = Vous avez rejoint { $store } en tant que { $member }. Ce lien vous ferait rejoindre sans votre nom : rien n’a été modifié.
store-not-the-invited = { $url } n’est pas la { -plugin-store } de l’invitation ({ $reason }). Rien n’a été rejoint.
store-token-not-kept = Impossible de conserver le jeton de cette { -plugin-store }. { $reason }
store-invite-unusable = Cette invitation a déjà été utilisée, a été annulée ou a expiré. Demandez-en une nouvelle.
store-open-link-unusable = Ce lien ne permet plus à personne de rejoindre. Demandez au propriétaire de la { -plugin-store } un nouveau lien ou une invitation.
plugin-reserved-name = { $plugin } est le nom d’une { -source } intégrée, il ne peut pas être un { -plugin }.
store-plugin-withdrawn = { $store } ne propose plus { $plugin }.
store-plugin-changed = { $store } a modifié { $plugin } depuis qu’il a été affiché. Consultez-le de nouveau avant de l’installer.
plugin-from-other-store = { $plugin } est installé depuis { $store }, qui le tient à jour.
plugin-duplicate-file = { $path } déclare déjà { $plugin }. Supprimez d’abord ce fichier, sinon deux copies s’exécuteraient.
plugin-install-failed = Impossible d’installer { $plugin }. { $reason }
plugin-publish-failed = Impossible de publier { $plugin }. { $reason }
plugin-missing-locally = { $plugin } n’est plus dans le dossier du { -plugin }.
store-invite-needs-link = Cet { -device } a rejoint { $store } avec son lien. Collez le même lien sur l’autre { -device }.
store-link-malformed = La { -plugin-store } a envoyé un lien qui ne fonctionne pas. Demandez à son propriétaire de vérifier la { -plugin-store }. { $reason }
store-link-wrong-key = La { -plugin-store } a créé un lien avec une autre clé. Vérifiez l’adresse utilisée pour la lancer.
store-admin-missing = { $store } n’a pas de page d’administration. Son serveur est peut-être plus ancien.
store-admin-link-foreign = { $store } a envoyé un lien d’administration vers une autre adresse.
store-open-join-unsupported = Le serveur de { $store } est plus ancien et ne permet pas de rejoindre avec un lien. Mettez-le d’abord à jour.
source-switched-off = Désactivé : { $name }. Cette { -source } est ignorée lors de la recherche d’épisodes.
source-switched-on = Activé : { $name }.
plugin-not-loaded = { $id } n’est pas chargé. Vérifiez qu’il est installé sous les { -source-pl }.
release-pinned = { $name } sera utilisé pour ce titre.
notice-choice-not-saved = Impossible d’enregistrer ce choix. { $reason }
torrent-check-failed-running = La vérification de la connexion a échoué. Les torrents continuent de tourner.
torrent-vpn-waiting = Les torrents démarrent une fois le contrôle VPN validé. { $reason }
torrent-vpn-unverified = Les torrents démarrent une fois le contrôle VPN validé. La connexion n’a pas encore été vérifiée.
torrent-no-index = Les torrents ont besoin d’un { -torrent-index }. { -app-name } n’en fournit aucun : ajoutez votre propre adresse dans les réglages.
torrent-interface-closed = { -app-name } s’est fermé avant le démarrage des torrents.
torrent-session-failed = Impossible de démarrer la session torrent. { $reason }
torrent-no-kill-switch = Rien sur cet { -device } n’arrête le trafic torrent si le VPN tombe. Lancez « { -app-name } --doctor » pour le configurer.
torrent-not-checked = Impossible de vérifier la connexion. { $reason }
sources-reloaded-torrents-pending = { -source-cap-pl } rechargées. Les torrents démarrent une fois le contrôle VPN validé.
sources-reloaded-torrents-off = Torrents désactivés. La session a été arrêtée.
job-anilist-limit = En attente de la limite de requêtes d’AniList
notice-anilist-limit = Attente de { $wait } à cause de la limite d’AniList.
notice-anilist-unreachable = AniList ne répond pas. Affichage des données d’un autre service pour l’instant.
notice-metadata-cached = Aucun service de métadonnées n’a répondu. Affichage des dernières données en cache.
notice-anilist-back = AniList est de nouveau disponible.
notice-party-not-built = { -watch-party-cap-pl } ne sont pas inclus dans cette application.
