source-picker-seeders-linux = { $count ->
    [one] { $count } сидер
    [few] { $count } сидера
    [many] { $count } сидеров
   *[other] { $count } сидеров
 }
sidebar-show = Показать боковую панель
activity-continue = Загружаем, что вы смотрите
activity-calendar = Загружаем календарь
activity-episodes = Загружаем серии
activity-sources = Проверяем { -source-pl }
activity-downloads = Загружаем список загрузок
activity-browse = Загружаем названия
size-per-second = { $size }/с
time-now = Сейчас
time-just-now = Только что
spoken-hours = { $count ->
    [one] { $count } час
    [few] { $count } часа
    [many] { $count } часов
   *[other] { $count } часов
 }
spoken-minutes = { $count ->
    [one] { $count } минута
    [few] { $count } минуты
    [many] { $count } минут
   *[other] { $count } минут
 }
spoken-seconds = { $count ->
    [one] { $count } секунда
    [few] { $count } секунды
    [many] { $count } секунд
   *[other] { $count } секунд
 }
continue-row-caught-up-airs = Всё просмотрено · E{ $episode } выходит { $when }
continue-row-caught-up-not-out = Всё просмотрено · E{ $episode } ещё не вышла
language-name-english = Английский
language-name-japanese = Японский
language-name-spanish = Испанский
language-name-portuguese = Португальский
language-name-french = Французский
language-name-german = Немецкий
language-name-italian = Итальянский
language-name-russian = Русский
language-name-arabic = Арабский
language-name-indonesian = Индонезийский
language-name-korean = Корейский
language-name-chinese = Китайский
seasonal-format-all = Все
seasonal-format-ona-ova = ONA и OVA
title-open-named = Открыть «{ $title }»
continue-row-spoken = { $title }, { $detail }
continue-preview = Предпросмотр
up-next-airs-at = Выходит в { $time }
