queue-episodes-left = { $count ->
    [one] Un episodio rimasto in { $title }
    [many] { $count } episodi rimasti in { $title }
   *[other] { $count } episodi rimasti in { $title }
 }
episodes-count = { $count ->
    [one] { $count } episodio
    [many] { $count } episodi
   *[other] { $count } episodi
 }
progress-episodes = { $total ->
    [one] { $watched } di { $total } episodio
    [many] { $watched } di { $total } episodi
   *[other] { $watched } di { $total } episodi
 }
duration-days-hours = { $days } g { $hours } h
group-in-progress = In corso
group-new-this-week = Nuovi questa settimana
continue-empty-title = Non hai ancora guardato nulla
continue-empty-detail = Scegli qualcosa da Stagionale, oppure cerca un titolo.
continue-empty-action = Sfoglia Stagionale
continue-load-failed = Impossibile caricare { -continue-cap }
continue-heading = { -continue-cap }
continue-airing-today = In onda oggi
continue-this-season = Questa stagione
continue-hide = Nascondi da { -continue-cap }
continue-hidden = Nascosta da { -continue-cap }.
continue-drop = Sposta in Abbandonati
continue-more-for = Altro per { $title }
continue-row-resume = Riprendi E{ $episode } · { $clock }
continue-row-resume-dubbed = Riprendi E{ $episode } { -dubbed } · { $clock }
continue-row-start = Riproduci E{ $episode }
continue-row-start-dubbed = Riproduci E{ $episode } { -dubbed }
continue-row-start-aired = Riproduci E{ $episode } · uscito { $ago }
continue-row-caught-up = Tutto visto · E{ $episode } { $when }
continue-row-caught-up-plain = Tutto visto
continue-widget-connect = Collega AniList per vederlo qui.
continue-caught-up = Hai visto tutto.
title-button-resume = Riprendi l'episodio { $episode } · { $time }
title-button-play = Riproduci l'episodio { $episode }
title-button-add-source = Aggiungi una { -source } per riprodurre
library-status-watching = In visione
library-status-planning = In programma
library-status-completed = Completato
library-status-paused = In pausa
library-status-dropped = Abbandonato
library-sort-updated = Ultima modifica
library-sort-title = Titolo
library-sort-score = Voto
library-sort-button = Ordina: { $sort }
library-sort-label = Ordina per { $sort }
library-signed-out-title = Le tue liste arrivano da AniList
library-signed-out-detail = Accedi per sincronizzare ciò che guardi e dove ti sei fermato.
library-signed-out-action = Accedi ad AniList
library-list-empty = Nessun titolo nella tua lista { $list }.
library-list-empty-detail = I titoli che aggiungi a questa lista su AniList compaiono qui.
library-list-load-failed = Impossibile caricare la tua lista { $list }
library-loading = Caricamento delle tue liste
library-add-to-list = Aggiungi alla lista
library-add-planning = Aggiungi a In programma
library-added-planning = Aggiunta a In programma.
library-on-your-list = Nella tua lista
library-in-your-list = In lista
progress-episodes-short = E{ $done } di { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Riproduci E{ $episode } { -dubbed } · uscito { $ago }
up-next-heading = { -up-next-cap } per te
up-next-starts = Inizia { $date }
up-next-reason-new-season = Nuova stagione
up-next-reason-planning = Nella tua lista In programma
up-next-reason-recommended = Perché hai finito { $title }
up-next-reason-recommended-plain = Consigliato per te
up-next-because-detail = Consigliato su AniList da chi ha apprezzato { $title }
up-next-reason-airing-today = In onda oggi
up-next-add-planning = Aggiungi a In programma
up-next-not-interested = Non mi interessa
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } non comparirà più tra i consigli per te.
up-next-hidden-until-airs = { $title } è nascosto fino alla messa in onda.
up-next-broadcast = Episodio { $episode } · { $when }
up-next-out = uscito { $ago }
