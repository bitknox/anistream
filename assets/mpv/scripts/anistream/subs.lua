-- Lift the subtitles clear of the controls while the controls are up.
--
-- With the control row and the seek bar showing, the bar ran straight through the bottom
-- subtitle line. Like the streaming players, the subtitles now rise by the height the controls
-- take and come back down when they leave, moving with the bar: the lift is the bar's own fade
-- value applied to the full lift, read on the frames the overlay draws, so the two share one
-- timing and one curve, reversals included.
--
-- `sub-pos` rather than `sub-margin-y`: most anime subtitles are ASS, which the margin does not
-- move. `sub-pos` moves both kinds, and positioned signs keep their place either way. Only in
-- anistream mode, where the controls are ours and their height is known.
--
-- Measured on a 16:10 display at fullscreen with a 16:9 video: ten points of `sub-pos` moved a
-- subtitle 180 px, about ten per cent of the video's height plus its letterbox, and at rest the
-- subtitle's bottom sat inside the bottom letterbox. So the lift is the part of the controls
-- that reaches above the letterbox, taken as a share of the video's height, which errs a
-- little high for plain subtitles rather than leaving them under the bar.

local config = require("config")
local overlay = require("overlay")
local screen = require("screen")

local M = {}

local GAP = 6 -- px at scale 1, between the lifted subtitles and the seek band

local dims = { w = 0, h = 0, mt = 0, mb = 0 }
mp.observe_property("osd-dimensions", "native", function(_, value)
    if type(value) == "table" then
        dims = { w = value.w or 0, h = value.h or 0, mt = value.mt or 0, mb = value.mb or 0 }
    end
end)

-- The viewer's own position, which every lift is measured from and every drop returns to.
local base = nil
-- What this script last wrote, so a change that is not ours (mpv's r and R) can be told apart
-- and kept.
local applied = nil
local lifted = 0 -- points of sub-pos currently taken off `base`

mp.observe_property("sub-pos", "number", function(_, value)
    if not value or not applied then
        return
    end
    if math.abs(value - applied) > 0.01 then
        -- The viewer moved them while lifted: keep the move, under the lift.
        base = (base or value) + (value - applied)
        applied = value
    end
end)

-- How many points of `sub-pos` clear the controls, for the screen as it is now.
local function lift_points()
    -- A window always has a size; only an output with no window (a headless run) reports
    -- none, and a plain 1080p frame stands in for it.
    local w, h = dims.w, dims.h
    if h <= 0 then
        w, h = 1920, 1080
    end
    local video_h = h - dims.mt - dims.mb
    if video_h <= 0 then
        return 0
    end
    local s = screen.scale()
    local g = screen.timeline_geometry(s, w, h)
    local reach = (h - g.band_top) + GAP * s - dims.mb
    if reach <= 0 then
        return 0
    end
    return reach / video_h * 100
end

-- Follow `bar`'s fade: the full lift while it is shown, none while hidden, and the same share of
-- it at every frame between.
function M.follow(bar)
    overlay.on_frame(function()
        local share = config.opts.uosc and bar.fade.value or 0
        local points = share > 0 and lift_points() * share or 0
        -- Steps too small to see are skipped, but never the last one down: the fade eases out,
        -- so its tail can leave a lift under the threshold, and skipping the drop from there
        -- left the subtitles a hundredth of a point off where the viewer had them, still lifted.
        if math.abs(points - lifted) < 0.01 and (points ~= 0 or lifted == 0) then
            return
        end
        if base == nil then
            base = mp.get_property_number("sub-pos", 100)
        end
        lifted = points
        if points == 0 then
            -- Back where the viewer left them; nothing of ours to remember.
            mp.set_property_number("sub-pos", base)
            applied, base = nil, nil
        else
            applied = base - points
            mp.set_property_number("sub-pos", applied)
        end
    end)
end

-- What tests read: points lifted right now.
function M.lifted()
    return lifted
end

-- Leave the viewer's position as it was when the window goes.
mp.register_event("shutdown", function()
    if base then
        mp.set_property_number("sub-pos", base)
    end
end)

return M
