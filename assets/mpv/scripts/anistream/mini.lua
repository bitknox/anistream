-- The mini player: a small borderless window on top of everything, in a screen corner.
--
-- Entering remembers every property it is about to touch and leaving writes each one back,
-- so the round trip is exact rather than "back to some sensible default". `geometry` is the
-- one that moves the window; writing the old value back makes mpv re-apply it, which restores
-- the size it chose before, and `current-window-scale` then puts back a size the user had
-- dragged to by hand.
--
-- Wayland does not let a client position its own window, so there the window shrinks and
-- stays on top but goes wherever the compositor puts it.

local app = require("app")
local config = require("config")
local screen = require("screen")

local M = {}

-- In anistream mode the mini window drops uosc's control row (its client id below is how
-- uosc tracks who asked) and our seek bar moves down to the edge.
local function compact(on)
    screen.mini = on
    if config.opts.uosc then
        screen.to_uosc("disable-elements", "anistream-mini", on and "controls" or "")
    end
end

-- A quarter of the screen's width, inset from the bottom-right corner.
local GEOMETRY = "25%-32-48"

local PROPERTIES = { "ontop", "border", "geometry", "autofit", "autofit-larger", "autofit-smaller" }

local saved = nil

function M.active()
    return saved ~= nil
end

local function enter()
    saved = { values = {} }
    for _, name in ipairs(PROPERTIES) do
        saved.values[name] = mp.get_property_native(name)
    end
    saved.fullscreen = mp.get_property_native("fullscreen") == true
    saved.maximized = mp.get_property_native("window-maximized") == true
    saved.scale = mp.get_property_native("current-window-scale")

    -- A fullscreen or maximised window ignores geometry; leave those states first.
    if saved.fullscreen then
        mp.set_property_native("fullscreen", false)
    end
    if saved.maximized then
        mp.set_property_native("window-maximized", false)
    end
    mp.set_property_native("autofit", "")
    mp.set_property_native("autofit-larger", "")
    mp.set_property_native("autofit-smaller", "")
    mp.set_property_native("border", false)
    mp.set_property_native("ontop", true)
    mp.set_property_native("geometry", GEOMETRY)
    compact(true)
end

local function leave()
    local s = saved
    saved = nil
    compact(false)
    for _, name in ipairs(PROPERTIES) do
        if s.values[name] ~= nil then
            mp.set_property_native(name, s.values[name])
        end
    end
    if type(s.scale) == "number" and s.scale > 0 then
        local now = mp.get_property_native("current-window-scale")
        if type(now) == "number" and math.abs(now - s.scale) > 0.001 then
            mp.set_property_native("current-window-scale", s.scale)
        end
    end
    if s.maximized then
        mp.set_property_native("window-maximized", true)
    end
    if s.fullscreen then
        mp.set_property_native("fullscreen", true)
    end
end

function M.toggle()
    if saved then
        leave()
    else
        enter()
    end
    app.changed()
end

-- Going fullscreen from the mini window means the viewer wants the big picture back: put the
-- window properties back under it, so leaving fullscreen does not drop into a corner.
mp.observe_property("fullscreen", "bool", function(_, value)
    if value and saved then
        local s = saved
        saved = nil
        compact(false)
        for _, name in ipairs(PROPERTIES) do
            if s.values[name] ~= nil then
                mp.set_property_native(name, s.values[name])
            end
        end
        app.changed()
    end
end)

return M
