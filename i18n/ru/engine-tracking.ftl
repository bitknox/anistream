notice-signed-in = Вы вошли в { $tracker }.
notice-signed-in-keychain = Вы вошли в { $tracker }. Токен сохранён в системной связке ключей.
notice-signed-in-file = Вы вошли в { $tracker }. Связки ключей нет, поэтому токен хранится в файле, доступном только вам, на этом устройстве.
notice-signed-in-unsaved = Вы вошли в { $tracker }, но токен не сохранён, поэтому в следующий раз придётся войти снова. { $reason }
notice-token-not-saved = Не удалось сохранить токен входа. Попробуйте войти снова. { $reason }
notice-sign-in-failed = Не удалось войти в { $tracker }. Попробуйте ещё раз. { $reason }
notice-sign-in-stopped = Вход в { $tracker } остановился до завершения. Попробуйте ещё раз. { $reason }
notice-token-exchange-failed = Не удалось получить токен от { $tracker }. Попробуйте войти снова. { $reason }
notice-browser-not-opened = Не удалось открыть браузер для входа. Проверьте, что он установлен, и попробуйте ещё раз. { $reason }
notice-sign-in-not-waiting = Вход в { $tracker } сейчас не ожидает ответа. Начните вход заново.
notice-sign-in-wrong-request = Вход в { $tracker } не вернулся с этого запроса. Начните вход заново.
notice-sign-in-no-token = Вход в { $tracker } вернулся без токена. Попробуйте ещё раз.
notice-mal-client-missing-build = Не удалось завершить вход в MyAnimeList: в этой сборке нет клиента для него.
notice-kitsu-signs-in-on-accounts = Kitsu входит через экран «Аккаунты».
notice-sign-in-unsupported = Вход в { $tracker } пока не поддерживается.
notice-sign-in-needs-redirect = Вход в { $tracker } требует перенаправления в браузере, которое это приложение принять не может.
notice-anilist-implicit-flow = Этот способ входа в AniList работает только в приложении { -app-name }. Чтобы войти здесь, задайте для { $setting } значение «loopback» в файле настроек.
notice-anilist-client-missing = AniList требует идентификатор клиента и секрет. Задайте { $client } и { $secret } в файле настроек, затем войдите снова. Чтобы получить адрес для входа, выполните { -app-name } --login-url.
notice-anilist-paste-flow = Вставить код здесь нельзя. Закройте программу и запустите { -app-name } --login.
notice-mal-client-missing = MyAnimeList требует идентификатор клиента. Зарегистрируйте приложение с типом «other» на myanimelist.net/apiconfig, затем задайте { $setting } в файле настроек.
status-waiting-for-redirect = Ожидание перенаправления от { $tracker }…
notice-mal-no-refresh-token = MyAnimeList не выдал токен обновления. Войти придётся снова примерно через месяц.
notice-client-id-missing = Сначала задайте { $setting } в файле настроек.
notice-signed-out = Вы вышли из { $tracker }.
notice-signed-out-queue = { $count ->
    [one] Вы вышли из { $tracker }. Одно изменение в очереди сохранено для следующего входа.
    [few] Вы вышли из { $tracker }. { $count } изменения в очереди сохранены для следующего входа.
    [many] Вы вышли из { $tracker }. { $count } изменений в очереди сохранены для следующего входа.
   *[other] Вы вышли из { $tracker }. { $count } изменения в очереди сохранены для следующего входа.
 }
notice-sign-out-failed = Не удалось выйти из { $tracker }. Попробуйте ещё раз. { $reason }
notice-not-saved-because = Не удалось сохранить настройку. { $cause }
notice-protection-saved = Защита торрентов сохранена.
notice-protection-not-checked = Не удалось проверить защиту торрентов. { $cause }
notice-proxy-password-not-kept = Не удалось сохранить пароль прокси. Введите его ещё раз, чтобы попробовать сохранить. { $reason }
notice-show-setting-refused = Не удалось сохранить настройку. { $value } не подходит для { $setting }.
notice-show-not-reset = Не удалось сбросить настройки этого тайтла. { $reason }
notice-show-no-such-subtitles = Не удалось сохранить выбор субтитров. Для этого тайтла такие субтитры раньше не использовались.
notice-torrents-waiting-for-check = Торренты ждут, пока проверка подключения снова пройдёт.
notice-mapping-data-refreshed = Данные сопоставления обновлены.
notice-mapping-data-failed = Не удалось обновить данные сопоставления ({ $datasets }). Повтор через { $wait }. { $cause }
notice-episode-aired = { $title }, серия { $episode }, вышла, пока вас не было.
notice-episodes-aired = { $count ->
    [one] Вышла новая серия тайтла, который вы смотрите. Она есть в календаре.
    [few] Вышли { $count } новые серии тайтлов, которые вы смотрите. Они есть в календаре.
    [many] Вышли { $count } новых серий тайтлов, которые вы смотрите. Они есть в календаре.
   *[other] Вышли { $count } новые серии тайтлов, которые вы смотрите. Они есть в календаре.
 }
notice-update-available = Вышла версия { $tag }. Чтобы установить её, выполните { -app-name } --update.
cause-confirm-no-protection = Сначала подтвердите, что хотите обойтись без защиты.
cause-confirm-warn-only = Сначала подтвердите режим «только предупреждать».
cause-proxy-needs-address = Для прокси нужны хост и порт.
cause-proxy-address-unusable = Адрес прокси не подходит для проверки.
cause-server-unreachable = Не удалось связаться с сервером.
