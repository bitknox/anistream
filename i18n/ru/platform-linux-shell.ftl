shell-open-stores = Открыть { -store }ы
shell-restart = Перезапустить
shell-view-release = Открыть
shell-no-background = { -app-name } здесь не может работать в фоне, поэтому загрузки останавливаются при закрытии окна
shell-main-menu = Главное меню
shell-connect-tracker = Подключить { $tracker }
shell-changes-waiting = { $count ->
    [one] Одно изменение ждёт синхронизации
    [few] { $count } изменения ждут синхронизации
    [many] { $count } изменений ждут синхронизации
   *[other] { $count } изменения ждут синхронизации
 }
shell-tracker-synced = { $tracker } · синхронизировано
shortcuts-group-lists = Списки
shortcuts-go-to-section = Перейти к разделу по номеру
shortcuts-show = Сочетания клавиш
shortcuts-close-window = Закрыть окно
shortcuts-quit = Выйти
shortcuts-play-selected = Воспроизвести выбранный тайтл
shortcuts-seek-5 = Назад или вперёд на 5 секунд
shortcuts-seek-30 = Назад или вперёд на 30 секунд
shortcuts-seek-10 = Назад или вперёд на 10 секунд
shortcuts-volume = Громкость вверх или вниз
shortcuts-skip = Пропустить вступление или титры
shortcuts-undo-skip = Отменить автоматический пропуск
shortcuts-speed = Быстрее или медленнее
shortcuts-answer-card = Ответить на карточку в углу
shortcuts-escape = Закрыть карточку, мини-плеер, полноэкранный режим, затем плеер
close-stop-and-close = Остановить и закрыть
close-body-downloading = { $count ->
    [one] Одна серия загружается или ждёт очереди. Продолжить загрузку после закрытия окна или остановить?
    [few] { $count } серии загружаются или ждут очереди. Продолжить загрузку после закрытия окна или остановить их?
    [many] { $count } серий загружаются или ждут очереди. Продолжить загрузку после закрытия окна или остановить их?
   *[other] { $count } серии загружаются или ждут очереди. Продолжить загрузку после закрытия окна или остановить их?
 }
close-body-seeding = { $count ->
    [one] Одна серия раздаётся. Продолжить раздачу после закрытия окна или остановить её?
    [few] { $count } серии раздаются. Продолжить раздачу после закрытия окна или остановить их?
    [many] { $count } серий раздаются. Продолжить раздачу после закрытия окна или остановить их?
   *[other] { $count } серии раздаются. Продолжить раздачу после закрытия окна или остановить их?
 }
close-body-both = Серии: { $downloading } загружаются или ждут, { $seeding } раздаются. Продолжить их после закрытия окна или остановить?
tv-status-not-found = Не найдено: { $name }
tv-connecting-title = Подключение к телевизору
tv-code-expired = Код истёк
tv-row-pairs-with = Сопряжение с этим телевизором
tv-row-away = Сопряжён, не виден в этой сети
tv-row-new = Ещё не сопряжён
tv-problem-announce = { $heading }. { $problem }
tv-none-found-announce = Телевизоры не найдены. { $detail }
tv-title-episode = { $title }, серия { $episode }
tv-episode = Серия { $episode }
tv-episode-titled = Серия { $episode } · { $title }
tv-fingerprint-note = Телевизор показывает это { -device } под кодом { $code }. Если удалить здесь телевизор, он забудет и это { -device }.
tv-paired-seen-now = { $paired } · в сети только что
tv-track-choice = { $row }, { $track }
shell-device-label = Компьютер с Linux
