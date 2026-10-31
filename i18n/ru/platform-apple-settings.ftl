settings-download-cellular = Загрузка по мобильной сети
settings-syncplay = { -watch-party-cap-pl } через Syncplay
settings-syncplay-server = Сервер Syncplay
settings-syncplay-room = Комната Syncplay
settings-start-full-screen = Запускать в полноэкранном режиме
settings-hook-command = Команда
settings-no-match = Такой настройки нет.
settings-results = Результаты
settings-upscaling-source-apple = 480p { -source }
settings-upscaling-compare = Исходный 480p { -source } рядом с вариантом «{ $mode }»
settings-upscaling-note-none = Без обработки { -source } просто масштабируется до размера окна.
settings-advanced-none = Дополнительных настроек нет. Плеер работает с собственными настройками { -app-name }.
settings-aside-check-title = Проверка соединения
settings-aside-sources-apple = { -source-cap-pl } проверяются по порядку, сверху вниз. У сериала может быть свой { -source }, который выбирается на его странице.
settings-aside-advanced = Настройка здесь может отменить параметр воспроизведения, например апскейлинг или ночной режим. Большинству эта панель не нужна.
settings-quality-detail = Высокое качество выбирает лучшую версию, которую предлагает { -source }, и занимает больше места.
settings-merge-subtitles-detail = Субтитры остаются с серией, куда бы ни переместился файл.
settings-keep-seeding-detail-mac = Загрузка, которая продолжает раздачу, хранится в кеше { -app-name }, а не в папке загрузок.
settings-keep-seeding-detail = Загрузка, которая продолжает раздачу, хранится в кеше { -app-name }.
settings-watched-downloads = Просмотренные загрузки
settings-watched-one = { $episodes } из { $title } · { $size }
settings-watched-many = { $episodes } · { $size }
settings-delete-watched-title = { $count ->
    [one] Удалить { $count } просмотренную загрузку?
    [few] Удалить { $count } просмотренные загрузки?
    [many] Удалить { $count } просмотренных загрузок?
   *[other] Удалить { $count } просмотренных загрузок?
 }
settings-delete-watched-detail = Файлы удаляются здесь. Серии, которые ещё не просмотрены, остаются.
settings-folder-default = Хранится в кеше торрентов
settings-storage-unwatched = Ещё не просмотрено
settings-storage-used = Загрузки занимают { $size }.
settings-storage-used-free = Загрузки занимают { $size }. Свободно на диске: { $free }.
settings-torrents-warn-note = Режим «Предупреждать» только просит подтвердить действие, как и режим «Без защиты».
settings-source-count = { $count ->
    [one] { $count } { -source }
    [few] { $count } { -source }а
    [many] { $count } { -source }ов
   *[other] { $count } { -source }ов
 }
settings-accounts-sync-note = Прогресс и изменения списков синхронизируются с { $tracker }. { -library-cap } и «Мой список» в календаре показывают ваши списки { $tracker }.
settings-conflicts = { $count ->
    [one] { $count } название отличается от { $tracker }
    [few] { $count } названия отличаются от { $tracker }
    [many] { $count } названий отличаются от { $tracker }
   *[other] { $count } названий отличаются от { $tracker }
 }
settings-review = Проверить
settings-changes-waiting = { $count ->
    [one] { $count } изменение ждёт синхронизации
    [few] { $count } изменения ждут синхронизации
    [many] { $count } изменений ждут синхронизации
   *[other] { $count } изменений ждут синхронизации
 }
