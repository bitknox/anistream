episode-short-dubbed = Эп. { $episode } · { -dubbed-cap }
episode-long-dubbed = Эпизод { $episode } · { -dubbed-cap }
downloads-live-paused = { $count ->
    [one] { $count } эпизод на паузе
    [few] { $count } эпизода на паузе
    [many] { $count } эпизодов на паузе
   *[other] { $count } эпизода на паузе
 }
downloads-live-lead = { $title }, эп. { $episode }
downloads-live-left = Осталось { $size }
downloads-live-progress = { $done } из { $total }
downloads-live-paused-one = На паузе
downloads-live-waiting = Ожидание
settings-seed-ratio-until = До соотношения { $ratio }
settings-upload-rate = { $size }/с
settings-coming-up-dubbed = { $count ->
    [one] { $names } сообщит, когда выйдет дубляж.
    [few] { $names } сообщат, когда выйдут дубляжи.
    [many] { $names } сообщат, когда выйдут дубляжи.
   *[other] { $names } сообщат, когда выйдут дубляжи.
 }
settings-coming-up-switched-off = { $count ->
    [one] Уведомление для { $names } выключено в собственных настройках.
    [few] Уведомления для { $names } выключены в собственных настройках.
    [many] Уведомления для { $names } выключены в собственных настройках.
   *[other] Уведомления для { $names } выключены в собственных настройках.
 }
settings-coming-up-opens = Открывает настройки тайтла.
torrents-check-note-warn = { -app-name } проверяет, откуда уходит трафик торрентов, когда торрент запускается и каждую минуту, пока он работает, и предупреждает, если проверка не прошла. Защищают вас VPN или прокси-сервер.
torrents-check-note-stop = { -app-name } проверяет, откуда уходит трафик торрентов, когда торрент запускается и каждую минуту, пока он работает, и останавливает торренты, если проверка не прошла. Защищают вас VPN или прокси-сервер.
torrents-protect-proxy-name = Прокси SOCKS5
torrents-protect-vpn-name = Системный VPN
torrents-warn-body = { -app-name } будет предупреждать вместо остановки. Если VPN или прокси-сервер не работает, другие люди могут видеть ваш IP-адрес.
torrents-proxy-as = как { $user }
title-facts-aired = Вышло { $dates }
title-facts-airing = Выходит с { $date }
title-facts-airs = Выйдет с { $date }
media-dates-range = с { $from } по { $to }
title-also-on-more = { $more ->
    [one] { $first } и ещё { $more }
    [few] { $first } и ещё { $more }
    [many] { $first } и ещё { $more }
   *[other] { $first } и ещё { $more }
 }
title-adult-mark = 18+
downloads-storage-used-free = { $used } занято · { $free } свободно
downloads-storage-used = { $used } занято
downloads-time-left = осталось { $time }
downloads-rate = { $size }/с
downloads-uploaded = Отдано { $size }
downloads-ratio-of = { $ratio } из { $goal }
downloads-group-summary = { $count ->
    [one] { $count } эпизод · { $size }
    [few] { $count } эпизода · { $size }
    [many] { $count } эпизодов · { $size }
   *[other] { $count } эпизода · { $size }
 }
episode-menu-download = Скачать
episode-menu-download-paused = Загрузка на паузе
reason-timeout-anilist = AniList слишком долго отвечает.
reason-timeout-server = Сервер слишком долго отвечает.
reason-busy-anilist = AniList занят. Попробуйте ещё раз через минуту.
reason-busy-server = Сервер занят. Попробуйте ещё раз через минуту.
reason-problem-anilist = В AniList произошла ошибка.
reason-problem-server = На сервере произошла ошибка.
reason-silent-anilist = AniList не отвечает.
reason-silent-server = Сервер не отвечает.
updates-installing-plain = Установка
updates-closes-as-installs = { -app-name } закроется во время установки.
updates-open-release = Открыть страницу
country-korea = Южная Корея
search-genres-picked = { $count ->
    [one] { $count } жанр
    [few] { $count } жанра
    [many] { $count } жанров
   *[other] { $count } жанра
 }
