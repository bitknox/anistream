tv-pair-request-held = { $phone } хочет подключиться. Код { $code }
tv-add-store-to-play = Добавьте { -plugin-store }, чтобы смотреть
tv-title-page = Страница тайтла
tv-continue-offline-failed = Сейчас { -app-name } не может подключиться к AniList.
tv-continue-empty = Пока нет ничего, чтобы { -continue }.
tv-continue-browse = Открыть этот сезон
tv-open-title-page = Открыть страницу тайтла
tv-sort-title = Сортировка
tv-on-my-list = В моём списке
tv-seasonal-unlisted = Из сезона { $season } в ваших списках пока ничего нет.
tv-load-failed-named = Не удалось загрузить { $name }.
tv-library-empty = Список { $name } пуст.
tv-lists-signed-out = Ваши списки хранятся в { -tracker }. Войдите, чтобы увидеть их здесь.
tv-connect-tracker = Подключить { -tracker }
tv-change-status = Изменить статус
tv-add-to-planning = Добавить в «Запланировано»
tv-progress-of = { $done } / { $total }
tv-progress-unknown = { $done } / ?
tv-calendar-signed-out-note = Войдите, чтобы видеть только свои тайтлы
tv-calendar-time-was = { $time } (было { $was })
tv-player-undo-pill = { $what } · Отменить
tv-player-episode-row = E{ $number } · { $title }
tv-player-episode-playing = Воспроизводится
tv-player-up-next-line = { -up-next-cap }: E{ $number } · { $title }
tv-player-up-next-bare = { -up-next-cap }: E{ $number }
tv-player-caption-note = Оформление берётся из настроек субтитров системы
tv-search-typing-phone = Ввод с телефона
tv-search-system-keyboard = Системная клавиатура
tv-search-space = Пробел
tv-search-season-in = Сезон { $year } года
tv-search-genres-offline = Жанры загрузятся, когда телевизор подключится к сети
tv-seasons-position = { $at } из { $total }
tv-seasons-count = { $count ->
    [one] { $count } сезон
    [few] { $count } сезона
    [many] { $count } сезонов
   *[other] { $count } сезона
 }
tv-episode-mark-up-to-here = Отметить просмотренными до этого эпизода
tv-card-label-watched = { $label }, просмотрено
tv-section-this-tv = Этот телевизор
tv-info-from = Откуда
tv-info-state = Состояние
tv-store-revoked = Больше не пускает этот телевизор
tv-seeding-note = Раздача на этом телевизоре идёт по возможности: только пока открыт { -app-name }.
tv-sign-out-title = Выйти из { $name }?
tv-account-not-signed-in = Вход не выполнен
tv-account-signed-in-as = Вход выполнен: { $name }
tv-remote-name-label = Имя этого телевизора для телефонов
tv-remote-findable = Разрешить телефонам в этой сети находить этот телевизор
tv-phone-used-today = Использовался сегодня
tv-phone-used-yesterday = Использовался вчера
tv-phone-paired-today = Подключён сегодня
tv-phone-paired-yesterday = Подключён вчера
tv-watch-next-row = Показывать { -app-name } на главном экране
tv-watch-next-name = { -continue-cap }
tv-image-cache-clear = Очистить кэш изображений
tv-image-cache-clearing = Очистка
tv-licence-page-encryption = Шифрование страниц
tv-licence-page-encryption-value = noble-ciphers, по лицензии MIT
tv-update-available = Доступно
tv-update-waiting-install = Ожидание разрешения Android на установку
tv-update-auto-check = Проверять обновления автоматически
tv-update-auto-check-detail = Раз в день, { -release-pl } { -app-name }
tv-phone-used-days = { $count ->
    [one] Использовался { $count } день назад
    [few] Использовался { $count } дня назад
    [many] Использовался { $count } дней назад
   *[other] Использовался { $count } дня назад
 }
tv-phone-paired-days = { $count ->
    [one] Подключён { $count } день назад
    [few] Подключён { $count } дня назад
    [many] Подключён { $count } дней назад
   *[other] Подключён { $count } дня назад
 }
