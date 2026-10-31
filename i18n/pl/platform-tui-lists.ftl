lists-terminal-too-small = Potrzebny większy terminal
downloads-screen-title = Pobieranie
downloads-column-title = Tytuł
downloads-column-state = Stan
downloads-column-progress = Postęp
downloads-empty-heading = Nic nie czeka w kolejce
downloads-empty-hint = Naciśnij { $key } na tytule lub odcinku, aby dodać go do kolejki offline.
downloads-row-episode = Odc. { $episode }
downloads-row-episode-ahead = Odc. { $episode } · z góry
downloads-note-sizes = { $downloaded } / { $total }
downloads-note-waiting-metadata = Oczekiwanie na metadane
downloads-note-torrents-off = Torrenty wyłączone
downloads-note-waiting-start = Czeka na rozpoczęcie
downloads-state-queued = W kolejce
downloads-state-active = Pobieranie
downloads-state-paused = Wstrzymane
downloads-state-done = Pobrane
downloads-state-failed = Błąd
downloads-detail-ahead = Pobrane z wyprzedzeniem, usunięte dzień po obejrzeniu
downloads-detail-with-ahead = { $detail } · pobrane z wyprzedzeniem, usunięte dzień po obejrzeniu
downloads-detail-paused-at = Wstrzymane na { $downloaded } z { $total } (pobieranie wznowi się stamtąd)
downloads-detail-paused-early = Wstrzymane przed rozpoczęciem
downloads-detail-torrents-off = Pobieranie wymaga torrentów, a te są wyłączone (naciśnij { $key }, aby otworzyć ustawienia)
downloads-detail-queued = W kolejce (rozpocznie się, gdy zwolni się miejsce do pobierania)
downloads-detail-waiting-metadata = Oczekiwanie na metadane torrenta
downloads-seeding-heading = Seedowanie po obejrzeniu
downloads-seeding-detail = { $ratio } z { $goal } · zostało { $left } · wysłano { $uploaded }
downloads-failed-no-reason = Pobieranie się nie udało i nie zapisano przyczyny.
downloads-not-finished = To pobieranie jeszcze się nie skończyło.
accounts-screen-title = Konta
accounts-badge-sign-in = { $tracker } { $mark } zaloguj się
accounts-needs-sign-in = { $tracker } wymaga ponownego logowania. Naciśnij { $key }, aby otworzyć konta.
accounts-conflicts-found = { $count ->
    [one] Znaleziono 1 różnicę synchronizacji. Naciśnij { $key }, aby ją przejrzeć.
    [few] Znaleziono { $count } różnice synchronizacji. Naciśnij { $key }, aby je przejrzeć.
    [many] Znaleziono { $count } różnic synchronizacji. Naciśnij { $key }, aby je przejrzeć.
   *[other] Znaleziono { $count } różnic synchronizacji. Naciśnij { $key }, aby je przejrzeć.
 }
accounts-conflicts-none = Nie ma różnic synchronizacji do przejrzenia.
accounts-no-tracker = Nie połączono żadnego { -tracker }a. Naciśnij { $key }, aby otworzyć konta.
accounts-finish-signing-in = Dokończ logowanie do { $tracker }
accounts-step-open = 1. Otwórz   { $url }
accounts-step-enter = 2. Wpisz
accounts-code-expires-seconds = Wygasa za { $seconds } s
accounts-code-expires-minutes = Wygasa za { $minutes } min
accounts-code-expired = Kod wygasł
accounts-column-tracker = { -tracker-cap }
accounts-column-state = Stan
accounts-column-queued = W kolejce
accounts-column-token = Token
accounts-empty-heading = Nie włączono { -tracker }ów
accounts-empty-hint = Dodaj { -tracker } do { $setting } w config.toml. Historia oglądania działa bez żadnego.
accounts-state-sign-in-again = Zaloguj ponownie
accounts-state-connected = Połączono
accounts-state-not-connected = Nie połączono
accounts-pull-failed-today = Nie udało się pobrać listy o { $time }: { $error }
accounts-pull-failed-dated = Nie udało się pobrać listy { $date } o { $time }: { $error }
accounts-pull-failed = Nie udało się pobrać listy: { $error }
accounts-token-files = Tokeny są przechowywane w plikach z uprawnieniami 0600, ponieważ pęk kluczy systemu jest niedostępny.
accounts-enter-sign-out = ↵ Wyloguj
accounts-enter-sign-in = ↵ Zaloguj
accounts-action-sign-in-again = Zaloguj ponownie
accounts-action-sign-out = Wyloguj
accounts-action-sign-in = Zaloguj
accounts-rows-empty = Nie włączono { -tracker }ów. Dodaj { -tracker } do { $setting } w config.toml.
accounts-row-signed-in-as = Zalogowano jako { $user }
accounts-row-connected = Połączono
accounts-row-not-connected = Nie połączono
accounts-row-queued = { $count ->
    [one] { $count } w kolejce
    [few] { $count } w kolejce
    [many] { $count } w kolejce
   *[other] { $count } w kolejce
 }
