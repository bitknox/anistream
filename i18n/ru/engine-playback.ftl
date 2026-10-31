notice-reason = { $reason }
cause-no-sources = { -source-cap-pl } не настроены. Добавьте { -source } на экране «{ -source-cap-pl }».
notice-not-aired = Эпизод { $episode } выходит { $when }.
notice-not-aired-yet = Эпизод { $episode } ещё не вышел. Эпизод { $next } выходит { $when }.
cause-airs-today = сегодня в { $time }
cause-airs-tomorrow = завтра в { $time }
cause-airs-in-days = { $days ->
    [one] через { $days } день в { $time }
    [few] через { $days } дня в { $time }
    [many] через { $days } дней в { $time }
   *[other] через { $days } дня в { $time }
 }
cause-airs-on-date = на { $date } в { $time }
cause-no-playable-stream = { -source-cap } ответил, но воспроизводить нечего. Попробуйте другой.
cause-title-no-match = Не удалось сопоставить название: { $detail }
cause-source-gone = { -source-cap } { $source } больше недоступен. Выберите другой.
cause-release-failed = Не удалось запустить этот { -release }: { $detail }
cause-sources-timed-out = { -source-cap-pl } слишком долго не отвечали. Попробуйте ещё раз или выберите другой { -source }.
cause-no-source-has-episode = Ни один { -source } не содержит эпизод { $episode }: { $detail }
cause-source-cannot-download = Этот { -source } показывает эпизоды, но скачать их нельзя.
cause-no-source-could-list-show = Ни один { -source } не смог перечислить эпизоды этого аниме, поэтому скачать нельзя.
cause-subtitles-merged = { $tracks ->
    [one] { $tracks } дорожка субтитров добавлена в видео.
    [few] { $tracks } дорожки субтитров добавлены в видео.
    [many] { $tracks } дорожек субтитров добавлено в видео.
   *[other] { $tracks } дорожки субтитров добавлены в видео.
 }
cause-subtitles-left-apart = Субтитры остались отдельными файлами рядом с видео. { $reason }
cause-already-in-downloads = { $count ->
    [one] { $count } эпизод уже в загрузках.
    [few] { $count } эпизода уже в загрузках.
    [many] { $count } эпизодов уже в загрузках.
   *[other] { $count } эпизода уже в загрузках.
 }
status-looking-up-title = Ищем название…
status-loading-sources = Загружаем { -source-pl }…
status-searching-sources = Запрашиваем { -source-pl } об эпизоде { $episode }…
notice-player-failed-to-start = Не удалось запустить mpv. Проверьте, что mpv запускается сам по себе. ({ $reason })
notice-stream-no-video = Из этого потока не пришёл ни один кадр. Пробуем поток { $next } из { $total }.
notice-player-could-not-play = mpv не смог воспроизвести это. { $reason }
notice-player-exited-silently = mpv закрылся без воспроизведения и без объяснения причины. Попробуйте другой { -source }.
notice-player-not-playing = mpv не начал воспроизведение через { $seconds } с. У { -source }а может не быть данных. Проверьте, что mpv сам воспроизводит файлы.
notice-no-player = Эта версия { -app-name } не умеет воспроизводить видео.
notice-mpv-missing = { $binary } не найден. Установите mpv, чтобы воспроизводить эпизоды.
notice-mpv-old = mpv { $version } может воспроизводить, но плеер { -app-name } для кнопки пропуска, карточки «{ -up-next-cap }», меню и превью требует mpv версии 0.36 или новее.
notice-subtitles-unavailable = Субтитры этого { -source }а не удалось загрузить. Выберите другой { -source }, чтобы повторить попытку.
notice-opened-in-browser = Открыто в браузере.
notice-could-not-open = Не удалось открыть ссылку. { $reason }
notice-trailer-playing = Трейлер воспроизводится в mpv.
notice-trailer-opened = Трейлер открыт в браузере.
notice-trailer-failed = Не удалось открыть трейлер. { $reason }
notice-filler-skipped = Пропущен филлер. { -up-next-cap }: эпизод { $episode }.
notice-audio-now-subbed = Теперь аудио: { -subbed }.
notice-audio-now-dubbed = Теперь аудио: { -dubbed }.
notice-already-first-episode = Это уже первый эпизод.
notice-last-episode = Следующего эпизода нет. Эпизод { $episode } последний.
notice-no-next-episode = Следующего эпизода нет.
notice-no-previous-episode = Предыдущего эпизода нет.
notice-match-reset-failed = Не удалось сбросить сопоставление. { $reason }
notice-match-save-failed = Не удалось сохранить это сопоставление. { $reason }
notice-match-did-not-stick = Сопоставление не закрепилось. { -source-cap } мог измениться. Попробуйте выбрать { -source } ещё раз.
notice-source-search-empty = По запросу «{ $query }» ничего не найдено. Попробуйте другой запрос.
notice-source-pinned = { $source } будет использоваться для этого названия.
notice-source-automatic = Снова автоматический выбор { -source }а для этого названия.
notice-source-choice-failed = Не удалось сохранить выбор. { $reason }
notice-no-source-could-list = Ни один { -source } не смог перечислить эпизоды этого аниме: { $reason }
notice-episodes-failed = Не удалось загрузить эпизоды. { $reason }
notice-match-uncertain = { $source } нашёл несколько подходящих аниме и не может определить, какое из них нужное. Пока эпизоды взяты из AniList.
notice-continue-hide-failed = Не удалось убрать это из «{ -continue-cap }». { $reason }
notice-download-queued = Эпизод { $episode } из «{ $title }» добавлен в очередь.
notice-download-already-queued = Эпизод { $episode } из «{ $title }» уже в загрузках.
notice-download-starting-now = Запускаем эпизод { $episode } из «{ $title }».
notice-download-cancelled = Загрузка отменена.
notice-download-file-deleted = Загрузка и файл удалены.
notice-download-removed-file-gone = Загрузка удалена. Файла уже не было.
notice-download-removed-file-kept = Загрузка удалена, но её файл удалить не удалось. { $reason }
notice-download-removed = Загрузка удалена.
notice-downloads-cleared = { $count ->
    [one] Из списка удалена { $count } завершённая загрузка.
    [few] Из списка удалены { $count } завершённые загрузки.
    [many] Из списка удалено { $count } завершённых загрузок.
   *[other] Из списка удалены { $count } завершённые загрузки.
 }
