stores-needs-larger-terminal = Нужен терминал большего размера
stores-list-heading = { -plugin-store-cap-pl }
stores-page-heading = { -plugin-store-cap }
stores-members-placeholder-heading = Участники
stores-column-store = { -store-cap }
stores-column-plugins = { -plugin-cap-pl }
stores-column-you = Вы
stores-column-state = Состояние
stores-none-joined = Пока ни один { -store } не присоединён
stores-role-owner = Владелец
stores-role-member = Участник
stores-join-row = + Присоединиться к { -store }у
stores-state-revoked = Доступ отозван
stores-state-offline = Нет связи
stores-state-up-to-date = Актуально
stores-state-updates = { $count ->
    [one] { $count } обновление
    [few] { $count } обновления
    [many] { $count } обновлений
   *[other] { $count } обновления
 }
stores-footer-revoked = Этот { -store } больше не принимает это { -device }. Его владелец может отправить новое приглашение.
stores-footer-offline = { $url } ({ $reason })
stores-key = Ключ { $fingerprint }
stores-joined-without-name = Присоединено без имени
stores-joined-as = Присоединено как { $name }
stores-footer-join = Вставьте приглашение или ссылку { -store }а, которую вам дали
stores-owner-tag = Хозяин
stores-anyone-can-join = любой, у кого есть ссылка, может присоединиться
stores-note-offline = Нет связи, показан последний полученный список ({ $reason })
stores-note-revoked = Этот { -store } больше не принимает это { -device }. Установленное продолжает работать.
stores-column-plugin = { -plugin-cap }
stores-column-version = Версия
stores-empty-owner = Пока ничего не опубликовано. Опубликуйте { -plugin } ниже.
stores-empty-member = Пока ничего не опубликовано
stores-line-link-device = Связать другое { -device }
stores-line-invite = Пригласить кого-то
stores-line-members = Участники
stores-line-who-can-join = Кто может присоединиться
stores-line-publish = Опубликовать { -plugin }
stores-line-admin = Открыть страницу администратора
stores-line-leave = Покинуть этот { -store }
stores-plugin-not-installed = Не установлен
stores-plugin-installed = Установлен
stores-plugin-update-from = Обновление с { $from }
stores-plugin-added-by-hand = Добавлен вручную
stores-plugin-from-another-store = Из другого { -store }а
stores-footer-update-hosts = Обновление также хочет обращаться к { $hosts }
stores-footer-may-contact = Может обращаться к { $hosts }
stores-footer-link-device = Одноразовая ссылка, которая присоединит другое ваше { -device } от вашего имени
stores-footer-invite = Одноразовая ссылка для нового человека
stores-footer-members = Кто присоединился, и как отозвать доступ
stores-footer-close-join = ↵ Пускать только приглашённых и остановить ссылку
stores-footer-open-join = ↵ Разрешить присоединиться без имени любому, у кого есть ссылка { -store }а
stores-footer-publish = Предложить { -plugin } из папки { -plugin }ов на этом компьютере
stores-footer-admin = Управлять участниками, приглашениями и { -plugin-pl } в браузере
stores-footer-leave = { -plugin-cap-pl }, установленные из него, остаются на этом компьютере и перестают обновляться
stores-who-open = Любой, у кого есть ссылка
stores-who-invite-only = Только приглашённые вами
stores-label-offers = Предлагает
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
    [few] { $count } { -plugin }а
    [many] { $count } { -plugin }ов
   *[other] { $count } { -plugin }а
 }
