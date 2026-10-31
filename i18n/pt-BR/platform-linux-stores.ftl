stores-device-count = { $count ->
    [one] { $count } { -device }
    [many] { $count } { -device-pl }
   *[other] { $count } { -device-pl }
 }
stores-dialog-title = { $title }, { $store }
stores-owner-unreachable = Não foi possível acessar a { -plugin-store }
stores-owner-no-answer = A { -plugin-store } não respondeu
stores-owner-no-answer-detail = Ela pode estar ocupada, ou o computador dela pode estar desligado.
stores-owner-needs-connection = A { -plugin-store } precisa de conexão
stores-owner-needs-connection-detail = Tente de novo quando este { -device } voltar a ter conexão.
stores-owner-link-offline = Criar um link precisa de conexão com a { -plugin-store }.
stores-send-link-until = Envie este link. Ele funciona uma vez, até { $until }.
stores-open-on-detail = Entram sem informar nome. Desativar isso interrompe o link; ativar de novo cria outro.
stores-open-off-detail = Só pessoas convidadas podem entrar. Ativado, qualquer pessoa com o link entra, sem informar nome.
stores-open-no-names = Nenhum nome é guardado.
stores-open-keep-working = Eles continuam funcionando até você removê-los.
stores-device-seen = Visto { $when }
stores-device-never-seen = Não visto desde que entrou
stores-publish-short = Publicar um { -plugin }
stores-publish-none-hint = Um { -plugin } que você criar ou instalar aparece aqui.
stores-not-published = Ainda não está na { -plugin-store }
stores-store-offers = A { -plugin-store } oferece { $version }
stores-published-current = Atualizado na { -plugin-store }
stores-published-state = Publicado
stores-publish-label = Publicar { $name } { $version }