notice-download-finished = Эпизод { $episode } из «{ $title }» загружен.
notice-download-finished-note = Эпизод { $episode } из «{ $title }» загружен. { $note }
notice-download-failed = Не удалось загрузить эпизод { $episode } из «{ $title }». { $reason }
notice-downloads-none-queued = Ничего не добавлено в очередь.
notice-downloads-none-queued-stopped = Ничего не добавлено в очередь. { $reason }
notice-downloads-none-new = Ничего нового не добавлено в очередь. { $existing }
notice-downloads-none-new-stopped = Ничего нового не добавлено в очередь. { $existing } { $reason }
notice-episodes-queued-existing = { $count ->
    [one] В очередь добавлен { $count } эпизод. { $existing }
    [few] В очередь добавлено { $count } эпизода. { $existing }
    [many] В очередь добавлено { $count } эпизодов. { $existing }
   *[other] В очередь добавлено { $count } эпизода. { $existing }
 }
notice-episodes-queued-stopped = { $count ->
    [one] В очередь добавлен { $count } эпизод, затем добавление остановилось. { $reason }
    [few] В очередь добавлено { $count } эпизода, затем добавление остановилось. { $reason }
    [many] В очередь добавлено { $count } эпизодов, затем добавление остановилось. { $reason }
   *[other] В очередь добавлено { $count } эпизода, затем добавление остановилось. { $reason }
 }
notice-episodes-queued-existing-stopped = { $count ->
    [one] В очередь добавлен { $count } эпизод, затем добавление остановилось. { $existing } { $reason }
    [few] В очередь добавлено { $count } эпизода, затем добавление остановилось. { $existing } { $reason }
    [many] В очередь добавлено { $count } эпизодов, затем добавление остановилось. { $existing } { $reason }
   *[other] В очередь добавлено { $count } эпизода, затем добавление остановилось. { $existing } { $reason }
 }
notice-keep-next-full = Недостаточно свободного места, чтобы сохранить следующие эпизоды в загрузках. Освободите место, чтобы загрузить больше.
party-needs-server = { -watch-party-cap }: сначала выберите сервер в настройке syncplay.server. Предлагаем { $server }, управляется { $operator }.
party-choose-server = { -watch-party-cap }: сначала выберите сервер. Предлагаем { $server }, управляется { $operator }.
notice-no-party = { -watch-party-cap-pl } недоступны в этой версии { -app-name }.
party-needs-room = { -watch-party-cap } требует комнату. Сначала выберите её.
party-server-lost = { -watch-party-cap }: связь с сервером потеряна. Пробуем снова.
party-rejoined = { -watch-party-cap } снова на связи.
party-left = Вы покинули { -watch-party }.
