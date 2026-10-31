store-joined = Dołączono do { $store } jako { $member }.
store-joined-unnamed = Dołączono do { $store } bez nazwy.
store-left = Opuszczono { $store }. Jego { -plugin-pl } pozostają zainstalowane.
plugin-removed = Usunięto { $plugin }.
store-member-removed = Usunięto. { -plugin-store-cap } nie przyjmuje już te { -device-pl }.
store-invite-cancelled = Zaproszenie anulowano. Link przestał działać.
store-open-join-on = Teraz dołączyć może każdy, kto ma link. Skopiuj go z ekranu członków.
store-open-join-off = Teraz mogą dołączać tylko zaproszone osoby.
store-open-join-off-devices = { $devices ->
    [one] Teraz mogą dołączać tylko zaproszone osoby. Dołączone linkiem { -device } działa dalej, dopóki nie zostanie usunięte.
    [few] Teraz mogą dołączać tylko zaproszone osoby. Dołączone linkiem { -device-pl } ({ $devices }) działają dalej, dopóki nie zostaną usunięte.
    [many] Teraz mogą dołączać tylko zaproszone osoby. Dołączone linkiem { -device-pl } ({ $devices }) działają dalej, dopóki nie zostaną usunięte.
   *[other] Teraz mogą dołączać tylko zaproszone osoby. Dołączone linkiem { -device-pl } ({ $devices }) działają dalej, dopóki nie zostaną usunięte.
 }
store-device-removed = { -plugin-store-cap } nie przyjmuje już to { -device }.
store-devices-removed = Usunięto wszystkie { -device-pl }, które dołączyły linkiem.
plugin-published = Opublikowano { $plugin } { $version }.
plugin-unpublished = Nie oferuje się już: { $plugin }.
store-admin-opened = Otwarto stronę administracyjną w przeglądarce. Link działa raz, przez 10 minut.
store-admin-open-failed = Nie udało się otworzyć przeglądarki ({ $reason }). Na komputerze, na którym działa { -plugin-store }, uruchom „{ -app-name } store admin”, aby wydrukować link.
plugin-installed-unused = Zainstalowano { $plugin } { $version }, ale { -plugin-pl } są wyłączone, więc { -plugin } nie zostanie wykorzystana.
plugin-installed-name-clash = Zainstalowano { $plugin }, ale { -source } wbudowane ma nazwę { $id }, więc { -plugin } nie została wczytana.
plugin-installed = Zainstalowano { $plugin } { $version }.
plugin-installed-not-loaded = Zainstalowano { $plugin }, ale nie udało się wczytać. { $reason }
plugin-updates-ready = { $count ->
    [one] { -plugin-cap }: gotowa aktualizacja: { $names }.
    [few] { -plugin-cap-pl }: gotowe aktualizacje: { $names }.
    [many] { -plugin-cap-pl }: gotowe aktualizacje: { $names }.
   *[other] { -plugin-cap-pl }: gotowe aktualizacje: { $names }.
 }
