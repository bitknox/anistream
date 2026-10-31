store-joined = Te uniste a { $store } como { $member }.
store-joined-unnamed = Te uniste a { $store } sin nombre.
store-left = Saliste de { $store }. Sus { -plugin-pl } siguen instalados.
plugin-removed = Se quitó { $plugin }.
store-member-removed = Se quitó al miembro. Sus { -device-pl } dejaron de funcionar con esta { -plugin-store }.
store-invite-cancelled = Invitación cancelada. El enlace ya no funciona.
store-open-join-on = Cualquiera con el enlace de la { -plugin-store } puede unirse ahora. Cópialo desde la pantalla de miembros.
store-open-join-off = Ahora solo pueden unirse las personas a las que invitas.
store-open-join-off-devices = { $devices ->
    [one] Ahora solo pueden unirse las personas a las que invitas. El { -device } que se unió con el enlace sigue funcionando hasta que lo quites.
    [many] Ahora solo pueden unirse las personas a las que invitas. Los { $devices } { -device-pl } que se unieron con el enlace siguen funcionando hasta que los quites.
   *[other] Ahora solo pueden unirse las personas a las que invitas. Los { $devices } { -device-pl } que se unieron con el enlace siguen funcionando hasta que los quites.
 }
store-device-removed = Ese { -device } ya no funciona con esta { -plugin-store }.
store-devices-removed = Se quitaron todos los { -device-pl } que se unieron con el enlace.
plugin-published = Se publicó { $plugin } { $version }.
plugin-unpublished = { $plugin } ya no se ofrece.
store-admin-opened = Se abrió la página de administración en tu navegador. El enlace funciona una vez, durante 10 minutos.
store-admin-open-failed = No se pudo abrir el navegador ({ $reason }). En el equipo de la { -plugin-store }, ejecuta "{ -app-name } store admin" para imprimir un enlace.
plugin-installed-unused = Se instaló { $plugin } { $version }, pero los { -plugin-pl } están desactivados en el orden de las { -source-pl }, así que no se usa.
plugin-installed-name-clash = Se instaló { $plugin }, pero { $id } es el nombre de una { -source } integrada, así que no se cargó.
plugin-installed = Se instaló { $plugin } { $version }.
plugin-installed-not-loaded = Se instaló { $plugin }, pero no se cargó. { $reason }
plugin-updates-ready = { $count ->
    [one] Actualización de { -plugin-cap } lista: { $names }.
    [many] Actualizaciones de { -plugin-cap } listas: { $names }.
   *[other] Actualizaciones de { -plugin-cap } listas: { $names }.
 }
plugins-load-failed = No se pudieron cargar los { -plugin-pl }. { $reason }
store-token-missing = No hay token para { $store } en este { -device }. Vuelve a unirte con una invitación nueva.
store-not-joined = Ya no estás en esa { -plugin-store }.
store-key-damaged = La clave de esta { -plugin-store } en este { -device } está dañada. Sal de la { -plugin-store } y vuelve a unirte con una invitación nueva.
store-device-refused = { $store } ya no acepta este { -device }. Vuelve a unirte con una invitación nueva.
store-unreachable = No se pudo conectar con { $store }. { $reason }
store-moved = Ya te uniste a { $store } en { $current }, pero este enlace apunta a { $target }. Si la { -plugin-store } cambió de dirección, sal primero y vuelve a unirte.
store-join-would-unname = Te uniste a { $store } como { $member }. Este enlace te uniría otra vez sin tu nombre, así que no se cambió nada.
store-not-the-invited = { $url } no es la { -plugin-store } de la invitación ({ $reason }). No te uniste a nada.
store-token-not-kept = No se pudo guardar el token de esta { -plugin-store }. { $reason }
store-invite-unusable = Esa invitación ya se usó, se canceló o venció. Pide una nueva.
store-open-link-unusable = Este enlace ya no deja unirse a nadie. Pídele al responsable de la { -plugin-store } un enlace nuevo o una invitación.
plugin-reserved-name = { $plugin } es el nombre de una { -source } integrada, así que no puede ser un { -plugin }.
store-plugin-withdrawn = { $store } ya no ofrece { $plugin }.
store-plugin-changed = { $store } cambió { $plugin } desde que se mostró. Revísalo otra vez antes de instalarlo.
plugin-from-other-store = { $plugin } se instaló desde { $store }, que lo mantiene actualizado.
plugin-duplicate-file = { $path } ya declara { $plugin }. Quita ese archivo primero, o correrían dos copias.
plugin-install-failed = No se pudo instalar { $plugin }. { $reason }
plugin-publish-failed = No se pudo publicar { $plugin }. { $reason }
plugin-missing-locally = { $plugin } ya no está en la carpeta de { -plugin-pl }.
store-invite-needs-link = Este { -device } se unió a { $store } con su enlace. Pega el mismo enlace en el otro { -device }.
store-link-malformed = La { -plugin-store } envió un enlace que no funciona. Pídele al responsable que revise la { -plugin-store }. { $reason }
store-link-wrong-key = La { -plugin-store } creó un enlace con otra clave. Revisa la dirección con la que se inició.
store-admin-missing = { $store } no tiene página de administración. Su servidor puede ser más antiguo.
store-admin-link-foreign = { $store } envió un enlace de administración a otra dirección.
store-open-join-unsupported = El servidor de { $store } es más antiguo y no puede dejar que cualquiera se una con un enlace. Actualízalo primero.
source-switched-off = { $name } está desactivado. Se omite al buscar episodios.
source-switched-on = { $name } está activado.
plugin-not-loaded = { $id } no está cargado. Revisa que esté instalado en { -source-cap-pl }.
release-pinned = { $name } se usará para este título.
notice-choice-not-saved = No se pudo guardar esa elección. { $reason }
torrent-check-failed-running = La revisión de la conexión falló. Los torrents siguen activos.
torrent-vpn-waiting = Los torrents empiezan cuando pase la protección de VPN. { $reason }
torrent-vpn-unverified = Los torrents empiezan cuando pase la protección de VPN. Todavía no se verificó la conexión.
torrent-interface-closed = { -app-name } se cerró antes de que empezaran los torrents.
torrent-session-failed = No se pudo iniciar la sesión de torrents. { $reason }
torrent-no-kill-switch = Nada en este { -device } detiene el tráfico de torrents si cae la VPN. Ejecuta "{ -app-name } --doctor" para configurarlo.
torrent-not-checked = No se pudo revisar la conexión. { $reason }
sources-reloaded-torrents-pending = { -source-cap-pl } recargadas. Los torrents empiezan cuando pase la protección de VPN.
sources-reloaded-torrents-off = Los torrents están desactivados. La sesión se detuvo.
job-anilist-limit = Esperando por el límite de solicitudes de AniList
notice-anilist-limit = Esperando { $wait } por el límite de solicitudes de AniList.
notice-anilist-unreachable = AniList no responde. Por ahora se muestran datos de otro servicio.
notice-metadata-cached = Ningún servicio de metadatos respondió. Se muestra lo último que se guardó en caché.
notice-anilist-back = AniList volvió.
notice-party-not-built = Las { -watch-party-pl } no están incluidas en esta app.
