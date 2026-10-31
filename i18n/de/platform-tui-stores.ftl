stores-needs-larger-terminal = Größeres Terminal nötig
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Mitglieder
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = Du
stores-column-state = Status
stores-none-joined = Noch keinem { -plugin-store } beigetreten
stores-role-owner = Besitzer
stores-role-member = Mitglied
stores-join-row = + Einem { -store } beitreten
stores-state-revoked = Zugriff entzogen
stores-state-offline = Offline
stores-state-up-to-date = Aktuell
stores-state-updates = { $count ->
    [one] { $count } Update
   *[other] { $count } Updates
 }
stores-footer-revoked = Dieser { -store } nimmt dieses { -device } nicht mehr an. Der Besitzer kann eine neue Einladung schicken.
stores-footer-offline = { $url } ({ $reason })
stores-key = Schlüssel { $fingerprint }
stores-joined-without-name = Ohne Namen beigetreten
stores-joined-as = Beigetreten als { $name }
stores-footer-join = Einladung oder Link eines { -store }s einfügen, den du bekommen hast
stores-owner-tag = Eigner
stores-anyone-can-join = Jeder mit dem Link kann beitreten
stores-note-offline = Offline, zeigt die letzte Liste ({ $reason })
stores-note-revoked = Dieser { -store } nimmt dieses { -device } nicht mehr an. Was er installiert hat, funktioniert weiter.
stores-column-plugin = { -plugin-cap }
stores-column-version = Version
stores-empty-owner = Noch nichts veröffentlicht. Veröffentliche unten ein { -plugin }.
stores-empty-member = Noch nichts veröffentlicht
stores-line-link-device = Weiteres { -device } verknüpfen
stores-line-invite = Jemanden einladen
stores-line-members = Mitglieder
stores-line-who-can-join = Wer beitreten kann
stores-line-publish = Ein { -plugin } veröffentlichen
stores-line-admin = Admin-Seite öffnen
stores-line-leave = Diesen { -store } verlassen
stores-plugin-not-installed = Nicht installiert
stores-plugin-installed = Installiert
stores-plugin-update-from = Update von { $from }
stores-plugin-added-by-hand = Von Hand hinzugefügt
stores-plugin-from-another-store = Von einem anderen { -store }
stores-footer-update-hosts = Das Update möchte zusätzlich { $hosts } kontaktieren
stores-footer-may-contact = Darf { $hosts } kontaktieren
stores-footer-link-device = Ein einmaliger Link, der ein weiteres { -device } mit deinem Konto verbindet
stores-footer-invite = Ein einmaliger Link für jemanden Neuen
stores-footer-members = Wer beigetreten ist, und Zugriff entziehen
stores-footer-close-join = ↵ Nur Eingeladene beitreten lassen und den Link stoppen
stores-footer-open-join = ↵ Jeder mit dem Link des { -store }s darf ohne Namen beitreten
stores-footer-publish = Ein { -plugin } aus dem Verzeichnis dieses Rechners anbieten
stores-footer-admin = Mitglieder, Einladungen und { -plugin-pl } im Browser verwalten
stores-footer-leave = { -plugin-cap-pl }, die von hier installiert wurden, bleiben auf diesem Rechner und aktualisieren sich nicht mehr
stores-who-open = Jeder mit dem Link
stores-who-invite-only = Nur Eingeladene
stores-label-offers = Angebote
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
   *[other] { $count } { -plugin-pl }
 }
stores-offered-plugin = { $name } { $version }  darf { $hosts } kontaktieren
stores-offered-installed = Installiert
stores-offered-installed-version = { $from } installiert
stores-offered-added-by-hand = Von Hand hinzugefügt
stores-offered-from-other = Von { $other }
stores-offered-more = und { $count } weitere
stores-hosts-none = nichts
stores-loading = Wird geladen…
stores-member-you = { $name } (du)
stores-column-name = Name
stores-column-role = Rolle
stores-column-devices = { -device-cap-pl }
stores-column-last-seen = Zuletzt gesehen
stores-members-heading = Mitglieder · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Noch nicht genutzte Einladungen
stores-invite-another-device = { $name }, ein weiteres { -device }
stores-seen-never = Nie
stores-ago-just-now = Gerade eben
stores-ago-today = Heute
stores-ago-yesterday = Gestern
stores-ago-days = { $days ->
    [one] Vor { $days } Tag
   *[other] Vor { $days } Tagen
 }
stores-ago-inline-just-now = gerade eben
stores-ago-inline-today = heute
stores-ago-inline-yesterday = gestern
stores-ago-inline-days = { $days ->
    [one] vor { $days } Tag
   *[other] vor { $days } Tagen
 }
stores-expired = Abgelaufen
stores-expires-today = Läuft heute ab
stores-expires-tomorrow = Läuft morgen ab
stores-expires-days = { $days ->
    [one] Läuft in { $days } Tag ab
   *[other] Läuft in { $days } Tagen ab
 }
stores-open-heading = Jeder mit dem Link kann beitreten · { $joined } damit beigetreten
stores-closed-heading = Mit dem Link beigetreten, bevor er geschlossen wurde · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
   *[other] { $count } { -device-pl }
 }
stores-open-link-row = Der Link des { -store }s
stores-open-link-note = Anzeigen und kopieren
stores-anonymous-joined = Beigetreten { $when }
stores-anonymous-seen = Gesehen { $when }
stores-anonymous-never-seen = Noch nicht gesehen
stores-remove-devices = { $count ->
    [one] Entfernen
   *[other] Alle { $count } entfernen
 }
