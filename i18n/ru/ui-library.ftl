queue-episodes-left = { $count ->
    [one] Осталась { $count } серия в «{ $title }»
    [few] Осталось { $count } серии в «{ $title }»
    [many] Осталось { $count } серий в «{ $title }»
   *[other] Осталось { $count } серий в «{ $title }»
 }
episodes-count = { $count ->
    [one] { $count } серия
    [few] { $count } серии
    [many] { $count } серий
   *[other] { $count } серий
 }
progress-episodes = { $total ->
    [one] { $watched } из { $total } серия
    [few] { $watched } из { $total } серии
    [many] { $watched } из { $total } серий
   *[other] { $watched } из { $total } серий
 }
duration-days-hours = { $days } д { $hours } ч
group-in-progress = В процессе
group-new-this-week = Новое на этой неделе
continue-empty-title = Вы ещё ничего не смотрели
continue-empty-detail = Выберите что-нибудь в разделе «Сезонное» или найдите название.
continue-empty-action = Открыть «Сезонное»
continue-load-failed = Не удалось загрузить { -continue-cap }
continue-heading = { -continue-cap }
continue-airing-today = Выходит сегодня
continue-this-season = Этот сезон
continue-hide = Скрыть из «{ -continue-cap }»
continue-hidden = Скрыто из «{ -continue-cap }».
continue-drop = Перенести в «Брошено»
continue-more-for = Ещё для «{ $title }»
continue-row-resume = Продолжить E{ $episode } · { $clock }
continue-row-resume-dubbed = Продолжить E{ $episode } { -dubbed } · { $clock }
continue-row-start = Смотреть E{ $episode }
continue-row-start-dubbed = Смотреть E{ $episode } { -dubbed }
continue-row-start-aired = Смотреть E{ $episode } · вышла { $ago }
continue-row-caught-up = Всё просмотрено · E{ $episode } { $when }
continue-row-caught-up-plain = Всё просмотрено
continue-widget-connect = Подключите AniList, чтобы видеть список здесь.
continue-caught-up = Вы всё посмотрели.
title-button-resume = Продолжить серию { $episode } · { $time }
title-button-play = Смотреть серию { $episode }
title-button-add-source = Добавьте { -source }, чтобы смотреть
library-status-watching = Смотрю
library-status-planning = Запланировано
library-status-completed = Завершено
library-status-paused = На паузе
library-status-dropped = Брошено
library-sort-updated = Последнее изменение
library-sort-title = Название
library-sort-score = Оценка
library-sort-button = Сортировка: { $sort }
library-sort-label = Сортировать по полю «{ $sort }»
library-signed-out-title = Ваши списки берутся из AniList
library-signed-out-detail = Войдите, чтобы синхронизировать просмотренное и то, на чём вы остановились.
library-signed-out-action = Войти в AniList
library-list-empty = В списке «{ $list }» ничего нет
library-list-empty-detail = Сериалы, которые вы добавите в этот список на AniList, появятся здесь.
library-list-load-failed = Не удалось загрузить список «{ $list }»
library-loading = Загружаем ваши списки
library-add-to-list = Добавить в список
library-add-planning = Добавить в «Запланировано»
library-added-planning = Добавлено в «Запланировано».
library-on-your-list = В вашем списке
library-in-your-list = В списке
progress-episodes-short = E{ $done } из { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Смотреть E{ $episode } { -dubbed } · вышла { $ago }
up-next-heading = { -up-next-cap } для вас
up-next-starts = Начнётся { $date }
up-next-reason-new-season = Новый сезон
up-next-reason-planning = В списке «Запланировано»
up-next-reason-recommended = Потому что вы досмотрели «{ $title }»
up-next-reason-recommended-plain = Рекомендовано для вас
up-next-because-detail = Рекомендовано на AniList тем, кому понравился «{ $title }»
up-next-reason-airing-today = Выходит сегодня
up-next-add-planning = Добавить в «Запланировано»
up-next-not-interested = Не интересно
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = «{ $title }» не показывается в разделе «{ -up-next-cap }» для вас.
up-next-hidden-until-airs = «{ $title }» скрыто до выхода.
up-next-broadcast = Серия { $episode } · { $when }
up-next-out = вышла { $ago }
