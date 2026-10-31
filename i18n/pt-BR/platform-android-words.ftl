episode-short-dubbed = E{ $episode } · { -dubbed-cap }
episode-long-dubbed = Episódio { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
    [one] Um episódio pausado
    [many] { $count } episódios pausados
   *[other] { $count } episódios pausados
 }
downloads-live-lead = { $title } E{ $episode }
downloads-live-left = { $size } restantes
downloads-live-progress = { $done } de { $total }
downloads-live-paused-one = Pausado
downloads-live-waiting = Aguardando
settings-seed-ratio-until = Até a razão { $ratio }
settings-upload-rate = { $size }/s
settings-coming-up-dubbed = { $count ->
    [one] { $names } avisa quando a dublagem sair.
    [many] { $names } avisam quando as dublagens saírem.
   *[other] { $names } avisam quando as dublagens saírem.
 }
settings-coming-up-switched-off = { $count ->
    [one] { $names } está desativado nas próprias configurações.
    [many] { $names } estão desativados nas próprias configurações.
   *[other] { $names } estão desativados nas próprias configurações.
 }
settings-coming-up-opens = Um título abre as suas configurações.
torrents-check-note-warn = { -app-name } verifica por onde sai o tráfego de torrents quando um torrent começa e a cada minuto enquanto estiver ativo, e avisa se essa verificação falhar. Sua VPN ou proxy é o que protege você.
torrents-check-note-stop = { -app-name } verifica por onde sai o tráfego de torrents quando um torrent começa e a cada minuto enquanto estiver ativo, e interrompe os torrents se essa verificação falhar. Sua VPN ou proxy é o que protege você.
torrents-protect-proxy-name = Proxy SOCKS5
torrents-protect-vpn-name = VPN neste { -device }
torrents-warn-body = { -app-name } vai avisar você em vez de pausar. Se sua VPN ou proxy estiver fora do ar, outras pessoas podem ver o seu endereço IP.
torrents-proxy-as = como { $user }
title-facts-aired = Exibido em { $dates }
title-facts-airing = Em exibição desde { $date }
title-facts-airs = Estreia em { $date }
media-dates-range = { $from } a { $to }
title-also-on-more = { $more ->
    [one] { $first } e mais um
    [many] { $first } e mais { $more }
   *[other] { $first } e mais { $more }
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } usados · { $free } livres
downloads-storage-used = { $used } usados
downloads-time-left = { $time } restantes
downloads-rate = { $size }/s
downloads-uploaded = { $size } enviados
downloads-ratio-of = { $ratio } de { $goal }
downloads-group-summary = { $count ->
    [one] Um episódio · { $size }
    [many] { $count } episódios · { $size }
   *[other] { $count } episódios · { $size }
 }
episode-menu-download = Baixar
episode-menu-download-paused = Download pausado
reason-timeout-anilist = O AniList demorou demais para responder.
reason-timeout-server = O servidor demorou demais para responder.
reason-busy-anilist = O AniList está ocupado. Tente de novo em um minuto.
reason-busy-server = O servidor está ocupado. Tente de novo em um minuto.
reason-problem-anilist = O AniList teve um problema.
reason-problem-server = O servidor teve um problema.
reason-silent-anilist = O AniList não respondeu.
reason-silent-server = O servidor não respondeu.
updates-installing-plain = Instalando
updates-closes-as-installs = { -app-name } fecha enquanto instala
updates-open-release = Abrir página
country-korea = Coreia do Sul
search-genres-picked = { $count ->
    [one] Um gênero
    [many] { $count } gêneros
   *[other] { $count } gêneros
 }
search-formats-picked = { $count ->
    [one] Um formato
    [many] { $count } formatos
   *[other] { $count } formatos
 }
search-statuses-picked = { $count ->
    [one] Um status
    [many] { $count } status
   *[other] { $count } status
 }
filter-score-any = Qualquer
filter-years-from = A partir de { $year }
filter-years-between = De { $from } a { $to }
filter-tag-rank-title = Classificação mínima da tag
filter-tag = Tag
search-filter-by = Filtrar por { $name }
search-loading-genres = Carregando gêneros
search-paused-rate-limit = A busca enquanto você digita está pausada por um momento, para respeitar os limites do AniList. Use a tecla de busca para pesquisar agora.
action-clear-all = Limpar tudo
filter-source-material = Baseado em
search-detail-episode = { $status } · E{ $episode }
sources-use = Usar { $name }
sources-move = Mover { $name }
sources-reorder = Reordenar
sources-version-available = { $version } disponível
sources-update-from = { -release-cap } { $version } de { $store }.
sources-detail-off = Desativada. Ela mantém a posição e não é usada até ser ativada de novo.
sources-fallback-title = { -source-cap }
reason-source-timeout = Demorou demais para responder.
reason-source-busy = Está ocupada. Tente de novo em um minuto.
reason-source-problem = Teve um problema.
reason-source-turned-away = Recusou o pedido.
reason-source-no-address = Não foi possível encontrar o endereço dela.
reason-source-unreachable = Não foi possível conectar-se a ela.
stores-updates-waiting = { $count ->
    [one] Uma atualização
    [many] { $count } atualizações
   *[other] { $count } atualizações
 }
