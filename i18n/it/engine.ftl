notice-saved = Impostazione salvata: { $label }.
notice-not-saved = Impossibile salvare l'impostazione. { $reason }
notice-title-load-failed = Impossibile caricare questo titolo. Controlla la connessione e riprova.
notice-playback-failed = La riproduzione non è riuscita. Prova un'altra { -source }.
notice-engine-fault = Qualcosa non ha funzionato in { -app-name }. Ha continuato a funzionare, ma se una schermata sembra bloccata, esci e riaprila.
notice-database-set-aside = Il database era danneggiato, quindi { -app-name } ne ha creato uno nuovo. Il file vecchio è conservato in { $path }. I tuoi { -tracker-pl } ripristineranno ciò che contengono alla prossima sincronizzazione.
start-failed-folder = { -app-name } non riesce a creare la cartella { $path }. Controlla che il disco abbia spazio e che la cartella sia scrivibile, poi riprova. { $reason }
start-failed-config = { $path } contiene un'impostazione che { -app-name } non può usare: { $problem }. Correggila o rimuovila, poi riprova.
start-failed-database = { -app-name } non riesce ad aprire il suo database in { $path }. Se un'altra app { -app-name } è aperta in questa cartella, chiudila e riprova. { $reason }
start-failed-network = { -app-name } non riesce a configurare la connessione di rete con le impostazioni di rete in config.toml. { $reason }
start-failed = { -app-name } non riesce ad avviarsi. Chiudi e riapri. { $reason }
start-failed-remote = Il telecomando non è riuscito ad avviarsi. { $reason }
start-failed-player = Il player non è riuscito ad avviarsi. { $reason }
notice-episodes-queued = { $count ->
    [one] Messo in coda un episodio.
    [many] Messi in coda { $count } episodi.
   *[other] Messi in coda { $count } episodi.
 }
