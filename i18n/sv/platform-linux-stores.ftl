stores-device-count = { $count ->
    [one] { $count } { -device }
   *[other] { $count } { -device-pl }
 }
stores-dialog-title = { $title }, { $store }
stores-owner-unreachable = Det gick inte att nå { -plugin-store }
stores-owner-no-answer = { -plugin-store } svarade inte
stores-owner-no-answer-detail = Den kanske är upptagen, eller så är datorn den körs på avstängd.
stores-owner-needs-connection = { -plugin-store } behöver en anslutning
stores-owner-needs-connection-detail = Försök igen när den här { -device }en är online igen.
stores-owner-link-offline = För att skapa en länk behövs en anslutning till { -plugin-store }.
stores-send-link-until = Skicka länken. Den fungerar en gång, till { $until }.
stores-open-on-detail = De går med utan namn. Stänger du av det upphör länken. Slår du på det igen skapas en ny.
stores-open-off-detail = Bara personer du bjuder in kan gå med. När det är på kan vem som helst med länken gå med, utan namn.
stores-open-no-names = Inga namn sparas.
stores-open-keep-working = De fortsätter fungera tills du tar bort dem.
stores-device-seen = Sedd { $when }
stores-device-never-seen = Inte sedd sedan anslutningen
stores-publish-short = Publicera ett { -plugin }
stores-publish-none-hint = Ett { -plugin } du bygger eller installerar visas här.
stores-not-published = Inte i { -plugin-store } än
stores-store-offers = { -plugin-store } har { $version }
stores-published-current = Uppdaterad i { -plugin-store }
stores-published-state = Publicerad
stores-publish-label = Publicera { $name } { $version }
