title-heading = Тайтл
title-nothing-selected-note = Ничего не выбрано
title-nothing-selected = Ничего не выбрано. Выделите тайтл и повторите действие.
title-trailer-off-page = Откройте страницу тайтла, чтобы найти трейлер.
title-recommendations-off-page = Откройте страницу тайтла, чтобы увидеть рекомендации.
title-seasons-off-page = Откройте страницу тайтла, чтобы увидеть связанные сезоны.
title-no-trailer = Для этого тайтла трейлер не указан.
title-no-recommendations = Рекомендаций для этого тайтла нет.
title-no-seasons = Связанных сезонов для этого тайтла нет.
title-pin-off-page = Откройте страницу тайтла, чтобы выбрать { -source } для него.
title-no-sources-yet = { -source-cap-pl } пока не подключены. Попробуйте ещё раз через минуту.
title-status-needs-tracker = Статус в списке хранится на { -tracker }е, но ни один не подключён. Чтобы войти, используйте :accounts.
title-rating-needs-tracker = Оценки хранятся на { -tracker }е, но ни один не подключён. Чтобы войти, используйте :accounts.
title-status-set = Установлен статус: { $status }. В очереди на отправку: { $trackers }.
title-rated = Оценка: { $score }/10. В очереди на отправку: { $trackers }.
title-status-watching = Смотрю
title-status-planning = Запланировано
title-status-completed = Просмотрено
title-status-paused = На паузе
title-status-dropped = Брошено
title-status-completed-hint = Ставит прогресс на последний эпизод
title-rating-10 = Шедевр
title-rating-9 = Отлично
title-rating-8 = Очень хорошо
title-rating-7 = Хорошо
title-rating-6 = Неплохо
title-rating-5 = Средне
title-rating-4 = Слабо
title-rating-3 = Плохо
title-rating-2 = Ужасно
title-rating-1 = Отвратительно
title-season = Сезон { $number }
title-relation-prequel = Приквел
title-relation-parent = Оригинал
title-relation-sequel = Сиквел
title-source-automatic = Автоматически (лучший доступный { -source })
title-source-failing = Не работает
title-release-first = Тот { -release }, который { -source } ставит первым
title-release-seeders = { $count ->
    [one] { $count } сид
    [few] { $count } сида
    [many] { $count } сидов
   *[other] { $count } сидов
 }
title-available-on = Доступно на: { $services }
title-up-next = { -up-next-cap }: эпизод { $next }
title-progress-aired = { $done } из { $aired } вышедших
title-progress-watched = Просмотрено: { $done }
episodes-heading = Эпизоды
episodes-column-title = Название
episodes-column-run = Длит.
episodes-column-watched = Просмотр
episodes-done = Готово
episodes-airs-in = Через { $span }
episodes-kind-canon = Канон
episodes-kind-mixed = Смешанный
episodes-kind-filler = Филлер
episodes-kind-anime-canon = Аниме-канон
episodes-filter-all = Все эпизоды
episodes-filter-unwatched = Непросмотренные
episodes-filter-no-filler = Без филлеров
episodes-showing-all = Показаны все эпизоды.
episodes-showing-unwatched = Показаны непросмотренные эпизоды.
episodes-showing-no-filler = Показаны эпизоды без филлеров.
episodes-dub-through = Дубляж до эпизода { $through }
episodes-dub-complete = { -dubbed-cap }
episodes-empty-filtered = Ничего не подходит под фильтр
episodes-empty-filtered-hint = Фильтр скрыл все эпизоды. Нажмите { $key }, чтобы изменить его.
episodes-empty-no-source = Эпизодов пока нет
episodes-empty-no-source-hint = AniList пока не показывает эпизоды этого тайтла, и ни один { -source } не готов. Нажмите { $key }, чтобы открыть экран «{ -source-cap-pl }».
episodes-empty-unaired = Ещё ничего не вышло
episodes-empty-unaired-hint = Эпизод { $episode } выйдет через { $span }.
episodes-empty-none-listed = Список эпизодов пуст
episodes-empty-none-listed-hint = { -source-cap } не показал эпизоды для этого тайтла. Нажмите { $search }, чтобы найти нужное совпадение вручную, или { $pick }, чтобы выбрать другой { -source }.
episodes-no-releases = Для этого эпизода нет { -release }ов для выбора.
episodes-add-source-to-download = Перед загрузкой добавьте { -source }. Нажмите { $key }, чтобы открыть экран «{ -source-cap-pl }».
episodes-mark-none-listed = Нет эпизодов для отметки.
episodes-mark-nothing-before = Перед первым эпизодом ничего нет.
episodes-mark-already-watched = Все эпизоды до { $episode } уже просмотрены.
episodes-marked-watched = { $count ->
    [one] Отмечен как просмотренный { $count } эпизод.
    [few] Отмечены как просмотренные { $count } эпизода.
    [many] Отмечены как просмотренные { $count } эпизодов.
   *[other] Отмечены как просмотренные { $count } эпизодов.
 }
episodes-range-invalid = Не удалось прочитать диапазон. Укажите один эпизод (4), диапазон (1-12) или все эпизоды с номера (7-).
episodes-range-empty = В этом диапазоне нет эпизодов. Попробуйте другой.
episodes-queueing = { $count ->
    [one] В очередь на загрузку добавлен { $count } эпизод.
    [few] В очередь на загрузку добавлены { $count } эпизода.
    [many] В очередь на загрузку добавлены { $count } эпизодов.
   *[other] В очередь на загрузку добавлены { $count } эпизодов.
 }
episodes-match-off-page = Откройте страницу тайтла, чтобы исправить совпадение.
episodes-match-nothing = Ничего не выбрано, чтобы найти совпадение заново.
episodes-matched = Сопоставлено с { $title }.
episodes-release-dual = { -subbed-cap } и { -dubbed }
episodes-release-dub = { -dubbed-cap }
episodes-release-current-pick = Текущий выбор
