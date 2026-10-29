# Text only the Android phone and tablet apps show on the browse screens: the title page, search,
# Continue, library, calendar, downloads and the frame around them.

# Accessibility label of the account button in the top bar when someone is signed in.
# @only android
# @arg name: text
account-button-named = { $name }, accounts and settings

# Button on a notice that opens the plugin stores, where its cause can be fixed.
# @only android
# @max 24
notice-open-stores = Open { -plugin-store-pl }

# Line under Today in the calendar when My list is chosen and nothing on it airs in the next four weeks.
# @only android
# @max 60
calendar-empty-mine-ahead = Nothing on your list airs in the next 4 weeks.

# Line above the first day of the calendar: it goes no further back.
# @only android
# @max 60
calendar-floor-line = The calendar goes back 3 months.

# Line below the last day of the calendar: it goes no further ahead.
# @only android
# @max 60
calendar-ceiling-line = The calendar goes 4 weeks ahead.

# Line below the last day when no more broadcasts are scheduled before the calendar's end.
# @only android
# @max 60
calendar-end-line = Nothing later is scheduled yet.

# Activity line while earlier broadcasts load.
# @only android
# @max 60
calendar-loading-earlier-line = Loading earlier broadcasts

# Activity line while later broadcasts load.
# @only android
# @max 60
calendar-loading-later-line = Loading later broadcasts

# Line where earlier broadcasts would be when they could not be loaded.
# @only android
# @max 60
calendar-earlier-failed-line = Couldn't load earlier broadcasts

# Line where later broadcasts would be when they could not be loaded.
# @only android
# @max 60
calendar-later-failed-line = Couldn't load later broadcasts

# Button under the line saying nothing on the person's list airs: shows every airing show.
# @only android
# @max 30
calendar-show-all-airing = Show all airing

# What a screen reader says for the line that marks the time now. The time is already worded.
# @only android
# @arg time: text
calendar-now-label = Now, { $time }