stores-checked-ago = verificada · { $ago }
stores-preview-key-link = A chave corresponde ao link
stores-preview-key-invite = A chave corresponde ao convite
stores-key-verified = Verificada
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = a { -plugin-store }
stores-link-sheet-detail = Abra este link no seu outro { -device } para entrar na { -plugin-store } de lá. Ele funciona uma vez.
accounts-synced-ago = Sincronizado · { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Abre Contas
accounts-card-description-signed-out = Não conectado. Abre Contas
accounts-signed-in-plain = Conectado
settings-summary-notify-on = Novos episódios ativados
settings-summary-notify-off = Novos episódios desativados
settings-summary-sources = { $count ->
    [one] Uma { -source }
    [many] { $count } { -source-pl }
   *[other] { $count } { -source-pl }
 }
settings-summary-no-sources = Nenhuma { -source-pl } ainda
settings-summary-torrents-on = torrents ativados
settings-summary-torrents-off = torrents desativados
library-storage-on-device = { $size } neste { -device }
library-storage-none = Nada neste { -device }
library-detail-of = E{ $next } de { $total }
library-detail-next = E{ $next }
library-sort-last-watched = Último assistido
library-sort-current = Atual
settings-notify-permission-note = { -app-name } pede permissão na primeira vez que você ativar isto.
settings-notify-refused = Desativado: o Android não permite notificações do { -app-name }
settings-notify-system-title = Configurações de notificação
settings-notify-system-detail = Som, vibração e tela de bloqueio
settings-titles-line-signed-out = Em inglês enquanto você estiver desconectado do AniList
settings-titles-line-account-language = { $language }, da sua conta do AniList
settings-titles-line-follows = Segue a sua conta do AniList
settings-titles-line-english = Romaji quando não houver título em inglês
settings-titles-line-romaji = Títulos japoneses em letras latinas
settings-titles-note-account = Segue a sua conta do AniList, que está em { $language }. Enquanto você estiver desconectado do AniList, os títulos aparecem em inglês.
settings-titles-note-follows = Segue a sua conta do AniList. Enquanto você estiver desconectado do AniList, os títulos aparecem em inglês.
settings-titles-note-english-missing = { $title } não tem título em inglês no AniList, então mantém o título em romaji.
settings-titles-note-english = Títulos em inglês como o AniList os lista, e romaji quando não houver.
settings-titles-note-romaji = Títulos japoneses em letras latinas, como o AniList os lista.
settings-index-none = Nenhum definido
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = Melhor esforço neste { -device }: o compartilhamento só funciona enquanto o { -app-name } estiver aberto.
settings-downloads-count = { $count ->
    [one] Um episódio
    [many] { $count } episódios
   *[other] { $count } episódios
 }
settings-anilist-footer = O progresso e as mudanças na lista são sincronizados com o AniList. { -library-cap } mostra suas listas do AniList.
accounts-sign-in-again-button = Entrar de novo
accounts-review-differences = { $count ->
    [one] Revisar uma diferença
    [many] Revisar { $count } diferenças
   *[other] Revisar { $count } diferenças
 }
settings-licence-title = Licença
settings-licence-value = MIT, código aberto e não comercial
settings-typefaces-value = Roboto Flex e Zen Kaku Gothic New, sob a SIL Open Font License 1.1
settings-libraries-value = JNA e AndroidX, sob a Apache License 2.0
settings-report-bug-value = Abre o GitHub no seu navegador
downloads-storage-share-description = Os downloads ocupam { $size }
settings-aside-notifications = Uma notificação quando sai um episódio de um título que você acompanha: o título e o número do episódio, nunca o nome dele. Um título pode desativar as próprias notificações nas configurações dele.
settings-aside-downloads = Os downloads ficam no espaço próprio do { -app-name } neste { -device }, que outros apps não conseguem ler. Remover o app remove também os downloads.
settings-aside-connection-check = Verificação de conexão
settings-aside-sources = { -source-cap-pl } são testadas em ordem, de cima para baixo. Um título pode manter a própria { -source }, escolhida na página dele.
settings-upscaling-description = A mesma imagem: a metade esquerda em 480p original, a direita com { $mode }
settings-upscaling-source = 480p original
settings-upscaling-note-off-original = O 480p original, ampliado para caber sem processamento.
torrents-sheet-turn-on-title = Ativar torrents
torrents-sheet-protect-title = Proteger o tráfego de torrents
torrents-sheet-turn-on-button = Ativar
torrents-sheet-host = Host
torrents-sheet-port = Porta
torrents-sheet-proxy-auth-note = Nome de usuário e senha só se o proxy pedir.
torrents-sheet-edit-proxy-title = Editar proxy
title-facts-also-on = Também em
title-fetch-failed-detail = O AniList não respondeu.
continue-resize-queue = Redimensionar a fila
continue-go-to-title = Ir para o título
continue-short-left = { $episode } · { $left }
continue-airing-out = E{ $episode } · { $time } · saiu { $ago }
continue-airing-in = E{ $episode } · { $time } · sai em { $countdown }
calendar-line-out = { $episode } saiu { $ago }
calendar-line-in = { $episode } sai em { $countdown }
nav-accounts-and-settings-user = { $user }, contas e configurações
episode-unaired-label = Episódio { $number }, { $title }. Ainda não saiu
episode-unaired-label-plain = Episódio { $number }. Ainda não saiu
episode-unaired-airs-label = Episódio { $number }, { $title }. Estreia { $airs }, ainda não saiu
episode-unaired-airs-label-plain = Episódio { $number }. Estreia { $airs }, ainda não saiu
tv-sheet-on-network = Na rede
tv-sheet-not-listed = Não apareceu
tv-sheet-scan = Escanear o código da TV
tv-sheet-code-on = Código em { $name }
tv-sheet-pairing-button = Pareando
tv-sheet-connecting = Conectando à TV
tv-sheet-type-code = Digite o código
tv-sheet-volume = Volume da TV
tv-sheet-volume-down = Diminuir volume
tv-sheet-volume-up = Aumentar volume
tv-sheet-volume-value = { $value } de 100
tv-sheet-nothing-playing = Nada em reprodução
tv-sheet-position = Posição na TV
tv-sheet-position-state = { $at } de { $total }
tv-sheet-request-titled = { $title }, episódio { $episode }
tv-sheet-request-plain = Episódio { $episode }
tv-sheet-footer = A TV lista este { -device } com o código { $code }. Se você esquecer uma TV aqui, ela também esquece este { -device }.
tv-sheet-title-fallback = TV
episode-line-detail = E{ $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, hoje
calendar-day-label-airing = { $day }, episódios em exibição
calendar-day-label-today-airing = { $day }, hoje, episódios em exibição
continue-show-airing-in-calendar = Mostrar no Calendário os títulos que vão ao ar hoje
continue-show-season-in-seasonal = Mostrar esta temporada em Sazonal
search-searching-filters = Buscando { $filters }
episode-number-plain = Episódio { $number }
episode-number-title-text = Episódio { $number } · { $title }
player-position-state = { $at } de { $total }
player-sheet-finding = Buscando { -source-pl }…
player-sheet-none = Nenhuma { -source } encontrada para este episódio.
player-sheet-episodes-loading = A lista de episódios está a caminho.
player-dual-audio = { -subbed-cap } e { -dubbed }
notif-new-episodes = { $count ->
    [one] Um episódio novo
    [many] { $count } episódios novos
   *[other] { $count } episódios novos
 }
notif-hidden-text = Novo episódio
notif-episode-out = Episódio { $episode } saiu.
notif-episode-out-dubbed = Episódio { $episode } saiu, { -dubbed }.
notif-episode-out-next = Episódio { $episode } saiu. { -up-next-cap } para você: episódio { $next }.
notif-episode-out-dubbed-next = Episódio { $episode } saiu, { -dubbed }. { -up-next-cap } para você: episódio { $next }.
device-name-phone = Celular Android
device-name-tablet = Tablet Android
tv-session-starting = Iniciando em { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · faltam { $minutes } min
widget-line-of = E{ $episode } de { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = Em dia
widget-caught-up = Você está em dia.
widget-caught-up-next = { $title } E{ $episode } estreia { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Reproduzir episódio { $episode }, { -dubbed }, de { $title }
tv-action-resume = Retomar episódio { $episode }
tv-action-resume-dubbed = Retomar episódio { $episode } · { -dubbed-cap }
tv-action-out = Reproduzir episódio { $episode } (sai { $day })
tv-action-not-out = Reproduzir episódio { $episode } (ainda não saiu)
tv-out-today = hoje
tv-hero-resume = Retomar { $episode }
tv-hero-start = Reproduzir { $episode }
