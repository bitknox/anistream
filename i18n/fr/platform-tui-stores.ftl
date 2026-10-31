stores-needs-larger-terminal = Il faut un terminal plus grand
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Membres
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = Vous
stores-column-state = État
stores-none-joined = Aucune { -store } rejointe pour l'instant
stores-role-owner = Gérant
stores-role-member = Membre
stores-join-row = + Rejoindre une { -store }
stores-state-revoked = Accès retiré
stores-state-offline = Hors ligne
stores-state-up-to-date = À jour
stores-state-updates = { $count ->
    [one] { $count } mise à jour
    [many] { $count } mises à jour
   *[other] { $count } mises à jour
 }
stores-footer-revoked = Cette { -store } n'accepte plus cet { -device }. Son gérant peut envoyer une nouvelle invitation.
stores-footer-offline = { $url } ({ $reason })
stores-key = Clé { $fingerprint }
stores-joined-without-name = Rejoint sans nom
stores-joined-as = Rejoint sous le nom de { $name }
stores-footer-join = Collez l'invitation ou le lien reçu pour une { -store }
stores-owner-tag = Gérant
stores-anyone-can-join = Toute personne ayant le lien peut rejoindre
stores-note-offline = Hors ligne, affichage de la dernière liste reçue ({ $reason })
stores-note-revoked = Cette { -store } n'accepte plus cet { -device }. Ce qui a été installé continue de fonctionner.
stores-column-plugin = { -plugin-cap }
stores-column-version = Version
stores-empty-owner = Rien n'est encore publié. Publiez un { -plugin } ci-dessous.
stores-empty-member = Rien n'est encore publié
stores-line-link-device = Créer un lien pour un autre { -device }
stores-line-invite = Inviter quelqu'un
stores-line-members = Membres
stores-line-who-can-join = Qui peut rejoindre
stores-line-publish = Publier un { -plugin }
stores-line-admin = Ouvrir la page d'administration
stores-line-leave = Quitter cette { -store }
stores-plugin-not-installed = Non installé
stores-plugin-installed = Installé
stores-plugin-update-from = Mise à jour depuis { $from }
stores-plugin-added-by-hand = Ajouté à la main
stores-plugin-from-another-store = Venu d'une autre { -store }
stores-footer-update-hosts = La mise à jour veut aussi contacter { $hosts }
stores-footer-may-contact = Peut contacter { $hosts }
stores-footer-link-device = Un lien à usage unique pour rejoindre sous votre nom depuis un autre de vos { -device-pl }
stores-footer-invite = Un lien à usage unique pour quelqu'un de nouveau
stores-footer-members = Qui a rejoint, et retirer l'accès de quelqu'un
stores-footer-close-join = ↵ Ne laisser rejoindre que les personnes invitées, et arrêter le lien
stores-footer-open-join = ↵ Laisser rejoindre quiconque a le lien de la { -store }, sans nom
stores-footer-publish = Proposer un { -plugin } depuis le dossier des { -plugin-pl } de cet { -device }
stores-footer-admin = Gérer les membres, les invitations et les { -plugin-pl } dans votre navigateur
stores-footer-leave = { -plugin-cap-pl } installés depuis cette { -store } restent sur cet { -device } et cessent de se mettre à jour
stores-who-open = Toute personne ayant le lien
stores-who-invite-only = Seules les personnes invitées
stores-label-offers = Propose
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
    [many] { $count } { -plugin-pl }
   *[other] { $count } { -plugin-pl }
 }
stores-offered-plugin = { $name } { $version }  peut contacter { $hosts }
stores-offered-installed = Installé
stores-offered-installed-version = { $from } installé
stores-offered-added-by-hand = Ajouté à la main
stores-offered-from-other = Depuis { $other }
stores-offered-more = et { $count } de plus
stores-hosts-none = rien
stores-loading = Chargement…
stores-member-you = { $name } (vous)
stores-column-name = Nom
stores-column-role = Rôle
stores-column-devices = { -device-cap-pl }
stores-column-last-seen = Vu pour la dernière fois
stores-members-heading = Membres · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Invitations non utilisées
stores-invite-another-device = { $name }, un autre { -device }
stores-seen-never = Jamais
stores-ago-just-now = À l'instant
stores-ago-today = Aujourd'hui
stores-ago-yesterday = Hier
stores-ago-days = { $days ->
    [one] Il y a { $days } jour
    [many] Il y a { $days } jours
   *[other] Il y a { $days } jours
 }
stores-ago-inline-just-now = à l'instant
stores-ago-inline-today = aujourd'hui
stores-ago-inline-yesterday = hier
stores-ago-inline-days = { $days ->
    [one] il y a { $days } jour
    [many] il y a { $days } jours
   *[other] il y a { $days } jours
 }
stores-expired = Expirée
stores-expires-today = Expire aujourd'hui
stores-expires-tomorrow = Expire demain
stores-expires-days = { $days ->
    [one] Expire dans { $days } jour
    [many] Expire dans { $days } jours
   *[other] Expire dans { $days } jours
 }
stores-open-heading = Toute personne ayant le lien peut rejoindre · { $joined } via ce lien
stores-closed-heading = Rejoint avec le lien avant sa fermeture · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
    [many] { $count } { -device-pl }
   *[other] { $count } { -device-pl }
 }
stores-open-link-row = Le lien de la { -store }
stores-open-link-note = Afficher et copier
stores-anonymous-joined = Rejoint { $when }
stores-anonymous-seen = Vu { $when }
stores-anonymous-never-seen = Pas encore vu
stores-remove-devices = { $count ->
    [one] Le retirer
    [many] Retirer les { $count }
   *[other] Retirer les { $count }
 }
