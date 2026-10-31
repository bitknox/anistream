notice-reason = { $reason }
cause-no-sources = Inga { -source-pl } är inställda. Lägg till en på skärmen { -source-cap-pl }.
notice-not-aired = Avsnitt { $episode } sänds { $when }.
notice-not-aired-yet = Avsnitt { $episode } har inte sänts än. Avsnitt { $next } sänds { $when }.
cause-airs-today = idag kl. { $time }
cause-airs-tomorrow = imorgon kl. { $time }
cause-airs-in-days = { $days ->
    [one] om en dag kl. { $time }
   *[other] om { $days } dagar kl. { $time }
 }
cause-airs-on-date = den { $date } kl. { $time }
cause-no-playable-stream = { -source } svarade, men det fanns inget att spela. Prova en annan.
cause-title-no-match = Det gick inte att matcha den här titeln: { $detail }
cause-source-gone = { -source } { $source } är inte längre tillgänglig. Välj en annan.
cause-release-failed = Det gick inte att starta vald { -release }: { $detail }
cause-sources-timed-out = { -source-pl } svarade för sent. Försök igen eller välj en annan { -source }.
cause-no-source-has-episode = Ingen { -source } har avsnitt { $episode }: { $detail }
cause-source-cannot-download = Den här { -source }n strömmar avsnitt men kan inte ladda ner dem.
cause-no-source-could-list-show = Ingen { -source } kunde lista den här serien, så den kan inte laddas ner.
cause-subtitles-merged = { $tracks ->
    [one] Ett undertextspår lades in i videon.
   *[other] { $tracks } undertextspår lades in i videon.
 }
cause-subtitles-left-apart = Undertexterna ligger kvar som separata filer. { $reason }
cause-already-in-downloads = { $count ->
    [one] Ett avsnitt finns redan i nedladdningarna.
   *[other] { $count } avsnitt finns redan i nedladdningarna.
 }
status-looking-up-title = Slår upp titeln…
status-loading-sources = Läser in { -source-pl }…
status-searching-sources = Söker i { -source-pl } efter avsnitt { $episode }…
notice-player-failed-to-start = Det gick inte att starta mpv. Kontrollera att den startar på egen hand. ({ $reason })
notice-stream-no-video = Ingen bild från strömmen. Provar { $next } av { $total }.
notice-player-could-not-play = mpv kunde inte spela upp det här. { $reason }
notice-player-exited-silently = mpv stängdes utan att spela något och utan att säga varför. Prova en annan { -source }.
notice-player-not-playing = mpv har inte börjat spela efter { $seconds } s. Den här { -source }n kan sakna data. Kontrollera att mpv spelar upp en fil på egen hand.
notice-no-player = Den här versionen av { -app-name } kan inte spela video.
notice-mpv-missing = { $binary } hittades inte. Installera mpv för att spela avsnitt.
notice-mpv-old = mpv { $version } kan spela, men { -app-name }s spelare kräver mpv 0.36 eller nyare för hoppknappen, kortet { -up-next-cap }, menyerna och miniatyrerna.
notice-subtitles-unavailable = Undertexterna för den här { -source }n kunde inte läsas in. Välj en annan { -source } för att försöka igen.
notice-opened-in-browser = Öppnades i webbläsaren.
notice-could-not-open = Det gick inte att öppna det. { $reason }
notice-trailer-playing = Trailern spelas i mpv.
notice-trailer-opened = Trailern öppnades i webbläsaren.
notice-trailer-failed = Det gick inte att öppna trailern. { $reason }
notice-filler-skipped = Fyllnadsavsnitt hoppades över. { -up-next-cap } är avsnitt { $episode }.
notice-audio-now-subbed = Ljudet är nu { -subbed }.
notice-audio-now-dubbed = Ljudet är nu { -dubbed }.
notice-already-first-episode = Du är redan på första avsnittet.
notice-last-episode = Det finns inget avsnitt efter det här. Avsnitt { $episode } är det sista.
notice-no-next-episode = Det finns inget avsnitt efter det här.
notice-no-previous-episode = Det finns inget föregående avsnitt.
notice-match-reset-failed = Det gick inte att återställa matchningen. { $reason }
notice-match-save-failed = Det gick inte att spara matchningen. { $reason }
notice-match-did-not-stick = Matchningen höll inte. Den { -source } kan ha ändrats. Försök välja den igen.
notice-source-search-empty = Inget hittades för “{ $query }”. Prova en annan sökning.
notice-source-pinned = { $source } används för den här titeln.
notice-source-automatic = Tillbaka till automatiskt val av { -source }.
notice-source-choice-failed = Det gick inte att spara valet. { $reason }
notice-no-source-could-list = Ingen { -source } kunde lista den här serien: { $reason }
notice-episodes-failed = Det gick inte att läsa in avsnitten. { $reason }
notice-match-uncertain = { $source } hittade fler än en serie som kan vara den här, så avsnitten kommer från AniList tills vidare.
notice-continue-hide-failed = Det gick inte att ta bort titeln från { -continue-cap }. { $reason }
notice-download-queued = Avsnitt { $episode } av { $title } lades i kön.
notice-download-already-queued = Avsnitt { $episode } av { $title } finns redan i nedladdningarna.
notice-download-starting-now = Startar avsnitt { $episode } av { $title } nu.
notice-download-cancelled = Nedladdningen avbröts.
notice-download-file-deleted = Nedladdningen och filen togs bort.
notice-download-removed-file-gone = Nedladdningen togs bort. Filen fanns redan inte längre.
notice-download-removed-file-kept = Nedladdningen togs bort, men filen gick inte att radera. { $reason }
notice-download-removed = Nedladdningen togs bort.
notice-downloads-cleared = { $count ->
    [one] Rensade en avslutad nedladdning.
   *[other] Rensade { $count } avslutade nedladdningar.
 }
notice-download-finished = Laddade ner avsnitt { $episode } av { $title }.
notice-download-finished-note = Laddade ner avsnitt { $episode } av { $title }. { $note }
notice-download-failed = Det gick inte att ladda ner avsnitt { $episode } av { $title }. { $reason }
notice-downloads-none-queued = Ingenting lades i kön.
notice-downloads-none-queued-stopped = Ingenting lades i kön. { $reason }
notice-downloads-none-new = Inget nytt lades i kön. { $existing }
notice-downloads-none-new-stopped = Inget nytt lades i kön. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] Ett avsnitt lades i kön. { $existing }
   *[other] { $count } avsnitt lades i kön. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] Ett avsnitt lades i kön, sedan stoppade det. { $reason }
   *[other] { $count } avsnitt lades i kön, sedan stoppade det. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] Ett avsnitt lades i kön, sedan stoppade det. { $existing } { $reason }
   *[other] { $count } avsnitt lades i kön, sedan stoppade det. { $existing } { $reason }
 }
notice-keep-next-full = Inte tillräckligt med ledigt utrymme för att hålla kommande avsnitt nedladdade. Frigör lite utrymme för att ladda ner fler.
party-needs-server = Välj först en server för { -watch-party }, som syncplay.server. { $server } drivs av { $operator }.
party-choose-server = Välj först en server för { -watch-party }. { $server } drivs av { $operator }.
notice-no-party = { -watch-party-cap-pl } finns inte i den här versionen av { -app-name }.
party-needs-room = En { -watch-party } behöver ett rum att träffas i. Välj ett först.
party-server-lost = Servern för { -watch-party } tappades. Försöker igen.
party-rejoined = Tillbaka i { -watch-party }.
party-left = Lämnade { -watch-party }.
