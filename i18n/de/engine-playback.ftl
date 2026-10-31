notice-reason = { $reason }
cause-no-sources = Keine { -source-pl } eingerichtet. Füge eine im Bereich { -source-cap-pl } hinzu.
notice-not-aired = Folge { $episode } wird { $when } ausgestrahlt.
notice-not-aired-yet = Folge { $episode } ist noch nicht ausgestrahlt. Folge { $next } wird { $when } ausgestrahlt.
cause-airs-today = heute um { $time }
cause-airs-tomorrow = morgen um { $time }
cause-airs-in-days = { $days ->
    [one] in einem Tag um { $time }
   *[other] in { $days } Tagen um { $time }
 }
cause-airs-on-date = am { $date } um { $time }
cause-no-playable-stream = Die { -source } hat geantwortet, aber nichts zum Abspielen geliefert. Probiere eine andere aus.
cause-title-no-match = Dieser Titel konnte nicht zugeordnet werden: { $detail }
cause-source-gone = Die { -source } { $source } ist nicht mehr verfügbar. Wähle eine andere aus.
cause-release-failed = Dieses { -release } konnte nicht gestartet werden: { $detail }
cause-sources-timed-out = Die { -source-pl } haben zu lange nicht geantwortet. Versuche es erneut oder wähle eine andere { -source }.
cause-no-source-has-episode = Keine { -source } hat Folge { $episode }: { $detail }
cause-source-cannot-download = Diese { -source } bietet Folgen nur zum Streamen an, nicht zum Herunterladen.
cause-no-source-could-list-show = Keine { -source } konnte diese Serie auflisten. Sie kann deshalb nicht heruntergeladen werden.
cause-subtitles-merged = { $tracks ->
    [one] Eine Untertitelspur wurde eingefügt.
   *[other] { $tracks } Untertitelspuren wurden eingefügt.
 }
cause-subtitles-left-apart = Die Untertitel bleiben als separate Dateien. { $reason }
cause-already-in-downloads = { $count ->
    [one] Eine Folge ist bereits in den Downloads.
   *[other] { $count } Folgen sind bereits in den Downloads.
 }
