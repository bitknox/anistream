stores-needs-larger-terminal = Precisa de um terminal maior
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Membros
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = Você
stores-column-state = Estado
stores-none-joined = Nenhuma { -plugin-store } ainda
stores-role-owner = Dono
stores-role-member = Membro
stores-join-row = + Entrar em uma { -store }
stores-state-revoked = Acesso removido
stores-state-offline = Sem conexão
stores-state-up-to-date = Em dia
stores-state-updates = { $count ->
    [one] { $count } atualização
    [many] { $count } atualizações
   *[other] { $count } atualizações
 }
stores-footer-revoked = Esta { -store } não aceita mais este { -device }. O dono pode enviar um novo convite.
stores-footer-offline = { $url } ({ $reason })
stores-key = Chave { $fingerprint }
stores-joined-without-name = Entrou sem nome
stores-joined-as = Entrou como { $name }
stores-footer-join = Cole o convite ou o link da { -store } que você recebeu
stores-owner-tag = Dono
stores-anyone-can-join = Qualquer pessoa com o link pode entrar
stores-note-offline = Sem conexão, mostrando a última lista recebida ({ $reason })
stores-note-revoked = Esta { -store } não aceita mais este { -device }. O que foi instalado continua funcionando.
stores-column-plugin = { -plugin-cap }
stores-column-version = { -release-cap }
stores-empty-owner = Nada publicado ainda. Publique um { -plugin } abaixo.
stores-empty-member = Nada publicado ainda
stores-line-link-device = Vincular outro { -device }
stores-line-invite = Convidar alguém
stores-line-members = Membros
stores-line-who-can-join = Quem pode entrar
stores-line-publish = Publicar um { -plugin }
stores-line-admin = Abrir a página de administração
stores-line-leave = Sair desta { -store }
stores-plugin-not-installed = Não instalado
stores-plugin-installed = Instalado
stores-plugin-update-from = Atualização de { $from }
stores-plugin-added-by-hand = Adicionado manualmente
stores-plugin-from-another-store = De outra { -store }
stores-footer-update-hosts = A atualização também precisa acessar { $hosts }
stores-footer-may-contact = Pode acessar { $hosts }
stores-footer-link-device = Um link de uso único que vincula outro dos seus computadores como você
stores-footer-invite = Um link de uso único para alguém novo
stores-footer-members = Quem já entrou, e remover o acesso de alguém
stores-footer-close-join = ↵ Deixar entrar só as pessoas convidadas e parar o link
stores-footer-open-join = ↵ Deixar qualquer pessoa com o link da { -store } entrar, sem nome
stores-footer-publish = Oferecer um { -plugin } da pasta de { -plugin-pl } deste computador
stores-footer-admin = Gerenciar membros, convites e { -plugin-pl } no navegador
stores-footer-leave = { -plugin-cap-pl } instalados por ela continuam neste computador e param de atualizar
stores-who-open = Qualquer pessoa com o link
stores-who-invite-only = Só pessoas que você convidar
stores-label-offers = Oferece
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
    [many] { $count } { -plugin-pl }
   *[other] { $count } { -plugin-pl }
 }
stores-offered-plugin = { $name } { $version }  pode acessar { $hosts }
stores-offered-installed = Instalado
stores-offered-installed-version = { $from } instalado
stores-offered-added-by-hand = Adicionado manualmente
stores-offered-from-other = De { $other }
stores-offered-more = e mais { $count }
stores-hosts-none = nada
stores-loading = Carregando…
stores-member-you = { $name } (você)
stores-column-name = Nome
stores-column-role = Função
stores-column-last-seen = Visto pela última vez
stores-members-heading = Membros · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Convites ainda não usados
stores-invite-another-device = { $name }, outro { -device }
stores-seen-never = Nunca
stores-ago-just-now = Agora mesmo
stores-ago-today = Hoje
stores-ago-yesterday = Ontem
stores-ago-days = { $days ->
    [one] Há { $days } dia
    [many] Há { $days } dias
   *[other] Há { $days } dias
 }
stores-ago-inline-just-now = agora mesmo
stores-ago-inline-today = hoje
stores-ago-inline-yesterday = ontem
stores-ago-inline-days = { $days ->
    [one] há { $days } dia
    [many] há { $days } dias
   *[other] há { $days } dias
 }
stores-expired = Expirado
stores-expires-today = Expira hoje
stores-expires-tomorrow = Expira amanhã
stores-expires-days = { $days ->
    [one] Expira em { $days } dia
    [many] Expira em { $days } dias
   *[other] Expira em { $days } dias
 }
stores-open-heading = Qualquer pessoa com o link pode entrar · Entradas pelo link: { $joined }
stores-closed-heading = Entradas pelo link antes de ele ser fechado · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
    [many] { $count } { -device-pl }
   *[other] { $count } { -device-pl }
 }
stores-open-link-row = O link da { -store }
stores-open-link-note = Mostrar e copiar
stores-anonymous-joined = Entrou { $when }
stores-anonymous-seen = Visto { $when }
stores-anonymous-never-seen = Ainda não visto
stores-remove-devices = { $count ->
    [one] Remover
    [many] Remover todos os { $count }
   *[other] Remover todos os { $count }
 }
