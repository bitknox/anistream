notice-saved = { $label } gespeichert.
notice-not-saved = Diese Einstellung konnte nicht gespeichert werden. { $reason }
notice-title-load-failed = Dieser Titel konnte nicht geladen werden. Prüfe deine Verbindung und versuche es erneut.
notice-playback-failed = Die Wiedergabe ist fehlgeschlagen. Probiere eine andere { -source }.
notice-engine-fault = In { -app-name } ist etwas schiefgelaufen. Es läuft weiter, aber wenn ein Bildschirm hängen bleibt, beende { -app-name } und öffne es erneut.
notice-database-set-aside = Die Datenbank war beschädigt, deshalb hat { -app-name } eine neue angelegt. Die alte Datei liegt unter { $path }. Deine { -tracker-pl } holen beim nächsten Abgleich zurück, was sie gespeichert haben.
start-failed-folder = { -app-name } konnte den Ordner { $path } nicht anlegen. Prüfe, ob die Festplatte genug Platz hat und der Ordner beschreibbar ist, und versuche es dann erneut. { $reason }
start-failed-config = { $path } enthält eine Einstellung, die { -app-name } nicht verwenden kann: { $problem }. Korrigiere oder entferne sie und versuche es dann erneut.
start-failed-database = { -app-name } konnte die Datenbank unter { $path } nicht öffnen. Ist ein anderes { -app-name } in diesem Ordner geöffnet, beende es und versuche es erneut. { $reason }
start-failed-network = { -app-name } konnte die Netzwerkverbindung nicht aus den Netzwerkeinstellungen in config.toml einrichten. { $reason }
start-failed = { -app-name } konnte nicht gestartet werden. Beende { -app-name } und öffne es erneut. { $reason }
start-failed-remote = Die Fernbedienung konnte nicht gestartet werden. { $reason }
start-failed-player = Der Player konnte nicht gestartet werden. { $reason }
notice-episodes-queued = { $count ->
    [one] Eine Folge in die Warteschlange gestellt.
   *[other] { $count } Folgen in die Warteschlange gestellt.
 }