search-formats-picked = { $count ->
    [one] { $count } формат
    [few] { $count } формата
    [many] { $count } форматов
   *[other] { $count } формата
 }
search-statuses-picked = { $count ->
    [one] { $count } статус
    [few] { $count } статуса
    [many] { $count } статусов
   *[other] { $count } статуса
 }
filter-score-any = Любой
filter-years-from = { $year } и позже
filter-years-between = с { $from } по { $to }
filter-tag-rank-title = Ранг тега не ниже
filter-tag = Тег
search-filter-by = Добавить фильтр: { $name }
search-loading-genres = Загрузка жанров
search-paused-rate-limit = Поиск во время ввода приостановлен ненадолго, чтобы не превышать лимиты AniList. Нажмите кнопку поиска, чтобы искать сейчас.
action-clear-all = Очистить всё
filter-source-material = На основе
search-detail-episode = { $status } · эп. { $episode }
sources-use = Использовать { $name }
sources-move = Переместить { $name }
sources-reorder = Изменить порядок
sources-version-available = Версия { $version } доступна
sources-update-from = Версия { $version } из { -store }а { $store }.
sources-detail-off = Выключен. Он сохраняет место в списке и не проверяется, пока его снова не включат.
sources-fallback-title = { -source-cap }
reason-source-timeout = Ответ занял слишком много времени.
reason-source-busy = { -source-cap } занят. Попробуйте ещё раз через минуту.
reason-source-problem = У { -source }а возникла проблема.
reason-source-turned-away = { -source-cap } отклонил запрос.
reason-source-no-address = Адрес { -source }а не найден.
reason-source-unreachable = Не удалось подключиться к { -source }у.
stores-updates-waiting = { $count ->
    [one] { $count } обновление
    [few] { $count } обновления
    [many] { $count } обновлений
   *[other] { $count } обновления
 }
stores-checked-ago = проверен { $ago }
stores-preview-key-link = Ключ совпадает со ссылкой
stores-preview-key-invite = Ключ совпадает с приглашением
stores-key-verified = Проверено
stores-fallback-title = { -plugin-store-cap }
stores-fallback-plugin = { -plugin-cap }
stores-fallback-name = { -plugin-store }
stores-link-sheet-detail = Откройте эту ссылку с другого { -device-pl }, чтобы подключиться к { -store }у плагинов. Она работает один раз.
accounts-synced-ago = Синхронизировано { $ago }
accounts-card-line = { $tracker } · { $status }
accounts-card-description = { $name }, { $line }. Открывает «Аккаунты»
accounts-card-description-signed-out = Вход не выполнен. Открывает «Аккаунты»
accounts-signed-in-plain = Вход выполнен
settings-summary-notify-on = Новые эпизоды включены
settings-summary-notify-off = Новые эпизоды выключены
settings-summary-sources = { $count ->
    [one] { $count } { -source }
    [few] { $count } { -source }а
    [many] { $count } { -source }ов
   *[other] { $count } { -source }а
 }
