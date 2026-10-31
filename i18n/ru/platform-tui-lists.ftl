lists-terminal-too-small = Нужен терминал большего размера
downloads-screen-title = Загрузки
downloads-column-title = Название
downloads-column-state = Состояние
downloads-column-progress = Прогресс
downloads-empty-heading = Очередь пуста
downloads-empty-hint = Нажмите { $key } на тайтл или серию, чтобы поставить их в очередь на загрузку.
downloads-row-episode = Серия { $episode }
downloads-row-episode-ahead = Серия { $episode } · заранее
downloads-note-sizes = { $downloaded } из { $total }
downloads-note-waiting-metadata = Ждём метаданные
downloads-note-torrents-off = Торренты выключены
downloads-note-waiting-start = Ждёт запуска
downloads-state-queued = В очереди
downloads-state-active = Загружается
downloads-state-paused = Пауза
downloads-state-done = Загружено
downloads-state-failed = Ошибка
downloads-detail-ahead = Загружено заранее, удалится через день после просмотра
downloads-detail-with-ahead = { $detail } · загружено заранее, удалится через день после просмотра
downloads-detail-paused-at = Приостановлено на { $downloaded } из { $total }, продолжится с этого места
downloads-detail-paused-early = Приостановлено до начала загрузки
downloads-detail-torrents-off = Для загрузок нужны торренты, а они выключены (нажмите { $key }, чтобы открыть настройки)
downloads-detail-queued = В очереди (начнётся, когда освободится слот загрузки)
downloads-detail-waiting-metadata = Ждём метаданные торрента
downloads-seeding-heading = Раздача после просмотра
downloads-seeding-detail = { $ratio } из { $goal } · осталось { $left } · отдано { $uploaded }
downloads-failed-no-reason = Загрузка не удалась, причина не указана.
downloads-not-finished = Загрузка ещё не завершена.
accounts-screen-title = Аккаунты
accounts-badge-sign-in = { $tracker } { $mark } войти
accounts-needs-sign-in = Нужно снова войти в { $tracker }. Нажмите { $key }, чтобы открыть аккаунты.
accounts-conflicts-found = { $count ->
    [one] Найдено { $count } расхождение синхронизации. Нажмите { $key }, чтобы его проверить.
    [few] Найдено { $count } расхождения синхронизации. Нажмите { $key }, чтобы их проверить.
    [many] Найдено { $count } расхождений синхронизации. Нажмите { $key }, чтобы их проверить.
   *[other] Найдено { $count } расхождения синхронизации. Нажмите { $key }, чтобы их проверить.
 }
accounts-conflicts-none = Расхождений синхронизации нет.
accounts-no-tracker = { -tracker-cap } не подключён. Нажмите { $key }, чтобы открыть аккаунты.
accounts-finish-signing-in = Завершите вход в { $tracker }
accounts-step-open = 1. Откройте   { $url }
accounts-step-enter = 2. Введите
accounts-code-expires-seconds = Истекает через { $seconds } с
accounts-code-expires-minutes = Истекает через { $minutes } мин
accounts-code-expired = Код истёк
accounts-column-tracker = { -tracker-cap }
accounts-column-state = Состояние
accounts-column-queued = В очереди
accounts-column-token = Токен
accounts-empty-heading = { -tracker-cap-pl } не включены
accounts-empty-hint = Добавьте { -tracker } в настройку { $setting } файла config.toml. История просмотра работает и без них.
accounts-state-sign-in-again = Войти снова
accounts-state-connected = Подключено
accounts-state-not-connected = Не подключено
accounts-pull-failed-today = Не удалось загрузить список в { $time }: { $error }
accounts-pull-failed-dated = Не удалось загрузить список { $date } в { $time }: { $error }
accounts-pull-failed = Не удалось загрузить список: { $error }
accounts-token-files = Токены хранятся в файлах с правами 0600, так как системная связка ключей недоступна.
accounts-enter-sign-out = ↵ Выйти
accounts-enter-sign-in = ↵ Войти
accounts-action-sign-in-again = Войти снова
accounts-action-sign-out = Выйти
accounts-action-sign-in = Войти
accounts-rows-empty = { -tracker-cap-pl } не включены. Добавьте { -tracker } в настройку { $setting } файла config.toml.
accounts-row-signed-in-as = Вы вошли как { $user }
accounts-row-connected = Подключено
accounts-row-not-connected = Не подключено
accounts-row-queued = { $count ->
    [one] { $count } в очереди
    [few] { $count } в очереди
    [many] { $count } в очереди
   *[other] { $count } в очереди
 }
