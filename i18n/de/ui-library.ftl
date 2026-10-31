queue-episodes-left = { $count ->
    [one] Noch eine Folge in { $title }
   *[other] Noch { $count } Folgen in { $title }
 }
episodes-count = { $count ->
    [one] { $count } Folge
   *[other] { $count } Folgen
 }
progress-episodes = { $total ->
    [one] { $watched } von { $total } Folge
   *[other] { $watched } von { $total } Folgen
 }
duration-days-hours = { $days } T { $hours } h
group-in-progress = Laufend
group-new-this-week = Neu diese Woche
continue-empty-title = Noch nichts geschaut
continue-empty-detail = Wähle etwas aus Seasonal oder suche nach einem Titel.
continue-empty-action = Seasonal durchsuchen
continue-load-failed = { -continue-cap } konnte nicht geladen werden
continue-heading = { -continue-cap }
continue-airing-today = Heute erscheint
continue-this-season = Diese Saison
continue-hide = Von { -continue-cap } entfernen
continue-hidden = Aus { -continue-cap } entfernt.
continue-drop = In „Abgebrochen“ verschieben
continue-more-for = Weitere Optionen für { $title }
continue-row-resume = E{ $episode } fortsetzen · { $clock }
continue-row-resume-dubbed = E{ $episode }, { -dubbed }, fortsetzen · { $clock }
continue-row-start = E{ $episode } abspielen
continue-row-start-dubbed = E{ $episode }, { -dubbed }, abspielen
continue-row-start-aired = E{ $episode } abspielen · erschienen { $ago }
continue-row-caught-up = Auf dem neuesten Stand · E{ $episode } { $when }
continue-row-caught-up-plain = Auf dem neuesten Stand
continue-widget-connect = Verbinde AniList, um es hier zu sehen.
continue-caught-up = Du bist auf dem neuesten Stand.
title-button-resume = Folge { $episode } fortsetzen · { $time }
title-button-play = Folge { $episode } abspielen
title-button-add-source = Eine { -source } zum Abspielen hinzufügen
library-status-watching = Wird geschaut
library-status-planning = Geplant
library-status-completed = Abgeschlossen
library-status-paused = Pausiert
library-status-dropped = Abgebrochen
library-sort-updated = Zuletzt geändert
library-sort-title = Titel
library-sort-score = Wertung
library-sort-button = Sortierung: { $sort }
library-sort-label = Sortieren nach { $sort }
library-signed-out-title = Deine Listen kommen von AniList
library-signed-out-detail = Melde dich an, um abzugleichen, was du schaust und wo du aufgehört hast.
library-signed-out-action = Bei AniList anmelden
library-list-empty = Nichts in deiner Liste „{ $list }“
library-list-empty-detail = Titel, die du in dieser Liste bei AniList hinzufügst, erscheinen hier.
library-list-load-failed = Deine Liste „{ $list }“ konnte nicht geladen werden
library-loading = Deine Listen werden geladen
library-add-to-list = Zur Liste hinzufügen
library-add-planning = Zu Geplant hinzufügen
library-added-planning = Zu Geplant hinzugefügt.
library-on-your-list = Auf deiner Liste
library-in-your-list = In deiner Liste
progress-episodes-short = E{ $done } von { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = E{ $episode }, { -dubbed }, abspielen · erschienen { $ago }
up-next-heading = { -up-next-cap } für dich
up-next-starts = Startet am { $date }
up-next-reason-new-season = Neue Staffel
up-next-reason-planning = Auf deiner Liste „Geplant“
up-next-reason-recommended = Weil du { $title } beendet hast
up-next-reason-recommended-plain = Für dich empfohlen
up-next-because-detail = Bei AniList von Leuten empfohlen, denen { $title } gefallen hat
up-next-reason-airing-today = Erscheint heute
up-next-add-planning = Zu Geplant hinzufügen
up-next-not-interested = Kein Interesse
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } ist für dich aus „{ -up-next-cap }“ ausgeblendet.
up-next-hidden-until-airs = { $title } bleibt ausgeblendet, bis die Serie startet.
up-next-broadcast = Folge { $episode } · { $when }
up-next-out = erschienen { $ago }
