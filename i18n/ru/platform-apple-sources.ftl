sources-order-detail-edit = { -source-cap-pl } проверяются сверху вниз. Нажмите «Изменить», чтобы поменять порядок.
sources-update-to = Обновить до { $version }
sources-update-available-version = Доступна версия { $version }
stores-store-label = { -plugin-store-cap }
stores-checked-at = Проверено { $when }
stores-updates-available = { $count ->
    [one] Одно обновление
    [few] Обновлений: { $count }
    [many] Обновлений: { $count }
   *[other] Обновлений: { $count }
 }
stores-installed-version = Установлена версия { $version }
stores-plugins-count = { $count ->
    [one] Один { -plugin }
    [few] { -plugin-pl }: { $count }
    [many] { -plugin-pl }: { $count }
   *[other] { -plugin-pl }: { $count }
 }
stores-devices-count = { $count ->
    [one] Одно { -device }
    [few] { -device-pl }: { $count }
    [many] { -device-pl }: { $count }
   *[other] { -device-pl }: { $count }
 }
stores-replace-by-hand-detail = Версия { $version } из { $store } заменит копию, добавленную вручную.
stores-open-join-on-detail = Они подключаются без имени. Если выключить, ссылка перестанет работать. Если включить снова, будет создана новая.
stores-open-join-off-detail = Подключаются только приглашённые. Если включить, по ссылке сможет зайти любой, без имени.
stores-open-join-no-names = Имена не сохраняются.
stores-open-join-keep-working = Они продолжают работать, пока вы их не удалите.
stores-device-seen-at = Заходило { $when }
stores-device-not-seen = Больше не появлялось
stores-publish-offered = { -plugin-store } предлагает { $version }
stores-publish-not-yet = { -plugin-store } пока не предлагает его.
torrents-sheet-turn-on = Включить торренты
torrents-sheet-protect = Защита торрентов
torrents-sheet-edit-proxy = Изменить прокси
torrents-turn-on-button = Включить
torrents-edit-proxy = Изменить прокси
torrents-proxy-optional = Необязательно
torrents-warn-only-detail = { -app-name } будет предупреждать вместо паузы. Если VPN или прокси отключится, другие смогут увидеть ваш IP-адрес.
torrents-proxy-line-user = { $address } · как { $user }
party-leave-and-quit = Выйти и закрыть
party-leave-detail-window = Закрытие окна выведет вас из { $room }. Остальные продолжат без вас.
party-choose-source-title = Выберите { -source } для эпизода { $episode }
party-choose-source = Выбрать { -source }
tv-generic-name = Телевизор
tv-row-paired = Подключён
tv-row-paired-away = Подключён, сейчас не виден в этой сети
tv-row-not-paired = Ещё не подключён
tv-row-pair-hint = Подключается к этому телевизору
tv-picker-title-episode = { $title }, эпизод { $episode }
tv-connecting-generic = Подключение к телевизору
tv-section-network = В этой сети
tv-section-not-listed = Не в списке
tv-scan-code = Отсканировать код на телевизоре
tv-code-on = Код на { $name }
tv-nothing-playing = Ничего не воспроизводится
tv-remote-action = Показать пульт
tv-status-cant-reach = Нет связи с { $name }. Проверьте, что он включён и подключён к этой же сети.
tv-bar-cant-reach = Нет связи с { $name }
tv-bar-not-found = Не найден: { $name }
tv-paired-tvs = Подключённые телевизоры
tv-title-list = Телевизоры
tv-settings-footnote = Телевизор видит это { -device } под кодом { $code }. Если забыть телевизор здесь, он забудет и это { -device }.
tv-seen-just-now = только что
stores-link-send-once = Отправьте эту ссылку для { $who }. Она работает один раз, до { $until }.
stores-link-send-once-anyone = Отправьте эту ссылку тому, для кого она. Она работает один раз, до { $until }.
sources-remove-title = Удалить { $name }?
sources-remove-detail-device = { -device } потеряет его. { -plugin-store } сможет установить его снова.