stores-offered-plugin = { $name } { $version } может обращаться к { $hosts }
stores-offered-installed = Установлен
stores-offered-installed-version = Установлена { $from }
stores-offered-added-by-hand = Добавлен вручную
stores-offered-from-other = Из { $other }
stores-offered-more = и ещё { $count }
stores-hosts-none = ни к чему
stores-loading = Загрузка…
stores-member-you = { $name } (вы)
stores-column-name = Имя
stores-column-role = Роль
stores-column-devices = { -device-cap-pl }
stores-column-last-seen = Последний раз в сети
stores-members-heading = Участники · { $name }
stores-unnamed = { -plugin-store-cap }
stores-invites-heading = Неиспользованные приглашения
stores-invite-another-device = { $name }, другое { -device }
stores-seen-never = Никогда
stores-ago-just-now = Только что
stores-ago-today = Сегодня
stores-ago-yesterday = Вчера
stores-ago-days = { $days ->
    [one] { $days } день назад
    [few] { $days } дня назад
    [many] { $days } дней назад
   *[other] { $days } дня назад
 }
stores-ago-inline-just-now = только что
stores-ago-inline-today = сегодня
stores-ago-inline-yesterday = вчера
stores-ago-inline-days = { $days ->
    [one] { $days } день назад
    [few] { $days } дня назад
    [many] { $days } дней назад
   *[other] { $days } дня назад
 }
stores-expired = Истекло
stores-expires-today = Истекает сегодня
stores-expires-tomorrow = Истекает завтра
stores-expires-days = { $days ->
    [one] Истекает через { $days } день
    [few] Истекает через { $days } дня
    [many] Истекает через { $days } дней
   *[other] Истекает через { $days } дня
 }
stores-open-heading = Любой, у кого есть ссылка, может присоединиться · по ней присоединились: { $joined }
stores-closed-heading = Присоединились по ссылке до её закрытия · { $joined }
stores-joined-devices = { $count ->
    [one] { $count } { -device }
    [few] { $count } { -device-pl }
    [many] { $count } устройств
   *[other] { $count } { -device-pl }
 }
stores-open-link-row = Ссылка { -store }а
stores-open-link-note = Показать и скопировать
stores-anonymous-joined = Присоединилось { $when }
stores-anonymous-seen = Последний раз в сети { $when }
stores-anonymous-never-seen = Ещё не появлялось
stores-remove-devices = { $count ->
    [one] Удалить его
    [few] Удалить все { $count }
    [many] Удалить все { $count }
   *[other] Удалить все { $count }
 }
stores-footer-you = Вы владелец. У { -store }а всегда есть владелец.
stores-footer-remove-member = ↵ Отозвать доступ
stores-footer-cancel-invite = ↵ Отменить это приглашение
stores-footer-show-link = ↵ Показать ссылку, чтобы скопировать и отправить
stores-footer-remove-anonymous = ↵ Удалить это { -device }, известное только по времени присоединения
stores-footer-remove-all-anonymous = ↵ Удалить все { -device-pl }, которые присоединились по ссылке
stores-link-copied = Скопировано в буфер обмена, если терминал это позволяет, или выделите ссылку, чтобы скопировать.
stores-link-select = Выделите, чтобы скопировать.
stores-open-link-warning = Любой, у кого она есть, может присоединиться без имени, сколько угодно раз.
stores-open-link-lasts = Работает, пока { -store } снова не откроется только для приглашённых.
stores-invite-works-once = Работает один раз. { $expiry }.
stores-invite-preview-safe = Открытие в браузере показывает, как присоединиться, и не использует приглашение.
stores-confirm-join = Присоединиться к { $name }?
stores-confirm-leave = Покинуть { $name }?
stores-confirm-remove-member = Отозвать доступ у { $name }?
stores-confirm-cancel-invite = Отменить приглашение для { $name }?
stores-confirm-unpublish = Перестать предлагать { $name }?
stores-confirm-open = Разрешить любому, у кого есть ссылка, присоединиться к { $name }?
stores-confirm-close = Пускать в { $name } только приглашённых?
stores-confirm-remove-device = Удалить это { -device }?
stores-confirm-remove-devices = { $count ->
    [one] Удалить { -device }, присоединившееся по ссылке?
    [few] Удалить все { $count } { -device-pl }, присоединившиеся по ссылке?
    [many] Удалить все { $count } устройств, присоединившиеся по ссылке?
   *[other] Удалить все { $count } { -device-pl }, присоединившиеся по ссылке?
 }