status-looking-up-title = Titel wird gesucht…
status-loading-sources = { -source-pl } werden geladen…
status-searching-sources = { -source-pl } werden nach Folge { $episode } durchsucht…
notice-player-failed-to-start = mpv konnte nicht gestartet werden. Prüfe, ob es allein läuft. ({ $reason })
notice-stream-no-video = Kein Bild aus diesem Stream. Versuche { $next } von { $total }.
notice-player-could-not-play = mpv konnte das nicht abspielen. { $reason }
notice-player-exited-silently = mpv wurde beendet, ohne etwas abzuspielen und ohne Grund. Probiere eine andere { -source }.
notice-player-not-playing = mpv hat nach { $seconds } s noch nicht mit der Wiedergabe begonnen. Die { -source } hat vielleicht keine Daten. Prüfe, ob mpv allein eine Datei abspielt.
notice-no-player = Diese Version von { -app-name } kann keine Videos abspielen.
notice-mpv-missing = { $binary } wurde nicht gefunden. Installiere mpv, um Folgen abzuspielen.
notice-mpv-old = mpv { $version } kann abspielen, aber der Player von { -app-name } braucht für die Überspringen-Taste, die Karte „{ -up-next-cap }“, Menüs und Vorschaubilder mpv 0.36 oder neuer.
notice-subtitles-unavailable = Die Untertitel dieser { -source } konnten nicht geladen werden. Wähle eine andere { -source }, um es erneut zu versuchen.
notice-opened-in-browser = Im Browser geöffnet.
notice-could-not-open = Konnte das nicht öffnen. { $reason }
notice-trailer-playing = Trailer läuft in mpv.
notice-trailer-opened = Trailer im Browser geöffnet.
notice-trailer-failed = Der Trailer konnte nicht geöffnet werden. { $reason }
notice-filler-skipped = Füllfolgen übersprungen. { -up-next-cap } ist Folge { $episode }.
notice-audio-now-subbed = Ton ist jetzt { -subbed }.
notice-audio-now-dubbed = Ton ist jetzt { -dubbed }.
notice-already-first-episode = Schon bei der ersten Folge.
notice-last-episode = Es gibt keine nächste Folge. Folge { $episode } ist die letzte.
notice-no-next-episode = Es gibt keine nächste Folge.
notice-no-previous-episode = Es gibt keine vorherige Folge.
notice-match-reset-failed = Die Zuordnung konnte nicht zurückgesetzt werden. { $reason }
notice-match-save-failed = Diese Zuordnung konnte nicht gespeichert werden. { $reason }
notice-match-did-not-stick = Diese Zuordnung hat nicht gehalten. Vielleicht hat sich die { -source } geändert. Wähle sie erneut aus.
notice-source-search-empty = Nichts gefunden für „{ $query }“. Versuche eine andere Suche.
notice-source-pinned = { $source } wird für diesen Titel verwendet.
notice-source-automatic = Zurück zur automatischen Auswahl der { -source }.
notice-source-choice-failed = Diese Auswahl konnte nicht gespeichert werden. { $reason }
notice-no-source-could-list = Keine { -source } konnte diese Serie auflisten: { $reason }
notice-episodes-failed = Die Folgen konnten nicht geladen werden. { $reason }
notice-match-uncertain = { $source } hat mehr als eine Serie gefunden, die diese sein könnte. Deshalb stammen die Folgen vorerst von AniList.
notice-continue-hide-failed = Konnte nicht aus { -continue-cap } entfernt werden. { $reason }
notice-download-queued = Folge { $episode } von { $title } zur Warteschlange hinzugefügt.
notice-download-already-queued = Folge { $episode } von { $title } ist bereits in den Downloads.
notice-download-starting-now = Folge { $episode } von { $title } wird jetzt gestartet.
notice-download-cancelled = Download abgebrochen.
notice-download-file-deleted = Download und Datei gelöscht.
notice-download-removed-file-gone = Download entfernt. Die Datei war schon weg.
notice-download-removed-file-kept = Der Download wurde entfernt, aber die Datei konnte nicht gelöscht werden. { $reason }
notice-download-removed = Download entfernt.
notice-downloads-cleared = { $count ->
    [one] Ein abgeschlossener Download wurde entfernt.
   *[other] { $count } abgeschlossene Downloads wurden entfernt.
 }
notice-download-finished = Folge { $episode } von { $title } heruntergeladen.
notice-download-finished-note = Folge { $episode } von { $title } heruntergeladen. { $note }
notice-download-failed = Folge { $episode } von { $title } konnte nicht heruntergeladen werden. { $reason }
notice-downloads-none-queued = Nichts in die Warteschlange gestellt.
notice-downloads-none-queued-stopped = Nichts in die Warteschlange gestellt. { $reason }
notice-downloads-none-new = Nichts Neues in die Warteschlange gestellt. { $existing }
notice-downloads-none-new-stopped = Nichts Neues in die Warteschlange gestellt. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] Eine Folge in die Warteschlange gestellt. { $existing }
   *[other] { $count } Folgen in die Warteschlange gestellt. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] Eine Folge in die Warteschlange gestellt, dann wurde angehalten. { $reason }
   *[other] { $count } Folgen in die Warteschlange gestellt, dann wurde angehalten. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] Eine Folge in die Warteschlange gestellt, dann wurde angehalten. { $existing } { $reason }
   *[other] { $count } Folgen in die Warteschlange gestellt, dann wurde angehalten. { $existing } { $reason }
 }
notice-keep-next-full = Nicht genug freier Speicher, um die nächsten Folgen herunterzuladen. Gib Speicherplatz frei, um mehr herunterzuladen.
party-needs-server = Wähle zuerst einen Server für die { -watch-party } als syncplay.server. { $server } wird von { $operator } betrieben.
party-choose-server = Wähle zuerst einen Server für die { -watch-party }. { $server } wird von { $operator } betrieben.
notice-no-party = { -watch-party-cap-pl } sind in dieser Version von { -app-name } nicht verfügbar.
party-needs-room = Eine { -watch-party } braucht einen Raum zum Treffen. Wähle zuerst einen aus.
party-server-lost = Verbindung zum Server der { -watch-party } verloren. Neuer Versuch läuft.
party-rejoined = Wieder in der { -watch-party }.
party-left = Du hast die { -watch-party } verlassen.
