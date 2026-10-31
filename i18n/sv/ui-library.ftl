queue-episodes-left = { $count ->
    [one] Ett avsnitt kvar i { $title }
   *[other] { $count } avsnitt kvar i { $title }
 }
episodes-count = { $count ->
    [one] { $count } avsnitt
   *[other] { $count } avsnitt
 }
progress-episodes = { $total ->
    [one] { $watched } av { $total } avsnitt
   *[other] { $watched } av { $total } avsnitt
 }
duration-days-hours = { $days } d { $hours } h
group-in-progress = Pågår
group-new-this-week = Nytt den här veckan
continue-empty-title = Inget sett än
continue-empty-detail = Välj något under Säsong, eller sök efter en titel.
continue-empty-action = Bläddra i Säsong
continue-load-failed = Det gick inte att läsa { -continue-cap }
continue-heading = Serier du tittar på
continue-airing-today = Sänds idag
continue-this-season = Den här säsongen
continue-hide = Dölj från { -continue-cap }
continue-hidden = Dold från { -continue-cap }.
continue-drop = Markera som avbruten
continue-more-for = Mer för { $title }
continue-row-resume = Fortsätt på E{ $episode } · { $clock }
continue-row-resume-dubbed = Fortsätt på E{ $episode } { -dubbed } · { $clock }
continue-row-start = Spela E{ $episode }
continue-row-start-dubbed = Spela E{ $episode } { -dubbed }
continue-row-start-aired = Spela E{ $episode } · släpptes { $ago }
continue-row-caught-up = Ikapp · E{ $episode } { $when }
continue-row-caught-up-plain = Ikapp
continue-widget-connect = Koppla AniList för att se det här.
continue-caught-up = Du är ikapp.
title-button-resume = Fortsätt E{ $episode } · { $time }
title-button-play = Spela avsnitt { $episode }
title-button-add-source = Lägg till en { -source } att spela från
library-status-watching = Tittar nu
library-status-planning = Planerar
library-status-completed = Klar
library-status-paused = Pausad
library-status-dropped = Avbruten
library-sort-updated = Senast uppdaterad
library-sort-title = Titel
library-sort-score = Betyg
library-sort-button = Sortera: { $sort }
library-sort-label = Sortera efter { $sort }
library-signed-out-title = Dina listor kommer från AniList
library-signed-out-detail = Logga in för att synka vad du tittar på och var du slutade.
library-signed-out-action = Logga in på AniList
library-list-empty = Listan { $list } är tom
library-list-empty-detail = Titlar du lägger till i den här listan på AniList visas här.
library-list-load-failed = Det gick inte att läsa in listan { $list }
library-loading = Läser in dina listor
library-add-to-list = Lägg till i lista
library-add-planning = Lägg till i planeringslistan
library-added-planning = Lade till i planeringslistan.
library-on-your-list = På din lista
library-in-your-list = I din lista
progress-episodes-short = E{ $done } av { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Spela E{ $episode } { -dubbed } · släpptes { $ago }
up-next-heading = { -up-next-cap } för dig
up-next-starts = Börjar { $date }
up-next-reason-new-season = Ny säsong
up-next-reason-planning = På din planeringslista
up-next-reason-recommended = Eftersom du slutade { $title }
up-next-reason-recommended-plain = Rekommenderas för dig
up-next-because-detail = Rekommenderas på AniList av personer som gillade { $title }
up-next-reason-airing-today = Sänds idag
up-next-add-planning = Lägg till i planeringslistan
up-next-not-interested = Inte intresserad
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } är dold från { -up-next-cap } för dig
up-next-hidden-until-airs = { $title } är dold tills den börjar sändas
up-next-broadcast = Avsnitt { $episode } · { $when }
up-next-out = släpptes { $ago }
