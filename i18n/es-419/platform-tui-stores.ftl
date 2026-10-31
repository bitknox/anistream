stores-needs-larger-terminal = Hace falta una terminal más grande
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Miembros
stores-column-store = { -store-cap }
stores-column-plugins = Cantidad
stores-column-you = Tú
stores-column-state = Estado
stores-none-joined = Todavía no te has unido a ninguna { -store }
stores-role-owner = Dueño
stores-role-member = Miembro
stores-join-row = + Unirse a una { -store }
stores-state-revoked = Acceso retirado
stores-state-offline = Sin conexión
stores-state-up-to-date = Al día
stores-state-updates = { $count ->
    [one] { $count } actualización
    [many] { $count } actualizaciones
   *[other] { $count } actualizaciones
 }
stores-footer-revoked = Esta { -store } ya no acepta este { -device }. Su dueño puede enviar una invitación nueva.
stores-footer-offline = { $url } ({ $reason })
stores-key = Clave { $fingerprint }
stores-joined-without-name = Unido sin nombre
stores-joined-as = Unido como { $name }
stores-footer-join = Pega la invitación o el enlace de la { -store } que te dieron
stores-owner-tag = Dueño
stores-anyone-can-join = Cualquiera con el enlace puede unirse
stores-note-offline = Sin conexión. Se muestra la última lista que envió ({ $reason })
stores-note-revoked = Esta { -store } ya no acepta este { -device }. Lo que instaló sigue funcionando.
stores-column-plugin = { -plugin-cap }
stores-column-version = Versión
stores-empty-owner = Todavía no hay nada publicado. Publica un { -plugin } abajo.
stores-empty-member = Todavía no hay nada publicado
stores-line-link-device = Enlazar otro { -device }
stores-line-invite = Invitar a alguien
stores-line-members = Miembros
stores-line-who-can-join = Quién puede unirse
stores-line-publish = Publicar un { -plugin }
stores-line-admin = Abrir la página de administración
stores-line-leave = Salir de esta { -store }
stores-plugin-not-installed = No instalado
stores-plugin-installed = Instalado
stores-plugin-update-from = Actualizar desde { $from }
stores-plugin-added-by-hand = Agregado a mano
stores-plugin-from-another-store = De otra { -store }
stores-footer-update-hosts = La actualización también quiere contactar con { $hosts }
stores-footer-may-contact = Puede contactar con { $hosts }
stores-footer-link-device = Un enlace de un solo uso que une otra de tus computadoras como tú
stores-footer-invite = Un enlace de un solo uso para alguien nuevo
stores-footer-members = Quién se ha unido, y cómo quitar el acceso a alguien
stores-footer-close-join = ↵ Dejar que solo entren las personas que invites, y detener el enlace
stores-footer-open-join = ↵ Dejar que cualquiera con el enlace de la { -store } se una, sin nombre
stores-footer-publish = Ofrecer un { -plugin } desde la carpeta de { -plugin-pl } de esta computadora
stores-footer-admin = Administrar miembros, invitaciones y { -plugin-pl } en tu navegador
stores-footer-leave = { -plugin-cap-pl } instalados desde ella se quedan en esta computadora y dejan de actualizarse
stores-who-open = Cualquiera con el enlace
stores-who-invite-only = Solo las personas que invites
stores-label-offers = Ofrece
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
    [many] { $count } { -plugin-pl }
   *[other] { $count } { -plugin-pl }
 }
stores-offered-plugin = { $name } { $version }  puede contactar con { $hosts }
stores-offered-installed = Instalado
stores-offered-installed-version = { $from } instalado
stores-offered-added-by-hand = Agregado a mano
stores-offered-from-other = De { $other }
stores-offered-more = y { $count } más
stores-hosts-none = nada
stores-loading = Cargando…
stores-member-you = { $name } (tú)
stores-column-name = Nombre
stores-column-role = Rol
stores-column-devices = Cantidad
stores-column-last-seen = Última actividad
stores-members-heading = Miembros · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Invitaciones sin usar
stores-invite-another-device = { $name }, otro { -device }
stores-seen-never = Nunca
stores-ago-just-now = Hace un momento
stores-ago-today = Hoy
stores-ago-yesterday = Ayer
stores-ago-days = { $days ->
    [one] Hace { $days } día
    [many] Hace { $days } días
   *[other] Hace { $days } días
 }
stores-ago-inline-just-now = hace un momento
stores-ago-inline-today = hoy
stores-ago-inline-yesterday = ayer
stores-ago-inline-days = { $days ->
    [one] hace { $days } día
    [many] hace { $days } días
   *[other] hace { $days } días
 }
stores-expired = Vencida
stores-expires-today = Vence hoy
stores-expires-tomorrow = Vence mañana
stores-expires-days = { $days ->
    [one] Vence en { $days } día
    [many] Vence en { $days } días
   *[other] Vence en { $days } días
 }
stores-open-heading = Cualquiera con el enlace puede unirse · Unidos con el enlace: { $joined }
stores-closed-heading = Unidos con el enlace antes de cerrarlo · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
    [many] { $count } { -device-pl }
   *[other] { $count } { -device-pl }
 }
stores-open-link-row = El enlace de la { -store }
stores-open-link-note = Mostrar y copiar
stores-anonymous-joined = Unido { $when }
stores-anonymous-seen = Visto { $when }
stores-anonymous-never-seen = Todavía no visto
stores-remove-devices = { $count ->
    [one] Quitarlo
    [many] Quitar todos ({ $count })
   *[other] Quitar todos ({ $count })
 }
