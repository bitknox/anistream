stores-device-count = { $count ->
    [one] { $count } { -device }
    [many] { $count } { -device-pl }
   *[other] { $count } { -device-pl }
 }
stores-dialog-title = { $title }, { $store }
stores-owner-unreachable = Impossibile raggiungere il { -plugin-store }
stores-owner-no-answer = Il { -plugin-store } non risponde
stores-owner-no-answer-detail = Potrebbe essere occupato o il suo computer spento.
stores-owner-needs-connection = Il { -plugin-store } richiede una connessione
stores-owner-needs-connection-detail = Riprova quando questo { -device } sarà di nuovo connesso.
stores-owner-link-offline = Per creare un link serve una connessione al { -plugin-store }.
stores-send-link-until = Invia questo link. Funziona una sola volta, fino a { $until }.
stores-open-on-detail = Entrano senza nome. Disattivarlo interrompe il link; riattivarlo ne crea uno nuovo.
stores-open-off-detail = Possono entrare solo le persone che inviti. Se attivato, chiunque abbia il link entra, senza nome.
stores-open-no-names = Nessun nome viene conservato.
stores-open-keep-working = Continuano a funzionare finché non li rimuovi.
stores-device-seen = Visto { $when }
stores-device-never-seen = Non visto da quando è entrato
stores-publish-short = Pubblica un { -plugin }
stores-publish-none-hint = Un { -plugin } che crei o installi compare qui.
stores-not-published = Non ancora nel { -plugin-store }
stores-store-offers = Il { -plugin-store } offre la { -release } { $version }
stores-published-current = Aggiornato nel { -plugin-store }
stores-published-state = Pubblicato
stores-publish-label = Pubblica { $name } { $version }