stores-footer-you = Vous êtes le gérant. Une { -store } garde toujours son gérant.
stores-footer-remove-member = ↵ Retirer son accès
stores-footer-cancel-invite = ↵ Annuler cette invitation
stores-footer-show-link = ↵ Afficher le lien pour le copier et le partager
stores-footer-remove-anonymous = ↵ Retirer cet { -device }, identifié seulement par la date de son arrivée
stores-footer-remove-all-anonymous = ↵ Retirer chaque { -device } ayant rejoint avec le lien
stores-link-copied = Copié dans le presse-papiers lorsque le terminal le permet, sinon sélectionnez-le pour le copier.
stores-link-select = Sélectionnez-le pour le copier.
stores-open-link-warning = Toute personne qui possède le lien peut rejoindre, sans nom et autant de fois qu'elle veut.
stores-open-link-lasts = Il reste valable jusqu'à ce que la { -store } repasse sur invitation uniquement.
stores-invite-works-once = Utilisable une seule fois. { $expiry }.
stores-invite-preview-safe = L'ouvrir dans un navigateur explique comment rejoindre sans la consommer.
stores-confirm-join = Rejoindre { $name } ?
stores-confirm-leave = Quitter { $name } ?
stores-confirm-remove-member = Retirer { $name } ?
stores-confirm-cancel-invite = Annuler l'invitation de { $name } ?
stores-confirm-unpublish = Arrêter de proposer { $name } ?
stores-confirm-open = Laisser toute personne ayant le lien rejoindre { $name } ?
stores-confirm-close = Ne laisser rejoindre { $name } que les personnes invitées ?
stores-confirm-remove-device = Retirer cet { -device } ?
stores-confirm-remove-devices = { $count ->
    [one] Retirer l'{ -device } rejoint avec le lien ?
    [many] Retirer les { $count } { -device-pl } rejoints avec le lien ?
   *[other] Retirer les { $count } { -device-pl } rejoints avec le lien ?
 }
stores-confirm-fallback = Confirmer
stores-label-address = Adresse
stores-label-key = Clé
stores-label-run-by = Géré par
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (correspond au lien)
stores-key-matches-invite = { $fingerprint }  (correspond à l'invitation)
stores-join-open = Toute personne ayant le lien peut rejoindre.
stores-join-anonymous = Vous rejoignez sans nom, et un autre { -device } rejoint avec le même lien.
stores-join-already = Déjà rejoint. Rejoindre à nouveau donne à cet { -device } un nouveau jeton.
stores-join-device-note = Le gérant de la { -store } voit ce nom. Saisissez pour le modifier.
stores-leave-plugins = { -plugin-cap-pl } installés depuis cette { -store } restent sur cet { -device } et cessent de se mettre à jour.
stores-leave-revoked = L'accès de cet { -device } à la { -store } est révoqué.
stores-remove-member-devices = { $count ->
    [one] Son { -device } cesse de fonctionner avec cette { -store } immédiatement.
    [many] Ses { $count } { -device-pl } cessent de fonctionner avec cette { -store } immédiatement.
   *[other] Ses { $count } { -device-pl } cessent de fonctionner avec cette { -store } immédiatement.
 }
stores-remove-member-plugins = Les { -plugin-cap-pl } qu'ils ont déjà installés restent sur leurs { -device-pl }.
stores-cancel-invite-link = Le lien ne fonctionne plus.
stores-unpublish-members = Les membres qui l'ont installé le gardent, et il cesse de se mettre à jour.
stores-open-anyone = Toute personne ayant le lien de la { -store } peut la rejoindre, sans nom.
stores-open-link-new = Le lien est renouvelé à chaque fois, et l'ancien ne fonctionne plus.
stores-open-members-see = Les membres voient que la { -store } est ouverte.
stores-close-link = Le lien ne fonctionne plus, et plus personne ne rejoint sans invitation.
stores-close-devices = Les { -device-pl } ayant rejoint avec lui restent actifs jusqu'à ce que vous les retiriez dans Membres.
stores-anonymous-one-stops = Il cesse de fonctionner avec cette { -store } immédiatement.
stores-anonymous-all-stop = Ils cessent de fonctionner avec cette { -store } immédiatement.
stores-anonymous-plugins = Les { -plugin-cap-pl } déjà installés restent en place.
stores-anonymous-still-joins = Tant que la { -store } est ouverte, le lien permet toujours à quiconque de rejoindre.
stores-plugin-may-contact = Peut contacter { $hosts }
stores-plugin-cannot = Il ne peut pas lire de fichiers, ouvrir de connexions réseau ni voir votre environnement.
stores-plugin-update-adds = La mise à jour ajoute { $hosts } à ce qu'il peut contacter.
stores-plugin-added-by-hand-note = { $id }.wasm est déjà dans le dossier des { -plugin-pl }, ajouté à la main.
stores-plugin-other-store-note = Installé depuis une autre { -store }, qui le tient à jour.
stores-choice-install = Installer
stores-choice-update = Mettre à jour vers { $version }
stores-choice-replace = Remplacer la copie ajoutée à la main
stores-choice-remove = Retirer de cet { -device }
stores-choice-unpublish = Arrêter de le proposer dans cette { -store }
stores-choice-cancel = Annuler
stores-publish-new = Nouveau
stores-publish-published = Publié
stores-publish-replaces = Remplace { $version }
stores-toast-nothing-to-publish = Votre dossier des { -plugin-pl } ne contient aucun { -plugin } à publier. Lancez « { $command } » pour voir où il se trouve.
stores-toast-that-is-you = C'est vous. Une { -store } garde toujours son gérant.