plugins-load-failed = Nie udało się wczytać { -plugin-pl }. { $reason }
store-token-missing = { -device-cap } nie ma zapisanego tokenu dla { $store }. Dołącz ponownie nowym zaproszeniem.
store-not-joined = { -plugin-store-cap } nie jest już dołączony.
store-key-damaged = { -plugin-store-cap } ma uszkodzony klucz podpisu. Opuść go i dołącz ponownie nowym zaproszeniem.
store-device-refused = { $store } nie przyjmuje już to { -device }. Dołącz ponownie nowym zaproszeniem.
store-unreachable = Nie udało się połączyć z { $store }. { $reason }
store-moved = { $store } jest dołączony pod adresem { $current }, ale ten link wskazuje na { $target }. Jeśli { -plugin-store } się przeniósł, najpierw go opuść i dołącz ponownie.
store-join-would-unname = Dołączono do { $store } jako { $member }. Ten link dołączyłby ponownie bez podanej nazwy, więc nic nie zmieniono.
store-not-the-invited = Nie udało się potwierdzić, że pod { $url } jest { -plugin-store } z zaproszenia ({ $reason }). Nic nie dołączono.
store-token-not-kept = { -plugin-store-cap }: nie udało się zapisać tokenu. { $reason }
store-invite-unusable = To zaproszenie zostało już użyte, anulowane lub wygasło. Poproś o nowe.
store-open-link-unusable = Ten link nie pozwala już nikomu dołączyć. Poproś właściciela o nowy link albo zaproszenie.
plugin-reserved-name = { -plugin-cap } nie może nazywać się { $plugin }, bo tak nazywa się { -source } wbudowane.
store-plugin-withdrawn = { $store } nie oferuje już { $plugin }.
store-plugin-changed = { $store } zmienił { $plugin } od chwili pokazania. Sprawdź to ponownie przed instalacją.
plugin-from-other-store = Zainstalowano { $plugin } z { $store }. { $store } zapewnia aktualizacje.
plugin-duplicate-file = { $path } już deklaruje { $plugin }. Najpierw usuń ten plik, bo uruchomią się dwie kopie.
plugin-install-failed = Nie udało się zainstalować { $plugin }. { $reason }
plugin-publish-failed = Nie udało się opublikować { $plugin }. { $reason }
plugin-missing-locally = { $plugin } nie ma w folderze, w którym są { -plugin-pl }.
store-invite-needs-link = To { -device } dołączyło do { $store } linkiem. Wklej ten sam link na drugim { -device }.
store-link-malformed = { -plugin-store-cap } wysłał link, który nie daje się odczytać. Poproś o sprawdzenie. { $reason }
store-link-wrong-key = { -plugin-store-cap } wysłał link z innym kluczem. Sprawdź adres, z którego zaczęto.
store-admin-missing = { $store } nie ma strony administracyjnej. Jego serwer może być starszy.
store-admin-link-foreign = { $store } wysłał link administracyjny do innego adresu.
store-open-join-unsupported = Serwer { $store } jest starszy i nie pozwala nikomu dołączyć linkiem. Najpierw go zaktualizuj.
source-switched-off = Wyłączono { $name }. Przy szukaniu odcinków pomija się to { -source }.
source-switched-on = Włączono { $name }.
plugin-not-loaded = Nie wczytano { $id }. Sprawdź instalację.
release-pinned = Dla tego tytułu wybrano: { $name }.
notice-choice-not-saved = Nie udało się zapisać wyboru. { $reason }
torrent-check-failed-running = Sprawdzenie połączenia nie powiodło się. Torrenty działają dalej.
torrent-vpn-waiting = Torrenty ruszą po przejściu kontroli VPN. { $reason }
torrent-vpn-unverified = Torrenty ruszą po przejściu kontroli VPN. Połączenie nie zostało jeszcze sprawdzone.
torrent-interface-closed = Zamknięto { -app-name }, zanim ruszyły torrenty.
torrent-session-failed = Nie udało się uruchomić sesji torrentów. { $reason }
torrent-no-kill-switch = Żadna reguła zapory nie zatrzyma ruchu torrentów, jeśli VPN się rozłączy. Uruchom „{ -app-name } --doctor”, aby to skonfigurować.
torrent-not-checked = Nie udało się sprawdzić połączenia. { $reason }
sources-reloaded-torrents-pending = { -source-cap-pl } ponownie wczytane. Torrenty ruszą po przejściu kontroli VPN.
sources-reloaded-torrents-off = Torrenty są wyłączone. Sesja została zatrzymana.
job-anilist-limit = Czekanie na limit zapytań AniList
notice-anilist-limit = Czekanie { $wait } na limit zapytań AniList.
notice-anilist-unreachable = AniList nie odpowiada. Na razie pokazano dane z innego serwisu.
notice-metadata-cached = Żadna usługa metadanych nie odpowiedziała. Pokazano ostatnie zapisane dane.
notice-anilist-back = AniList znowu odpowiada.
notice-party-not-built = { -watch-party-cap-pl } nie są w tej wersji aplikacji.
