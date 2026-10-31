party-status = { -watch-party-cap }: состояние
party-members = Участники
party-choose-a-source = Выберите { -source }
party-sources-none-listed = Ни один { -source } не содержит эпизод { $episode }. { -source-cap-pl } дают { -plugin-pl }, которые { -plugin-store } добавляет.
party-sources-found-line = { $count ->
    [one] Один { -source } содержит эпизод. Выбранный { -source } запускает { -watch-party }.
    [few] { -source-cap-pl } с эпизодом: { $count }. Выбранный { -source } запускает { -watch-party }.
    [many] { -source-cap-pl } с эпизодом: { $count }. Выбранный { -source } запускает { -watch-party }.
   *[other] { -source-cap-pl } с эпизодом: { $count }. Выбранный { -source } запускает { -watch-party }.
 }
player-could-not-start = Плеер не запустился. Попробуйте ещё раз или выберите другой { -source }.
player-file-unreadable = Не удалось прочитать этот файл. Выберите другой.
player-inhibit-reason = Воспроизведение эпизода
player-time-left = Осталось { $time }
player-note-waited = { $note } · { $seconds } с
player-starting-plain = Запуск
player-undo-tooltip = Отменить (U)
player-time-opening = { $clock } · Опенинг
player-time-ending = { $clock } · Эндинг
player-channels-mono = Моно
player-channels-stereo = Стерео
player-channels = { $count ->
    [one] Один канал
    [few] { $count } канала
    [many] { $count } каналов
   *[other] { $count } канала
 }
player-translation-label = { -subbed-cap } или { -dubbed }
