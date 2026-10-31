notice-saved = Configuração salva: { $label }.
notice-not-saved = Não foi possível salvar essa configuração. { $reason }
notice-title-load-failed = Não foi possível carregar este título. Verifique sua conexão e tente de novo.
notice-playback-failed = A reprodução falhou. Tente outra { -source }.
notice-engine-fault = Algo deu errado dentro do { -app-name }. Ele continuou funcionando, mas se alguma tela parecer travada, saia e abra de novo.
notice-database-set-aside = O banco de dados estava danificado, então o { -app-name } criou um novo. O arquivo antigo foi guardado em { $path }. Os { -tracker-pl } trazem de volta o que guardam na próxima sincronização.
start-failed-folder = O { -app-name } não conseguiu criar a pasta { $path }. Verifique se o disco tem espaço e se é possível gravar nela, e tente de novo. { $reason }
start-failed-config = { $path } tem uma configuração que o { -app-name } não consegue usar: { $problem }. Corrija ou remova, depois tente de novo.
start-failed-database = O { -app-name } não conseguiu abrir o banco de dados em { $path }. Se outro { -app-name } estiver aberto nesta pasta, feche-o e tente de novo. { $reason }
start-failed-network = O { -app-name } não conseguiu configurar a conexão de rede com as configurações de rede do config.toml. { $reason }
start-failed = O { -app-name } não conseguiu iniciar. Saia e abra de novo. { $reason }
start-failed-remote = O controle remoto não conseguiu iniciar. { $reason }
start-failed-player = O player não conseguiu iniciar. { $reason }
notice-episodes-queued = { $count ->
    [one] Um episódio adicionado à fila.
    [many] { $count } episódios adicionados à fila.
   *[other] { $count } episódios adicionados à fila.
 }