accounts-row-queued-held = { $queued } в очереди, { $held } ждут привязки к id трекера
accounts-conflict-keep-mine = Оставить моё
accounts-conflict-keep-theirs = Взять из трекера
accounts-conflict-row = { $title }  ·  { $field }: у меня { $local } / у трекера { $remote }
lists-library-title = { -library-cap }
lists-remove-wrong-list = Скрывать тайтлы можно только в списке «{ -continue-cap }».
lists-continue-removed = { $title } скрыт из списка «{ -continue-cap }». Если смотреть его снова, он вернётся.
lists-nothing-selected = Ничего не выбрано.
lists-search-prompt = Поиск: { $query }
lists-load-failed = Не удалось загрузить список
lists-loading = Загрузка…
lists-empty-press-enter = Нажмите ↵, чтобы искать
lists-empty-type-to-search = Введите запрос для поиска
lists-empty-browse-filtered = Нажмите ↵, чтобы просмотреть с этими фильтрами.
lists-empty-browse-popular = Нажмите ↵, чтобы посмотреть самые популярные.
lists-empty-no-matches = Ничего не найдено
lists-empty-fewer-filters = Попробуйте меньше фильтров.
lists-empty-shorter-query = Попробуйте более короткий запрос или название в ромадзи.
lists-empty-nothing-watched = Вы ещё ничего не смотрели
lists-empty-ways-on = Нажмите { $seasonal }, чтобы открыть этот сезон, или { $search }, чтобы искать.
lists-empty-nothing-to-continue = Нечего { -continue }
lists-empty-hidden-come-back = Скрытые тайтлы вернутся, когда вы начнёте их смотреть. Нажмите { $seasonal }, чтобы открыть этот сезон, или { $search }, чтобы искать.
lists-empty-nothing-here = Здесь пока пусто
lists-library-no-account = Аккаунт не подключён
lists-library-works-without = История просмотра работает и без аккаунта. { -library-cap } повторяет список, который ведёт { -tracker }.
lists-library-enable-tracker = Добавьте { -tracker } в настройку { $setting } файла config.toml, затем нажмите { $key }, чтобы открыть аккаунты.
lists-library-sign-in = Нажмите { $key }, чтобы открыть аккаунты и войти.
lists-segment-watching = Смотрю
lists-segment-planning = В планах
lists-segment-completed = Просмотрено
lists-segment-paused = Отложено
lists-segment-dropped = Брошено
lists-episode = Серия { $episode }
lists-episode-dubbed = { $episode } · { -dubbed-cap }
lists-score = { $score }%
lists-row-resume-percent = Серия { $episode } · { $percent }%
lists-row-resume-clock = Серия { $episode } · { $clock }
lists-episode-out = Серия { $episode } вышла
lists-episode-in = Серия { $episode } через { $countdown }
lists-next-in = Следующая через { $countdown }
lists-episode-out-ago = Серия { $episode } вышла { $countdown } назад
lists-episode-out-just-now = Серия { $episode } вышла только что
lists-time-in = Через { $countdown }
lists-time-ago = { $countdown } назад
lists-time-just-now = Только что
lists-time-approximate = ~{ $time }
lists-day-today = Сегодня
lists-day-yesterday = Вчера
lists-day-tomorrow = Завтра
lists-preview-continue-percent = { -continue-cap }: серия { $episode } на { $clock } ({ $percent }%)
lists-preview-continue = { -continue-cap }: серия { $episode } на { $clock }
lists-progress-aired = { $total ->
    [one] Вышло { $done } из { $total } серии
    [few] Вышло { $done } из { $total } серий
    [many] Вышло { $done } из { $total } серий
   *[other] Вышло { $done } из { $total } серий
 }
lists-progress-watched = { $count ->
    [one] Просмотрена { $count } серия
    [few] Просмотрено { $count } серии
    [many] Просмотрено { $count } серий
   *[other] Просмотрено { $count } серий
 }
lists-filter-genre = Жанр
lists-filter-year = Год
lists-filter-season = Сезон
lists-filter-format = Формат
lists-filter-status = Статус
lists-filter-sort = Сортировка
lists-filter-any = Любой
lists-sort-relevance = По релевантности
lists-season-winter = Зима
lists-season-spring = Весна
lists-season-summer = Лето
lists-season-fall = Осень
lists-format-tv = ТВ
lists-format-tv-short = ТВ-короткометражка
lists-format-movie = Фильм
lists-format-ova = OVA
lists-format-ona = ONA
lists-format-special = Спецвыпуск
lists-format-music = Музыка
lists-status-airing = Выходит
lists-status-finished = Завершено
lists-status-upcoming = Скоро выйдет
lists-sort-popularity = Популярность
lists-sort-score = Оценка
lists-sort-trending = В тренде
lists-sort-newest = Новые
