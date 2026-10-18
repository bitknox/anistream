-- The stall prompt: the stream has stopped delivering mid-episode, so offer another source.
--
-- The host decides when a stall is long enough to ask about and owns the switch; this only
-- asks. The seconds count on locally between the host's reports, so the number keeps moving
-- while nothing else on screen does.

local app = require("app")
local card = require("card")
local util = require("util")

local stall = nil -- { seconds, since }
local ticker = nil

local M

local function stop()
    stall = nil
    if ticker then
        ticker:kill()
        ticker = nil
    end
    app.changed()
end

local function switch()
    if not stall then
        return
    end
    util.emit("stall-switch")
    stop()
end

local function wait()
    if not stall then
        return
    end
    util.emit("stall-wait")
    stop()
end

local function seconds()
    if not stall then
        return 0
    end
    return math.floor(stall.seconds + mp.get_time() - stall.since)
end

M = card.element("corner", function()
    return {
        obi = true,
        rows = {
            {
                left = { { parts = { { "Source is buffering", "body" } } } },
                right = { { parts = { { string.format("%d s", seconds()), "hint" } } } },
            },
            {
                left = {
                    {
                        id = "stall-switch",
                        fn = switch,
                        parts = { { "Switch source", "primary" }, { "Enter", "hint" } },
                    },
                    { id = "stall-wait", fn = wait, parts = { { "Keep waiting", "quiet" }, { "Esc", "hint" } } },
                },
            },
        },
    }
end)
M.id = "stall"

function M.show(payload)
    stall = { seconds = util.num(payload.seconds) or 0, since = mp.get_time() }
    if not ticker then
        ticker = mp.add_periodic_timer(1, app.changed)
    end
    app.changed()
end

M.clear = stop
M.switch = switch
M.wait = wait

function M.wants()
    return stall ~= nil
end

function M.keys()
    return { ENTER = switch, KP_ENTER = switch, ESC = wait }
end

return M
