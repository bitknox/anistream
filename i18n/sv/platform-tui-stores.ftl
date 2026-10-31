stores-needs-larger-terminal = Terminalen är för liten
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Medlemmar
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = Du
stores-column-state = Status
stores-none-joined = Inga anslutna än
stores-role-owner = Ägare
stores-role-member = Medlem
stores-join-row = + Anslut till en { -store }
stores-state-revoked = Åtkomst borttagen
stores-state-offline = Offline
stores-state-up-to-date = Uppdaterad
stores-state-updates = { $count ->
    [one] { $count } uppdatering
   *[other] { $count } uppdateringar
 }
stores-footer-revoked = Den här { -store }n tar inte längre emot den här { -device }n. Ägaren kan skicka en ny inbjudan.
stores-footer-offline = { $url } ({ $reason })
stores-key = Nyckel { $fingerprint }
stores-joined-without-name = Ansluten utan namn
stores-joined-as = Ansluten som { $name }
stores-footer-join = Klistra in inbjudan eller länken till { -store }n du fick
stores-owner-tag = Ägare
stores-anyone-can-join = Alla med länken kan ansluta
stores-note-offline = Offline. Visar den senaste listan ({ $reason })
stores-note-revoked = Den här { -store }n tar inte längre emot den här { -device }n. Det som är installerat fortsätter fungera.
stores-column-plugin = { -plugin-cap }
stores-column-version = Version
stores-empty-owner = Inget publicerat än. Publicera ett { -plugin } nedan.
stores-empty-member = Inget publicerat än
stores-line-link-device = Länka en annan { -device }
stores-line-invite = Bjud in någon
stores-line-members = Medlemmar
stores-line-who-can-join = Vem som kan ansluta
stores-line-publish = Publicera ett { -plugin }
stores-line-admin = Öppna adminsidan
stores-line-leave = Lämna { -store }n
stores-plugin-not-installed = Inte installerat
stores-plugin-installed = Installerat
stores-plugin-update-from = Uppdatering från { $from }
stores-plugin-added-by-hand = Tillagt manuellt
stores-plugin-from-another-store = Från en annan { -store }
stores-footer-update-hosts = Uppdateringen vill också kontakta { $hosts }
stores-footer-may-contact = Får kontakta { $hosts }
stores-footer-link-device = En engångslänk som ansluter en annan av dina datorer under ditt namn
stores-footer-invite = En engångslänk för någon ny
stores-footer-members = Vilka som har anslutit och hur du tar bort åtkomst
stores-footer-close-join = ↵ Släpp bara in personer du bjuder in och stäng länken
stores-footer-open-join = ↵ Låt vem som helst med länken ansluta utan namn
stores-footer-publish = Erbjud ett { -plugin } från tilläggsmappen på den här enheten
stores-footer-admin = Hantera medlemmar, inbjudningar och { -plugin-pl } i webbläsaren
stores-footer-leave = { -plugin-cap-pl } som installerats härifrån finns kvar på den här enheten och uppdateras inte längre.
stores-who-open = Alla med länken
stores-who-invite-only = Bara personer du bjuder in
stores-label-offers = Erbjuder
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
   *[other] { $count } { -plugin-pl }
 }
stores-offered-plugin = { $name } { $version } får kontakta { $hosts }
stores-offered-installed = Installerat
stores-offered-installed-version = { $from } installerat
stores-offered-added-by-hand = Tillagt manuellt
stores-offered-from-other = Från { $other }
stores-offered-more = och { $count } till
stores-hosts-none = ingenting
stores-loading = Läser in…
stores-member-you = { $name } (du)
stores-column-name = Namn
stores-column-role = Roll
stores-column-devices = { -device-cap-pl }
stores-column-last-seen = Senast sedd
stores-members-heading = Medlemmar · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Ej använda inbjudningar
stores-invite-another-device = { $name }, en annan { -device }
stores-seen-never = Aldrig
stores-ago-just-now = Just nu
stores-ago-today = Idag
stores-ago-yesterday = Igår
stores-ago-days = { $days ->
    [one] För { $days } dag sedan
   *[other] För { $days } dagar sedan
 }
stores-ago-inline-just-now = just nu
stores-ago-inline-today = idag
stores-ago-inline-yesterday = igår
stores-ago-inline-days = { $days ->
    [one] för { $days } dag sedan
   *[other] för { $days } dagar sedan
 }
stores-expired = Utgången
stores-expires-today = Går ut idag
stores-expires-tomorrow = Går ut imorgon
stores-expires-days = { $days ->
    [one] Går ut om { $days } dag
   *[other] Går ut om { $days } dagar
 }
stores-open-heading = Alla med länken kan ansluta · { $joined } anslutna med den
stores-closed-heading = Anslutna med länken innan den stängdes · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
   *[other] { $count } { -device-pl }
 }
stores-open-link-row = Länken till { -store }n
stores-open-link-note = Visa och kopiera
stores-anonymous-joined = Ansluten { $when }
stores-anonymous-seen = Sedd { $when }
stores-anonymous-never-seen = Inte sedd än
stores-remove-devices = { $count ->
    [one] Ta bort den
   *[other] Ta bort alla { $count }
 }
