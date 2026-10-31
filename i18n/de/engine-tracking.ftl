notice-signed-in = Bei { $tracker } angemeldet.
notice-signed-in-keychain = Bei { $tracker } angemeldet. Das Token liegt im Schlüsselbund des Systems.
notice-signed-in-file = Bei { $tracker } angemeldet. Es ist kein Schlüsselbund verfügbar, deshalb liegt das Token in einer privaten Datei auf diesem { -device }.
notice-signed-in-unsaved = Bei { $tracker } angemeldet, aber das Token wurde nicht gespeichert. Beim nächsten Mal musst du dich erneut anmelden. { $reason }
notice-token-not-saved = Das Anmelde-Token konnte nicht gespeichert werden. Melde dich erneut an. { $reason }
notice-sign-in-failed = Anmeldung bei { $tracker } fehlgeschlagen. Versuche es noch einmal. { $reason }
notice-sign-in-stopped = Die Anmeldung bei { $tracker } wurde beendet, bevor sie fertig war. Versuche es noch einmal. { $reason }
notice-token-exchange-failed = Kein Token von { $tracker } erhalten. Melde dich erneut an. { $reason }
notice-browser-not-opened = Für die Anmeldung konnte kein Browser geöffnet werden. Prüfe, ob einer installiert ist, und versuche es erneut. { $reason }
notice-sign-in-not-waiting = Auf diese Anmeldung bei { $tracker } wartet gerade nichts. Starte die Anmeldung erneut.
notice-sign-in-wrong-request = Die Antwort von { $tracker } passt nicht zu dieser Anfrage. Starte die Anmeldung erneut.
notice-sign-in-no-token = Die Anmeldung bei { $tracker } kam ohne Token zurück. Versuche es noch einmal.
notice-mal-client-missing-build = Die Anmeldung bei MyAnimeList konnte nicht abgeschlossen werden: Dieser Build hat dafür keinen Client.
notice-kitsu-signs-in-on-accounts = Die Anmeldung bei Kitsu erfolgt über den Bildschirm Konten.
notice-sign-in-unsupported = Die Anmeldung bei { $tracker } wird noch nicht unterstützt.
notice-sign-in-needs-redirect = Die Anmeldung bei { $tracker } braucht eine Weiterleitung aus dem Browser, die diese App nicht annehmen kann.
notice-anilist-implicit-flow = Diese AniList-Anmeldemethode funktioniert nur in der App { -app-name }. Um hier anzumelden, setze { $setting } in der Konfigurationsdatei auf „loopback“.
notice-anilist-client-missing = AniList braucht eine Client-ID und ein Geheimnis. Setze { $client } und { $secret } in der Konfigurationsdatei und melde dich dann erneut an. Um die Anmeldeadresse zu erhalten, führe { -app-name } --login-url aus.
notice-anilist-paste-flow = Einen Code einzufügen ist hier nicht möglich. Beende das Programm und starte { -app-name } stattdessen mit --login.
notice-mal-client-missing = MyAnimeList braucht eine Client-ID. Registriere auf myanimelist.net/apiconfig eine App vom Typ „other“ und setze dann { $setting } in der Konfigurationsdatei.
status-waiting-for-redirect = Warte darauf, dass { $tracker } zurückleitet…
notice-mal-no-refresh-token = MyAnimeList hat kein Refresh-Token ausgegeben. In etwa einem Monat musst du dich erneut anmelden.
notice-client-id-missing = Setze zuerst { $setting } in der Konfigurationsdatei.
notice-signed-out = Bei { $tracker } abgemeldet.
notice-signed-out-queue = { $count ->
    [one] Bei { $tracker } abgemeldet. Eine wartende Änderung wird für die nächste Anmeldung aufbewahrt.
   *[other] Bei { $tracker } abgemeldet. { $count } wartende Änderungen werden für die nächste Anmeldung aufbewahrt.
 }
notice-sign-out-failed = Abmeldung bei { $tracker } fehlgeschlagen. Versuche es noch einmal. { $reason }
notice-not-saved-because = Diese Einstellung konnte nicht gespeichert werden. { $cause }
notice-protection-saved = Torrent-Schutz gespeichert.
notice-protection-not-checked = Der Torrent-Schutz konnte nicht geprüft werden. { $cause }
notice-proxy-password-not-kept = Das Passwort des Proxys konnte nicht gespeichert werden. Gib es erneut ein, um das Speichern zu versuchen. { $reason }
notice-show-setting-refused = Diese Einstellung konnte nicht gespeichert werden. { $value } ist kein gültiger Wert für { $setting }.
notice-show-not-reset = Die Einstellungen dieser Serie konnten nicht zurückgesetzt werden. { $reason }
notice-show-no-such-subtitles = Diese Untertitelauswahl konnte nicht gespeichert werden. Die Serie hat diese Untertitel bisher nicht verwendet.
notice-torrents-waiting-for-check = Torrents warten, bis die Verbindungsprüfung wieder besteht.
notice-mapping-data-refreshed = Zuordnungsdaten aktualisiert.
notice-mapping-data-failed = Die Zuordnungsdaten ({ $datasets }) konnten nicht aktualisiert werden. Neuer Versuch in { $wait }. { $cause }
notice-episode-aired = { $title }, Folge { $episode }, ist erschienen, während du weg warst.
notice-episodes-aired = { $count ->
    [one] Eine neue Folge einer Serie, die du schaust, ist erschienen. Sie steht im Kalender.
   *[other] { $count } neue Folgen von Serien, die du schaust, sind erschienen. Sie stehen im Kalender.
 }
notice-update-available = { $tag } ist da. Führe { -app-name } --update aus, um es zu installieren.
cause-confirm-no-protection = Bestätige zuerst, dass du keinen Schutz willst.
cause-confirm-warn-only = Bestätige zuerst, dass du nur gewarnt werden willst.
cause-proxy-needs-address = Ein Proxy braucht einen Host und einen Port.
cause-proxy-address-unusable = Die Adresse des Proxys kann für eine Prüfung nicht verwendet werden.
cause-server-unreachable = Der Server war nicht erreichbar.