settings-summary-no-sources = { -source-cap }ов пока нет
settings-summary-torrents-on = торренты включены
settings-summary-torrents-off = торренты выключены
library-storage-on-device = Занято: { $size }
library-storage-none = Ничего не загружено
library-detail-of = Эп. { $next } из { $total }
library-detail-next = Эп. { $next }
library-sort-last-watched = Последний просмотр
library-sort-current = Выбрано
settings-notify-permission-note = { -app-name } запрашивает разрешение при первом включении.
settings-notify-refused = Выключено: Android не разрешает уведомления { -app-name }.
settings-notify-system-title = Настройки уведомлений
settings-notify-system-detail = Звук, вибрация и экран блокировки
settings-titles-line-signed-out = Английский, пока вход в AniList не выполнен
settings-titles-line-account-language = { $language }, из аккаунта AniList
settings-titles-line-follows = Как в аккаунте AniList
settings-titles-line-english = Ромадзи, если английского названия нет
settings-titles-line-romaji = Японские названия латиницей
settings-titles-note-account = Берётся из аккаунта AniList, где задан язык: { $language }. Пока вход в AniList не выполнен, названия показываются на английском.
settings-titles-note-follows = Берётся из аккаунта AniList. Пока вход в AniList не выполнен, названия показываются на английском.
settings-titles-note-english-missing = У { $title } нет английского названия в AniList, поэтому оно остаётся в ромадзи.
settings-titles-note-english = Английские названия, как их указывает AniList, а там, где их нет, ромадзи.
settings-titles-note-romaji = Японские названия латиницей, как их указывает AniList.
settings-index-none = Не задано
settings-stores-with-updates = { $stores } · { $updates }
settings-seeding-best-effort = По возможности: раздача идёт, только пока открыт { -app-name }.
settings-downloads-count = { $count ->
    [one] { $count } эпизод
    [few] { $count } эпизода
    [many] { $count } эпизодов
   *[other] { $count } эпизода
 }
settings-anilist-footer = Прогресс и изменения списков синхронизируются с AniList. { -library-cap } показывает списки AniList.
accounts-sign-in-again-button = Войти снова
accounts-review-differences = { $count ->
    [one] Проверить { $count } различие
    [few] Проверить { $count } различия
    [many] Проверить { $count } различий
   *[other] Проверить { $count } различия
 }
settings-licence-title = Лицензия
settings-licence-value = MIT, открытый код, без коммерческого использования
settings-typefaces-value = Roboto Flex и Zen Kaku Gothic New, по лицензии SIL Open Font License 1.1
settings-libraries-value = JNA и AndroidX, по лицензии Apache License 2.0
settings-report-bug-value = Открывает GitHub в браузере
downloads-storage-share-description = Загрузки занимают { $size }
settings-aside-notifications = Одно уведомление, когда выходит новый эпизод отслеживаемого тайтла: название тайтла и номер эпизода, но не название эпизода. Тайтл можно отключить в его собственных настройках.
settings-aside-downloads = Загрузки хранятся в собственном хранилище { -app-name }, которое другие приложения не читают. Удаление приложения удаляет и загрузки.
settings-aside-connection-check = Проверка соединения
settings-aside-sources = { -source-cap-pl } проверяются по порядку, сверху вниз. Тайтл может иметь свой { -source }, который выбирается на его странице.
settings-upscaling-description = Один и тот же кадр: слева оригинал 480p, справа с { $mode }
settings-upscaling-source = Оригинал 480p
settings-upscaling-note-off-original = Оригинал 480p, вписанный в экран без обработки.
torrents-sheet-turn-on-title = Включить торренты
torrents-sheet-protect-title = Защитить трафик торрентов
torrents-sheet-turn-on-button = Включить
torrents-sheet-host = Хост
torrents-sheet-port = Порт
torrents-sheet-proxy-auth-note = Имя пользователя и пароль нужны, только если прокси-сервер их запрашивает.
torrents-sheet-edit-proxy-title = Изменить прокси
title-facts-also-on = Также на
title-fetch-failed-detail = AniList не ответил.
continue-resize-queue = Изменить размер очереди
continue-go-to-title = Перейти к тайтлу
continue-short-left = { $episode } · осталось { $left }
continue-airing-out = Эп. { $episode } · { $time } · вышло { $ago }
continue-airing-in = Эп. { $episode } · { $time } · через { $countdown }
calendar-line-out = { $episode } вышел { $ago }
calendar-line-in = { $episode } через { $countdown }
nav-accounts-and-settings-user = { $user }, аккаунты и настройки
episode-unaired-label = Эпизод { $number }, { $title }. Ещё не вышел
episode-unaired-label-plain = Эпизод { $number }. Ещё не вышел
episode-unaired-airs-label = Эпизод { $number }, { $title }. Выйдет { $airs }, ещё не вышел
episode-unaired-airs-label-plain = Эпизод { $number }. Выйдет { $airs }, ещё не вышел
tv-sheet-on-network = В этой сети
tv-sheet-not-listed = Нет в списке
tv-sheet-scan = Сканировать код на телевизоре
tv-sheet-code-on = Код на { $name }
tv-sheet-pairing-button = Сопряжение
tv-sheet-connecting = Подключение к телевизору
tv-sheet-type-code = Введите код
tv-sheet-volume = Громкость на телевизоре
tv-sheet-volume-down = Уменьшить громкость
tv-sheet-volume-up = Увеличить громкость
tv-sheet-volume-value = { $value } из 100
tv-sheet-nothing-playing = Ничего не воспроизводится
tv-sheet-position = Позиция на телевизоре
tv-sheet-position-state = { $at } из { $total }
tv-sheet-request-titled = { $title }, эпизод { $episode }
tv-sheet-request-plain = Эпизод { $episode }
tv-sheet-footer = Телевизор показывает это { -device } с кодом { $code }. Если удалить телевизор здесь, он забудет и это { -device }.
tv-sheet-title-fallback = Телевизор
episode-line-detail = Эп. { $number } · { $detail }
calendar-day-label = { $day }
calendar-day-label-today = { $day }, сегодня
calendar-day-label-airing = { $day }, выходят эпизоды
calendar-day-label-today-airing = { $day }, сегодня, выходят эпизоды
continue-show-airing-in-calendar = Показать выходящие сегодня в календаре
continue-show-season-in-seasonal = Показать этот сезон в разделе «Сезон»
search-searching-filters = Ищем { $filters }
episode-number-plain = Эпизод { $number }
episode-number-title-text = Эпизод { $number } · { $title }
player-position-state = { $at } из { $total }
player-sheet-finding = Ищем { -source-pl }…
player-sheet-none = { -source-cap }ов для этого эпизода не найдено.
player-sheet-episodes-loading = Список эпизодов скоро появится.
player-dual-audio = { -subbed-cap } и { -dubbed }
notif-new-episodes = { $count ->
    [one] { $count } новый эпизод
    [few] { $count } новых эпизода
    [many] { $count } новых эпизодов
   *[other] { $count } новых эпизода
 }