stores-footer-you = Du är ägaren. En { -store } behåller alltid sin ägare.
stores-footer-remove-member = ↵ Ta bort deras åtkomst
stores-footer-cancel-invite = ↵ Avbryt den här inbjudan
stores-footer-show-link = ↵ Visa länken, för att kopiera och dela
stores-footer-remove-anonymous = ↵ Ta bort den här { -device }n, känd endast via när den anslöt
stores-footer-remove-all-anonymous = ↵ Ta bort alla { -device-pl } som anslöt med länken
stores-link-copied = Kopierad till urklipp där terminalen tillåter det, annars markera den för att kopiera.
stores-link-select = Markera den för att kopiera.
stores-open-link-warning = Alla med länken kan ansluta utan namn, så många gånger de vill.
stores-open-link-lasts = Den fungerar tills { -store }n återgår till endast inbjudningar.
stores-invite-works-once = Fungerar en gång. { $expiry }.
stores-invite-preview-safe = Att öppna den i en webbläsare visar hur man ansluter utan att använda upp den.
stores-confirm-join = Anslut till { $name }?
stores-confirm-leave = Lämna { $name }?
stores-confirm-remove-member = Ta bort { $name }?
stores-confirm-cancel-invite = Avbryt inbjudan för { $name }?
stores-confirm-unpublish = Sluta erbjuda { $name }?
stores-confirm-open = Låt alla med länken ansluta till { $name }?
stores-confirm-close = Släpp bara in personer du bjuder in till { $name }?
stores-confirm-remove-device = Ta bort den här { -device }n?
stores-confirm-remove-devices = { $count ->
    [one] Ta bort { -device }n som anslöt med länken?
   *[other] Ta bort alla { $count } { -device-pl } som anslöt med länken?
 }
stores-confirm-fallback = Bekräfta
stores-label-address = Adress
stores-label-key = Nyckel
stores-label-run-by = Drivs av
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (stämmer med länken)
stores-key-matches-invite = { $fingerprint }  (stämmer med inbjudan)
stores-join-open = Alla med länken kan ansluta.
stores-join-anonymous = Du ansluter utan namn, och en annan { -device } ansluter med samma länk.
stores-join-already = Redan ansluten. Om du ansluter igen får den här { -device }n en ny token.
stores-join-device-note = Ägaren till { -store }n ser det här namnet. Skriv för att ändra det.
stores-leave-plugins = { -plugin-cap-pl } som installerats härifrån finns kvar på den här enheten och uppdateras inte längre.
stores-leave-revoked = Åtkomsten för den här { -device }n till { -store }n är återkallad.
stores-remove-member-devices = { $count ->
    [one] Deras { -device } slutar fungera med den här { -store }n direkt.
   *[other] Deras { $count } { -device-pl } slutar fungera med den här { -store }n direkt.
 }
stores-remove-member-plugins = { -plugin-cap-pl } som de redan installerat finns kvar på deras { -device-pl }.
stores-cancel-invite-link = Länken slutar fungera.
stores-unpublish-members = Medlemmar som installerat den behåller den, och den uppdateras inte längre.
stores-open-anyone = Alla med länken till { -store }n kan ansluta utan namn.
stores-open-link-new = Länken är ny varje gång, så en tidigare slutar fungera.
stores-open-members-see = Medlemmarna ser att { -store }n är öppen.
stores-close-link = Länken slutar fungera, och ingen ny ansluter utan inbjudan.
stores-close-devices = { -device-cap-pl } som anslöt med den fortsätter fungera tills du tar bort dem under medlemmar.
stores-anonymous-one-stops = Den slutar fungera med den här { -store }n direkt.
stores-anonymous-all-stop = De slutar fungera med den här { -store }n direkt.
stores-anonymous-plugins = { -plugin-cap-pl } som redan är installerade ligger kvar där de är.
stores-anonymous-still-joins = Medan { -store }n är öppen ansluter länken vem som helst.
stores-plugin-may-contact = Får kontakta { $hosts }
stores-plugin-cannot = Den kan inte läsa filer, öppna nätverksanslutningar eller se din miljö.
stores-plugin-update-adds = Uppdateringen lägger till { $hosts } bland det som den får kontakta.
stores-plugin-added-by-hand-note = { $id }.wasm finns redan i tilläggsmappen, tillagd manuellt.
stores-plugin-other-store-note = Installerat från en annan { -store }, som håller den uppdaterad.
stores-choice-install = Installera
stores-choice-update = Uppdatera till { $version }
stores-choice-replace = Ersätt kopian som lades till manuellt
stores-choice-remove = Ta bort från den här enheten
stores-choice-unpublish = Sluta erbjuda det i den här { -store }n
stores-choice-cancel = Avbryt
stores-publish-new = Ny
stores-publish-published = Publicerad
stores-publish-replaces = Ersätter { $version }
stores-toast-invite-checked = Inbjudan är giltig, men något annat är öppet. Klistra in den igen för att ansluta.
stores-toast-nothing-to-publish = Det finns inga { -plugin-pl } i tilläggsmappen att publicera. Kör "{ $command }" för att se var den ligger.
stores-toast-that-is-you = Det är du. En { -store } behåller alltid sin ägare.
