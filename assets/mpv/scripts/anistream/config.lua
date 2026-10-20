-- Options, the palette, and what the other on-screen scripts are configured to occupy.
--
-- The palette arrives as `--script-opts-append=anistream-<name>=<value>` from the host, with
-- defaults equal to the values it sends, so the script also looks right when someone loads it
-- by hand. The player is always the dark palette: a video window does not follow the terminal.

local mp_options = require("mp.options")

local M = {}

M.opts = {
    ground = "#161A2E", -- dusk indigo
    text = "#E4E0D6", -- silk
    dim = "#9298B5", -- haze
    obi = "#F2A64B", -- amber, the one saturated accent
    state = "#7FC7D9", -- water
    alert = "#E06B54", -- vermilion
    font = "Zen Kaku Gothic New",
    -- Set by the host in its own launch mode, where it bundles uosc beside this script. It is
    -- also what "anistream mode" means here: the window is ours to paint between episodes.
    uosc = false,
    -- The host's process id, and how often (seconds) to check it is still running. See host.lua.
    host_pid = 0,
    host_check = 5,
    -- Whether overlay-add may scale the Up Next still, where mpv can (0.38). Off, the script
    -- resamples it as it must on older players; tests turn it off to run that on any mpv.
    scaled_still = true,
}
mp_options.read_options(M.opts, "anistream")

local function parse_hex(value, fallback)
    local hex = tostring(value or ""):gsub("^#", "")
    if not hex:match("^%x%x%x%x%x%x$") then
        hex = fallback:gsub("^#", "")
    end
    return {
        tonumber(hex:sub(1, 2), 16),
        tonumber(hex:sub(3, 4), 16),
        tonumber(hex:sub(5, 6), 16),
    }
end

-- `t` is how far from `a` toward `b`, matching the palette's own derivations.
local function mix(a, b, t)
    return {
        math.floor(a[1] + (b[1] - a[1]) * t + 0.5),
        math.floor(a[2] + (b[2] - a[2]) * t + 0.5),
        math.floor(a[3] + (b[3] - a[3]) * t + 0.5),
    }
end

-- ASS wants colours as &HBBGGRR&.
local function ass(rgb)
    return string.format("&H%02X%02X%02X&", rgb[3], rgb[2], rgb[1])
end

local defaults = {
    ground = "#161A2E",
    text = "#E4E0D6",
    dim = "#9298B5",
    obi = "#F2A64B",
    state = "#7FC7D9",
    alert = "#E06B54",
}
local rgb = {}
for name, fallback in pairs(defaults) do
    rgb[name] = parse_hex(M.opts[name], fallback)
end

M.color = {
    ground = ass(rgb.ground),
    text = ass(rgb.text),
    dim = ass(rgb.dim),
    obi = ass(rgb.obi),
    state = ass(rgb.state),
    alert = ass(rgb.alert),
    -- Derived, never picked, exactly as the TUI and the website derive them.
    rule = ass(mix(rgb.ground, rgb.text, 0.22)),
    raised = ass(mix(rgb.ground, rgb.text, 0.06)),
    -- A hovered button: one step past raised, enough to read as "under the pointer"
    -- without becoming a second accent.
    hover = ass(mix(rgb.ground, rgb.text, 0.14)),
}
M.rgb = rgb
M.font = M.opts.font ~= "" and M.opts.font or "Zen Kaku Gothic New"

-- Panels are flat ground at this opacity: the video stays faintly present behind them.
M.PANEL_OPACITY = 0.88

-- The type scale, in pixels at scale 1, shared by everything drawn. Sized to be read from a
-- sofa, not a desk: labels sit around 25 px cap height in a 1080p window on a HiDPI screen.
-- Hints (key names, counts, secondary notes) are one step down and never smaller.
M.TYPE = {
    body = 18,
    time = 18,
    hint = 15,
}

---------------------------------------------------------------------------------------------
-- Other scripts' options
--
-- The skip button and the cards sit above whatever bar is on screen, and the bar's height is
-- set by its own options. uosc does publish `user-data/osc/margins`, but only for elements that
-- are pinned visible — a bar that appears on hover reports zero, and a button placed against
-- that would sit under the bar the moment the pointer moves. So the footprint is computed from
-- the options instead, and the published margins only ever push it further up.
---------------------------------------------------------------------------------------------

local function read_conf(name)
    local values = {}
    local path = mp.find_config_file("script-opts/" .. name .. ".conf")
    if not path then
        return values
    end
    local file = io.open(path, "r")
    if not file then
        return values
    end
    for line in file:lines() do
        local key, value = line:match("^%s*([%w_%-]+)%s*=%s*(.-)%s*$")
        if key then
            values[key] = value
        end
    end
    file:close()
    return values
end

local conf_cache = {}
local cli = mp.get_property_native("options/script-opts") or {}
mp.observe_property("options/script-opts", "native", function(_, value)
    cli = type(value) == "table" and value or {}
end)

-- An option of another script: `--script-opts` first (that is how the host passes uosc's),
-- then the user's `script-opts/<script>.conf`, then the default.
function M.script_opt(script, key, default)
    local value = cli[script .. "-" .. key]
    if value == nil then
        conf_cache[script] = conf_cache[script] or read_conf(script)
        value = conf_cache[script][key]
    end
    if value == nil then
        return default
    end
    if type(default) == "number" then
        return tonumber(value) or default
    end
    return value
end

return M
