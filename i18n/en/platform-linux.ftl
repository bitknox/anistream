# Under a release in the source picker: how many people share it right now.
# @only linux
# @arg count: int
# @max 24
source-picker-seeders-linux = { $count ->
    [one] One seeder
   *[other] { $count } seeders
 }