settings-synced-just-now = Синхронизировано только что
settings-synced-ago = Синхронизировано { $ago }
settings-privacy-note = Что отправляет { -app-name } и что хранится только здесь
settings-read-online = Читать онлайн
settings-presence-note = Расширенный статус показывает обложку, полосу прогресса и ссылку на сериал на { $tracker }. Discord сам загружает обложку с { $tracker }. Для этого нужно показывать название.
settings-presence-cover = Обложка
settings-presence-button = Открыть на { $tracker }
settings-presence-nothing = Смотрю аниме
settings-presence-episode-of = Серия { $episode } из { $total }
settings-presence-episode = Серия { $episode }
settings-presence-on = Ваш профиль Discord показывает это, пока воспроизводится серия.
settings-presence-off = Выключено. Когда включено, ваш профиль Discord показывает это, пока воспроизводится серия.
settings-notify-permission = { -app-name } запрашивает разрешение при первом включении.
settings-week-empty = Ничего не выходит в ближайшие 7 дней у сериалов, которые вы отслеживаете.
settings-week-loading = Загрузка недели…
settings-system-app-mac = Системные настройки
settings-system-app-ios = Настройки
settings-notify-not-allowed = Для { -app-name } не разрешено в «{ $settings }»
settings-week-dub-one = { $show } присылает уведомление, когда выходит дубляж.
settings-week-dub-many = { $shows } присылают уведомления, когда выходят их дубляжи.
settings-week-off-one = { $show } выключено в его собственных настройках.
settings-week-off-many = { $shows } выключены в их собственных настройках.
settings-week-row-opens = Нажатие на строку открывает настройки этого сериала.
settings-when-today = Сегодня { $time }
settings-when-tomorrow = Завтра { $time }
settings-when-day = { $day } { $time }
settings-week-row-label = { $title }, серия { $episode }, { $when }
settings-week-row-hint = Открывает настройки этого сериала
settings-notify-explain-on = Одно уведомление, когда выходит новая серия сериала из списка «Смотрю». Время выхода берётся из { $tracker }. Сначала действует настройка самого сериала.
settings-notify-explain-off = Выключено. Уведомляют только сериалы, включённые в их собственных настройках.
settings-preview-show = Сериал, который вы смотрите
settings-preview-words = Серия 9 вышла.
settings-preview-label = Уведомление: { $show }, { $words }
settings-show-notices-label = Новые серии: { $value }
settings-this-show = Этот сериал
settings-show-off-not-watching = Выключено · не смотрю
settings-option-natural = Естественный · ArtCNN
settings-option-sharp = Резкий · Anime4K
settings-option-mpv = Ваши настройки mpv
settings-option-until-ratio = До соотношения { $ratio }
settings-option-unlimited = Без ограничения
settings-option-per-second = { $size }/с
settings-option-standard = Стандартное
settings-option-higher = Высокое
settings-option-adaptive = Адаптивная
settings-option-immersive = Погружающая
settings-torrents-value-on = Торренты включены
settings-torrents-value-off = Торренты выключены
settings-seed-ratio-detail = Как получится: раздача идёт только пока открыт { -app-name }.
settings-review-differences = { $count ->
    [one] Проверить { $count } различие
    [few] Проверить { $count } различия
    [many] Проверить { $count } различий
   *[other] Проверить { $count } различий
 }
settings-swipe-resume = Продолжить
settings-swipe-retry = Повторить
settings-stores-value-updates = { $count } · { $updates }
show-settings-row-label = { $label }:
show-settings-unnamed-track = Дорожка без названия
settings-title-line-signed-out = Английский, пока вы не вошли в { $tracker }
settings-title-line-from-account = { $language }, из вашего аккаунта { $tracker }
settings-title-line-english = Ромадзи, если английского названия нет
settings-title-line-romaji = Японские названия латиницей
settings-title-note-signed-out = Пока вы не вошли в { $tracker }, названия показываются на английском.
settings-title-note-follows = Следует языку из вашего аккаунта { $tracker }. { $then }
settings-title-note-follows-set = Следует языку из вашего аккаунта { $tracker }, где задано: { $language }. { $then }
settings-title-note-kept = У «{ $title }» нет английского названия на { $tracker }, поэтому оно остаётся в ромадзи.
settings-title-note-english = Английские названия, как их указывает { $tracker }, а там, где их нет, ромадзи.
settings-title-note-romaji = Японские названия латиницей, как их указывает { $tracker }.
settings-folder-choose = Выбрать
settings-trailers-row = Показывать трейлеры
settings-trailers-note-apple = Трейлеры без звука идут с YouTube, который видит ваш IP-адрес. Не воспроизводятся в режиме низкого объёма данных, при включённом уменьшении движения и в режиме энергосбережения.
