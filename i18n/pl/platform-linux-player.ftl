party-status = Stan: { -watch-party-cap }
party-members = Uczestnicy
party-choose-a-source = Wybierz { -source }
party-sources-none-listed = Żadne { -source } nie ma odcinka { $episode }. { -source-cap-pl } dodają { -plugin-pl }, które { -plugin-store } dostarcza.
party-sources-found-line = { $count ->
    [one] Jedno { -source } ma go. Wybrane odtwarza się: { -watch-party }.
    [few] { $count } { -source-pl } mają go. Wybrane odtwarza się: { -watch-party }.
    [many] { $count } źródeł ma go. Wybrane odtwarza się: { -watch-party }.
   *[other] { $count } { -source-pl } mają go. Wybrane odtwarza się: { -watch-party }.
 }
player-could-not-start = Odtwarzacz nie wystartował. Spróbuj ponownie albo wybierz inne { -source }.
player-file-unreadable = Nie udało się odczytać tego pliku. Wybierz inny.
player-inhibit-reason = Odtwarzanie odcinka
player-time-left = Zostało { $time }
player-note-waited = { $note } · { $seconds } s
player-starting-plain = Uruchamianie
player-undo-tooltip = Cofnij (U)
player-time-opening = { $clock } · Czołówka
player-time-ending = { $clock } · Zakończenie
player-channels-mono = Mono
player-channels-stereo = Stereo
player-channels = { $count ->
    [one] Jeden kanał
    [few] { $count } kanały
    [many] { $count } kanałów
   *[other] { $count } kanału
 }
player-translation-label = { -subbed-cap } lub { -dubbed }
