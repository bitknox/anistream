stores-device-count = { $count ->
    [one] { $count } { -device }
    [many] { $count } { -device-pl }
   *[other] { $count } { -device-pl }
 }
stores-dialog-title = { $title }, { $store }
stores-owner-unreachable = No se pudo contactar con la { -plugin-store }
stores-owner-no-answer = La { -plugin-store } no respondió
stores-owner-no-answer-detail = Puede que esté ocupada o que su computadora esté apagada.
stores-owner-needs-connection = La { -plugin-store } necesita conexión
stores-owner-needs-connection-detail = Vuelve a intentarlo cuando este { -device } tenga conexión.
stores-owner-link-offline = Crear un enlace necesita conexión con la { -plugin-store }.
stores-send-link-until = Envía este enlace. Funciona una vez, hasta { $until }.
stores-open-on-detail = Entran sin dar un nombre. Al desactivarlo se detiene el enlace; al activarlo de nuevo se crea otro.
stores-open-off-detail = Solo entran las personas que invites. Si se activa, cualquiera con el enlace entra, sin dar un nombre.
stores-open-no-names = No se guarda ningún nombre.
stores-open-keep-working = Siguen funcionando hasta que los quites.
stores-device-seen = Visto { $when }
stores-device-never-seen = Sin ver desde que se unió
stores-publish-short = Publicar un { -plugin }
stores-publish-none-hint = Aquí aparece un { -plugin } que compilas o instalas.
stores-not-published = Todavía no está en la { -plugin-store }
stores-store-offers = La { -plugin-store } tiene la versión { $version }
stores-published-current = Al día en la { -plugin-store }
stores-published-state = Publicado
stores-publish-label = Publicar { $name } { $version }
