-- Which player UI is on screen (uosc, mpv's stock OSC, or neither), and how much of the
-- window's edges it can occupy.

local config = require("config")
local app = require("app")

local M = {}

-- uosc announces itself with a broadcast when it loads. Scripts start in parallel, so that
-- broadcast can arrive before this script has registered for it; the probe below covers the
-- other order: `script-message-to` fails outright when no client of that name exists.
local seen_uosc = false
local probed_at = -math.huge
mp.register_script_message("uosc-version", function()
    seen_uosc = true
end)

function M.uosc()
    if config.opts.uosc or seen_uosc then
        return true
    end
    -- An unregistered message name is ignored by uosc, so the probe has no side effect. A miss
    -- is remembered for a second: layout asks on every frame of a fade.
    local now = mp.get_time()
    if now - probed_at > 1 then
        probed_at = now
        seen_uosc = mp.commandv("script-message-to", "uosc", "anistream-probe") ~= nil
    end
    return seen_uosc
end

function M.to_uosc(...)
    mp.commandv("script-message-to", "uosc", ...)
end

local margins = { l = 0, r = 0, t = 0, b = 0 }
mp.observe_property("user-data/osc/margins", "native", function(_, value)
    if type(value) == "table" then
        margins = {
            l = tonumber(value.l) or 0,
            r = tonumber(value.r) or 0,
            t = tonumber(value.t) or 0,
            b = tonumber(value.b) or 0,
        }
    else
        margins = { l = 0, r = 0, t = 0, b = 0 }
    end
    app.changed()
end)

local osc_enabled = false
mp.observe_property("osc", "bool", function(_, value)
    osc_enabled = value == true
    app.changed()
end)

-- uosc's own scale: HiDPI factor times its `scale`, or `scale_fullscreen` when the window fills
-- the screen. Whatever lines up with its control row (the seek bar, the volume, the time, and
-- what sits clear of them) is sized by this, at every window size.
local function uosc_factor()
    local p = app.player
    local fullish = p.fullscreen or p.maximized
    if M.uosc() then
        return fullish and config.script_opt("uosc", "scale_fullscreen", 1.3)
            or config.script_opt("uosc", "scale", 1)
    end
    return fullish and 1.3 or 1
end

function M.row_scale()
    return math.max(0.5, (app.player.hidpi or 1) * uosc_factor())
end

-- The scale for everything else: uosc's, except that a small window (the mini player, a tiled
-- corner) gets smaller cards, down to 60%: full-size ones would cover a third of a
-- 400-point-tall picture. uosc does not shrink its buttons, so the row keeps `row_scale`.
function M.scale()
    local p = app.player
    local factor = uosc_factor()
    local logical_h = (p.osd_h or 0) / (p.hidpi or 1)
    if logical_h > 0 and logical_h < 450 then
        factor = factor * math.max(0.6, logical_h / 450)
    end
    return math.max(0.5, (p.hidpi or 1) * factor)
end

-- Set by mini.lua: the mini player shows no control row, so the seek bar sits at the bottom.
M.mini = false

-- Where our own seek bar goes (anistream mode): a grab band just above uosc's control row,
-- its line through the middle, and the elapsed time beside play/pause.
local SEEK_BAND = 36

function M.timeline_geometry(s, w, h)
    local size = config.script_opt("uosc", "controls_size", 32)
    local margin = config.script_opt("uosc", "controls_margin", 8)
    local controls = config.script_opt("uosc", "controls", "") ~= "never" and not M.mini
    local row_top = controls and h - (margin + size) * s or h - margin * s
    local band_bottom = row_top - 2 * s
    local band_top = band_bottom - SEEK_BAND * s
    -- uosc draws each icon at 70% of its button, centred: the line's ends meet the icons'
    -- visible edges rather than the buttons' invisible ones.
    local inset = (margin + size * 0.15) * s
    -- The left of the control row, after uosc's play/pause button: our volume (a speaker
    -- button the size of uosc's, a short slider, its value), then the elapsed time. Without a
    -- control row (the mini player) there is nowhere quiet for either.
    local row_y = h - (margin + size / 2) * s
    local speaker_x = (margin + size + 8) * s
    local slider_x0 = speaker_x + (size + 4) * s
    local slider_x1 = slider_x0 + 88 * s
    local volume = controls and app.player.has_audio
    return {
        x0 = inset,
        x1 = w - inset,
        band_top = band_top,
        band_bottom = band_bottom,
        line_y = (band_top + band_bottom) / 2,
        row_y = row_y,
        row_top = row_top,
        row_bottom = row_top + size * s,
        button = size * s,
        volume = volume and {
            speaker_x = speaker_x,
            slider_x0 = slider_x0,
            slider_x1 = slider_x1,
            value_x = slider_x1 + 12 * s,
        } or nil,
        time_x = controls and (volume and slider_x1 + (12 + 34 + 16) * s or speaker_x) or nil,
        time_y = row_y,
    }
end

-- Pixels at the window's edges that a player bar may cover, whether or not it is showing.
function M.insets()
    local h = app.player.osd_h
    local bottom, top, right = 0, 0, 0

    if config.opts.uosc then
        -- Our mode: uosc's control row and our seek bar above it, top bar off. The corner's
        -- right edge lines up exactly with the seek bar's end and the last icon.
        local g = M.timeline_geometry(M.row_scale(), app.player.osd_w, h)
        return { bottom = h - g.band_top, top = 0, right = g.x0, align_right = g.x0 }
    elseif M.uosc() then
        local s = M.row_scale()
        local disabled = "," .. tostring(config.script_opt("uosc", "disable_elements", "")) .. ","
        if not disabled:find(",timeline,", 1, true) then
            bottom = bottom
                + (config.script_opt("uosc", "timeline_size", 40)
                    + config.script_opt("uosc", "timeline_border", 1)) * s
        end
        local controls = config.script_opt("uosc", "controls", "default")
        if controls ~= "never" and not disabled:find(",controls,", 1, true) then
            bottom = bottom
                + (config.script_opt("uosc", "controls_size", 32)
                    + 2 * config.script_opt("uosc", "controls_margin", 8)) * s
        end
        if config.script_opt("uosc", "top_bar", "no-border") ~= "never" then
            top = config.script_opt("uosc", "top_bar_size", 40) * s
        end
        right = config.script_opt("uosc", "controls_margin", 8) * s
    elseif osc_enabled then
        -- The stock OSC's bars are 56 units tall on a canvas 720 units high when it scales with
        -- the window (the default), or 56 pixels when it does not. The older box layout floats
        -- a little higher.
        local layout = config.script_opt("osc", "layout", "bottombar")
        local units = ({ bottombar = 56, slimbottombar = 25, topbar = 0, slimtopbar = 0 })[layout]
            or 92
        local osc_scale = (app.player.fullscreen and config.script_opt("osc", "scalefullscreen", 1))
            or config.script_opt("osc", "scalewindowed", 1)
        local by_window = mp.get_property_native("osd-scale-by-window") ~= false
        bottom = units * osc_scale * (by_window and h / 720 or app.player.hidpi)
        if layout == "topbar" or layout == "slimtopbar" then
            top = (layout == "topbar" and 56 or 25) * osc_scale
                * (by_window and h / 720 or app.player.hidpi)
        end
    end

    -- Pinned bars report themselves; never sit lower than that either.
    bottom = math.max(bottom, margins.b * h)
    top = math.max(top, margins.t * h)
    right = math.max(right, margins.r * app.player.osd_w)
    return { bottom = bottom, top = top, right = right }
end

return M