stores-footer-you = Você é o dono. Uma { -store } sempre mantém o dono.
stores-footer-remove-member = ↵ Remover o acesso desta pessoa
stores-footer-cancel-invite = ↵ Cancelar este convite
stores-footer-show-link = ↵ Mostrar o link para copiar e compartilhar
stores-footer-remove-anonymous = ↵ Remover este { -device }, identificado só pela data de entrada
stores-footer-remove-all-anonymous = ↵ Remover todos os { -device-pl } que entraram pelo link
stores-link-copied = Copiado para a área de transferência, quando o terminal permitir, ou selecione para copiar.
stores-link-select = Selecione para copiar.
stores-open-link-warning = Qualquer pessoa com ele pode entrar, sem nome, quantas vezes quiser.
stores-open-link-lasts = Funciona até a { -store } voltar a aceitar só convites.
stores-invite-works-once = Funciona uma vez. { $expiry }.
stores-invite-preview-safe = Abrir no navegador mostra como entrar e não usa o convite.
stores-confirm-join = Entrar em { $name }?
stores-confirm-leave = Sair de { $name }?
stores-confirm-remove-member = Remover { $name }?
stores-confirm-cancel-invite = Cancelar o convite de { $name }?
stores-confirm-unpublish = Parar de oferecer { $name }?
stores-confirm-open = Deixar qualquer pessoa com o link entrar em { $name }?
stores-confirm-close = Deixar entrar só as pessoas convidadas em { $name }?
stores-confirm-remove-device = Remover este { -device }?
stores-confirm-remove-devices = { $count ->
    [one] Remover o { -device } que entrou pelo link?
    [many] Remover todos os { $count } { -device-pl } que entraram pelo link?
   *[other] Remover todos os { $count } { -device-pl } que entraram pelo link?
 }
stores-confirm-fallback = Confirmar
stores-label-address = Endereço
stores-label-key = Chave
stores-label-run-by = Administrado por
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (corresponde ao link)
stores-key-matches-invite = { $fingerprint }  (corresponde ao convite)
stores-join-open = Qualquer pessoa com o link pode entrar.
stores-join-anonymous = Você entra sem nome, e outro { -device } entra com o mesmo link.
stores-join-already = Já entrou. Entrar de novo gera um novo token para este { -device }.
stores-join-device-note = O dono da { -store } vê este nome. Digite para alterar.
stores-leave-plugins = { -plugin-cap-pl } instalados por ela continuam neste computador e param de atualizar.
stores-leave-revoked = O acesso deste { -device } à { -store } foi revogado.
stores-remove-member-devices = { $count ->
    [one] O { -device } desta pessoa para de funcionar com esta { -store } imediatamente.
    [many] Os { $count } { -device-pl } desta pessoa param de funcionar com esta { -store } imediatamente.
   *[other] Os { $count } { -device-pl } desta pessoa param de funcionar com esta { -store } imediatamente.
 }
stores-remove-member-plugins = { -plugin-cap-pl } já instalados continuam nos computadores dessa pessoa.
stores-cancel-invite-link = O link deixa de funcionar.
stores-unpublish-members = Quem já instalou continua com ele, e ele deixa de atualizar.
stores-open-anyone = Qualquer pessoa com o link da { -store } pode entrar, sem nome.
stores-open-link-new = O link é novo a cada vez, e o anterior deixa de funcionar.
stores-open-members-see = Os membros veem que a { -store } está aberta.
stores-close-link = O link deixa de funcionar, e ninguém novo entra sem convite.
stores-close-devices = { -device-cap-pl } que entraram com ele continuam funcionando até você removê-los em Membros.
stores-anonymous-one-stops = Ele para de funcionar com esta { -store } imediatamente.
stores-anonymous-all-stop = Eles param de funcionar com esta { -store } imediatamente.
stores-anonymous-plugins = { -plugin-cap-pl } já instalados continuam onde estão.
stores-anonymous-still-joins = Enquanto a { -store } estiver aberta, o link ainda deixa qualquer pessoa entrar.
stores-plugin-may-contact = Pode acessar { $hosts }
stores-plugin-cannot = Não pode ler arquivos, abrir conexões de rede nem ver o seu ambiente.
stores-plugin-update-adds = A atualização passa a poder acessar { $hosts }.
stores-plugin-added-by-hand-note = { $id }.wasm já está na pasta de { -plugin-pl }, adicionado manualmente.
stores-plugin-other-store-note = Instalado a partir de outra { -store }, que o mantém atualizado.
stores-choice-install = Instalar
stores-choice-update = Atualizar para { $version }
stores-choice-replace = Substituir a cópia adicionada manualmente
stores-choice-remove = Remover deste computador
stores-choice-unpublish = Parar de oferecer nesta { -store }
stores-choice-cancel = Cancelar
stores-publish-new = Novo
stores-publish-published = Publicado
stores-publish-replaces = Substitui { $version }
stores-toast-invite-checked = O convite está válido, mas outra tela está aberta. Cole o convite de novo para entrar.
stores-toast-nothing-to-publish = Não há { -plugin-pl } para publicar na sua pasta de { -plugin-pl }. Execute "{ $command }" para ver onde ela fica.
stores-toast-that-is-you = É você. Uma { -store } sempre mantém o dono.
