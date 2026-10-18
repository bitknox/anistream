-- The skip button: "Skip opening" with its key, while the host has a skip on offer.
--
-- After eight seconds of the episode playing it turns quiet (dim label, lighter panel) rather
-- than leaving: the offer stands for the whole segment, but a button that stays loud through a
-- ninety-second opening the viewer chose to watch is a button they have to look past. Hovering
-- brings it back to full strength.

local app = require("app")
local card = require("card")
local util = require("util")

local QUIET_AFTER = 8

local offer = nil
local quiet = false
local quiet_timer = nil

local M

local function take()
    if not offer then
        return
    end
    util.emit("skip")
    -- Gone at once: the host seeks and clears the offer, but the press should answer now.
    offer = nil
    app.changed()
end

-- What was last shown, so the button can fade out after the offer itself is gone.
local last_label = ""

M = card.element("corner", function(ctx)
    last_label = offer and offer.label or last_label
    local label = "Skip " .. last_label
    local calm = quiet and ctx.hover ~= "skip"
    return {
        obi = true,
        quiet = calm,
        action = { id = "skip", fn = take },
        rows = {
            {
                left = {
                    { parts = { { label, calm and "quiet" or "primary" }, { "S", "hint" } } },
                },
            },
        },
    }
end)
M.id = "skip"

function M.offer(payload)
    offer = {
        label = util.str(payload.label) or util.str(payload.kind) or "segment",
        kind = util.str(payload.kind),
        to = util.num(payload.to),
    }
    quiet = false
    if quiet_timer then
        quiet_timer:kill()
    end
    quiet_timer = util.pause_timer(QUIET_AFTER, function()
        quiet = true
        app.changed()
    end)
    app.changed()
end

function M.clear()
    offer = nil
    if quiet_timer then
        quiet_timer:kill()
        quiet_timer = nil
    end
    app.changed()
end

function M.wants()
    return offer ~= nil
end

-- `script-binding anistream/skip`: works whenever there is an offer, shown or not, so a key
-- the user bound themselves behaves like a remote's dedicated button.
M.take = take

-- The keys the button takes while it is shown; the keys overlay lists them from here too.
-- s and S are both mpv's screenshot keys; while the button is up, either skips.
M.KEYS = { s = take, S = take, ENTER = take }

function M.keys()
    return M.KEYS
end

function M.label()
    return offer and offer.label or nil
end

return M
