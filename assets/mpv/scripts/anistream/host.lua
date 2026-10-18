-- Notice when the host has gone, and let the window go with it.
--
-- The host keeps one mpv window open between episodes (`--idle=yes`), so a window whose host was
-- closed, crashed or killed would otherwise stay up forever on the loading card. Every few
-- seconds this asks whether the host's process still exists. Once it does not, `idle` is turned
-- off: an idle window closes at once, and an episode that is still playing finishes first and
-- then closes, which is how an orphaned mpv behaved before the window outlived its file.

local config = require("config")

local M = {}

local pid = tonumber(config.opts.host_pid) or 0
local platform = mp.get_property("platform") or ""

-- `kill -0` sends no signal; it only says whether the process exists. On Windows, tasklist
-- filtered to the pid prints the process when it is there.
local function probe()
    if platform == "windows" then
        return { "tasklist", "/FI", "PID eq " .. pid, "/NH", "/FO", "CSV" }
    end
    return { "kill", "-0", tostring(pid) }
end

local function alive(result)
    if platform == "windows" then
        return result and result.stdout and result.stdout:find('"' .. pid .. '"', 1, true) ~= nil
    end
    return result and result.status == 0
end

function M.start()
    if pid <= 0 then
        return
    end
    local timer
    local checking = false
    timer = mp.add_periodic_timer(math.max(0.2, tonumber(config.opts.host_check) or 5), function()
        if checking then
            return
        end
        checking = true
        -- Asynchronous, so a slow process table never stalls a frame.
        mp.command_native_async({
            name = "subprocess",
            args = probe(),
            playback_only = false,
            capture_stdout = true,
            capture_stderr = true,
        }, function(_, result)
            checking = false
            if not alive(result) then
                timer:kill()
                mp.msg.info("the host has gone; the window closes when nothing is playing")
                mp.set_property("idle", "no")
            end
        end)
    end)
end

return M
