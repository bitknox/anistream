sources-order-detail-edit = { -source-cap-pl } są sprawdzane od góry do dołu. Stuknij Edytuj, aby zmienić kolejność.
sources-update-to = Zaktualizuj do { $version }
sources-update-available-version = Dostępna wersja { $version }
stores-store-label = { -plugin-store-cap }
stores-checked-at = Sprawdzono { $when }
stores-updates-available = { $count ->
    [one] Jedna aktualizacja
    [few] { $count } aktualizacje
    [many] { $count } aktualizacji
   *[other] { $count } aktualizacji
 }
stores-installed-version = Zainstalowano { $version }
stores-plugins-count = { $count ->
    [one] Jedna { -plugin }
    [few] { $count } { -plugin-pl }
    [many] { $count } wtyczek
   *[other] { $count } { -plugin-pl }
 }
stores-devices-count = { $count ->
    [one] Jedno { -device }
    [few] { $count } { -device-pl }
    [many] { $count } urządzeń
   *[other] { $count } { -device-pl }
 }
stores-replace-by-hand-detail = Wersja { $version } z { $store } zastępuje kopię dodaną ręcznie.
stores-open-join-on-detail = Dołączają bez nazwy. Wyłączenie zamyka link, a ponowne włączenie tworzy nowy.
stores-open-join-off-detail = Mogą dołączać tylko zaproszone osoby. Po włączeniu każdy z linkiem może dołączyć bez nazwy.
stores-open-join-no-names = Nie zapisuje się żadnych nazw.
stores-open-join-keep-working = Działają dalej, dopóki nie zostaną usunięte.
stores-device-seen-at = Widziano { $when }
stores-device-not-seen = Nie widziano od
stores-publish-offered = { -plugin-store-cap } oferuje { $version }
stores-publish-not-yet = Jeszcze niedostępna w: { -plugin-store }
torrents-sheet-turn-on = Włącz torrenty
torrents-sheet-protect = Ochrona torrentów
torrents-sheet-edit-proxy = Edytuj proxy
torrents-turn-on-button = Włącz
torrents-edit-proxy = Edytuj proxy
torrents-proxy-optional = Opcjonalne
torrents-warn-only-detail = { -app-name } ostrzeże zamiast wstrzymać. Jeśli VPN lub proxy nie działa, inni mogą zobaczyć adres IP.
torrents-proxy-line-user = { $address } · jako { $user }
party-leave-and-quit = Opuść i zamknij
party-leave-detail-window = Zamknięcie okna opuszcza { $room }. Pozostali uczestnicy oglądają dalej.
party-choose-source-title = Wybierz { -source } dla odcinka { $episode }
party-choose-source = Wybierz { -source }
tv-generic-name = Telewizor
tv-row-paired = Sparowany
tv-row-paired-away = Sparowany, nie widać go w tej sieci
tv-row-not-paired = Jeszcze niesparowany
tv-row-pair-hint = Paruje z tym telewizorem
tv-picker-title-episode = { $title }, odcinek { $episode }
tv-connecting-generic = Łączenie z telewizorem
tv-section-network = W tej sieci
tv-section-not-listed = Poza listą
tv-scan-code = Zeskanuj kod na telewizorze
tv-code-on = Kod na { $name }
tv-nothing-playing = Nic nie jest odtwarzane
tv-remote-action = Pokaż pilot
tv-status-cant-reach = Nie można połączyć się z { $name }. Sprawdź, czy jest włączony i w tej samej sieci.
tv-bar-cant-reach = Nie można połączyć z { $name }
tv-bar-not-found = Nie znaleziono: { $name }
tv-paired-tvs = Sparowane telewizory
tv-title-list = Telewizory
tv-settings-footnote = Telewizor widzi to { -device } z kodem { $code }. Zapomnienie telewizora tutaj usuwa też to { -device }.
tv-seen-just-now = przed chwilą
stores-link-send-once = Wyślij ten link do: { $who }. Działa raz, do { $until }.
stores-link-send-once-anyone = Wyślij ten link osobie, dla której jest. Działa raz, do { $until }.
sources-remove-title = Usunąć { $name }?
sources-remove-detail-device = Usuwa się ją z: { -device }. { -plugin-store-cap } może ją zainstalować ponownie.