notif-hidden-text = Новый эпизод
notif-episode-out = Эпизод { $episode } вышел.
notif-episode-out-dubbed = Эпизод { $episode } вышел, { -dubbed }.
notif-episode-out-next = Эпизод { $episode } вышел. Следующий по порядку: эпизод { $next }.
notif-episode-out-dubbed-next = Эпизод { $episode } вышел, { -dubbed }. Следующий по порядку: эпизод { $next }.
device-name-phone = Android-телефон
device-name-tablet = Android-планшет
tv-session-starting = Запускается на { $tv }
tv-session-line = { $line } · { $tv }
widget-line-left = { $episode } · осталось { $minutes } мин
widget-line-of = Эп. { $episode } из { $total }
widget-continue-title = { -continue-cap }
widget-caught-up-title = Всё посмотрено
widget-caught-up = Всё, что вышло, уже посмотрено.
widget-caught-up-next = { $title } эп. { $episode } выйдет { $airs }.
widget-day-date = { $day }
widget-play-dubbed-label = Воспроизвести эпизод { $episode }, { -dubbed }: { $title }
tv-action-resume = Продолжить эпизод { $episode }
tv-action-resume-dubbed = Продолжить эпизод { $episode } · { -dubbed-cap }
tv-action-out = Воспроизвести эпизод { $episode } (выйдет { $day })
tv-action-not-out = Воспроизвести эпизод { $episode } (ещё не вышел)
tv-out-today = сегодня
tv-hero-resume = Продолжить { $episode }
tv-hero-start = Воспроизвести { $episode }
