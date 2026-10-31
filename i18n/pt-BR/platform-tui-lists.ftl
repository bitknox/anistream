lists-terminal-too-small = É preciso um terminal maior
downloads-screen-title = Downloads
downloads-column-title = Título
downloads-column-state = Estado
downloads-column-progress = Progresso
downloads-empty-heading = Nada na fila
downloads-empty-hint = Pressione { $key } em um título ou episódio para baixá-lo offline.
downloads-row-episode = E{ $episode }
downloads-row-episode-ahead = E{ $episode } · adiantado
downloads-note-sizes = { $downloaded } / { $total }
downloads-note-waiting-metadata = Aguardando metadados
downloads-note-torrents-off = Torrents desativados
downloads-note-waiting-start = Aguardando início
downloads-state-queued = Na fila
downloads-state-active = Baixando
downloads-state-paused = Pausado
downloads-state-done = Baixado
downloads-state-failed = Falhou
downloads-detail-ahead = Baixado antecipadamente, removido um dia depois de assistir
downloads-detail-with-ahead = { $detail } · baixado antecipadamente, removido um dia depois de assistir
downloads-detail-paused-at = Pausado em { $downloaded } de { $total } (continua a partir daí)
downloads-detail-paused-early = Pausado antes de começar
downloads-detail-torrents-off = Os downloads precisam de torrents, que estão desativados (pressione { $key } para Configurações)
downloads-detail-queued = Na fila (começa quando houver uma vaga para download)
downloads-detail-waiting-metadata = Aguardando os metadados do torrent
downloads-seeding-heading = Compartilhando depois de assistir
downloads-seeding-detail = { $ratio } de { $goal } · { $left } restantes · { $uploaded } enviados
downloads-failed-no-reason = O download falhou e nenhum motivo foi registrado.
downloads-not-finished = Esse download ainda não terminou.
accounts-screen-title = Contas
accounts-badge-sign-in = { $tracker } { $mark } entrar
accounts-needs-sign-in = Entre de novo em { $tracker }. Pressione { $key } para abrir Contas.
accounts-conflicts-found = { $count ->
    [one] 1 diferença de sincronização encontrada. Pressione { $key } para revisar.
    [many] { $count } diferenças de sincronização encontradas. Pressione { $key } para revisá-las.
   *[other] { $count } diferenças de sincronização encontradas. Pressione { $key } para revisá-las.
 }
accounts-conflicts-none = Não há diferenças de sincronização para revisar.
accounts-no-tracker = Nenhum { -tracker } está conectado. Pressione { $key } para abrir Contas.
accounts-finish-signing-in = Termine de entrar em { $tracker }
accounts-step-open = 1. Acesse   { $url }
accounts-step-enter = 2. Digite
accounts-code-expires-seconds = Expira em { $seconds } s
accounts-code-expires-minutes = Expira em { $minutes } min
accounts-code-expired = Código expirado
accounts-column-state = Estado
accounts-column-queued = Na fila
accounts-column-token = Token
accounts-empty-heading = Não há { -tracker-pl } ativados
accounts-empty-hint = Adicione um em { $setting } no config.toml. Seu histórico funciona sem nenhum.
accounts-state-sign-in-again = Entrar de novo
accounts-state-connected = Conectado
accounts-state-not-connected = Não conectado
accounts-pull-failed-today = A busca da sua lista falhou às { $time }: { $error }
accounts-pull-failed-dated = A busca da sua lista falhou em { $date } às { $time }: { $error }
accounts-pull-failed = A busca da sua lista falhou: { $error }
accounts-token-files = Os tokens são guardados em arquivos com permissões 0600 porque o chaveiro do sistema não está disponível.
accounts-enter-sign-out = ↵ Sair
accounts-enter-sign-in = ↵ Entrar
accounts-action-sign-in-again = Entrar de novo
accounts-action-sign-out = Sair
accounts-action-sign-in = Entrar
accounts-rows-empty = Não há { -tracker-pl } ativados. Adicione um em { $setting } no config.toml.
accounts-row-signed-in-as = Conectado como { $user }
accounts-row-connected = Conectado
accounts-row-not-connected = Não conectado
accounts-row-queued = { $count ->
    [one] 1 alteração na fila
    [many] { $count } alterações na fila
   *[other] { $count } alterações na fila
 }
