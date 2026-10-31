stores-needs-larger-terminal = Serve un terminale più grande
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Membri
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = Tu
stores-column-state = Stato
stores-none-joined = Non ti sei ancora unito a nessun { -store }
stores-role-owner = Titolare
stores-role-member = Membro
stores-join-row = + Unisciti a un { -store }
stores-state-revoked = Accesso revocato
stores-state-offline = Offline
stores-state-up-to-date = Aggiornato
stores-state-updates = { $count ->
    [one] { $count } aggiornamento
    [many] { $count } aggiornamenti
   *[other] { $count } aggiornamenti
 }
stores-footer-revoked = Questo { -store } non accetta più questo { -device }. Il titolare può inviare un nuovo invito.
stores-footer-offline = { $url } ({ $reason })
stores-key = Chiave { $fingerprint }
stores-joined-without-name = Unito senza nome
stores-joined-as = Unito come { $name }
stores-footer-join = Incolla l'invito o il link di { -store } che hai ricevuto
stores-owner-tag = Tuo
stores-anyone-can-join = Chiunque abbia il link può unirsi
stores-note-offline = Non raggiungibile, mostra l'ultimo elenco ricevuto ({ $reason })
stores-note-revoked = Questo { -store } non accetta più questo { -device }. Quello che ha installato continua a funzionare.
stores-column-plugin = { -plugin-cap }
stores-column-version = { -release-cap }
stores-empty-owner = Ancora niente da pubblicare. Pubblica un { -plugin } qui sotto.
stores-empty-member = Ancora niente da pubblicare
stores-line-link-device = Collega un altro { -device }
stores-line-invite = Invita qualcuno
stores-line-members = Membri
stores-line-who-can-join = Chi può unirsi
stores-line-publish = Pubblica un { -plugin }
stores-line-admin = Apri la pagina di amministrazione
stores-line-leave = Lascia questo { -store }
stores-plugin-not-installed = Non installato
stores-plugin-installed = Installato
stores-plugin-update-from = Aggiornamento da { $from }
stores-plugin-added-by-hand = Aggiunto a mano
stores-plugin-from-another-store = Da un altro { -store }
stores-footer-update-hosts = L'aggiornamento vuole anche contattare { $hosts }
stores-footer-may-contact = Può contattare { $hosts }
stores-footer-link-device = Un link monouso che unisce un altro dei tuoi computer come te
stores-footer-invite = Un link monouso per qualcuno di nuovo
stores-footer-members = Chi si è unito, e come rimuovere l'accesso a qualcuno
stores-footer-close-join = ↵ Consenti di unirsi solo alle persone che inviti e interrompi il link
stores-footer-open-join = ↵ Consenti a chiunque abbia il link di { -store } di unirsi, senza nome
stores-footer-publish = Offri un { -plugin } dalla cartella dei { -plugin-pl } di questo computer
stores-footer-admin = Gestisci membri, inviti e { -plugin-pl } nel browser
stores-footer-leave = { -plugin-cap-pl } installati da qui restano su questo computer e smettono di aggiornarsi
stores-who-open = Chiunque abbia il link
stores-who-invite-only = Solo le persone che inviti
stores-label-offers = Offre
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
    [many] { $count } { -plugin-pl }
   *[other] { $count } { -plugin-pl }
 }
stores-offered-plugin = { $name } { $version }  può contattare { $hosts }
stores-offered-installed = Installato
stores-offered-installed-version = { $from } installato
stores-offered-added-by-hand = Aggiunto a mano
stores-offered-from-other = Da { $other }
stores-offered-more = e altri { $count }
stores-hosts-none = nessuno
stores-loading = Caricamento…
stores-member-you = { $name } (tu)
stores-column-name = Nome
stores-column-role = Ruolo
stores-column-devices = Disp.
stores-column-last-seen = Ultimo accesso
stores-members-heading = Membri · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Inviti non ancora usati
stores-invite-another-device = { $name }, un altro { -device }
stores-seen-never = Mai
stores-ago-just-now = Adesso
stores-ago-today = Oggi
stores-ago-yesterday = Ieri
stores-ago-days = { $days ->
    [one] { $days } giorno fa
    [many] { $days } giorni fa
   *[other] { $days } giorni fa
 }
stores-ago-inline-just-now = adesso
stores-ago-inline-today = oggi
stores-ago-inline-yesterday = ieri
stores-ago-inline-days = { $days ->
    [one] { $days } giorno fa
    [many] { $days } giorni fa
   *[other] { $days } giorni fa
 }
stores-expired = Scaduto
stores-expires-today = Scade oggi
stores-expires-tomorrow = Scade domani
stores-expires-days = { $days ->
    [one] Scade tra { $days } giorno
    [many] Scade tra { $days } giorni
   *[other] Scade tra { $days } giorni
 }
stores-open-heading = Chiunque abbia il link può unirsi · { $joined } uniti con il link
stores-closed-heading = Uniti con il link prima che fosse chiuso · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
    [many] { $count } { -device-pl }
   *[other] { $count } { -device-pl }
 }
stores-open-link-row = Il link di { -store }
stores-open-link-note = Mostra e copia
stores-anonymous-joined = Unito { $when }
stores-anonymous-seen = Visto { $when }
stores-anonymous-never-seen = Non ancora visto
stores-remove-devices = { $count ->
    [one] Rimuovilo
    [many] Rimuovili tutti ({ $count })
   *[other] Rimuovili tutti ({ $count })
 }
