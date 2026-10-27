# Under a release in the source picker: how many people share it right now.
# @only windows
# @arg count: int
# @max 24
source-picker-seeders = { $count ->
    [one] One seeder
   *[other] { $count } seeders
 }
