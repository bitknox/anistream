queue-episodes-left = { $count ->
    [one] Został jeden odcinek: { $title }
    [few] Zostały { $count } odcinki: { $title }
    [many] Zostało { $count } odcinków: { $title }
   *[other] Zostało { $count } odcinka: { $title }
 }
episodes-count = { $count ->
    [one] { $count } odcinek
    [few] { $count } odcinki
    [many] { $count } odcinków
   *[other] { $count } odcinka
 }
progress-episodes = { $total ->
    [one] { $watched } z { $total } odcinka
    [few] { $watched } z { $total } odcinków
    [many] { $watched } z { $total } odcinków
   *[other] { $watched } z { $total } odcinków
 }
duration-days-hours = { $days } d { $hours } g
group-in-progress = W toku
group-new-this-week = Nowe w tym tygodniu
continue-empty-title = Nic jeszcze nie obejrzano
continue-empty-detail = Wybierz coś z ekranu Sezonowe albo wyszukaj tytuł.
continue-empty-action = Przeglądaj Sezonowe
continue-load-failed = Nie udało się wczytać: { -continue-cap }
continue-heading = { -continue-cap }
continue-airing-today = Emitowane dzisiaj
continue-this-season = W tym sezonie
continue-hide = Ukryj w sekcji „{ -continue-cap }”
continue-hidden = Ukryto w sekcji „{ -continue-cap }”.
continue-drop = Przenieś do porzuconych
continue-more-for = Więcej: { $title }
continue-row-resume = Wznów E{ $episode } · { $clock }
continue-row-resume-dubbed = Wznów E{ $episode } { -dubbed } · { $clock }
continue-row-start = Odtwórz E{ $episode }
continue-row-start-dubbed = Odtwórz E{ $episode } { -dubbed }
continue-row-start-aired = Odtwórz E{ $episode } · wyszedł { $ago }
continue-row-caught-up = Na bieżąco · E{ $episode } { $when }
continue-row-caught-up-plain = Na bieżąco
continue-widget-connect = Połącz AniList, aby zobaczyć to tutaj.
continue-caught-up = Na bieżąco. Nic więcej do obejrzenia.
title-button-resume = Wznów odcinek { $episode } · { $time }
title-button-play = Odtwórz odcinek { $episode }
title-button-add-source = Dodaj { -source }, aby odtworzyć
library-status-watching = Oglądam
library-status-planning = Planowane
library-status-completed = Ukończone
library-status-paused = Wstrzymane
library-status-dropped = Porzucone
library-sort-updated = Ostatnio zmienione
library-sort-title = Tytuł
library-sort-score = Ocena
library-sort-button = Sortowanie: { $sort }
library-sort-label = Sortuj według: { $sort }
library-signed-out-title = Listy pochodzą z AniList
library-signed-out-detail = Zaloguj się, aby synchronizować obejrzane tytuły i miejsca, w których przerwano.
library-signed-out-action = Zaloguj się do AniList
library-list-empty = Brak tytułów na liście: { $list }
library-list-empty-detail = Tytuły dodane do tej listy w AniList pojawią się tutaj.
library-list-load-failed = Nie udało się wczytać listy: { $list }
library-loading = Wczytywanie list
library-add-to-list = Dodaj do listy
library-add-planning = Dodaj do: Planowane
library-added-planning = Dodano do: Planowane.
library-on-your-list = Już na liście
library-in-your-list = Na liście
progress-episodes-short = E{ $done } z { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Odtwórz E{ $episode } { -dubbed } · wyszedł { $ago }
up-next-heading = { -up-next-cap }
up-next-starts = Rusza { $date }
up-next-reason-new-season = Nowy sezon
up-next-reason-planning = Na liście: Planowane
up-next-reason-recommended = Bo skończono: { $title }
up-next-reason-recommended-plain = Polecane
up-next-because-detail = Polecane na AniList przez osoby, którym podobał się { $title }
up-next-reason-airing-today = Emitowane dzisiaj
up-next-add-planning = Dodaj do: Planowane
up-next-not-interested = Nie interesuje mnie
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } jest ukryte w sekcji „{ -up-next-cap }”
up-next-hidden-until-airs = { $title } jest ukryte do emisji
up-next-broadcast = Odcinek { $episode } · { $when }
up-next-out = wyszedł { $ago }
