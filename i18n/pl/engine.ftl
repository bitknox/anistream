notice-saved = Zapisano: { $label }.
notice-not-saved = Nie udało się zapisać tego ustawienia. { $reason }
notice-title-load-failed = Nie udało się wczytać tego tytułu. Sprawdź połączenie i spróbuj ponownie.
notice-playback-failed = Odtwarzanie się nie powiodło. Wybierz inne { -source }.
notice-engine-fault = Coś poszło nie tak w { -app-name }. Program działa dalej, ale jeśli ekran wygląda na zawieszony, zamknij go i otwórz ponownie.
notice-database-set-aside = Baza danych była uszkodzona, więc { -app-name } utworzył nową. Stary plik zachowano w: { $path }. { -tracker-cap-pl } przywrócą swoje dane przy następnej synchronizacji.
start-failed-folder = { -app-name } nie mógł utworzyć folderu { $path }. Sprawdź, czy na dysku jest miejsce i czy do folderu można zapisywać, a potem spróbuj ponownie. { $reason }
start-failed-config = { $path } zawiera ustawienie, którego { -app-name } nie może użyć: { $problem }. Popraw je lub usuń, a potem spróbuj ponownie.
start-failed-database = { -app-name } nie mógł otworzyć bazy danych w: { $path }. Jeśli w tym folderze jest otwarta inna aplikacja { -app-name }, zamknij ją i spróbuj ponownie. { $reason }
start-failed-network = { -app-name } nie mógł skonfigurować połączenia sieciowego według ustawień w config.toml. { $reason }
start-failed = { -app-name } nie mógł się uruchomić. Zamknij go i otwórz ponownie. { $reason }
start-failed-remote = Pilot nie mógł zacząć nasłuchiwania ani usunąć telefonu. { $reason }
start-failed-player = Odtwarzacz nie mógł się uruchomić. { $reason }
notice-episodes-queued = { $count ->
    [one] Dodano do kolejki { $count } odcinek.
    [few] Dodano do kolejki { $count } odcinki.
    [many] Dodano do kolejki { $count } odcinków.
   *[other] Dodano do kolejki { $count } odcinków.
 }
