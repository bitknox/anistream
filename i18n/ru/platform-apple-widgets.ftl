widget-episode-short = Эп. { $episode }
widget-episode-long = Эпизод { $episode }
widget-day-tomorrow = завтра
widget-next-line = { -up-next-cap }: { $when } · { $title } · Эпизод { $episode }
widget-next-line-short = { -up-next-cap }: { $when }, { $title } эп. { $episode }
widget-next-time = { -up-next-cap } { $time }
widget-airs-day = { $title }, эп. { $episode }, выходит { $day }.
widget-out-ago = Вышел { $time } назад
widget-in-time = Через { $time }
widget-airing-title-small = Сегодня
widget-nothing-airing-short = Ничего не выходит.
widget-open-to-update = Откройте { -app-name }, чтобы обновить.
widget-nothing-more-today = Сегодня больше ничего
widget-tomorrow-none = Завтра ничего не выходит
widget-tomorrow-first = Завтра: { $time } · { $title }
widget-tomorrow-more = Завтра: { $time } · { $title } и ещё { $count }
widget-continue-title-apple = { -continue-cap }
widget-continue-description = Следующий эпизод того, что вы смотрите.
widget-airing-description = Эпизоды на сегодня. Тайтлы из вашего списка отмечены.
widget-connect-detail-long = Подключите { $tracker } или запустите эпизод, чтобы увидеть здесь, что вы смотрите.
widget-connect-detail-short = Подключите { $tracker } или запустите эпизод, чтобы увидеть здесь.
widget-connect-tracker = Подключить { $tracker }
widget-caught-up-title-apple = Вы всё посмотрели
widget-caught-up-none = Больше нечего { -continue }.
widget-caught-up-next-apple = Следующее из вашего списка:
widget-line-left-apple = { $episode } · { $left }
widget-line-resume = Продолжить { $episode }
widget-action-resume = Продолжить { $episode }
widget-action-start = Смотреть { $episode }
widget-a11y-episode = эпизод { $episode }
widget-a11y-episode-marked = { $episode }, { $mark }
widget-a11y-resume = { $minutes ->
    [one] { $episode }, осталась одна минута
    [few] { $episode }, осталось { $minutes } минуты
    [many] { $episode }, осталось { $minutes } минут
   *[other] { $episode }, осталось { $minutes } минуты
 }
widget-a11y-start = начать { $episode }
widget-a11y-row = { $title }, { $what }
widget-a11y-airing = { $title }, эпизод { $episode }, { $time }
widget-a11y-then-watched = { $label }, просмотрено
widget-a11y-then-out = { $label }, вышел
widget-a11y-then-list = { $label }, в вашем списке
widget-out-now = Уже вышел
widget-airs-at = Выходит в { $time }
widget-aired-at = Вышел в { $time }
widget-a11y-airs-at = { $title }, эпизод { $episode }, выходит в { $time }
widget-a11y-out-now = { $title }, эпизод { $episode }, уже вышел
widget-downloaded-line = Эпизод { $episode } загружен
widget-downloaded-ready = Можно смотреть без интернета
widget-downloading-title = { $count ->
    [one] Загружается один эпизод
    [few] Загружаются { $count } эпизода
    [many] Загружаются { $count } эпизодов
   *[other] Загружаются { $count } эпизода
 }
widget-downloads-paused = На паузе
widget-downloads-then-more = { $title } эп. { $episode }, и ещё { $count }
widget-size-left = Осталось { $size }
widget-time-left = Осталось { $time }
widget-continues-when-open = Продолжится, когда вы откроете { -app-name }
widget-bytes-of = { $done } из { $total }
widget-speed = { $size }/с
widget-downloads-episode = Эпизод { $episode }
widget-a11y-downloaded = { $title }, эпизод { $episode }, загружен
widget-a11y-downloading = { $title }, эпизод { $episode }, { $percent }
widget-a11y-then-paused = { $label }, на паузе
widget-a11y-then-time-left = { $label }, осталось { $time }
widget-a11y-then-count = { $count ->
    [one] { $label }, загружается один эпизод
    [few] { $label }, загружаются { $count } эпизода
    [many] { $label }, загружаются { $count } эпизодов
   *[other] { $label }, загружаются { $count } эпизода
 }
widget-play-episode-short = Смотреть эп. { $episode }
widget-play-episode-long = Смотреть эпизод { $episode }
widget-resume-downloads = Возобновить загрузки
widget-pause-downloads = Приостановить загрузки
widget-paused-line = На паузе · осталось { $left }
widget-a11y-playback-paused = { $minutes ->
    [one] { $title }, эпизод { $episode }, на паузе, осталась одна минута
    [few] { $title }, эпизод { $episode }, на паузе, осталось { $minutes } минуты
    [many] { $title }, эпизод { $episode }, на паузе, осталось { $minutes } минут
   *[other] { $title }, эпизод { $episode }, на паузе, осталось { $minutes } минуты
 }
widget-a11y-playback-playing = { $minutes ->
    [one] { $title }, эпизод { $episode }, воспроизводится, осталась одна минута
    [few] { $title }, эпизод { $episode }, воспроизводится, осталось { $minutes } минуты
    [many] { $title }, эпизод { $episode }, воспроизводится, осталось { $minutes } минут
   *[other] { $title }, эпизод { $episode }, воспроизводится, осталось { $minutes } минуты
 }
