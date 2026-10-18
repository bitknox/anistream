-- A brief play or pause mark in the middle of the picture when playback is paused or resumed,
-- in anistream mode. A small flat panel with the icon, faded in and out: uosc's own indicator
-- grows as it fades, and nothing here scales.

local app = require("app")
local config = require("config")

local M = { id = "pause_flash", region = "center" }

local SHOW_FOR = 0.5
local SIZE = 64 -- the panel, square, at scale 1
local ICON_FONT = "MaterialIconsRound-Regular"

local until_time = 0
local timer = nil
local paused = mp.get_property_native("pause") == true

-- Compared with the start-up value, so the observer's first report is not a toggle.
mp.observe_property("pause", "bool", function(_, value)
    if not config.opts.uosc or (value == true) == paused then
        return
    end
    paused = value == true
    -- Not between episodes or under a loading card: there is no picture to pause.
    if app.player.idle then
        return
    end
    until_time = mp.get_time() + SHOW_FOR
    if timer then
        timer:kill()
    end
    timer = mp.add_timeout(SHOW_FOR + 0.01, app.changed)
    app.changed()
end)

function M.wants()
    return mp.get_time() < until_time
end

function M.measure(ctx)
    return ctx:px(SIZE), ctx:px(SIZE)
end

function M.draw(ctx, x, y, w, h)
    ctx:panel(x, y, x + w, y + h)
    local icon = { size = ctx:px(40), font = ICON_FONT, color = config.color.text }
    ctx:text(x + w / 2, y + h / 2, 5, paused and "pause" or "play_arrow", icon)
end

return M
