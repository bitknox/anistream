notice-signed-in = Sesión iniciada en { $tracker }.
notice-signed-in-keychain = Sesión iniciada en { $tracker }. El token está en el llavero del sistema.
notice-signed-in-file = Sesión iniciada en { $tracker }. No hay llavero disponible, así que el token está en un archivo privado de este { -device }.
notice-signed-in-unsaved = Sesión iniciada en { $tracker }, pero el token no se guardó, así que tendrás que volver a iniciar sesión la próxima vez. { $reason }
notice-token-not-saved = No se pudo guardar el token de inicio de sesión. Vuelve a iniciar sesión. { $reason }
notice-sign-in-failed = No se pudo iniciar sesión en { $tracker }. Inténtalo de nuevo. { $reason }
notice-sign-in-stopped = El inicio de sesión en { $tracker } se detuvo antes de terminar. Inténtalo de nuevo. { $reason }
notice-token-exchange-failed = No se pudo obtener un token de { $tracker }. Vuelve a iniciar sesión. { $reason }
notice-browser-not-opened = No se pudo abrir un navegador para iniciar sesión. Revisa que haya uno instalado e inténtalo de nuevo. { $reason }
notice-sign-in-not-waiting = No hay ningún inicio de sesión en { $tracker } esperando eso. Vuelve a empezar el inicio de sesión.
notice-sign-in-wrong-request = Ese inicio de sesión en { $tracker } no volvió desde esta solicitud. Vuelve a empezar el inicio de sesión.
notice-sign-in-no-token = El inicio de sesión en { $tracker } volvió sin token. Inténtalo de nuevo.
notice-mal-client-missing-build = No se pudo terminar el inicio de sesión en MyAnimeList: esta versión no tiene un cliente para hacerlo.
notice-kitsu-signs-in-on-accounts = Kitsu inicia sesión desde la pantalla de Cuentas.
notice-sign-in-unsupported = Todavía no se admite iniciar sesión en { $tracker }.
notice-sign-in-needs-redirect = Iniciar sesión en { $tracker } necesita una redirección del navegador, que esta app no puede recibir.
notice-anilist-implicit-flow = Este método de inicio de sesión de AniList solo funciona en la app de { -app-name }. Para iniciar sesión aquí, pon { $setting } en "loopback" en el archivo de configuración.
notice-anilist-client-missing = AniList necesita un id de cliente y un secreto. Pon { $client } y { $secret } en el archivo de configuración, y luego inicia sesión otra vez. Para obtener la dirección de inicio de sesión, ejecuta { -app-name } --login-url.
notice-anilist-paste-flow = No se puede pegar un código aquí. Cierra { -app-name } y ejecuta { -app-name } --login en su lugar.
notice-mal-client-missing = MyAnimeList necesita un id de cliente. Registra una app con el tipo "other" en myanimelist.net/apiconfig y luego pon { $setting } en el archivo de configuración.
status-waiting-for-redirect = Esperando a que { $tracker } redirija de vuelta…
notice-mal-no-refresh-token = MyAnimeList no emitió un token de actualización. Tendrás que volver a iniciar sesión en un mes aproximadamente.
notice-client-id-missing = Primero pon { $setting } en el archivo de configuración.
notice-signed-out = Sesión cerrada en { $tracker }.
notice-signed-out-queue = { $count ->
    [one] Sesión cerrada en { $tracker }. Un cambio en cola se conserva para el próximo inicio de sesión.
    [many] Sesión cerrada en { $tracker }. { $count } cambios en cola se conservan para el próximo inicio de sesión.
   *[other] Sesión cerrada en { $tracker }. { $count } cambios en cola se conservan para el próximo inicio de sesión.
 }
notice-sign-out-failed = No se pudo cerrar la sesión en { $tracker }. Inténtalo de nuevo. { $reason }
notice-not-saved-because = No se pudo guardar esa configuración. { $cause }
notice-protection-saved = Protección de torrents guardada.
notice-protection-not-checked = No se pudo revisar la protección de torrents. { $cause }
notice-proxy-password-not-kept = No se pudo guardar la contraseña del proxy. Escríbela otra vez para intentar guardarla. { $reason }
notice-show-setting-refused = No se pudo guardar esa configuración. { $value } no es un valor válido para { $setting }.
notice-show-not-reset = No se pudo restablecer la configuración de esta serie. { $reason }
notice-show-no-such-subtitles = No se pudo guardar esa elección de subtítulos. Esta serie no ha usado esos subtítulos antes.
notice-torrents-waiting-for-check = Los torrents esperan a que vuelva a pasar la revisión de conexión.
notice-mapping-data-refreshed = Datos de mapeo actualizados.
notice-mapping-data-failed = No se pudieron actualizar los datos de mapeo ({ $datasets }). Se intentará otra vez en { $wait }. { $cause }
notice-episode-aired = { $title }, episodio { $episode }, salió al aire mientras no estabas.
notice-episodes-aired = { $count ->
    [one] Salió al aire un episodio nuevo de una serie que ves. El calendario lo tiene.
    [many] Salieron al aire { $count } episodios nuevos de series que ves. El calendario los tiene.
   *[other] Salieron al aire { $count } episodios nuevos de series que ves. El calendario los tiene.
 }
notice-update-available = Ya está disponible { $tag }. Ejecuta { -app-name } --update para instalarlo.
cause-confirm-no-protection = Primero confirma que quieres no tener protección.
cause-confirm-warn-only = Primero confirma que solo quieres recibir avisos.
cause-proxy-needs-address = Un proxy necesita un host y un puerto.
cause-proxy-address-unusable = La dirección del proxy no es una que la revisión pueda usar.
cause-server-unreachable = No se pudo contactar con el servidor.
