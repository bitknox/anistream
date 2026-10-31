sources-title = { -source-cap-pl }
sources-empty = { -source-cap-pl } пока не установлены. Подключите { -plugin-store }, чтобы добавить их.
plugin-risk-warning = { -plugin-cap-pl } пишут другие люди, и они запускаются в ваше { -device }. Устанавливайте только у тех, кому доверяете.
source-picker-title = { -source-cap } для этого названия
source-picker-automatic = Автоматически
source-picker-automatic-detail = Каждый эпизод из первого { -source }а, где он есть
source-picker-coverage = { $of ->
    [one] { $covered } из { $of } эпизода
    [few] { $covered } из { $of } эпизода
    [many] { $covered } из { $of } эпизодов
   *[other] { $covered } из { $of } эпизода
 }
source-note-off = Выключен
source-note-resting = Не отвечает, скоро попробуем снова
source-note-gone = Не установлен
source-note-ambiguous = Не удалось сопоставить название
source-note-held-back = Задержан проверкой VPN
source-picker-releases = { -release-cap-pl }
source-picker-chosen = Выбран
source-pin-unavailable = { -source-cap } { $name } сейчас недоступен, поэтому эпизоды этого названия ищут другие { -source-pl }.
episode-unsourced = Ни один { -source } его не содержит
sources-order = Порядок проверки
sources-order-detail = { -source-cap-pl } проверяются сверху вниз. Перетащите за ручку, чтобы изменить порядок.
sources-order-detail-show = { -source-cap-pl } проверяются по порядку, сверху вниз. Для названия можно закрепить свой { -source }, выбранный на его странице.
sources-drag-hint = Перетащите, чтобы изменить порядок
sources-move-up = Поднять
sources-move-down = Опустить
sources-load-failed = Не удалось загрузить { -source-pl }
sources-empty-title = { -source-cap-pl } пока не установлены
sources-empty-detail = Подключите { -plugin-store }, чтобы добавить { -source-pl }. Нужна пригласительная ссылка, которую присылает владелец { -store }а.
sources-torrents = Торренты
sources-remote = Удалённый
sources-origin-config = Задан в файле настроек
sources-origin-built-in = Встроен в приложение
sources-origin-left-store = { -plugin-store-cap }, который вы покинули
sources-origin-by-hand = Добавлен вручную
sources-origin-no-store = Не из { -store }а
sources-origin-from = Из «{ $name }»
sources-origin-in = Место: { $place }
sources-status-not-answering = Не отвечает
sources-status-held-back = Задержан
sources-status-slow = Медленный
sources-status-update = Доступно обновление
sources-detail-not-answering = Не отвечает. { $why }
sources-detail-not-answering-plain = Не отвечает.
sources-detail-held-back = Задержан, пока не пройдёт проверка соединения.
sources-detail-disabled = Выключен. Он сохраняет своё место и не проверяется.
sources-detail-gone = Этого { -source }а больше нет.
sources-detail-site = { -source-cap } не ответил.
sources-version = Версия
sources-response-time = Время ответа
sources-no-answer = Нет ответа
sources-not-checked = Ещё не проверен
sources-contacts-label = Серверы
sources-listed-by-store = Указан: { -plugin-store-cap }
sources-contacts-one = Обращается к { $first }
sources-contacts-line = { $more ->
    [one] Обращается к: { $first } и ещё { $more }
    [few] Обращается к: { $first } и ещё { $more }
    [many] Обращается к: { $first } и ещё { $more }
   *[other] Обращается к: { $first } и ещё { $more }
 }
sources-contacts-none = Серверов для связи не указано
sources-no-hosts = Нет серверов
sources-update-contacts-one = Это обновление также обращается к { $first }.
sources-update-contacts = { $more ->
    [one] Это обновление также обращается к: { $first } и ещё { $more }.
    [few] Это обновление также обращается к: { $first } и ещё { $more }.
    [many] Это обновление также обращается к: { $first } и ещё { $more }.
   *[other] Это обновление также обращается к: { $first } и ещё { $more }.
 }
sources-update-title = Обновить { $name }?
sources-replace-title = Заменить { $name }?
sources-update-detail = Версия { $version } из { $store } заменит { $from }.
sources-replace-detail = Версия { $version } из { $store } заменит копию, добавленную вручную. { $detail }.
sources-open-settings = Открыть настройки { -source }а
sources-torrent-source = Торрент-{ -source }
sources-source-order-row = Порядок проверки
sources-torrents-note = Торренты входят в { -app-name }, а не { -plugin }. Защита трафика, проверка соединения и раздача настраиваются в разделе «{ -source-cap-pl }» в настройках.
sources-rebuilding = { -source-cap-pl } настраиваются заново
sources-response-ms = { $ms } мс
