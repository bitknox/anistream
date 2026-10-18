-- "Skipped opening · Undo" after the host skipped a segment on its own.
--
-- Auto-skip is right most of the time and badly wrong the rest (a cold open filed as a recap),
-- so it is announced with a way back. Six seconds of playback, then it goes.

local app = require("app")
local card = require("card")
local util = require("util")

local SHOW_FOR = 6
-- Paused, the note waits for the viewer, but not indefinitely: a paused player left alone
-- should come back to just the picture.
local SHOW_AT_MOST = 15

local note = nil
local timer = nil
local cap = nil

local M

local function dismiss()
    note = nil
    if timer then
        timer:kill()
        timer = nil
    end
    if cap then
        cap:kill()
        cap = nil
    end
    app.changed()
end

local function undo()
    if not note or not note.from then
        return
    end
    util.emit("undo-skip", { to = note.from })
    dismiss()
end

-- What was last shown, so the card can fade out after the note itself is gone.
local last = { label = "", from = nil }

M = card.element("corner", function()
    local shown = note or last
    local items = { { parts = { { "Skipped " .. shown.label, "quiet" } } } }
    if shown.from then
        items[2] = { parts = { { "Undo", "primary" }, { "U", "hint" } } }
    end
    return {
        obi = shown.from ~= nil,
        action = shown.from and { id = "undo", fn = undo } or nil,
        rows = { { left = items } },
    }
end)
M.id = "skipped"

function M.show(payload)
    note = {
        label = util.str(payload.label) or util.str(payload.kind) or "segment",
        from = util.num(payload.from),
    }
    last = note
    if timer then
        timer:kill()
    end
    if cap then
        cap:kill()
    end
    timer = util.pause_timer(SHOW_FOR, dismiss)
    cap = mp.add_timeout(SHOW_AT_MOST, dismiss)
    app.changed()
end

M.clear = dismiss
M.undo = undo

function M.wants()
    return note ~= nil
end

function M.keys()
    if note and note.from then
        return { u = undo }
    end
    return {}
end

return M
