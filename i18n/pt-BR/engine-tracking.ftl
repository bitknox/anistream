notice-signed-in = Conectado ao { $tracker }.
notice-signed-in-keychain = Conectado ao { $tracker }. O token está no chaveiro do sistema.
notice-signed-in-file = Conectado ao { $tracker }. Não há chaveiro disponível, então o token fica em um arquivo privado neste { -device }.
notice-signed-in-unsaved = Conectado ao { $tracker }, mas o token não foi salvo. Será preciso conectar de novo na próxima vez. { $reason }
notice-token-not-saved = Não foi possível salvar o token de entrada. Tente conectar de novo. { $reason }
notice-sign-in-failed = Não foi possível conectar ao { $tracker }. Tente de novo. { $reason }
notice-sign-in-stopped = A conexão com o { $tracker } parou antes de terminar. Tente de novo. { $reason }
notice-token-exchange-failed = Não foi possível receber um token do { $tracker }. Tente conectar de novo. { $reason }
notice-browser-not-opened = Não foi possível abrir um navegador para a conexão. Verifique se há um instalado e tente de novo. { $reason }
notice-sign-in-not-waiting = Nenhuma conexão com o { $tracker } aguarda por isso. Comece a conexão de novo.
notice-sign-in-wrong-request = A conexão com o { $tracker } não voltou deste pedido. Comece a conexão de novo.
notice-sign-in-no-token = A conexão com o { $tracker } voltou sem token. Tente de novo.
notice-mal-client-missing-build = Não foi possível concluir a conexão com o MyAnimeList: este app não inclui um cliente para ela.
notice-kitsu-signs-in-on-accounts = O Kitsu se conecta pela tela de Contas.
notice-sign-in-unsupported = Ainda não é possível conectar ao { $tracker }.
notice-sign-in-needs-redirect = Conectar ao { $tracker } precisa de um redirecionamento pelo navegador, que este app não consegue receber.
notice-anilist-implicit-flow = Este método de conexão do AniList funciona só no app { -app-name }. Para conectar aqui, defina { $setting } como "loopback" no arquivo de configuração.
notice-anilist-client-missing = O AniList precisa de um ID de cliente e de um segredo. Defina { $client } e { $secret } no arquivo de configuração e conecte de novo. Para obter o endereço de conexão, execute { -app-name } --login-url.
notice-anilist-paste-flow = Não é possível colar um código aqui. Saia e execute { -app-name } --login em vez disso.
notice-mal-client-missing = O MyAnimeList precisa de um ID de cliente. Registre um app do tipo "other" em myanimelist.net/apiconfig e defina { $setting } no arquivo de configuração.
status-waiting-for-redirect = Aguardando o { $tracker } redirecionar de volta…
notice-mal-no-refresh-token = O MyAnimeList não emitiu um token de atualização. Será preciso conectar de novo em cerca de um mês.
notice-client-id-missing = Defina { $setting } no arquivo de configuração primeiro.
notice-signed-out = Desconectado do { $tracker }.
notice-signed-out-queue = { $count ->
    [one] Desconectado do { $tracker }. Uma alteração na fila foi mantida para a próxima conexão.
    [many] Desconectado do { $tracker }. { $count } alterações na fila foram mantidas para a próxima conexão.
   *[other] Desconectado do { $tracker }. { $count } alterações na fila foram mantidas para a próxima conexão.
 }
notice-sign-out-failed = Não foi possível desconectar do { $tracker }. Tente de novo. { $reason }
notice-not-saved-because = Não foi possível salvar essa configuração. { $cause }
notice-protection-saved = Proteção de torrents salva.
notice-protection-not-checked = Não foi possível verificar a proteção de torrents. { $cause }
notice-proxy-password-not-kept = Não foi possível guardar a senha do proxy. Digite a senha de novo para tentar salvar. { $reason }
notice-show-setting-refused = Não foi possível salvar essa configuração. { $value } não é um valor aceito para { $setting }.
notice-show-not-reset = Não foi possível redefinir as configurações desta série. { $reason }
notice-show-no-such-subtitles = Não foi possível salvar essa escolha de legenda. Esta série nunca usou essas legendas.
notice-torrents-waiting-for-check = Os torrents aguardam a verificação da conexão passar de novo.
notice-mapping-data-refreshed = Dados de mapeamento atualizados.
notice-mapping-data-failed = Não foi possível atualizar os dados de mapeamento ({ $datasets }). Nova tentativa em { $wait }. { $cause }
notice-episode-aired = { $title }, episódio { $episode }, foi ao ar enquanto você estava fora.
notice-episodes-aired = { $count ->
    [one] Um novo episódio de uma série que você acompanha foi ao ar. Ele está no calendário.
    [many] { $count } novos episódios de séries que você acompanha foram ao ar. Eles estão no calendário.
   *[other] { $count } novos episódios de séries que você acompanha foram ao ar. Eles estão no calendário.
 }
notice-update-available = { $tag } está disponível. Execute { -app-name } --update para instalar.
cause-confirm-no-protection = Confirme antes que você quer não ter proteção.
cause-confirm-warn-only = Confirme antes a opção de só avisar.
cause-proxy-needs-address = Um proxy precisa de um endereço e de uma porta.
cause-proxy-address-unusable = O endereço do proxy não pode ser usado na verificação.
cause-server-unreachable = Não foi possível alcançar o servidor.
