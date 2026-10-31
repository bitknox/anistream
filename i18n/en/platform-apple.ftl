# Mac menu bar item. Title Case, as Apple's guidelines ask for the menu bar only.
menu-show-library = Show { -library-cap }

# Mac menu bar item that opens the source picker for the title on screen. Title Case, as the
# menu bar's items are. Everywhere else the picker is called source-picker-title.
# @only apple
# @max 28
menu-source-for-title = { -source-cap } for This Title

# Mac menu bar item that opens the sheet for choosing which of a source's titles this show is.
# Title Case, as the menu bar's items are. Everywhere else it is match-fix.
# @only apple
# @max 28
menu-fix-match = Fix This Match…

# Under a release in the source picker: how many people share it right now.
# @only apple
# @arg count: int
# @max 24
source-release-seeders = { $count ->
    [one] One seeder
   *[other] { $count } seeders
 }