stores-confirm-fallback = Подтверждение
stores-label-address = Адрес
stores-label-key = Ключ
stores-label-run-by = Управляют
stores-label-device = { -device-cap }
stores-key-matches-link = { $fingerprint }  (совпадает со ссылкой)
stores-key-matches-invite = { $fingerprint }  (совпадает с приглашением)
stores-join-open = Присоединиться может любой, у кого есть ссылка.
stores-join-anonymous = Вы присоединитесь без имени, и другое { -device } присоединится по той же ссылке.
stores-join-already = Уже присоединено. Повторное присоединение даст этому { -device } новый токен.
stores-join-device-note = Владелец { -store }а видит это имя. Введите другое, чтобы изменить его.
stores-leave-plugins = { -plugin-cap-pl }, установленные из него, остаются на этом компьютере и перестают обновляться.
stores-leave-revoked = Доступ этого { -device }а к { -store }у отозван.
stores-remove-member-devices = { $count ->
    [one] Их { -device } перестанет работать с этим { -store }ом сразу.
    [few] Их { $count } { -device-pl } перестанут работать с этим { -store }ом сразу.
    [many] Их { $count } устройств перестанут работать с этим { -store }ом сразу.
   *[other] Их { $count } { -device-pl } перестанут работать с этим { -store }ом сразу.
 }
stores-remove-member-plugins = { -plugin-cap-pl }, которые они уже установили, остаются на их компьютерах.
stores-cancel-invite-link = Ссылка перестанет работать.
stores-unpublish-members = Участники, которые его установили, сохранят его, и он перестанет обновляться.
stores-open-anyone = Любой, у кого есть ссылка { -store }а, сможет присоединиться без имени.
stores-open-link-new = Ссылка каждый раз новая, поэтому прежняя перестаёт работать.
stores-open-members-see = Участники увидят, что { -store } открыт.
stores-close-link = Ссылка перестанет работать, и никто новый не присоединится без приглашения.
stores-close-devices = { -device-cap-pl }, которые присоединились по ней, продолжат работать, пока вы не удалите их в разделе участников.
stores-anonymous-one-stops = Оно перестанет работать с этим { -store }ом сразу.
stores-anonymous-all-stop = Они перестанут работать с этим { -store }ом сразу.
stores-anonymous-plugins = { -plugin-cap-pl }, которые уже установлены, остаются на месте.
stores-anonymous-still-joins = Пока { -store } открыт, по ссылке по-прежнему может присоединиться любой.
stores-plugin-may-contact = Может обращаться к { $hosts }
stores-plugin-cannot = Он не может читать файлы, открывать сокеты или видеть ваше окружение.
stores-plugin-update-adds = Обновление добавляет хосты, к которым он может обращаться: { $hosts }.
stores-plugin-added-by-hand-note = { $id }.wasm уже есть в папке { -plugin }ов, добавлен вручную.
stores-plugin-other-store-note = Установлен из другого { -store }а, который держит его обновлённым.
stores-choice-install = Установить
stores-choice-update = Обновить до { $version }
stores-choice-replace = Заменить копию, добавленную вручную
stores-choice-remove = Удалить с этого компьютера
stores-choice-unpublish = Перестать предлагать в этом { -store }е
stores-choice-cancel = Отмена
stores-publish-new = Новый
stores-publish-published = Опубликован
stores-publish-replaces = Заменяет { $version }
stores-toast-invite-checked = Приглашение проверено, но что-то другое открыто. Вставьте его ещё раз, чтобы присоединиться.
stores-toast-nothing-to-publish = Для публикации в папке { -plugin }ов ничего нет. Выполните «{ $command }», чтобы узнать, где она.
stores-toast-that-is-you = Это вы. У { -store }а всегда есть владелец.
