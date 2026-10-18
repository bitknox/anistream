-- A short notice from the host, top left where mpv's own messages appear. One at a time: a new
-- toast replaces the one on screen rather than queueing behind it.
--
-- An alert carries a vermilion bar where the obi would be; an ordinary notice carries none,
-- since there is nothing in it to act on.

local app = require("app")
local card = require("card")
local config = require("config")
local util = require("util")

local DEFAULT_SECONDS = 3

local toast = nil
local last = { text = "" }
local timer = nil

local M

local function dismiss()
    toast = nil
    if timer then
        timer:kill()
        timer = nil
    end
    app.changed()
end

M = card.element("top-left", function()
    local shown = toast or last
    return {
        obi = shown.alert and config.color.alert or nil,
        action = { id = "toast", fn = dismiss },
        rows = { { left = { { parts = { { shown.text, "body" } } } } } },
    }
end)
M.id = "toast"

function M.show(text, level, seconds)
    text = util.str(text)
    if not text then
        return
    end
    toast = { text = text, alert = level == "alert" }
    last = toast
    if timer then
        timer:kill()
    end
    timer = mp.add_timeout(math.max(0.5, seconds or DEFAULT_SECONDS), dismiss)
    app.changed()
end

function M.wants()
    return toast ~= nil
end

function M.text()
    return toast and toast.text or nil
end

function M.keys()
    return {}
end

return M
