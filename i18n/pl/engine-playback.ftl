notice-reason = { $reason }
cause-no-sources = Żadne { -source-pl } nie są skonfigurowane. Dodaj je na ekranie { -source-cap-pl }.
notice-not-aired = Odcinek { $episode } zostanie wyemitowany { $when }.
notice-not-aired-yet = Odcinek { $episode } nie został jeszcze wyemitowany. Odcinek { $next } zostanie wyemitowany { $when }.
cause-airs-today = dziś o { $time }
cause-airs-tomorrow = jutro o { $time }
cause-airs-in-days = { $days ->
    [one] za jeden dzień o { $time }
    [few] za { $days } dni o { $time }
    [many] za { $days } dni o { $time }
   *[other] za { $days } dni o { $time }
 }
cause-airs-on-date = dnia { $date } o { $time }
cause-no-playable-stream = { -source-cap } odpowiedziało, ale nie ma nic do odtworzenia. Spróbuj inne { -source }.
cause-title-no-match = Nie udało się dopasować tego tytułu: { $detail }
cause-source-gone = { -source-cap } { $source } nie jest już dostępne. Wybierz inne { -source }.
cause-release-failed = Nie udało się uruchomić „{ -release }”: { $detail }
cause-sources-timed-out = { -source-cap-pl } nie odpowiedziały na czas. Spróbuj ponownie albo wybierz inne { -source }.
cause-no-source-has-episode = Żadne { -source } nie ma odcinka { $episode }: { $detail }
cause-source-cannot-download = To { -source } odtwarza odcinki, ale nie można ich pobrać.
cause-no-source-could-list-show = Żadne { -source } nie mogło wyświetlić tego tytułu, więc nie można go pobrać.
cause-subtitles-merged = { $tracks ->
    [one] Scalono jedną ścieżkę napisów.
    [few] Scalono { $tracks } ścieżki napisów.
    [many] Scalono { $tracks } ścieżek napisów.
   *[other] Scalono { $tracks } ścieżek napisów.
 }
cause-subtitles-left-apart = Napisy zostają w osobnych plikach. { $reason }
cause-already-in-downloads = { $count ->
    [one] Jeden odcinek jest już w kolejce pobierania.
    [few] { $count } odcinki są już w kolejce pobierania.
    [many] { $count } odcinków jest już w kolejce pobierania.
   *[other] { $count } odcinków jest już w kolejce pobierania.
 }
status-looking-up-title = Wyszukiwanie tytułu…
status-loading-sources = { -source-cap-pl } są wczytywane…
status-searching-sources = { -source-cap-pl } są przeszukiwane w poszukiwaniu odcinka { $episode }…
notice-player-failed-to-start = Nie udało się uruchomić mpv. Sprawdź, czy działa samodzielnie. ({ $reason })
notice-stream-no-video = Brak obrazu z tego strumienia. Sprawdzany jest strumień { $next } z { $total }.
notice-player-could-not-play = mpv nie mógł tego odtworzyć. { $reason }
notice-player-exited-silently = mpv zakończył pracę bez odtwarzania i nie podał przyczyny. Spróbuj inne { -source }.
notice-player-not-playing = mpv nie zaczął odtwarzać po { $seconds } s. { -source-cap } może nie mieć danych. Sprawdź, czy mpv odtwarza plik samodzielnie.
notice-no-player = Ta wersja { -app-name } nie potrafi odtwarzać wideo.
notice-mpv-missing = Nie znaleziono { $binary }. Zainstaluj mpv, aby odtwarzać odcinki.
notice-mpv-old = mpv { $version } potrafi odtwarzać, ale odtwarzacz { -app-name } wymaga mpv 0.36 lub nowszego do przycisku pomijania, menu, miniatur i karty „{ -up-next-cap }”.
notice-subtitles-unavailable = Napisów nie udało się wczytać. Wybierz inne { -source }, aby spróbować ponownie.
notice-opened-in-browser = Otwarto w przeglądarce.
notice-could-not-open = Nie udało się otworzyć. { $reason }
notice-trailer-playing = Zwiastun jest odtwarzany w mpv.
notice-trailer-opened = Zwiastun otwarto w przeglądarce.
notice-trailer-failed = Nie udało się otworzyć zwiastuna. { $reason }
notice-filler-skipped = Pominięto wypełniacz. { -up-next-cap } to odcinek { $episode }.
notice-audio-now-subbed = Dźwięk jest teraz { -subbed }.
notice-audio-now-dubbed = Dźwięk jest teraz { -dubbed }.
notice-already-first-episode = To już pierwszy odcinek.
notice-last-episode = Nie ma następnego odcinka. Odcinek { $episode } jest ostatni.
notice-no-next-episode = Nie ma następnego odcinka.
notice-no-previous-episode = Nie ma poprzedniego odcinka.
notice-match-reset-failed = Nie udało się zresetować dopasowania. { $reason }
notice-match-save-failed = Nie udało się zapisać tego dopasowania. { $reason }
notice-match-did-not-stick = Dopasowanie się nie utrzymało. { -source-cap } mogło się zmienić. Spróbuj wybrać { -source } ponownie.
notice-source-search-empty = Brak wyników dla „{ $query }”. Spróbuj innego wyszukiwania.
notice-source-pinned = Dla tego tytułu będzie używane { -source } { $source }.
notice-source-automatic = { -source-cap } jest znowu wybierane automatycznie.
notice-source-choice-failed = Nie udało się zapisać wyboru. { $reason }
notice-no-source-could-list = Żadne { -source } nie mogło wyświetlić tego tytułu: { $reason }
notice-episodes-failed = Nie udało się wczytać odcinków. { $reason }
notice-match-uncertain = W { $source } znaleziono więcej niż jeden tytuł, który może pasować. Na razie odcinki pochodzą z AniList.
notice-continue-hide-failed = Nie udało się usunąć z listy „{ -continue-cap }”. { $reason }
notice-download-queued = Dodano do kolejki pobierania: { $title }, odcinek { $episode }.
notice-download-already-queued = { $title }, odcinek { $episode }, jest już w kolejce pobierania.
notice-download-starting-now = Teraz pobieranie: { $title }, odcinek { $episode }.
notice-download-cancelled = Pobieranie anulowano.
notice-download-file-deleted = Usunięto pobranie i plik.
notice-download-removed-file-gone = Usunięto pobranie. Jego plik już nie istniał.
notice-download-removed-file-kept = Usunięto pobranie, ale nie udało się usunąć pliku. { $reason }
notice-download-removed = Usunięto pobranie.
notice-downloads-cleared = { $count ->
    [one] Wyczyszczono jedno zakończone pobranie.
    [few] Wyczyszczono { $count } zakończone pobrania.
    [many] Wyczyszczono { $count } zakończonych pobrań.
   *[other] Wyczyszczono { $count } zakończonych pobrań.
 }
