stores-device-count = { $count ->
    [one] { $count } { -device }
   *[other] { $count } { -device-pl }
 }
stores-dialog-title = { $title }, { $store }
stores-owner-unreachable = Verbindung zum { -plugin-store } nicht möglich
stores-owner-no-answer = Der { -plugin-store } hat nicht geantwortet
stores-owner-no-answer-detail = Er ist vielleicht beschäftigt, oder sein Rechner ist aus.
stores-owner-needs-connection = Der { -plugin-store } braucht eine Verbindung
stores-owner-needs-connection-detail = Versuche es erneut, sobald dieses { -device } wieder online ist.
stores-owner-link-offline = Für einen Link wird eine Verbindung zum { -plugin-store } benötigt.
stores-send-link-until = Sende diesen Link. Er funktioniert einmal, bis { $until }.
stores-open-on-detail = Sie treten ohne Namen bei. Ausschalten beendet den Link, wieder einschalten erstellt einen neuen.
stores-open-off-detail = Nur Personen, die du einlädst, können beitreten. Eingeschaltet kann jede Person mit dem Link ohne Namen beitreten.
stores-open-no-names = Es werden keine Namen gespeichert.
stores-open-keep-working = Sie funktionieren weiter, bis du sie entfernst.
stores-device-seen = Zuletzt gesehen { $when }
stores-device-never-seen = Seit dem Beitritt nicht gesehen
stores-publish-short = Ein { -plugin } veröffentlichen
stores-publish-none-hint = Ein { -plugin }, das du baust oder installierst, erscheint hier.
stores-not-published = Noch nicht im { -plugin-store }
stores-store-offers = Der { -plugin-store } bietet { $version }
stores-published-current = Im { -plugin-store } auf dem neuesten Stand
stores-published-state = Veröffentlicht
stores-publish-label = { $name } { $version } veröffentlichen