accounts-row-queued-held = { $queued } na fila, { $held } aguardando a correspondência de id
accounts-conflict-keep-mine = Manter o meu
accounts-conflict-keep-theirs = Manter o deles
accounts-conflict-row = { $title }  ·  { $field }: aqui { $local } / no { -tracker } { $remote }
lists-library-title = { -library-cap }
lists-remove-wrong-list = Você só pode ocultar títulos na lista { -continue-cap }.
lists-continue-removed = { $title } oculto de { -continue-cap }. Assistir de novo o traz de volta.
lists-nothing-selected = Nenhum título está selecionado.
lists-search-prompt = Buscar { $query }
lists-load-failed = Não foi possível carregar esta lista
lists-loading = Carregando…
lists-empty-press-enter = Pressione ↵ para buscar
lists-empty-type-to-search = Digite para buscar
lists-empty-browse-filtered = Pressione ↵ para navegar com estes filtros.
lists-empty-browse-popular = Pressione ↵ para navegar pelos mais populares.
lists-empty-no-matches = Nada encontrado
lists-empty-fewer-filters = Tente menos filtros.
lists-empty-shorter-query = Tente uma busca mais curta ou o título em romaji.
lists-empty-nothing-watched = Nada assistido ainda
lists-empty-ways-on = Pressione { $seasonal } para esta temporada, ou { $search } para buscar.
lists-empty-nothing-to-continue = Nada para { -continue }
lists-empty-hidden-come-back = Os títulos que você ocultou voltam quando você assistir a eles. Pressione { $seasonal } para esta temporada, ou { $search } para buscar.
lists-empty-nothing-here = Nada aqui ainda
lists-library-no-account = Nenhuma conta conectada
lists-library-works-without = Seu histórico funciona sem uma. A { -library } espelha a lista em um { -tracker }.
lists-library-enable-tracker = Adicione um em { $setting } no config.toml e pressione { $key } para abrir Contas.
lists-library-sign-in = Pressione { $key } para abrir Contas e entrar.
lists-segment-watching = Assistindo
lists-segment-planning = Planejado
lists-segment-completed = Concluído
lists-segment-paused = Pausado
lists-segment-dropped = Abandonado
lists-episode = E{ $episode }
lists-episode-dubbed = E{ $episode } · { -dubbed-cap }
lists-score = { $score }%
lists-row-resume-percent = { $episode } · { $percent }%
lists-row-resume-clock = { $episode } · { $clock }
lists-episode-out = { $episode } saiu
lists-episode-in = { $episode } em { $countdown }
lists-next-in = { -up-next-cap } em { $countdown }
lists-episode-out-ago = { $episode } saiu há { $countdown }
lists-episode-out-just-now = { $episode } saiu agora mesmo
lists-time-in = Em { $countdown }
lists-time-ago = Há { $countdown }
lists-time-just-now = Agora mesmo
lists-time-approximate = ~{ $time }
lists-day-today = Hoje
lists-day-yesterday = Ontem
lists-day-tomorrow = Amanhã
lists-preview-continue-percent = { -continue-cap } { $episode } em { $clock } ({ $percent }%)
lists-preview-continue = { -continue-cap } { $episode } em { $clock }
lists-progress-aired = { $total ->
    [one] { $done } de { $total } episódio saiu
    [many] { $done } de { $total } episódios saíram
   *[other] { $done } de { $total } episódios saíram
 }
lists-progress-watched = { $count ->
    [one] 1 episódio assistido
    [many] { $count } episódios assistidos
   *[other] { $count } episódios assistidos
 }
lists-filter-genre = Gênero
lists-filter-year = Ano
lists-filter-season = Temporada
lists-filter-format = Formato
lists-filter-status = Status
lists-filter-sort = Ordenar
lists-filter-any = Qualquer
lists-sort-relevance = Relevância
lists-season-winter = Inverno
lists-season-spring = Primavera
lists-season-summer = Verão
lists-season-fall = Outono
lists-format-tv = TV
lists-format-tv-short = TV curta
lists-format-movie = Filme
lists-format-ova = OVA
lists-format-ona = ONA
lists-format-special = Especial
lists-format-music = Música
lists-status-airing = Em exibição
lists-status-finished = Concluído
lists-status-upcoming = Em breve
lists-sort-popularity = Popularidade
lists-sort-score = Nota
lists-sort-trending = Em alta
lists-sort-newest = Mais recentes