notice-download-finished = Pobrano: { $title }, odcinek { $episode }.
notice-download-finished-note = Pobrano: { $title }, odcinek { $episode }. { $note }
notice-download-failed = Nie udało się pobrać: { $title }, odcinek { $episode }. { $reason }
notice-downloads-none-queued = Nic nie dodano do kolejki.
notice-downloads-none-queued-stopped = Nic nie dodano do kolejki. { $reason }
notice-downloads-none-new = Nic nowego nie dodano do kolejki. { $existing }
notice-downloads-none-new-stopped = Nic nowego nie dodano do kolejki. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] Dodano do kolejki jeden odcinek. { $existing }
    [few] Dodano do kolejki { $count } odcinki. { $existing }
    [many] Dodano do kolejki { $count } odcinków. { $existing }
   *[other] Dodano do kolejki { $count } odcinków. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] Dodano do kolejki jeden odcinek, potem zatrzymano. { $reason }
    [few] Dodano do kolejki { $count } odcinki, potem zatrzymano. { $reason }
    [many] Dodano do kolejki { $count } odcinków, potem zatrzymano. { $reason }
   *[other] Dodano do kolejki { $count } odcinków, potem zatrzymano. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] Dodano do kolejki jeden odcinek, potem zatrzymano. { $existing } { $reason }
    [few] Dodano do kolejki { $count } odcinki, potem zatrzymano. { $existing } { $reason }
    [many] Dodano do kolejki { $count } odcinków, potem zatrzymano. { $existing } { $reason }
   *[other] Dodano do kolejki { $count } odcinków, potem zatrzymano. { $existing } { $reason }
 }
notice-keep-next-full = Za mało wolnego miejsca, by zachować pobrane kolejne odcinki. Zwolnij trochę miejsca, aby pobrać więcej.
party-needs-server = Najpierw wybierz serwer, zanim rozpoczniesz { -watch-party } (syncplay.server). { $server } jest prowadzony przez { $operator }.
party-choose-server = Najpierw wybierz serwer, zanim rozpoczniesz { -watch-party }. { $server } jest prowadzony przez { $operator }.
notice-no-party = { -watch-party-cap-pl } nie są dostępne w tej wersji { -app-name }.
party-needs-room = { -watch-party-cap } wymaga pokoju, w którym można się spotkać. Najpierw wybierz pokój.
party-server-lost = Połączenie z serwerem zostało utracone. Trwa ponowna próba.
party-rejoined = Ponownie połączono. { -watch-party-cap } trwa.
party-left = Opuszczono { -watch-party }.