accounts-row-queued-held = { $queued } w kolejce, { $held } czeka na dopasowanie identyfikatora
accounts-conflict-keep-mine = Zostaw moje
accounts-conflict-keep-theirs = Zostaw ich
accounts-conflict-row = { $title }  ·  { $field }: moje { $local } / ich { $remote }
lists-library-title = { -library-cap }
lists-remove-wrong-list = Tytuły można ukryć tylko w sekcji { -continue-cap }.
lists-continue-removed = { $title } ukryto w sekcji { -continue-cap }. Po ponownym obejrzeniu wróci.
lists-nothing-selected = Nie wybrano tytułu.
lists-search-prompt = Szukaj: { $query }
lists-load-failed = Nie udało się wczytać tej listy
lists-loading = Wczytywanie…
lists-empty-press-enter = Naciśnij ↵, aby szukać
lists-empty-type-to-search = Wpisz, aby szukać
lists-empty-browse-filtered = Naciśnij ↵, aby przeglądać z tymi filtrami.
lists-empty-browse-popular = Naciśnij ↵, aby przeglądać najpopularniejsze.
lists-empty-no-matches = Brak wyników
lists-empty-fewer-filters = Użyj mniej filtrów.
lists-empty-shorter-query = Spróbuj krótszego zapytania albo tytułu w romaji.
lists-empty-nothing-watched = Nic jeszcze nie obejrzano
lists-empty-ways-on = Naciśnij { $seasonal }, aby zobaczyć ten sezon, albo { $search }, aby szukać.
lists-empty-nothing-to-continue = Brak tytułów w sekcji { -continue-cap }
lists-empty-hidden-come-back = Ukryte tytuły wrócą po ponownym obejrzeniu. Naciśnij { $seasonal }, aby zobaczyć ten sezon, albo { $search }, aby szukać.
lists-empty-nothing-here = Nic tu jeszcze nie ma
lists-library-no-account = Nie połączono konta
lists-library-works-without = Historia oglądania działa bez konta. { -library-cap } pokazuje listę z { -tracker }a.
lists-library-enable-tracker = Dodaj { -tracker } do { $setting } w config.toml, a potem naciśnij { $key }, aby otworzyć konta.
lists-library-sign-in = Naciśnij { $key }, aby otworzyć konta i się zalogować.
lists-segment-watching = Oglądane
lists-segment-planning = Planowane
lists-segment-completed = Ukończone
lists-segment-paused = Wstrzymane
lists-segment-dropped = Porzucone
lists-episode = Odc. { $episode }
lists-episode-dubbed = { $episode } · { -dubbed-cap }
lists-score = { $score }%
lists-row-resume-percent = { $episode } · { $percent }%
lists-row-resume-clock = { $episode } · { $clock }
lists-episode-out = Odc. { $episode } wyszedł
lists-episode-in = Odc. { $episode } za { $countdown }
lists-next-in = { -up-next-cap } za { $countdown }
lists-episode-out-ago = Odc. { $episode } wyszedł { $countdown } temu
lists-episode-out-just-now = Odc. { $episode } wyszedł przed chwilą
lists-time-in = Za { $countdown }
lists-time-ago = { $countdown } temu
lists-time-just-now = Przed chwilą
lists-time-approximate = ~{ $time }
lists-day-today = Dziś
lists-day-yesterday = Wczoraj
lists-day-tomorrow = Jutro
lists-preview-continue-percent = { -continue-cap } odc. { $episode } na { $clock } ({ $percent }%)
lists-preview-continue = { -continue-cap } odc. { $episode } na { $clock }
lists-progress-aired = { $total ->
    [one] { $done } z { $total } odcinek wyemitowany
    [few] { $done } z { $total } odcinki wyemitowane
    [many] { $done } z { $total } odcinków wyemitowanych
   *[other] { $done } z { $total } odcinków wyemitowanych
 }
lists-progress-watched = { $count ->
    [one] 1 odcinek obejrzany
    [few] { $count } odcinki obejrzane
    [many] { $count } odcinków obejrzanych
   *[other] { $count } odcinków obejrzanych
 }
lists-filter-genre = Gatunek
lists-filter-year = Rok
lists-filter-season = Sezon
lists-filter-format = Format
lists-filter-status = Status
lists-filter-sort = Sortowanie
lists-filter-any = Dowolny
lists-sort-relevance = Trafność
lists-season-winter = Zima
lists-season-spring = Wiosna
lists-season-summer = Lato
lists-season-fall = Jesień
lists-format-tv = TV
lists-format-tv-short = TV krótkie
lists-format-movie = Film
lists-format-ova = OVA
lists-format-ona = ONA
lists-format-special = Specjalny
lists-format-music = Muzyka
lists-status-airing = Emitowane
lists-status-finished = Zakończone
lists-status-upcoming = Nadchodzące
lists-sort-popularity = Popularność
lists-sort-score = Ocena
lists-sort-trending = Trendy
lists-sort-newest = Najnowsze
