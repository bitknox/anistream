notice-signed-in = Zalogowano do { $tracker }.
notice-signed-in-keychain = Zalogowano do { $tracker }. Token zapisano w pęku kluczy systemu.
notice-signed-in-file = Zalogowano do { $tracker }. Pęk kluczy jest niedostępny, więc token zapisano w prywatnym pliku na tym komputerze.
notice-signed-in-unsaved = Zalogowano do { $tracker }, ale token nie został zapisany, więc przy następnym uruchomieniu trzeba będzie zalogować się ponownie. { $reason }
notice-token-not-saved = Nie udało się zapisać tokenu logowania. Spróbuj zalogować się ponownie. { $reason }
notice-sign-in-failed = Nie udało się zalogować do { $tracker }. Spróbuj ponownie. { $reason }
notice-sign-in-stopped = Logowanie do { $tracker } zostało przerwane, zanim się zakończyło. Spróbuj ponownie. { $reason }
notice-token-exchange-failed = Nie udało się otrzymać tokenu od { $tracker }. Spróbuj zalogować się ponownie. { $reason }
notice-browser-not-opened = Nie udało się otworzyć przeglądarki do logowania. Sprawdź, czy któraś jest zainstalowana, i spróbuj ponownie. { $reason }
notice-sign-in-not-waiting = Żadne logowanie do { $tracker } nie czeka na tę odpowiedź. Rozpocznij logowanie od nowa.
notice-sign-in-wrong-request = Logowanie do { $tracker } nie wróciło w odpowiedzi na to żądanie. Rozpocznij logowanie od nowa.
notice-sign-in-no-token = Logowanie do { $tracker } wróciło bez tokenu. Spróbuj ponownie.
notice-mal-client-missing-build = Nie udało się dokończyć logowania do MyAnimeList: ta wersja nie zawiera klienta dla tej usługi.
notice-kitsu-signs-in-on-accounts = Logowanie do Kitsu odbywa się z ekranu kont.
notice-sign-in-unsupported = Logowanie do { $tracker } nie jest jeszcze obsługiwane.
notice-sign-in-needs-redirect = Logowanie do { $tracker } wymaga przekierowania w przeglądarce, którego ta aplikacja nie może odebrać.
notice-anilist-implicit-flow = Ta metoda logowania AniList działa tylko w aplikacji { -app-name }. Aby zalogować się tutaj, ustaw { $setting } na „loopback” w pliku konfiguracyjnym.
notice-anilist-client-missing = AniList wymaga identyfikatora klienta i sekretu. Ustaw { $client } i { $secret } w pliku konfiguracyjnym, a potem zaloguj się ponownie. Adres logowania podaje polecenie: { -app-name } --login-url.
notice-anilist-paste-flow = Wklejanie kodu nie jest tu możliwe. Zakończ działanie i uruchom zamiast tego { -app-name } --login.
notice-mal-client-missing = MyAnimeList wymaga identyfikatora klienta. Zarejestruj aplikację z typem „other” na myanimelist.net/apiconfig, a potem ustaw { $setting } w pliku konfiguracyjnym.
status-waiting-for-redirect = Czekanie, aż { $tracker } przekieruje z powrotem…
notice-mal-no-refresh-token = MyAnimeList nie wydało tokenu odświeżania. Logowanie wygaśnie za około miesiąc i trzeba będzie zalogować się ponownie.
notice-client-id-missing = Najpierw ustaw { $setting } w pliku konfiguracyjnym.
notice-signed-out = Wylogowano z { $tracker }.
notice-signed-out-queue = { $count ->
    [one] Wylogowano z { $tracker }. Jedna zmiana w kolejce zostanie zachowana na następne logowanie.
    [few] Wylogowano z { $tracker }. { $count } zmiany w kolejce zostaną zachowane na następne logowanie.
    [many] Wylogowano z { $tracker }. { $count } zmian w kolejce zostanie zachowanych na następne logowanie.
   *[other] Wylogowano z { $tracker }. { $count } zmian w kolejce zostanie zachowanych na następne logowanie.
 }
notice-sign-out-failed = Nie udało się wylogować z { $tracker }. Spróbuj ponownie. { $reason }
notice-not-saved-because = Nie udało się zapisać tego ustawienia. { $cause }
notice-protection-saved = Zapisano ustawienie ochrony torrentów.
notice-protection-not-checked = Nie udało się sprawdzić ochrony torrentów. { $cause }
notice-proxy-password-not-kept = Nie udało się zachować hasła do proxy. Wpisz je ponownie, aby spróbować zapisać. { $reason }
notice-show-setting-refused = Nie udało się zapisać tego ustawienia. { $value } nie jest prawidłową wartością dla { $setting }.
notice-show-not-reset = Nie udało się zresetować ustawień tego tytułu. { $reason }
notice-show-no-such-subtitles = Nie udało się zapisać tego wyboru napisów. Ten tytuł nie używał wcześniej tych napisów.
notice-torrents-waiting-for-check = Torrenty czekają, aż kontrola połączenia znów przejdzie.
notice-mapping-data-refreshed = Odświeżono dane mapowania.
notice-mapping-data-failed = Nie udało się odświeżyć danych mapowania ({ $datasets }). Ponowna próba za { $wait }. { $cause }
notice-episode-aired = { $title }, odcinek { $episode }, został wyemitowany, gdy aplikacja była zamknięta.
notice-episodes-aired = { $count ->
    [one] Jeden nowy odcinek z listy tytułów został wyemitowany. Jest w kalendarzu.
    [few] { $count } nowe odcinki z listy tytułów zostały wyemitowane. Są w kalendarzu.
    [many] { $count } nowych odcinków z listy tytułów zostało wyemitowanych. Są w kalendarzu.
   *[other] { $count } nowych odcinków z listy tytułów zostało wyemitowanych. Są w kalendarzu.
 }
notice-update-available = Wydano { $tag }. Uruchom { -app-name } --update, aby zainstalować aktualizację.
cause-confirm-no-protection = Najpierw trzeba potwierdzić, że ochrona ma być wyłączona.
cause-confirm-warn-only = Najpierw trzeba potwierdzić wybór „tylko ostrzeżenie”.
cause-proxy-needs-address = Proxy wymaga hosta i portu.
cause-proxy-address-unusable = Adres proxy nie nadaje się do sprawdzenia.
cause-server-unreachable = Nie udało się połączyć z serwerem.
