stores-needs-larger-terminal = Potrzebny większy terminal
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Członkowie
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = Ty
stores-column-state = Stan
stores-none-joined = Nie dołączono jeszcze do żadnego { -store }u
stores-role-owner = Zarządca
stores-role-member = Członek
stores-join-row = + Dołącz do { -store }u
stores-state-revoked = Dostęp odebrany
stores-state-offline = Brak połączenia
stores-state-up-to-date = Aktualny
stores-state-updates = { $count ->
    [one] { $count } aktualizacja
    [few] { $count } aktualizacje
    [many] { $count } aktualizacji
   *[other] { $count } aktualizacji
 }
stores-footer-revoked = { -device-cap } nie jest już przyjmowane przez ten { -store }. Właściciel może wysłać nowe zaproszenie.
stores-footer-offline = { $url } ({ $reason })
stores-key = Klucz { $fingerprint }
stores-joined-without-name = Dołączono bez nazwy
stores-joined-as = Dołączono jako { $name }
stores-footer-join = Wklej otrzymane zaproszenie albo link do { -store }u
stores-owner-tag = Właśc.
stores-anyone-can-join = Każdy z linkiem może dołączyć
stores-note-offline = Brak połączenia, pokazano ostatnią otrzymaną listę ({ $reason })
stores-note-revoked = { -device-cap } nie jest już przyjmowane przez ten { -store }. To, co zainstalowano, nadal działa.
stores-column-plugin = { -plugin-cap }
stores-column-version = Wersja
stores-empty-owner = Nic jeszcze nie opublikowano. Poniżej można opublikować { -plugin-pl }.
stores-empty-member = Nic jeszcze nie opublikowano
stores-line-link-device = Połącz kolejne { -device }
stores-line-invite = Zaproś kogoś
stores-line-members = Członkowie
stores-line-who-can-join = Kto może dołączyć
stores-line-publish = Opublikuj z katalogu
stores-line-admin = Otwórz panel administracyjny
stores-line-leave = Opuść ten { -store }
stores-plugin-not-installed = Nie zainstalowano
stores-plugin-installed = Zainstalowano
stores-plugin-update-from = Aktualizacja z { $from }
stores-plugin-added-by-hand = Dodano ręcznie
stores-plugin-from-another-store = Z innego { -store }u
stores-footer-update-hosts = Aktualizacja chce się też łączyć z: { $hosts }
stores-footer-may-contact = Może łączyć się z: { $hosts }
stores-footer-link-device = Jednorazowy link dołączający kolejne { -device } na tym samym koncie
stores-footer-invite = Jednorazowy link dla nowej osoby
stores-footer-members = Lista osób, które dołączyły, i odbieranie dostępu
stores-footer-close-join = ↵ Zezwalaj tylko zaproszonym osobom i zablokuj link
stores-footer-open-join = ↵ Pozwól każdemu z linkiem do { -store }u dołączyć bez nazwy
stores-footer-publish = Opublikuj z katalogu na tym komputerze
stores-footer-admin = Członkowie, zaproszenia i { -plugin-pl }: zarządzaj w przeglądarce
stores-footer-leave = { -plugin-cap-pl } zainstalowane z niego zostają na tym komputerze i przestają się aktualizować
stores-who-open = Każdy z linkiem
stores-who-invite-only = Tylko zaproszone osoby
stores-label-offers = Oferty
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
    [few] { $count } { -plugin-pl }
    [many] { $count } wtyczek
   *[other] { $count } wtyczek
 }
