stores-device-count = { $count ->
    [one] { $count } { -device }
    [many] { $count } { -device-pl }
   *[other] { $count } { -device-pl }
 }
stores-dialog-title = { $title }, { $store }
stores-owner-unreachable = Impossible de joindre la { -plugin-store }
stores-owner-no-answer = La { -plugin-store } ne répond pas
stores-owner-no-answer-detail = Elle est peut-être occupée, ou son ordinateur est éteint.
stores-owner-needs-connection = La { -plugin-store } nécessite une connexion
stores-owner-needs-connection-detail = Réessayez quand cet { -device } sera de nouveau connecté.
stores-owner-link-offline = Créer un lien nécessite une connexion à la { -plugin-store }.
stores-send-link-until = Envoyez ce lien. Il fonctionne une seule fois, jusqu’au { $until }.
stores-open-on-detail = Ils rejoignent sans nom. Désactiver cette option arrête le lien ; la réactiver en crée un nouveau.
stores-open-off-detail = Seules les personnes que vous invitez peuvent rejoindre. Une fois activé, toute personne disposant du lien rejoint, sans nom.
stores-open-no-names = Aucun nom n’est conservé.
stores-open-keep-working = Ils continuent de fonctionner jusqu’à ce que vous les retiriez.
stores-device-seen = Vu { $when }
stores-device-never-seen = Pas vu depuis son arrivée
stores-publish-short = Publier un { -plugin }
stores-publish-none-hint = Un { -plugin } que vous créez ou installez apparaît ici.
stores-not-published = Pas encore dans la { -plugin-store }
stores-store-offers = La { -plugin-store } propose { $version }
stores-published-current = À jour dans la { -plugin-store }
stores-published-state = Publié
stores-publish-label = Publier { $name } { $version }
