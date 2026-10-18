-- Small helpers shared by every module: tolerant JSON, talking to the host, and timers that
-- only count while the video is moving.

local utils = require("mp.utils")

local M = {}

-- A payload that is missing, empty, malformed or not an object reads as `{}`. The host owns the
-- protocol, but a script that throws on a bad message takes every later message down with it.
function M.parse_json(text)
    if type(text) ~= "string" or text == "" then
        return {}
    end
    local value = utils.parse_json(text)
    if type(value) ~= "table" then
        return {}
    end
    return value
end

-- Only the JSON: mpv 0.37 and older return `json, nil`, and the trailing nil handed on to
-- `commandv` is an argument that is not a string, which those versions reject.
function M.format_json(value)
    return (utils.format_json(value))
end

-- Fields from the host are read through these: a number may arrive as a string ("5") or a
-- string as a number (5), and either may be null.
function M.str(value)
    if type(value) == "string" then
        return value ~= "" and value or nil
    end
    if type(value) == "number" then
        return value == math.floor(value) and string.format("%d", value) or tostring(value)
    end
    return nil
end

function M.num(value)
    if type(value) == "number" then
        return value
    end
    if type(value) == "string" then
        return tonumber(value)
    end
    return nil
end

function M.tbl(value)
    return type(value) == "table" and value or nil
end

-- Tell the host something. Arrives as a `client-message` with
-- `args = ["anistream", event, json?]`.
function M.emit(event, payload)
    if payload ~= nil then
        mp.commandv("script-message", "anistream", event, M.format_json(payload))
    else
        mp.commandv("script-message", "anistream", event)
    end
end

-- UTF-8 aware character iteration, for truncating titles without cutting a kana in half.
function M.chars(text)
    local out = {}
    for char in tostring(text):gmatch("[%z\1-\127\194-\244][\128-\191]*") do
        out[#out + 1] = char
    end
    return out
end

function M.capitalize(text)
    text = tostring(text or "")
    return (text:gsub("^%l", string.upper))
end

---------------------------------------------------------------------------------------------
-- Pause-aware timers
--
-- "Show this for six seconds" means six seconds of the episode, not of the wall: a viewer who
-- pauses to read the Undo note should still find it there. One `pause` observer drives every
-- live timer rather than each one observing the property itself.
---------------------------------------------------------------------------------------------

local paused = mp.get_property_native("pause") == true
local live = {}

local Timer = {}
Timer.__index = Timer

local function arm(timer)
    timer.started = mp.get_time()
    timer.handle = mp.add_timeout(timer.remaining, function()
        timer.handle = nil
        live[timer] = nil
        timer.fn()
    end)
end

function M.pause_timer(seconds, fn)
    local timer = setmetatable({ remaining = seconds, fn = fn }, Timer)
    live[timer] = true
    if not paused then
        arm(timer)
    end
    return timer
end

function Timer:kill()
    if self.handle then
        self.handle:kill()
        self.handle = nil
    end
    live[self] = nil
end

mp.observe_property("pause", "bool", function(_, value)
    paused = value == true
    for timer in pairs(live) do
        if paused and timer.handle then
            timer.handle:kill()
            timer.handle = nil
            timer.remaining = math.max(0, timer.remaining - (mp.get_time() - timer.started))
        elseif not paused and not timer.handle then
            arm(timer)
        end
    end
end)

return M
