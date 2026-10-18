-- Two-point subtitle sync.
--
-- A track that drifts (cut for a different encode, or timed at the wrong frame rate) cannot be
-- fixed by a delay alone: it is early at one end and late at the other. Two lines pin it down.
-- The viewer picks a line while it is on screen, then marks the moment it should appear; twice,
-- far apart. That gives two (subtitle time, video time) pairs, and mpv shows a subtitle stamped
-- `s` at `s * sub-speed + sub-delay`, so
--
--     speed = (video2 - video1) / (sub2 - sub1)
--     delay = video2 - sub2 * speed
--
-- `sub-start` reports the line's own timestamp, before the current delay and speed, so the
-- result replaces whatever correction was in force rather than stacking on it.

local app = require("app")
local card = require("card")

local M = {}

-- The steps, each a short instruction. Ctrl+Left/Right is mpv's own "previous/next subtitle"
-- seek, the quickest way to put a chosen line on screen.
local STEPS = {
    "Play to a line near the start and press Enter while it shows",
    "Now seek to where that line is spoken and press Enter",
    "Now a line near the end: press Enter while it shows",
    "Seek to where it is spoken and press Enter",
}

-- Two lines closer than this give a speed dominated by the viewer's reaction time.
local MIN_SPAN = 10

local step = nil
local marks = {}

-- Pure: the correction that maps two subtitle timestamps onto two video times.
function M.solve(sub1, video1, sub2, video2)
    if not (sub1 and video1 and sub2 and video2) then
        return nil, "incomplete"
    end
    if math.abs(sub2 - sub1) < MIN_SPAN then
        return nil, "too close"
    end
    local speed = (video2 - video1) / (sub2 - sub1)
    if speed < 0.5 or speed > 2 then
        return nil, "implausible"
    end
    return { speed = speed, delay = video2 - sub2 * speed }
end

M.notify = function() end

local function finish()
    step = nil
    marks = {}
    app.changed()
end

function M.cancel()
    if step then
        finish()
        M.notify("Subtitle sync cancelled")
    end
end

local function mark()
    if not step then
        return
    end
    if step == 1 or step == 3 then
        local start = mp.get_property_native("sub-start")
        if type(start) ~= "number" then
            M.notify("No subtitle on screen. Ctrl+← and Ctrl+→ step through lines")
            return
        end
        marks[step] = start
    else
        local pos = mp.get_property_native("time-pos")
        if type(pos) ~= "number" then
            return
        end
        marks[step] = pos
    end

    if step < 4 then
        step = step + 1
        app.changed()
        return
    end

    local result, why = M.solve(marks[1], marks[2], marks[3], marks[4])
    finish()
    if not result then
        local reason = why == "too close" and "pick two lines further apart"
            or "the two marks disagree; try again"
        M.notify("Could not sync: " .. reason, true)
        return
    end
    mp.set_property_native("sub-speed", result.speed)
    mp.set_property_native("sub-delay", result.delay)
    M.notify(string.format("Subtitles synced  ·  delay %+.2f s  ·  speed %.4f", result.delay, result.speed))
end

function M.start()
    if step then
        M.cancel()
        return
    end
    local sid = mp.get_property_native("sid")
    if sid == false or sid == nil or sid == "no" then
        M.notify("Turn on a subtitle track first")
        return
    end
    step, marks = 1, {}
    app.changed()
end

function M.step()
    return step
end

M.element = card.element("top-center", function()
    return {
        obi = true,
        rows = {
            {
                left = { { parts = { { STEPS[step or 1], "body" } } } },
                right = {
                    { parts = { { string.format("%d of 4", step or 1), "hint" } } },
                    { id = "sync-cancel", fn = M.cancel, parts = { { "Cancel", "quiet" }, { "Esc", "hint" } } },
                },
            },
        },
    }
end)
M.element.id = "sync"

function M.element.wants()
    return step ~= nil
end

function M.element.keys()
    return { ENTER = mark, KP_ENTER = mark, ESC = M.cancel }
end

return M
