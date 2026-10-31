notice-signed-in = Connecté à { $tracker }.
notice-signed-in-keychain = Connecté à { $tracker }. Le jeton est enregistré dans le trousseau du système.
notice-signed-in-file = Connecté à { $tracker }. Aucun trousseau n'est disponible, donc le jeton est dans un fichier privé sur cet { -device }.
notice-signed-in-unsaved = Connecté à { $tracker }, mais le jeton n'a pas été enregistré. Il faudra vous reconnecter la prochaine fois. { $reason }
notice-token-not-saved = Impossible d'enregistrer le jeton de connexion. Essayez de vous reconnecter. { $reason }
notice-sign-in-failed = Impossible de vous connecter à { $tracker }. Réessayez. { $reason }
notice-sign-in-stopped = La connexion à { $tracker } s'est arrêtée avant la fin. Réessayez. { $reason }
notice-token-exchange-failed = Impossible d'obtenir un jeton de { $tracker }. Essayez de vous reconnecter. { $reason }
notice-browser-not-opened = Impossible d'ouvrir un navigateur pour la connexion. Vérifiez qu'un navigateur est installé, puis réessayez. { $reason }
notice-sign-in-not-waiting = Aucune connexion à { $tracker } n'attend cette réponse. Recommencez la connexion.
notice-sign-in-wrong-request = La connexion à { $tracker } ne correspond pas à la demande faite. Recommencez la connexion.
notice-sign-in-no-token = La connexion à { $tracker } est revenue sans jeton. Réessayez.
notice-mal-client-missing-build = Impossible de finir la connexion à MyAnimeList : cette version n'a pas de client pour elle.
notice-kitsu-signs-in-on-accounts = Kitsu se connecte depuis l'écran Comptes.
notice-sign-in-unsupported = La connexion à { $tracker } n'est pas encore prise en charge.
notice-sign-in-needs-redirect = La connexion à { $tracker } demande une redirection par le navigateur, que cette application ne peut pas recevoir.
notice-anilist-implicit-flow = Cette méthode de connexion AniList ne fonctionne que dans l'application { -app-name }. Pour vous connecter ici, mettez { $setting } à « loopback » dans le fichier de configuration.
notice-anilist-client-missing = AniList a besoin d'un identifiant et d'un secret de client. Renseignez { $client } et { $secret } dans le fichier de configuration, puis reconnectez-vous. Pour obtenir l'adresse de connexion, lancez { -app-name } --login-url.
notice-anilist-paste-flow = Coller un code n'est pas possible ici. Quittez et lancez { -app-name } --login à la place.
notice-mal-client-missing = MyAnimeList a besoin d'un identifiant de client. Enregistrez une application de type « other » sur myanimelist.net/apiconfig, puis réglez { $setting } dans le fichier de configuration.
status-waiting-for-redirect = En attente du retour de { $tracker }…
notice-mal-no-refresh-token = MyAnimeList n'a pas fourni de jeton de renouvellement. Vous devrez vous reconnecter dans un mois environ.
notice-client-id-missing = Réglez d'abord { $setting } dans le fichier de configuration.
notice-signed-out = Déconnecté de { $tracker }.
notice-signed-out-queue = { $count ->
    [one] Déconnecté de { $tracker }. Une modification en attente est conservée pour la prochaine connexion.
    [many] Déconnecté de { $tracker }. { $count } modifications en attente sont conservées pour la prochaine connexion.
   *[other] Déconnecté de { $tracker }. { $count } modifications en attente sont conservées pour la prochaine connexion.
 }
notice-sign-out-failed = Impossible de vous déconnecter de { $tracker }. Réessayez. { $reason }
notice-not-saved-because = Impossible d'enregistrer ce réglage. { $cause }
notice-protection-saved = Protection contre les torrents enregistrée.
notice-protection-not-checked = Impossible de vérifier la protection contre les torrents. { $cause }
notice-proxy-password-not-kept = Impossible de conserver le mot de passe du proxy. Saisissez-le à nouveau pour essayer de l'enregistrer. { $reason }
notice-show-setting-refused = Impossible d'enregistrer ce réglage. { $value } n'est pas une valeur valide pour { $setting }.
notice-show-not-reset = Impossible de réinitialiser les réglages de cette série. { $reason }
notice-show-no-such-subtitles = Impossible d'enregistrer ce choix de sous-titres. Cette série n'a pas utilisé ces sous-titres auparavant.
notice-torrents-waiting-for-check = Les torrents attendent que la vérification de la connexion réussisse de nouveau.
notice-mapping-data-refreshed = Données de correspondance actualisées.
notice-mapping-data-failed = Impossible d'actualiser les données de correspondance ({ $datasets }). Nouvelle tentative dans { $wait }. { $cause }
notice-episode-aired = { $title }, épisode { $episode }, a été diffusé pendant votre absence.
notice-episodes-aired = { $count ->
    [one] Un nouvel épisode d'une série que vous suivez a été diffusé. Le calendrier l'affiche.
    [many] { $count } nouveaux épisodes de séries que vous suivez ont été diffusés. Le calendrier les affiche.
   *[other] { $count } nouveaux épisodes de séries que vous suivez ont été diffusés. Le calendrier les affiche.
 }
notice-update-available = La version { $tag } est disponible. Lancez { -app-name } --update pour l'installer.
cause-confirm-no-protection = Confirmez d'abord que vous ne voulez aucune protection.
cause-confirm-warn-only = Confirmez d'abord que vous voulez seulement être averti.
cause-proxy-needs-address = Un proxy a besoin d'un hôte et d'un port.
cause-proxy-address-unusable = L'adresse du proxy ne peut pas être utilisée pour une vérification.
cause-server-unreachable = Le serveur est injoignable.