stores-offered-plugin = { $name } { $version }  może łączyć się z: { $hosts }
stores-offered-installed = Zainstalowano
stores-offered-installed-version = Zainstalowano { $from }
stores-offered-added-by-hand = Dodano ręcznie
stores-offered-from-other = Z { $other }
stores-offered-more = i jeszcze { $count }
stores-hosts-none = niczym
stores-loading = Wczytywanie…
stores-member-you = { $name } (ty)
stores-column-name = Nazwa
stores-column-role = Rola
stores-column-devices = { -device-cap-pl }
stores-column-last-seen = Ostatnia aktywność
stores-members-heading = Członkowie · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Niewykorzystane zaproszenia
stores-invite-another-device = { $name }, kolejne { -device }
stores-seen-never = Nigdy
stores-ago-just-now = Przed chwilą
stores-ago-today = Dzisiaj
stores-ago-yesterday = Wczoraj
stores-ago-days = { $days ->
    [one] { $days } dzień temu
    [few] { $days } dni temu
    [many] { $days } dni temu
   *[other] { $days } dni temu
 }
stores-ago-inline-just-now = przed chwilą
stores-ago-inline-today = dzisiaj
stores-ago-inline-yesterday = wczoraj
stores-ago-inline-days = { $days ->
    [one] { $days } dzień temu
    [few] { $days } dni temu
    [many] { $days } dni temu
   *[other] { $days } dni temu
 }
stores-expired = Wygasło
stores-expires-today = Wygasa dzisiaj
stores-expires-tomorrow = Wygasa jutro
stores-expires-days = { $days ->
    [one] Wygasa za { $days } dzień
    [few] Wygasa za { $days } dni
    [many] Wygasa za { $days } dni
   *[other] Wygasa za { $days } dni
 }
stores-open-heading = Każdy z linkiem może dołączyć · przez link dołączono: { $joined }
stores-closed-heading = Dołączono przez link przed zamknięciem · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
    [few] { $count } { -device-pl }
    [many] { $count } urządzeń
   *[other] { $count } urządzeń
 }
stores-open-link-row = Link { -store }u
stores-open-link-note = Pokaż i skopiuj
stores-anonymous-joined = Dołączono { $when }
stores-anonymous-seen = Widziano { $when }
stores-anonymous-never-seen = Jeszcze nie widziano
stores-remove-devices = { $count ->
    [one] Usuń je
    [few] Usuń wszystkie { $count }
    [many] Usuń wszystkie { $count }
   *[other] Usuń wszystkie { $count }
 }
stores-footer-you = To konto jest właścicielem. { -store-cap } zawsze zachowuje swojego właściciela.
stores-footer-remove-member = ↵ Odbierz dostęp
stores-footer-cancel-invite = ↵ Anuluj to zaproszenie
stores-footer-show-link = ↵ Pokaż link, aby go skopiować i udostępnić
stores-footer-remove-anonymous = ↵ Usuń to { -device }, znane tylko po czasie dołączenia
stores-footer-remove-all-anonymous = ↵ Usuń wszystkie { -device-pl }, które dołączyły przez link
stores-link-copied = Skopiowano do schowka, jeśli terminal na to pozwala. Można też zaznaczyć, aby skopiować.
stores-link-select = Zaznacz, aby skopiować.
stores-open-link-warning = Każdy z linkiem może dołączyć bez nazwy, ile razy chce.
stores-open-link-lasts = Działa do momentu, aż { -store } wróci do samych zaproszeń.
stores-invite-works-once = Działa jednorazowo. { $expiry }.
stores-invite-preview-safe = Otwarcie w przeglądarce pokazuje, jak dołączyć, i nie zużywa zaproszenia.
stores-confirm-join = Dołączyć do { $name }?
stores-confirm-leave = Opuścić { $name }?
stores-confirm-remove-member = Usunąć { $name }?
stores-confirm-cancel-invite = Anulować zaproszenie dla { $name }?
stores-confirm-unpublish = Przestać udostępniać { $name }?
stores-confirm-open = Pozwolić każdemu z linkiem dołączyć do { $name }?
stores-confirm-close = Zezwolić tylko zaproszonym osobom na dołączenie do { $name }?
stores-confirm-remove-device = Usunąć to { -device }?
stores-confirm-remove-devices = { $count ->
    [one] Usunąć { -device }, które dołączyło przez link?
    [few] Usunąć wszystkie { $count } { -device-pl }, które dołączyły przez link?
    [many] Usunąć wszystkie { $count } urządzeń, które dołączyły przez link?
   *[other] Usunąć wszystkie { $count } urządzeń, które dołączyły przez link?
 }
