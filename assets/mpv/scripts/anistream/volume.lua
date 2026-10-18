-- Volume, in anistream mode: a speaker button and a short slider in the control row, drawn
-- like the seek bar (a quiet track, amber for the level), with the level beside it.
--
-- uosc's own volume element is a tall filled slab painted in its single foreground colour, so
-- it is switched off (uosc-opts.conf) and this takes its place on the left of the row, next to
-- play/pause, where the row is empty and the position never shifts with the number of
-- buttons uosc shows on the right.
--
-- When the level changes while the controls are hidden (a key, the wheel, the host), a small
-- pill with the same speaker, track and number shows for a moment where the control would be.
-- The speaker toggles mute; muted, it shows the crossed-out speaker and the track goes quiet.

local app = require("app")
local config = require("config")
local draw = require("draw")
local overlay = require("overlay")
local screen = require("screen")
local timeline = require("timeline")

local M = { id = "volume", region = "canvas" }

local FLASH_FOR = 1.2
local LINE = 3
local LINE_ACTIVE = 6
-- uosc's icon font, which anistream mode bundles; the glyphs are Material Icons ligatures.
local ICON_FONT = "MaterialIconsRound-Regular"

local volume = mp.get_property_native("volume") or 100
local muted = mp.get_property_native("mute") == true
local flash_until = 0
local flash_timer = nil
local dragging = false
local slider = nil -- the slider's span from the last frame, for turning x into a level
-- Which form is on screen: the pill, or the control in the row. Decided while the element
-- wants to be shown and kept through its fade-out, because the overlay keeps drawing a fading
-- element: deciding per frame turned the row's control into the pill for the length of the
-- fade every time the controls hid, which read as the pill flashing.
local pill = false
-- The form last drawn, published for tests: "row", "pill", or nil before the first frame.
local drawn = nil

local function enabled()
    return config.opts.uosc
end

local function flashing()
    return mp.get_time() < flash_until
end

-- The inline control is part of the control row; the pill is for when the row is hidden.
local function inline()
    return timeline.awake() or dragging
end

function M.wants()
    local showing = enabled() and app.player.has_audio and not app.player.idle
        and (inline() or flashing())
    if showing then
        pill = not inline()
    end
    return showing
end

local function flash()
    if not enabled() or inline() then
        return
    end
    flash_until = mp.get_time() + FLASH_FOR
    if flash_timer then
        flash_timer:kill()
    end
    flash_timer = mp.add_timeout(FLASH_FOR + 0.01, app.changed)
    app.changed()
end

-- Compared with the values read at start-up, so the observers' first reports are not changes.
mp.observe_property("volume", "number", function(_, value)
    if value and value ~= volume then
        volume = value
        flash()
    end
    overlay.request()
end)
mp.observe_property("mute", "bool", function(_, value)
    if (value == true) ~= muted then
        muted = value == true
        flash()
    end
    overlay.request()
end)

---------------------------------------------------------------------------------------------
-- Input
---------------------------------------------------------------------------------------------

local function set_from_x(x)
    if not slider then
        return
    end
    local fraction = math.max(0, math.min(1, (x - slider.x0) / (slider.x1 - slider.x0)))
    mp.commandv("set", "volume", tostring(math.floor(fraction * 100 + 0.5)))
end

local function drag(x, _, phase)
    if phase == "down" then
        dragging = true
        if muted then
            mp.commandv("set", "mute", "no")
        end
    end
    set_from_x(x)
    if phase == "up" then
        dragging = false
    end
    timeline.poke()
end

local function toggle_mute()
    mp.commandv("cycle", "mute")
    timeline.poke()
end

-- Wheel steps, also bound to the wheel over the whole picture in anistream mode (main.lua).
function M.step(delta)
    mp.commandv("add", "volume", tostring(delta))
end

---------------------------------------------------------------------------------------------
-- Drawing
---------------------------------------------------------------------------------------------

local function speaker(ctx, cx, cy, size, outline)
    local icon = muted and "volume_off" or (volume < 50 and "volume_down" or "volume_up")
    local font = { size = size, font = ICON_FONT, color = config.color.text, outline = outline }
    ctx:text(cx, cy, 5, icon, font)
end

local function track(ctx, x0, x1, cy, active)
    local s = ctx.s
    local thick = draw.round((active and LINE_ACTIVE or LINE) * s)
    local y0 = draw.round(cy - thick / 2)
    local y1 = y0 + thick
    ctx:rect(x0, y0, x1, y1, config.color.ground, 0.3, thick / 2)
    ctx:rect(x0, y0, x1, y1, config.color.text, 0.25, thick / 2)
    local level = math.max(0, math.min(1, volume / 100))
    if level > 0 then
        ctx:rect(x0, y0, x0 + (x1 - x0) * level, y1, muted and config.color.dim or config.color.obi,
            muted and 0.6 or 1, thick / 2)
    end
end

local function value_text()
    return string.format("%d", math.floor(volume + 0.5))
end

function M.draw(ctx, _, _, w, h)
    local s = ctx.s
    local g = screen.timeline_geometry(s, w, h)
    local value_font = ctx:font(config.TYPE.time)
    value_font.color = muted and config.color.dim or config.color.text
    value_font.outline = math.max(1, s)

    if not pill and g.volume then
        drawn = "row"
        local v = g.volume
        local cy = g.row_y
        -- Speaker: a button the size of uosc's, drawn like its icons.
        speaker(ctx, v.speaker_x + g.button / 2, cy, g.button * 0.7, math.max(1, s))
        ctx:zone("volume-mute", v.speaker_x, g.row_top, v.speaker_x + g.button, g.row_bottom, toggle_mute)
        -- Slider: grab anywhere in the row's height.
        local active = dragging or ctx.hover == "volume"
        slider = { x0 = v.slider_x0, x1 = v.slider_x1 }
        track(ctx, v.slider_x0, v.slider_x1, cy, active)
        ctx:zone("volume", v.slider_x0 - 6 * s, g.row_top, v.slider_x1 + 6 * s, g.row_bottom, nil, drag)
        ctx:text(v.value_x, cy, 4, value_text(), value_font)
        return
    end

    -- The pill: the same three parts on a panel, bottom left, level with the corner cards.
    drawn = "pill"
    slider = nil
    local pad, icon = 12 * s, 22 * s
    local text_w = ctx:width("100", value_font)
    local pw = pad + icon + 10 * s + 88 * s + 12 * s + text_w + pad
    local ph = 40 * s
    local x0 = g.x0
    local y1 = g.band_top - 12 * s
    local y0 = y1 - ph
    local cy = (y0 + y1) / 2
    ctx:panel(x0, y0, x0 + pw, y1)
    speaker(ctx, x0 + pad + icon / 2, cy, icon)
    local tx0 = x0 + pad + icon + 10 * s
    track(ctx, tx0, tx0 + 88 * s, cy, false)
    value_font.outline = nil
    ctx:text(tx0 + 88 * s + 12 * s, cy, 4, value_text(), value_font)
end

function M.drawn()
    return drawn
end

return M
