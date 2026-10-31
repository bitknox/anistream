source-picker-seeders = { $count ->
    [one] Один сидер
    [few] { $count } сидера
    [many] { $count } сидеров
   *[other] { $count } сидера
 }
rate-per-second = { $size }/с
app-name = { -app-name }