stores-footer-you = Eres el dueño. Una { -store } siempre conserva a su dueño.
stores-footer-remove-member = ↵ Quitar su acceso
stores-footer-cancel-invite = ↵ Cancelar esta invitación
stores-footer-show-link = ↵ Mostrar el enlace para copiarlo y compartirlo
stores-footer-remove-anonymous = ↵ Quitar este { -device }, que solo se reconoce por cuándo se unió
stores-footer-remove-all-anonymous = ↵ Quitar cada { -device } que se unió con el enlace
stores-link-copied = Copiado al portapapeles donde la terminal lo permite, o selecciónalo para copiarlo.
stores-link-select = Selecciónalo para copiarlo.
stores-open-link-warning = Cualquiera con él puede unirse sin nombre, tantas veces como quiera.
stores-open-link-lasts = Funciona hasta que la { -store } vuelva a ser solo por invitación.
stores-invite-works-once = Funciona una vez. { $expiry }.
stores-invite-preview-safe = Abrirlo en un navegador muestra cómo unirse y no la consume.
stores-confirm-join = ¿Unirte a { $name }?
stores-confirm-leave = ¿Salir de { $name }?
stores-confirm-remove-member = ¿Quitar a { $name }?
stores-confirm-cancel-invite = ¿Cancelar la invitación de { $name }?
stores-confirm-unpublish = ¿Dejar de ofrecer { $name }?
stores-confirm-open = ¿Dejar que cualquiera con el enlace se una a { $name }?
stores-confirm-close = ¿Dejar que solo entren las personas invitadas a { $name }?
stores-confirm-remove-device = ¿Quitar este { -device }?
stores-confirm-remove-devices = { $count ->
    [one] ¿Quitar el { -device } que se unió con el enlace?
    [many] ¿Quitar todos los { $count } { -device-pl } que se unieron con el enlace?
   *[other] ¿Quitar todos los { $count } { -device-pl } que se unieron con el enlace?
 }
stores-confirm-fallback = Confirmar
stores-label-address = Dirección
stores-label-key = Clave
stores-label-run-by = Administrada por
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (coincide con el enlace)
stores-key-matches-invite = { $fingerprint }  (coincide con la invitación)
stores-join-open = Cualquiera con el enlace puede unirse.
stores-join-anonymous = Te unes sin nombre, y otro { -device } se une con el mismo enlace.
stores-join-already = Ya te uniste. Volver a unirte le da a este { -device } un token nuevo.
stores-join-device-note = El dueño de la { -store } ve este nombre. Escribe para cambiarlo.
stores-leave-plugins = { -plugin-cap-pl } instalados desde ella se quedan en esta computadora y dejan de actualizarse.
stores-leave-revoked = El acceso de este { -device } a la { -store } fue retirado.
stores-remove-member-devices = { $count ->
    [one] Su { -device } deja de funcionar con esta { -store } de inmediato.
    [many] Sus { $count } { -device-pl } dejan de funcionar con esta { -store } de inmediato.
   *[other] Sus { $count } { -device-pl } dejan de funcionar con esta { -store } de inmediato.
 }
stores-remove-member-plugins = { -plugin-cap-pl } que ya instalaron se quedan en sus computadoras.
stores-cancel-invite-link = El enlace deja de funcionar.
stores-unpublish-members = Los miembros que lo instalaron lo conservan, y deja de actualizarse.
stores-open-anyone = Cualquiera con el enlace de la { -store } puede unirse, sin nombre.
stores-open-link-new = El enlace es nuevo cada vez, así que uno anterior deja de funcionar.
stores-open-members-see = Los miembros ven que la { -store } está abierta.
stores-close-link = El enlace deja de funcionar, y nadie nuevo se une sin invitación.
stores-close-devices = { -device-cap-pl } que se unieron con él siguen funcionando hasta que los quites en Miembros.
stores-anonymous-one-stops = Deja de funcionar con esta { -store } de inmediato.
stores-anonymous-all-stop = Dejan de funcionar con esta { -store } de inmediato.
stores-anonymous-plugins = { -plugin-cap-pl } ya instalados se quedan donde están.
stores-anonymous-still-joins = Mientras la { -store } esté abierta, el enlace sigue dejando unirse a cualquiera.
stores-plugin-may-contact = Puede contactar con { $hosts }
stores-plugin-cannot = No puede leer archivos, abrir conexiones de red ni ver tu entorno.
stores-plugin-update-adds = La actualización agrega { $hosts } a lo que puede contactar.
stores-plugin-added-by-hand-note = { $id }.wasm ya está en la carpeta de { -plugin-pl }, agregado a mano.
stores-plugin-other-store-note = Instalado desde otra { -store }, que lo mantiene actualizado.
stores-choice-install = Instalar
stores-choice-update = Actualizar a { $version }
stores-choice-replace = Reemplazar la copia agregada a mano
stores-choice-remove = Quitar de esta computadora
stores-choice-unpublish = Dejar de ofrecerlo en esta { -store }
stores-choice-cancel = Cancelar
stores-publish-new = Nuevo
stores-publish-published = Publicado
stores-publish-replaces = Reemplaza { $version }
stores-toast-invite-checked = La invitación es válida, pero hay otra cosa abierta. Pégala de nuevo para unirte.
stores-toast-nothing-to-publish = No hay nada en tu carpeta de { -plugin-pl } para publicar. Ejecuta "{ $command }" para ver dónde está.
stores-toast-that-is-you = Ese eres tú. Una { -store } siempre conserva a su dueño.
