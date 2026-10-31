source-picker-seeders = { $count ->
    [one] Un compartidor
    [many] { $count } compartidores
   *[other] { $count } compartidores
 }
rate-per-second = { $size }/s
app-name = { -app-name }
