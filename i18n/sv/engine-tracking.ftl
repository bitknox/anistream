notice-signed-in = Inloggad på { $tracker }.
notice-signed-in-keychain = Inloggad på { $tracker }. Token sparades i systemets nyckelring.
notice-signed-in-file = Inloggad på { $tracker }. Ingen nyckelring finns, så token ligger i en privat fil på den här { -device }en.
notice-signed-in-unsaved = Inloggad på { $tracker }, men token sparades inte, så du måste logga in på nytt senare. { $reason }
notice-token-not-saved = Det gick inte att spara inloggningstoken. Försök logga in igen. { $reason }
notice-sign-in-failed = Det gick inte att logga in på { $tracker }. Försök igen. { $reason }
notice-sign-in-stopped = Inloggningen på { $tracker } avbröts innan den var klar. Försök igen. { $reason }
notice-token-exchange-failed = Det gick inte att få en token från { $tracker }. Försök logga in igen. { $reason }
notice-browser-not-opened = Det gick inte att öppna en webbläsare för inloggningen. Kontrollera att en är installerad och försök igen. { $reason }
notice-sign-in-not-waiting = Ingen inloggning till { $tracker } väntar på det. Starta inloggningen igen.
notice-sign-in-wrong-request = Inloggningen till { $tracker } kom inte tillbaka från den här begäran. Starta inloggningen igen.
notice-sign-in-no-token = Inloggningen till { $tracker } kom tillbaka utan token. Försök igen.
notice-mal-client-missing-build = Det gick inte att slutföra inloggningen till MyAnimeList: den här versionen har ingen klient för det.
notice-kitsu-signs-in-on-accounts = Kitsu loggar in från skärmen Konton.
notice-sign-in-unsupported = Inloggning till { $tracker } stöds inte än.
notice-sign-in-needs-redirect = Inloggning till { $tracker } kräver en omdirigering i webbläsaren, och den här appen kan inte ta emot den.
notice-anilist-implicit-flow = Den här AniList-inloggningsmetoden fungerar bara i appen { -app-name }. För att logga in här sätter du { $setting } till "loopback" i konfigurationsfilen.
notice-anilist-client-missing = AniList behöver ett klient-id och en hemlighet. Ange { $client } och { $secret } i konfigurationsfilen och logga sedan in igen. För att få inloggningsadressen kör du { -app-name } --login-url.
notice-anilist-paste-flow = Det går inte att klistra in en kod här. Avsluta och kör { -app-name } --login i stället.
notice-mal-client-missing = MyAnimeList behöver ett klient-id. Registrera en app med typen "other" på myanimelist.net/apiconfig och sätt sedan { $setting } i konfigurationsfilen.
status-waiting-for-redirect = Väntar på att { $tracker } ska skicka tillbaka…
notice-mal-no-refresh-token = MyAnimeList utfärdade ingen uppdateringstoken. Du behöver logga in igen om ungefär en månad.
notice-client-id-missing = Sätt { $setting } i konfigurationsfilen först.
notice-signed-out = Utloggad från { $tracker }.
notice-signed-out-queue = { $count ->
    [one] Utloggad från { $tracker }. En köad ändring sparas till den kommande inloggningen.
   *[other] Utloggad från { $tracker }. { $count } köade ändringar sparas till den kommande inloggningen.
 }
notice-sign-out-failed = Det gick inte att logga ut från { $tracker }. Försök igen. { $reason }
notice-not-saved-because = Det gick inte att spara inställningen. { $cause }
notice-protection-saved = Skyddet för torrenter sparades.
notice-protection-not-checked = Det gick inte att kontrollera skyddet för torrenter. { $cause }
notice-proxy-password-not-kept = Det gick inte att spara proxyns lösenord. Ange det igen för att försöka spara det. { $reason }
notice-show-setting-refused = Det gick inte att spara inställningen. { $value } är inte ett värde för { $setting }.
notice-show-not-reset = Det gick inte att återställa den här seriens inställningar. { $reason }
notice-show-no-such-subtitles = Det gick inte att spara det undertextvalet. Den här serien har inte använt de undertexterna förut.
notice-torrents-waiting-for-check = Torrenterna väntar på att anslutningskontrollen godkänns igen.
notice-mapping-data-refreshed = Mappningsdatan uppdaterades.
notice-mapping-data-failed = Det gick inte att uppdatera mappningsdatan ({ $datasets }). Försöker igen om { $wait }. { $cause }
notice-episode-aired = { $title }, avsnitt { $episode }, sändes medan du var borta.
notice-episodes-aired = { $count ->
    [one] Ett nytt avsnitt av en serie du följer sändes. Kalendern visar det.
   *[other] { $count } nya avsnitt av serier du följer sändes. Kalendern visar dem.
 }
notice-update-available = { $tag } är ute. Kör { -app-name } --update för att installera den.
cause-confirm-no-protection = Bekräfta först att du vill ha inget skydd.
cause-confirm-warn-only = Bekräfta först att du bara vill få en varning.
cause-proxy-needs-address = En proxy behöver en värd och en port.
cause-proxy-address-unusable = Proxyns adress kan inte användas för en kontroll.
cause-server-unreachable = Det gick inte att nå servern.
