## Licensed services: streaming services the viewer pays for. anistream opens them and never plays them.

# Title of the Licensed services page.
licensed-title = Licensed services

# Line at the top of the page.
licensed-body = { -app-name } can open titles in streaming services you pay for. It never plays them itself. Links come from AniList.

# Label of the region choice.
licensed-region = Region

# Line under the region choice.
licensed-region-line = Hides links AniList marks for other countries. Most links name no country, so choose your services too.

# Heading over the services the viewer pays for.
licensed-yours = Your services

# Row that shows the rest of the services.
# @arg count: int
licensed-show-all = Show all { $count }

# Switch for the line that names services the viewer does not use.
licensed-show-others = Show other services on title pages

# Line under that switch.
licensed-show-others-line = A line under the buttons names services you don't use.

# A Sources row's line: the services and the region.
# @arg services: text
# @arg region: text
licensed-row-line = { $services } · { $region }

# A Sources row's line before any service is chosen.
licensed-row-none = Choose the services you pay for

# Line while AniList's list of services has not arrived.
licensed-loading = Asking AniList for its list of services

# Main button of a title that plays only in a service.
# @arg service: text
# @max 32
title-watch-on = Watch on { $service }

# Menu item that opens a title in one of the viewer's services.
# @arg service: text
title-watch-on-item = Watch on { $service }

# Line under the buttons of a title that plays here and is also in a service.
# @arg services: text
title-also-on-services = Also on { $services }

# Line under the buttons of a title that is in none of the viewer's services.
# @arg services: text
title-listed-on = Not found in your { -source-pl }. Listed on { $services }.

# Link that opens the Licensed services page.
title-choose-services = Choose your services

# Button that undoes marking as watched.
licensed-mark-undo = Undo

# Tooltip on the glyph that says a row opens outside the app.
licensed-opens-outside = Opens in your browser

# Line under the buttons of a title that is also listed in services the viewer does not use.
# @arg services: text
title-others-line = Also listed on { $services }
