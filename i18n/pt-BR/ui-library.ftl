queue-episodes-left = { $count ->
    [one] Falta um episódio em { $title }
    [many] Faltam { $count } episódios em { $title }
   *[other] Faltam { $count } episódios em { $title }
 }
episodes-count = { $count ->
    [one] { $count } episódio
    [many] { $count } episódios
   *[other] { $count } episódios
 }
progress-episodes = { $total ->
    [one] { $watched } de { $total } episódio
    [many] { $watched } de { $total } episódios
   *[other] { $watched } de { $total } episódios
 }
duration-days-hours = { $days } d { $hours } h
group-in-progress = Em andamento
group-new-this-week = Novos nesta semana
continue-empty-title = Nada assistido ainda
continue-empty-detail = Escolha algo na Temporada ou procure um título.
continue-empty-action = Ver a Temporada
continue-load-failed = Não foi possível carregar { -continue-cap }
continue-heading = { -continue-cap }
continue-airing-today = Vai ao ar hoje
continue-this-season = Nesta temporada
continue-hide = Ocultar de { -continue-cap }
continue-hidden = Ocultado de { -continue-cap }.
continue-drop = Mover para Abandonado
continue-more-for = Mais opções para { $title }
continue-row-resume = Retomar E{ $episode } · { $clock }
continue-row-resume-dubbed = Retomar E{ $episode } { -dubbed } · { $clock }
continue-row-start = Reproduzir E{ $episode }
continue-row-start-dubbed = Reproduzir E{ $episode } { -dubbed }
continue-row-start-aired = Reproduzir E{ $episode } · saiu { $ago }
continue-row-caught-up = Em dia · E{ $episode } { $when }
continue-row-caught-up-plain = Em dia
continue-widget-connect = Conecte o AniList para vê-lo aqui.
continue-caught-up = Você está em dia.
title-button-resume = Retomar episódio { $episode } · { $time }
title-button-play = Reproduzir episódio { $episode }
title-button-add-source = Adicione uma { -source } para reproduzir
library-status-watching = Assistindo
library-status-planning = Planejado
library-status-completed = Concluído
library-status-paused = Pausado
library-status-dropped = Abandonado
library-sort-updated = Última atualização
library-sort-title = Título
library-sort-score = Nota
library-sort-button = Ordem: { $sort }
library-sort-label = Ordenar por { $sort }
library-signed-out-title = Suas listas vêm do AniList
library-signed-out-detail = Entre para sincronizar o que você assiste e onde parou.
library-signed-out-action = Entrar no AniList
library-list-empty = Nada na sua lista de { $list }
library-list-empty-detail = Os títulos que você adicionar a esta lista no AniList aparecem aqui.
library-list-load-failed = Não foi possível carregar sua lista de { $list }
library-loading = Carregando suas listas
library-add-to-list = Adicionar à lista
library-add-planning = Adicionar a Planejado
library-added-planning = Adicionado a Planejado.
library-on-your-list = Na sua lista
library-in-your-list = Na lista
progress-episodes-short = E{ $done } de { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Reproduzir E{ $episode } { -dubbed } · saiu { $ago }
up-next-heading = { -up-next-cap } para você
up-next-starts = Começa { $date }
up-next-reason-new-season = Nova temporada
up-next-reason-planning = Na sua lista Planejado
up-next-reason-recommended = Porque você concluiu { $title }
up-next-reason-recommended-plain = Recomendado para você
up-next-because-detail = Recomendado no AniList por quem gostou de { $title }
up-next-reason-airing-today = Vai ao ar hoje
up-next-add-planning = Adicionar a Planejado
up-next-not-interested = Não tenho interesse
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } não aparece mais em { -up-next-cap } para você.
up-next-hidden-until-airs = { $title } fica oculto até estrear.
up-next-broadcast = Episódio { $episode } · { $when }
up-next-out = saiu { $ago }
