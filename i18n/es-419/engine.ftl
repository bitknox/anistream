notice-saved = Se guardó { $label }.
notice-not-saved = No se pudo guardar ese ajuste. { $reason }
notice-title-load-failed = No se pudo cargar este título. Revisa tu conexión e inténtalo de nuevo.
notice-playback-failed = La reproducción falló. Prueba con otra { -source }.
notice-engine-fault = Algo salió mal dentro de { -app-name }. Siguió funcionando, pero si una pantalla parece trabada, ciérrala y ábrela de nuevo.
notice-database-set-aside = La base de datos estaba dañada, así que { -app-name } creó una nueva. El archivo anterior se guardó en { $path }. Tus { -tracker-pl } recuperan lo que tienen en la próxima sincronización.
start-failed-folder = { -app-name } no pudo crear la carpeta { $path }. Revisa que el disco tenga espacio y que se pueda escribir en la carpeta, y luego inténtalo de nuevo. { $reason }
start-failed-config = { $path } tiene un ajuste que { -app-name } no puede usar: { $problem }. Corrígelo o quítalo, y luego inténtalo de nuevo.
start-failed-database = { -app-name } no pudo abrir su base de datos en { $path }. Si otra app de { -app-name } está abierta en esta carpeta, ciérrala e inténtalo de nuevo. { $reason }
start-failed-network = { -app-name } no pudo preparar su conexión de red con los ajustes de red de config.toml. { $reason }
start-failed = { -app-name } no pudo iniciarse. Ciérrala y ábrela de nuevo. { $reason }
start-failed-remote = El control remoto no pudo iniciarse. { $reason }
start-failed-player = El reproductor no pudo iniciarse. { $reason }
notice-episodes-queued = { $count ->
    [one] Se puso un episodio en cola.
    [many] Se pusieron { $count } episodios en cola.
   *[other] Se pusieron { $count } episodios en cola.
 }