stores-footer-you = Du bist der Besitzer. Ein { -store } behält immer seinen Besitzer.
stores-footer-remove-member = ↵ Zugriff entziehen
stores-footer-cancel-invite = ↵ Diese Einladung abbrechen
stores-footer-show-link = ↵ Link anzeigen, zum Kopieren und Teilen
stores-footer-remove-anonymous = ↵ Dieses { -device } entfernen, erkennbar nur an der Beitrittszeit
stores-footer-remove-all-anonymous = ↵ Jedes { -device } entfernen, das mit dem Link beigetreten ist
stores-link-copied = In die Zwischenablage kopiert, sofern das Terminal es erlaubt. Sonst auswählen zum Kopieren.
stores-link-select = Zum Kopieren auswählen.
stores-open-link-warning = Jeder mit ihm kann ohne Namen beliebig oft beitreten.
stores-open-link-lasts = Er funktioniert, bis der { -store } wieder nur Einladungen annimmt.
stores-invite-works-once = Funktioniert einmal. { $expiry }.
stores-invite-preview-safe = Wer ihn im Browser öffnet, sieht, wie man beitritt. Die Einladung wird dabei nicht verbraucht.
stores-confirm-join = { $name } beitreten?
stores-confirm-leave = { $name } verlassen?
stores-confirm-remove-member = { $name } entfernen?
stores-confirm-cancel-invite = Einladung für { $name } abbrechen?
stores-confirm-unpublish = { $name } nicht mehr anbieten?
stores-confirm-open = Jeder mit dem Link { $name } beitreten lassen?
stores-confirm-close = Nur eingeladene Personen { $name } beitreten lassen?
stores-confirm-remove-device = Dieses { -device } entfernen?
stores-confirm-remove-devices = { $count ->
    [one] Das { -device } entfernen, das mit dem Link beigetreten ist?
   *[other] Alle { $count } { -device-pl } entfernen, die mit dem Link beigetreten sind?
 }
stores-confirm-fallback = Bestätigen
stores-label-address = Adresse
stores-label-key = Schlüssel
stores-label-run-by = Betrieben von
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (passt zum Link)
stores-key-matches-invite = { $fingerprint }  (passt zur Einladung)
stores-join-open = Jeder mit dem Link kann beitreten.
stores-join-anonymous = Du trittst ohne Namen bei, und ein weiteres { -device } tritt mit demselben Link bei.
stores-join-already = Bereits beigetreten. Erneut beitreten gibt diesem { -device } ein neues Token.
stores-join-device-note = Der Besitzer des { -store }s sieht diesen Namen. Tippen, um ihn zu ändern.
stores-leave-plugins = { -plugin-cap-pl }, die von hier installiert wurden, bleiben auf diesem Rechner und aktualisieren sich nicht mehr.
stores-leave-revoked = Der Zugriff für dieses { -device } auf den { -store } ist entzogen.
stores-remove-member-devices = { $count ->
    [one] Ihr { -device } funktioniert mit diesem { -store } sofort nicht mehr.
   *[other] Ihre { $count } { -device-pl } funktionieren mit diesem { -store } sofort nicht mehr.
 }
stores-remove-member-plugins = { -plugin-cap-pl }, die sie bereits installiert haben, bleiben auf ihren Rechnern.
stores-cancel-invite-link = Der Link funktioniert nicht mehr.
stores-unpublish-members = Mitglieder, die es installiert haben, behalten es, und es aktualisiert sich nicht mehr.
stores-open-anyone = Jeder mit dem Link des { -store }s kann ohne Namen beitreten.
stores-open-link-new = Der Link ist jedes Mal neu, damit ein früherer ungültig bleibt.
stores-open-members-see = Mitglieder sehen, dass der { -store } offen ist.
stores-close-link = Der Link funktioniert nicht mehr, und niemand Neues tritt ohne Einladung bei.
stores-close-devices = { -device-cap-pl }, die damit beigetreten sind, funktionieren weiter, bis du sie unter Mitglieder entfernst.
stores-anonymous-one-stops = Es funktioniert danach nicht mehr mit diesem { -store }.
stores-anonymous-all-stop = Sie funktionieren danach nicht mehr mit diesem { -store }.
stores-anonymous-plugins = { -plugin-cap-pl }, die bereits installiert sind, bleiben, wo sie sind.
stores-anonymous-still-joins = Solange der { -store } offen ist, lässt der Link weiterhin jeden beitreten.
stores-plugin-may-contact = Darf { $hosts } kontaktieren
stores-plugin-cannot = Es kann keine Dateien lesen, keine Verbindungen öffnen und deine Umgebung nicht sehen.
stores-plugin-update-adds = Das Update erlaubt zusätzlich den Kontakt zu { $hosts }.
stores-plugin-added-by-hand-note = { $id }.wasm liegt schon im Verzeichnis, von Hand hinzugefügt.
stores-plugin-other-store-note = Von einem anderen { -store } installiert, der es aktuell hält.
stores-choice-install = Installieren
stores-choice-update = Auf { $version } aktualisieren
stores-choice-replace = Die von Hand hinzugefügte Kopie ersetzen
stores-choice-remove = Von diesem Rechner entfernen
stores-choice-unpublish = In diesem { -store } nicht mehr anbieten
stores-choice-cancel = Abbrechen
stores-publish-new = Neu
stores-publish-published = Veröffentlicht
stores-publish-replaces = Ersetzt { $version }
stores-toast-invite-checked = Die Einladung ist gültig, aber etwas anderes ist offen. Zum Beitreten erneut einfügen.
stores-toast-nothing-to-publish = Im Verzeichnis liegt kein { -plugin }, das sich veröffentlichen ließe. Mit „{ $command }“ siehst du, wo es liegt.
stores-toast-that-is-you = Das bist du. Ein { -store } behält immer seinen Besitzer.
