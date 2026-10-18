-- The Up Next card: a still from the next episode when the host has one, its label and title,
-- Play now and Cancel, and a countdown in the final ten seconds when the episode is going to
-- roll on by itself.
--
-- It appears at `next.at` — the ending's start when the host knows it — or thirty seconds from
-- the end of anything longer than two minutes. Cancel holds for the rest of this episode: the
-- host stops rolling on, and the card does not come back when the playhead crosses `at` again.
--
-- The still is raw BGRA the host writes beside `next` (often a moment after it), shown with
-- mpv's `overlay-add`. Those bitmaps sit above every ASS overlay and cannot be masked, so it is
-- square-cornered, and it is removed the moment the card stops being shown rather than faded.
--
-- The playhead is only observed while a card could still appear; for most of an episode, and
-- for every episode with no next, this costs nothing per frame.

local app = require("app")
local card = require("card")
local util = require("util")

local COUNTDOWN_FROM = 10
local FALLBACK_LEAD = 30
local FALLBACK_MIN_DURATION = 120

-- overlay-add slot for the still. thumbfast takes 42 by default; mpv allows 0-63.
local OVERLAY_ID = 57
-- The still's size on the card, in base pixels (16:9).
local STILL_W, STILL_H = 160, 90

local info = nil -- the host's `next`, normalised
local last = { label = "", title = nil } -- what was last shown, for the fade-out
local cancelled = false
local played = false
local shown = false
local countdown = nil
local observing = false
local still_key = nil -- what overlay-add last placed, or nil when nothing is up

local M

local function trigger_at()
    if not info then
        return nil
    end
    if info.at then
        return info.at
    end
    local duration = app.player.duration
    if duration and duration > FALLBACK_MIN_DURATION then
        return duration - FALLBACK_LEAD
    end
    return nil
end

local function evaluate()
    local p = app.player
    local at = trigger_at()
    local pos = p.time_pos
    local want = info ~= nil and not cancelled and not played and not p.idle
        and at ~= nil and pos ~= nil and pos >= at
    local count = nil
    if want and info.auto and p.duration then
        local remaining = (p.duration - pos) / math.max(0.01, p.speed or 1)
        if remaining <= COUNTDOWN_FROM then
            count = math.max(0, math.ceil(remaining))
        end
    end
    if want ~= shown or count ~= countdown then
        shown, countdown = want, count
        app.changed()
    end
end

local function on_time(_, value)
    app.player.time_pos = value
    evaluate()
end

local function observe()
    local wanted = info ~= nil and not cancelled and not played
    if wanted and not observing then
        mp.observe_property("time-pos", "number", on_time)
        observing = true
    elseif not wanted and observing then
        mp.unobserve_property(on_time)
        observing = false
    end
end

---------------------------------------------------------------------------------------------
-- The still
---------------------------------------------------------------------------------------------

local function remove_still()
    if still_key then
        mp.commandv("overlay-remove", tostring(OVERLAY_ID))
        still_key = nil
        -- Published state follows on the next refresh, outside the frame being drawn.
        mp.add_timeout(0, app.changed)
    end
end

-- Put the still at (x, y) scaled to w x h; repeated calls with the same placement cost nothing.
local function place_still(thumb, x, y, w, h)
    x, y, w, h = math.floor(x + 0.5), math.floor(y + 0.5), math.floor(w + 0.5), math.floor(h + 0.5)
    local key = string.format("%s %d %d %d %d %d %d", thumb.path, thumb.width, thumb.height, x, y, w, h)
    if key == still_key then
        return
    end
    local ok = mp.commandv(
        "overlay-add", tostring(OVERLAY_ID), tostring(x), tostring(y), thumb.path, "0", "bgra",
        tostring(thumb.width), tostring(thumb.height), tostring(thumb.width * 4), tostring(w), tostring(h)
    )
    local was = still_key
    still_key = ok and key or nil
    if (was == nil) ~= (still_key == nil) then
        mp.add_timeout(0, app.changed)
    end
end

-- The host's thumbnail, if it is usable: a path and a positive size.
local function thumbnail(value)
    value = util.tbl(value)
    if not value then
        return nil
    end
    local path, width, height = util.str(value.path), util.num(value.width), util.num(value.height)
    if not path or not width or not height or width < 1 or height < 1 then
        return nil
    end
    return { path = path, width = math.floor(width), height = math.floor(height) }
end

---------------------------------------------------------------------------------------------
-- Actions
---------------------------------------------------------------------------------------------

local function play_now()
    if not shown then
        return
    end
    util.emit("play-next")
    played = true
    remove_still()
    observe()
    evaluate()
end

local function cancel()
    if not shown then
        return
    end
    util.emit("cancel-next")
    cancelled = true
    remove_still()
    observe()
    evaluate()
end

M = card.element("corner", function(ctx)
    local right = nil
    if countdown then
        right = { { parts = { { string.format("%d s", countdown), "hint" } } } }
    end
    local rows = {
        { left = { { parts = { { "Up next", "hint" } } } }, right = right },
        { left = { { parts = { { last.label, "body", fit = true } } } } },
    }
    if last.title then
        rows[#rows + 1] = { left = { { parts = { { last.title, "quiet", fit = true } } } } }
    end
    rows[#rows + 1] = {
        left = {
            { id = "play-next", fn = play_now, parts = { { "Play now", "primary" }, { "Enter", "hint" } } },
            { id = "cancel-next", fn = cancel, parts = { { "Cancel", "quiet" }, { "Esc", "hint" } } },
        },
    }
    local media = nil
    if last.thumbnail then
        media = {
            w = STILL_W,
            h = STILL_H,
            place = function(_, x, y, w, h)
                -- Only while the card is really up: a card fading out gives its still back now.
                if ctx.interactive and info and info.thumbnail then
                    place_still(info.thumbnail, x, y, w, h)
                else
                    remove_still()
                end
            end,
        }
    end
    return {
        obi = true,
        max_width = 420 + (media and STILL_W + 14 or 0),
        media = media,
        rows = rows,
    }
end)
M.id = "up_next"

function M.set(value)
    value = util.tbl(value)
    if not value then
        info = nil
        remove_still()
    else
        info = {
            label = util.str(value.label)
                or (util.str(value.number) and "Episode " .. util.str(value.number))
                or "Next episode",
            title = util.str(value.title),
            at = util.num(value.at),
            auto = value.auto == true,
            thumbnail = thumbnail(value.thumbnail),
        }
        if not info.thumbnail then
            remove_still()
        end
        last = info
    end
    observe()
    evaluate()
    -- A still arriving after the card is already up shows without waiting for anything else.
    app.changed()
end

-- A new file is a new episode: a Cancel or Play now pressed for the last one no longer holds.
function M.reset()
    cancelled, played = false, false
    remove_still()
    observe()
    evaluate()
end

-- main.lua calls this after deciding what holds the corner: the still never outlives the card.
function M.sync_still(visible)
    if not visible then
        remove_still()
    end
end

mp.register_event("shutdown", remove_still)

-- Duration, speed, idle: anything `evaluate` reads that is not the playhead.
M.evaluate = evaluate
M.play_now = play_now
M.cancel = cancel

function M.wants()
    return shown
end

function M.countdown()
    return countdown
end

function M.still()
    return still_key ~= nil
end

function M.keys()
    return { ENTER = play_now, KP_ENTER = play_now, ESC = cancel }
end

return M