stores-confirm-fallback = Potwierdź
stores-label-address = Adres
stores-label-key = Klucz
stores-label-run-by = Prowadzony przez
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (zgodny z linkiem)
stores-key-matches-invite = { $fingerprint }  (zgodny z zaproszeniem)
stores-join-open = Każdy z linkiem może dołączyć.
stores-join-anonymous = Dołączenie następuje bez nazwy, a kolejne { -device } może dołączyć tym samym linkiem.
stores-join-already = Już dołączono. Ponowne dołączenie tworzy nowy token, a poprzedni przestaje działać.
stores-join-device-note = Właściciel sklepu widzi tę nazwę. Wpisz, aby ją zmienić.
stores-leave-plugins = { -plugin-cap-pl } zainstalowane z niego zostają na tym komputerze i przestają się aktualizować.
stores-leave-revoked = { -device-cap } nie ma już dostępu do { -store }u.
stores-remove-member-devices = { $count ->
    [one] { -device-cap } tej osoby przestaje natychmiast działać w tym { -store }ie.
    [few] Ich { $count } { -device-pl } przestają natychmiast działać w tym { -store }ie.
    [many] Ich { $count } urządzeń przestaje natychmiast działać w tym { -store }ie.
   *[other] Ich { $count } urządzeń przestaje natychmiast działać w tym { -store }ie.
 }
stores-remove-member-plugins = { -plugin-cap-pl } już zainstalowane zostają na ich komputerach.
stores-cancel-invite-link = Link przestaje działać.
stores-unpublish-members = Członkowie, którzy ją zainstalowali, zachowują ją, a { -plugin } przestaje się aktualizować.
stores-open-anyone = Każdy z linkiem do { -store }u może dołączyć bez nazwy.
stores-open-link-new = Link jest za każdym razem nowy, więc poprzedni przestaje działać.
stores-open-members-see = Członkowie widzą, że { -store } jest otwarty.
stores-close-link = Link przestaje działać i nikt nowy nie dołącza bez zaproszenia.
stores-close-devices = { -device-cap-pl }, które dołączyły przez link, działają dalej, dopóki nie zostaną usunięte w sekcji Członkowie.
stores-anonymous-one-stops = Przestaje działać w tym { -store }ie od razu.
stores-anonymous-all-stop = Przestają działać w tym { -store }ie od razu.
stores-anonymous-plugins = { -plugin-cap-pl } już zainstalowane zostają tam, gdzie są.
stores-anonymous-still-joins = Dopóki { -store } jest otwarty, link dołącza każdego.
stores-plugin-may-contact = Może łączyć się z: { $hosts }
stores-plugin-cannot = Nie może czytać plików, otwierać gniazd ani zaglądać do środowiska.
stores-plugin-update-adds = Aktualizacja dodaje { $hosts } do listy, z którą może się łączyć.
stores-plugin-added-by-hand-note = { $id }.wasm już jest w katalogu, dodany ręcznie.
stores-plugin-other-store-note = Zainstalowano z innego { -store }u, który ją aktualizuje.
stores-choice-install = Zainstaluj
stores-choice-update = Zaktualizuj do { $version }
stores-choice-replace = Zastąp kopię dodaną ręcznie
stores-choice-remove = Usuń z tego komputera
stores-choice-unpublish = Przestań udostępniać w tym { -store }ie
stores-choice-cancel = Anuluj
stores-publish-new = Nowa
stores-publish-published = Opublikowano
stores-publish-replaces = Zastępuje { $version }
stores-toast-invite-checked = Zaproszenie jest poprawne, ale tej osoby nie ma już na liście { -store }ów. Wklej je ponownie, aby dołączyć.
stores-toast-nothing-to-publish = Nie ma nic do opublikowania w katalogu. Uruchom „{ $command }”, aby zobaczyć, gdzie się znajduje.
stores-toast-that-is-you = To ta osoba. { -store-cap } zawsze zachowuje właściciela.
