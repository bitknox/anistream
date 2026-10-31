notice-signed-in = Accesso effettuato a { $tracker }.
notice-signed-in-keychain = Accesso effettuato a { $tracker }. Il token è nel portachiavi del sistema.
notice-signed-in-file = Accesso effettuato a { $tracker }. Non c'è un portachiavi disponibile, quindi il token è in un file privato su questo { -device }.
notice-signed-in-unsaved = Accesso effettuato a { $tracker }, ma il token non è stato salvato: dovrai accedere di nuovo la prossima volta. { $reason }
notice-token-not-saved = Impossibile salvare il token di accesso. Prova ad accedere di nuovo. { $reason }
notice-sign-in-failed = Impossibile accedere a { $tracker }. Riprova. { $reason }
notice-sign-in-stopped = L'accesso a { $tracker } si è interrotto prima di finire. Riprova. { $reason }
notice-token-exchange-failed = Impossibile ottenere un token da { $tracker }. Prova ad accedere di nuovo. { $reason }
notice-browser-not-opened = Impossibile aprire un browser per l'accesso. Controlla che ce ne sia uno installato e riprova. { $reason }
notice-sign-in-not-waiting = Nessun accesso a { $tracker } è in attesa di questa risposta. Avvia di nuovo l'accesso.
notice-sign-in-wrong-request = L'accesso a { $tracker } non corrisponde a questa richiesta. Avvia di nuovo l'accesso.
notice-sign-in-no-token = L'accesso a { $tracker } è tornato senza token. Riprova.
notice-mal-client-missing-build = Impossibile completare l'accesso a MyAnimeList: questa build non ha un client per farlo.
notice-kitsu-signs-in-on-accounts = Per Kitsu l'accesso avviene dalla schermata Account.
notice-sign-in-unsupported = L'accesso a { $tracker } non è ancora supportato.
notice-sign-in-needs-redirect = L'accesso a { $tracker } richiede un reindirizzamento dal browser, che questa app non può ricevere.
notice-anilist-implicit-flow = Questo metodo di accesso ad AniList funziona solo nell'app { -app-name }. Per accedere qui, imposta { $setting } su "loopback" nel file di configurazione.
notice-anilist-client-missing = AniList richiede un id client e un segreto. Imposta { $client } e { $secret } nel file di configurazione, poi accedi di nuovo. Per ottenere l'indirizzo di accesso, esegui { -app-name } --login-url.
notice-anilist-paste-flow = Qui non è possibile incollare un codice. Esci ed esegui { -app-name } --login.
notice-mal-client-missing = MyAnimeList richiede un id client. Registra un'app di tipo "other" su myanimelist.net/apiconfig, poi imposta { $setting } nel file di configurazione.
status-waiting-for-redirect = In attesa che { $tracker } riporti al programma…
notice-mal-no-refresh-token = MyAnimeList non ha rilasciato un token di aggiornamento. Dovrai accedere di nuovo tra circa un mese.
notice-client-id-missing = Imposta prima { $setting } nel file di configurazione.
notice-signed-out = Disconnesso da { $tracker }.
notice-signed-out-queue = { $count ->
    [one] Disconnesso da { $tracker }. Una modifica in coda viene conservata per il successivo accesso.
    [many] Disconnesso da { $tracker }. { $count } modifiche in coda vengono conservate per il successivo accesso.
   *[other] Disconnesso da { $tracker }. { $count } modifiche in coda vengono conservate per il successivo accesso.
 }
notice-sign-out-failed = Impossibile disconnettersi da { $tracker }. Riprova. { $reason }
notice-not-saved-because = Impossibile salvare questa impostazione. { $cause }
notice-protection-saved = Protezione torrent salvata.
notice-protection-not-checked = Impossibile verificare la protezione torrent. { $cause }
notice-proxy-password-not-kept = Impossibile conservare la password del proxy. Inseriscila di nuovo per provare a salvarla. { $reason }
notice-show-setting-refused = Impossibile salvare questa impostazione. { $value } non è un valore valido per { $setting }.
notice-show-not-reset = Impossibile ripristinare le impostazioni di questa serie. { $reason }
notice-show-no-such-subtitles = Impossibile salvare questa scelta di sottotitoli. Questa serie non ha mai usato quei sottotitoli.
notice-torrents-waiting-for-check = I torrent attendono che il controllo della connessione torni positivo.
notice-mapping-data-refreshed = Dati di mappatura aggiornati.
notice-mapping-data-failed = Impossibile aggiornare i dati di mappatura ({ $datasets }). Nuovo tentativo tra { $wait }. { $cause }
notice-episode-aired = { $title }, episodio { $episode }, è andato in onda mentre eri via.
notice-episodes-aired = { $count ->
    [one] È andato in onda un nuovo episodio di una serie che segui. Il calendario lo mostra.
    [many] Sono andati in onda { $count } nuovi episodi di serie che segui. Il calendario li mostra.
   *[other] Sono andati in onda { $count } nuovi episodi di serie che segui. Il calendario li mostra.
 }
notice-update-available = È disponibile { $tag }. Esegui { -app-name } --update per installarla.
cause-confirm-no-protection = Conferma prima che non vuoi alcuna protezione.
cause-confirm-warn-only = Conferma prima che vuoi solo un avviso.
cause-proxy-needs-address = Un proxy richiede un host e una porta.
cause-proxy-address-unusable = L'indirizzo del proxy non è utilizzabile per un controllo.
cause-server-unreachable = Impossibile raggiungere il server.
