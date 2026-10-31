notice-reason = { $reason }
cause-no-sources = Não há { -source-pl } configuradas. Adicione uma na tela { -source-cap-pl }.
notice-not-aired = O episódio { $episode } vai ao ar { $when }.
notice-not-aired-yet = O episódio { $episode } ainda não foi ao ar. O episódio { $next } vai ao ar { $when }.
cause-airs-today = hoje às { $time }
cause-airs-tomorrow = amanhã às { $time }
cause-airs-in-days = { $days ->
    [one] em um dia às { $time }
    [many] em { $days } dias às { $time }
   *[other] em { $days } dias às { $time }
 }
cause-airs-on-date = em { $date } às { $time }
cause-no-playable-stream = A { -source } respondeu, mas não enviou nada para reproduzir. Tente outra.
cause-title-no-match = Não foi possível localizar este título nas { -source-pl }: { $detail }
cause-source-gone = A { -source } { $source } não está mais disponível. Escolha outra.
cause-release-failed = Não foi possível iniciar essa { -release }: { $detail }
cause-sources-timed-out = As { -source-pl } demoraram demais para responder. Tente de novo ou escolha outra { -source }.
cause-no-source-has-episode = Nenhuma { -source } tem o episódio { $episode }: { $detail }
cause-source-cannot-download = Esta { -source } transmite episódios, mas não é possível baixá-los.
cause-no-source-could-list-show = Nenhuma { -source } conseguiu listar esta série, então ela não pode ser baixada.
cause-subtitles-merged = { $tracks ->
    [one] Uma faixa de legenda foi incorporada ao vídeo.
    [many] { $tracks } faixas de legenda foram incorporadas ao vídeo.
   *[other] { $tracks } faixas de legenda foram incorporadas ao vídeo.
 }
cause-subtitles-left-apart = As legendas ficam como arquivos separados. { $reason }
cause-already-in-downloads = { $count ->
    [one] Um episódio já está nos downloads.
    [many] { $count } episódios já estão nos downloads.
   *[other] { $count } episódios já estão nos downloads.
 }