tv-update-installed = Установлена версия { $version }
tv-page-not-found-text = Этой страницы больше нет. Отсканируйте код на телевизоре ещё раз.
tv-page-not-allowed-text = Эта страница принимает только форму, которую показывает. Отсканируйте код на телевизоре ещё раз.
tv-page-next-link = он покажет { -plugin-store }, чтобы присоединиться
tv-page-next-torrent = проверьте данные там и нажмите «Включить»
tv-page-next-proxy = проверьте прокси там и нажмите «Сохранить»
tv-page-next-signin = он сейчас выполнит вход в { $tracker }
tv-page-open-link = Открыть { $tracker }
tv-page-step-open = { $link } и разрешите доступ для { -app-name }.
tv-page-step-copy = Скопируйте показанный код или весь адрес, на который открылась страница.
tv-page-step-paste = Вставьте его здесь и отправьте на телевизор.
tv-page-password = Имя пользователя или почта и пароль от { $tracker }. Они передаются на телевизор, который входит в { $tracker } и сохраняет только токен доступа.
tv-page-link = Вставьте пригласительную ссылку. Телевизор покажет { -plugin-store }, прежде чем подключиться.
tv-page-torrent = { -torrent-index }: { -app-name } запрашивает торренты.
tv-page-torrent-proxy = { -torrent-index }: { -app-name } запрашивает торренты. Их трафик идёт через прокси SOCKS5.
tv-page-proxy = Трафик торрентов идёт через прокси SOCKS5. { $kept } Телевизор проверит прокси, прежде чем сохранить его.
tv-page-label-code = Код
tv-page-title-torrents = Торренты для { -app-name }
tv-page-send = Отправить на телевизор
tv-page-scan-again = Отсканируйте код ещё раз
tv-page-sent-title = Отправлено на телевизор
tv-page-no-answer = Телевизор не ответил. Проверьте, что он включён и подключён к той же сети, что и этот телефон, и отправьте ещё раз.
tv-page-closes-once-sent = Страница закроется после отправки.
tv-use-phone = Использовать телефон
tv-type-it = Ввести вручную
tv-qr-label-proxy = QR-код страницы: прокси
tv-qr-label-index = QR-код страницы: { -torrent-index }
tv-scan-torrent-proxy = Отсканируйте камерой телефона, затем заполните { -torrent-index } и прокси на открывшейся странице.
tv-scan-proxy = Отсканируйте камерой телефона, затем измените прокси на открывшейся странице.
tv-scan-torrent = Отсканируйте камерой телефона, затем заполните { -torrent-index } на открывшейся странице.
tv-field-user-optional = Имя пользователя, если требуется
tv-field-password-optional = Пароль, если требуется
tv-qr-label-link = QR-код страницы со ссылкой
tv-preview-key = Ключ { $fingerprint }
tv-preview-more = и ещё { $count }
tv-already-joined = Уже подключено
tv-joining = Подключение
tv-signin-done-as = Вход в { $name } выполнен: { $user }
tv-signin-done = Вход в { $name } выполнен
tv-signin-not-on-tv = { $name } не выполняет вход на телевизоре.
tv-signin-did-not = { $name } не вошёл в аккаунт. Проверьте данные и отправьте их ещё раз со страницы.
tv-signin-device-steps = На телефоне отсканируйте QR-код или откройте адрес и введите этот код.
tv-signin-asking-code = Запрашиваем код
tv-signin-qr-page = QR-код страницы входа
tv-signin-qr-activation = QR-код адреса активации
tv-qr-label-pairing = QR-код для подключения
tv-qr-label-default = QR-код
tv-show-back-to-defaults = Сброшено к значениям по умолчанию
tv-card-progress-spoken = просмотрено { $percent }
tv-discussion-footer = Чтобы ответить, откройте обсуждение на телефоне.
tv-discussion-count-read-only = { $count ->
    [one] { $count } комментарий · только чтение
    [few] { $count } комментария · только чтение
    [many] { $count } комментариев · только чтение
   *[other] { $count } комментария · только чтение
 }
tv-discussion-likes = { $count ->
    [one] { $count } лайк
    [few] { $count } лайка
    [many] { $count } лайков
   *[other] { $count } лайка
 }
tv-discussion-select-spoiler = Нажмите OK, чтобы показать спойлер
updates-unknown-sources-path = Настройки › Приложения › Безопасность и ограничения › Неизвестные источники
tv-remote-default-name = Телевизор { -app-name }
