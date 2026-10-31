stores-device-count = { $count ->
    [one] { $count } { -device }
    [few] { $count } { -device-pl }
    [many] { $count } urządzeń
   *[other] { $count } { -device-pl }
 }
stores-dialog-title = { $title }, { $store }
stores-owner-unreachable = Nie udało się połączyć ze sklepem z wtyczkami
stores-owner-no-answer = { -plugin-store-cap } nie odpowiedział
stores-owner-no-answer-detail = Może być zajęty albo jego komputer jest wyłączony.
stores-owner-needs-connection = { -plugin-store-cap } wymaga połączenia
stores-owner-needs-connection-detail = Spróbuj ponownie, gdy połączenie wróci.
stores-owner-link-offline = Utworzenie linku wymaga połączenia ze sklepem z wtyczkami.
stores-send-link-until = Wyślij ten link. Działa jeden raz, do { $until }.
stores-open-on-detail = Dołączają bez podawania nazwy. Wyłączenie zatrzymuje link. Ponowne włączenie tworzy nowy.
stores-open-off-detail = Dołączyć mogą tylko zaproszone osoby. Po włączeniu każdy z linkiem dołączy bez podawania nazwy.
stores-open-no-names = Nazwy nie są zapisywane.
stores-open-keep-working = Działają, dopóki nie zostaną usunięte.
stores-device-seen = Widziane { $when }
stores-device-never-seen = Nie widziane od dołączenia
stores-publish-short = Opublikuj wtyczkę
stores-publish-none-hint = Tutaj pojawiają się { -plugin-pl } zbudowane lub zainstalowane na tym urządzeniu.
stores-not-published = Jeszcze nie ma jej w sklepie z wtyczkami
stores-store-offers = { -plugin-store-cap } oferuje { $version }
stores-published-current = Aktualne w sklepie z wtyczkami
stores-published-state = Opublikowano
stores-publish-label = Opublikuj { $name } { $version }