status-looking-up-title = Procurando o título…
status-loading-sources = Carregando { -source-pl }…
status-searching-sources = Buscando o episódio { $episode } em { -source-pl }…
notice-player-failed-to-start = Não foi possível iniciar o mpv. Verifique se ele roda sozinho. ({ $reason })
notice-stream-no-video = Nenhum vídeo nesta transmissão. Tentando a transmissão { $next } de { $total }.
notice-player-could-not-play = O mpv não conseguiu reproduzir isto. { $reason }
notice-player-exited-silently = O mpv fechou sem reproduzir nada e sem informar o motivo. Tente outra { -source }.
notice-player-not-playing = O mpv ainda não começou a reproduzir depois de { $seconds } s. A { -source } pode não ter dados. Verifique se o mpv reproduz um arquivo sozinho.
notice-no-player = Esta compilação do { -app-name } não consegue reproduzir vídeo.
notice-mpv-missing = { $binary } não foi encontrado. Instale o mpv para reproduzir episódios.
notice-mpv-old = O mpv { $version } pode reproduzir, mas o player do { -app-name } precisa do mpv 0.36 ou mais recente para o botão de pular, o cartão de { -up-next-cap }, os menus e as miniaturas.
notice-subtitles-unavailable = As legendas desta { -source } não puderam ser carregadas. Escolha outra { -source } para tentar de novo.
notice-opened-in-browser = Aberto no seu navegador.
notice-could-not-open = Não foi possível abrir isso. { $reason }
notice-trailer-playing = Trailer sendo reproduzido no mpv.
notice-trailer-opened = Trailer aberto no seu navegador.
notice-trailer-failed = Não foi possível abrir o trailer. { $reason }
notice-filler-skipped = Episódio de enchimento pulado. O { -up-next-cap } é o episódio { $episode }.
notice-audio-now-subbed = O áudio agora é { -subbed }.
notice-audio-now-dubbed = O áudio agora é { -dubbed }.
notice-already-first-episode = Já está no primeiro episódio.
notice-last-episode = Não há episódio seguinte. O episódio { $episode } é o último.
notice-no-next-episode = Não há episódio seguinte.
notice-no-previous-episode = Não há episódio anterior.
notice-match-reset-failed = Não foi possível redefinir a correspondência. { $reason }
notice-match-save-failed = Não foi possível salvar essa correspondência. { $reason }
notice-match-did-not-stick = Essa correspondência não foi salva. A { -source } pode ter mudado. Tente escolher de novo.
notice-source-search-empty = Nada encontrado para “{ $query }”. Tente uma busca diferente.
notice-source-pinned = A { -source } { $source } será usada para este título.
notice-source-automatic = Voltou à seleção automática de { -source } para este título.
notice-source-choice-failed = Não foi possível salvar essa escolha. { $reason }
notice-no-source-could-list = Nenhuma { -source } conseguiu listar esta série: { $reason }
notice-episodes-failed = Não foi possível carregar os episódios. { $reason }
notice-match-uncertain = { $source } encontrou mais de uma série que pode ser esta, então, por enquanto, os episódios vêm do AniList.
notice-continue-hide-failed = Não foi possível remover isso de { -continue-cap }. { $reason }
notice-download-queued = Episódio { $episode } de { $title } adicionado à fila.
notice-download-already-queued = O episódio { $episode } de { $title } já está nos downloads.
notice-download-starting-now = Iniciando o episódio { $episode } de { $title } agora.
notice-download-cancelled = Download cancelado.
notice-download-file-deleted = Download e arquivo excluídos.
notice-download-removed-file-gone = Download removido. O arquivo já não existia.
notice-download-removed-file-kept = Download removido, mas não foi possível excluir o arquivo. { $reason }
notice-download-removed = Download removido.
notice-downloads-cleared = { $count ->
    [one] Um download concluído foi removido da lista.
    [many] { $count } downloads concluídos foram removidos da lista.
   *[other] { $count } downloads concluídos foram removidos da lista.
 }
notice-download-finished = Episódio { $episode } de { $title } baixado.
notice-download-finished-note = Episódio { $episode } de { $title } baixado. { $note }
notice-download-failed = Não foi possível baixar o episódio { $episode } de { $title }. { $reason }
notice-downloads-none-queued = Nada foi adicionado à fila.
notice-downloads-none-queued-stopped = Nada foi adicionado à fila. { $reason }
notice-downloads-none-new = Nada novo foi adicionado à fila. { $existing }
notice-downloads-none-new-stopped = Nada novo foi adicionado à fila. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] Um episódio foi adicionado à fila. { $existing }
    [many] { $count } episódios foram adicionados à fila. { $existing }
   *[other] { $count } episódios foram adicionados à fila. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] Um episódio foi adicionado à fila, e então a busca parou. { $reason }
    [many] { $count } episódios foram adicionados à fila, e então a busca parou. { $reason }
   *[other] { $count } episódios foram adicionados à fila, e então a busca parou. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] Um episódio foi adicionado à fila, e então a busca parou. { $existing } { $reason }
    [many] { $count } episódios foram adicionados à fila, e então a busca parou. { $existing } { $reason }
   *[other] { $count } episódios foram adicionados à fila, e então a busca parou. { $existing } { $reason }
 }
notice-keep-next-full = Não há espaço livre suficiente para manter os próximos episódios baixados. Libere espaço para baixar mais.
party-needs-server = Escolha primeiro um servidor para a { -watch-party }, em syncplay.server. { $server } é mantido por { $operator }.
party-choose-server = Escolha primeiro um servidor para a { -watch-party }. { $server } é mantido por { $operator }.
notice-no-party = { -watch-party-cap-pl } não estão disponíveis nesta compilação do { -app-name }.
party-needs-room = Uma { -watch-party } precisa de uma sala. Escolha uma antes.
party-server-lost = A conexão com o servidor da { -watch-party } caiu. Tentando de novo.
party-rejoined = De volta à { -watch-party }.
party-left = Você saiu da { -watch-party }.