stores-footer-you = Sei il titolare. Un { -store } mantiene sempre il suo titolare.
stores-footer-remove-member = ↵ Rimuovi il suo accesso
stores-footer-cancel-invite = ↵ Annulla questo invito
stores-footer-show-link = ↵ Mostra il link da copiare e condividere
stores-footer-remove-anonymous = ↵ Rimuovi questo { -device }, riconoscibile solo dalla data di ingresso
stores-footer-remove-all-anonymous = ↵ Rimuovi ogni { -device } unito con il link
stores-link-copied = Copiato negli appunti, dove il terminale lo consente, oppure selezionalo per copiarlo.
stores-link-select = Selezionalo per copiarlo.
stores-open-link-warning = Chiunque lo abbia può unirsi, senza nome, quante volte vuole.
stores-open-link-lasts = Funziona finché il { -store } non torna ad accettare solo inviti.
stores-invite-works-once = Funziona una sola volta. { $expiry }.
stores-invite-preview-safe = Aprirlo nel browser mostra come unirsi, senza usarlo.
stores-confirm-join = Unirti a { $name }?
stores-confirm-leave = Lasciare { $name }?
stores-confirm-remove-member = Rimuovere { $name }?
stores-confirm-cancel-invite = Annullare l'invito per { $name }?
stores-confirm-unpublish = Smettere di offrire { $name }?
stores-confirm-open = Consentire a chiunque abbia il link di unirsi a { $name }?
stores-confirm-close = Consentire a { $name } di unirsi solo alle persone che inviti?
stores-confirm-remove-device = Rimuovere questo { -device }?
stores-confirm-remove-devices = { $count ->
    [one] Rimuovere il { -device } unito con il link?
    [many] Rimuovere tutti i { $count } { -device-pl } uniti con il link?
   *[other] Rimuovere tutti i { $count } { -device-pl } uniti con il link?
 }
stores-confirm-fallback = Conferma
stores-label-address = Indirizzo
stores-label-key = Chiave
stores-label-run-by = Gestito da
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (corrisponde al link)
stores-key-matches-invite = { $fingerprint }  (corrisponde all'invito)
stores-join-open = Chiunque abbia il link può unirsi.
stores-join-anonymous = Ti unisci senza nome, e un altro { -device } si unisce con lo stesso link.
stores-join-already = Già unito. Unirsi di nuovo dà a questo { -device } un nuovo token.
stores-join-device-note = Il titolare del { -store } vede questo nome. Scrivi per cambiarlo.
stores-leave-plugins = { -plugin-cap-pl } installati da qui restano su questo computer e smettono di aggiornarsi.
stores-leave-revoked = L'accesso di questo { -device } al { -store } è stato revocato.
stores-remove-member-devices = { $count ->
    [one] Il suo { -device } smette subito di funzionare con questo { -store }.
    [many] I suoi { $count } { -device-pl } smettono subito di funzionare con questo { -store }.
   *[other] I suoi { $count } { -device-pl } smettono subito di funzionare con questo { -store }.
 }
stores-remove-member-plugins = I { -plugin-cap-pl } che hanno già installato restano sui loro computer.
stores-cancel-invite-link = Il link smette di funzionare.
stores-unpublish-members = I membri che lo hanno installato lo tengono, e smette di aggiornarsi.
stores-open-anyone = Chiunque abbia il link di { -store } può unirsi, senza nome.
stores-open-link-new = Il link è nuovo ogni volta, quindi quello precedente non funziona più.
stores-open-members-see = I membri vedono che il { -store } è aperto.
stores-close-link = Il link smette di funzionare e nessuno nuovo può unirsi senza invito.
stores-close-devices = { -device-cap-pl } uniti con il link continuano a funzionare finché non li rimuovi tra i membri.
stores-anonymous-one-stops = Smette subito di funzionare con questo { -store }.
stores-anonymous-all-stop = Smettono subito di funzionare con questo { -store }.
stores-anonymous-plugins = I { -plugin-cap-pl } già installati restano dove sono.
stores-anonymous-still-joins = Finché il { -store } è aperto, il link fa ancora unire chiunque.
stores-plugin-may-contact = Può contattare { $hosts }
stores-plugin-cannot = Non può leggere file, aprire socket né vedere il tuo ambiente.
stores-plugin-update-adds = L'aggiornamento aggiunge { $hosts } a ciò che può contattare.
stores-plugin-added-by-hand-note = { $id }.wasm è già nella cartella dei { -plugin-pl }, aggiunto a mano.
stores-plugin-other-store-note = Installato da un altro { -store }, che lo tiene aggiornato.
stores-choice-install = Installa
stores-choice-update = Aggiorna a { $version }
stores-choice-replace = Sostituisci la copia aggiunta a mano
stores-choice-remove = Rimuovi da questo computer
stores-choice-unpublish = Smetti di offrirlo in questo { -store }
stores-choice-cancel = Annulla
stores-publish-new = Nuovo
stores-publish-published = Pubblicato
stores-publish-replaces = Sostituisce { $version }
stores-toast-invite-checked = L'invito è valido, ma è aperto qualcos'altro. Incollalo di nuovo per unirti.
stores-toast-nothing-to-publish = Non ci sono { -plugin-pl } nella cartella dei { -plugin-pl } da pubblicare. Esegui "{ $command }" per vedere dove si trova.
stores-toast-that-is-you = Sei tu. Un { -store } mantiene sempre il suo titolare.
