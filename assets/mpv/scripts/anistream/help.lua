-- The keys overlay: what each key does in this player, opened with ? or from the menu.
--
-- Nothing in it is written down twice. Each row names a command (one of ours, or one of mpv's
-- own such as `cycle pause`) and the keys come from `input-bindings` at the moment it opens: a
-- key appears only if pressing it would run that command now, so a rebind in input.conf shows
-- up and a key the user took for something else disappears. Keys that exist only while an
-- element is on screen (S while a skip is offered) come from that element's own `KEYS` table,
-- marked "while shown".
--
-- Drawn as the menu is drawn: a flat panel, rows separated by spacing alone, two columns. Key
-- names are set apart by colour and tracking rather than boxed keycaps.

local app = require("app")
local config = require("config")
local skip = require("elements/skip")
local skipped = require("elements/skipped")
local up_next = require("elements/up_next")

local M = { id = "help", region = "center" }

-- Base metrics, at scale 1.
local PAD_X = 24
local PAD_Y = 12
local HEADER = 40
local GROUP = 27 -- a group's heading row
local GROUP_GAP = 8 -- extra space above every heading but the first
local ROW = 27
local COLUMN_GAP = 40
local KEY_GAP = 14 -- between a label and its keys
local MAX_KEYS = 3

local open = false
local columns = nil -- built when opened: { { {heading=...} | {label, note, keys}, ... }, ... }
local footer = nil
local shown = nil -- what was last drawn, for the fade-out

---------------------------------------------------------------------------------------------
-- Keys, as a viewer reads them
---------------------------------------------------------------------------------------------

local NAMES = {
    SPACE = "Space", ENTER = "Enter", ESC = "Esc", BS = "Backspace", TAB = "Tab",
    LEFT = "←", RIGHT = "→", UP = "↑", DOWN = "↓",
    PGUP = "PgUp", PGDWN = "PgDn", HOME = "Home", END = "End", DEL = "Del", INS = "Ins",
    MBTN_LEFT = "Click", MBTN_LEFT_DBL = "Double-click", MBTN_RIGHT = "Right-click",
    MBTN_MID = "Middle-click", MBTN_BACK = "Back button", MBTN_FORWARD = "Forward button",
    WHEEL_UP = "Wheel", WHEEL_DOWN = "Wheel", MENU = "Menu key", SHARP = "#",
}

-- `N` is Shift+N to mpv, and a keycap reads "N" for the plain key: say so.
function M.pretty(key)
    local mods, base = {}, key
    while true do
        local mod, rest = base:match("^(%a+)%+(.+)$")
        if not mod or #rest == 0 then
            break
        end
        mods[#mods + 1] = mod:sub(1, 1):upper() .. mod:sub(2):lower()
        base = rest
    end
    local name = NAMES[base:upper()] or NAMES[base]
    if not name then
        if base:match("^%u$") then
            mods[#mods + 1] = "Shift"
            name = base
        elseif base:match("^%l$") then
            name = base:upper()
        elseif base:match("^KP_") then
            name = "Keypad " .. (NAMES[base:sub(4)] or base:sub(4))
        else
            name = base
        end
    end
    mods[#mods + 1] = name
    return table.concat(mods, "+")
end

-- Media keys on keyboards and remotes; true, but not what anyone looks up.
local MEDIA = {
    PLAY = true, PAUSE = true, PLAYPAUSE = true, PLAYONLY = true, PAUSEONLY = true, STOP = true,
    FORWARD = true, REWIND = true, NEXT = true, PREV = true, VOLUME_UP = true,
    VOLUME_DOWN = true, MUTE = true, POWER = true, MENU = true, CLOSE_WIN = true,
}

-- Which keys to show first: letters and digits, then named keys, the pointer and the wheel,
-- punctuation, the keypad (it mostly repeats keys already listed), media keys last.
local function rank(key)
    local base = key:match("([^+]+)$") or key
    if MEDIA[base] then
        return 6
    end
    if key:find("KP_") then
        return 5
    end
    if #base == 1 and not base:match("%w") then
        return 4
    end
    if key:find("MBTN") or key:find("WHEEL") then
        return 3
    end
    if #base == 1 then
        return 1
    end
    return 2
end

-- Bindings that only exist for a moment: a script's pointer zones (uosc's buttons, our own
-- click areas, enabled while the pointer is over them) and our keys for whatever is on screen.
-- They would otherwise stand in for the lasting binding underneath.
local function transient(b)
    if b.owner and b.section ~= "input_" .. b.owner and b.section ~= "input_forced_" .. b.owner then
        return true
    end
    return tostring(b.cmd):find("/ctx%-") ~= nil
end

-- For every key, the binding that would run if it were pressed now: the highest priority that
-- is active (mpv reports inactive ones as negative).
local function effective()
    local best = {}
    for _, b in ipairs(mp.get_property_native("input-bindings") or {}) do
        local p = tonumber(b.priority) or 0
        if b.key and b.cmd and p >= 0 and not transient(b) then
            local current = best[b.key]
            if not current or p > (tonumber(current.priority) or 0) then
                best[b.key] = b
            end
        end
    end
    return best
end

-- Does `cmd` run one of `wants`? A want is a script binding of ours (`name`) or an mpv
-- command prefix (`cmd`).
local function runs(cmd, wants)
    -- input.conf aligns its columns with spaces (`seek  5`); compare words, not spacing.
    cmd = cmd:gsub("%s+", " "):gsub("^ ", ""):gsub(" $", ""):gsub("^nonscalable ", "")
    for _, want in ipairs(wants) do
        if want.name and cmd == "script-binding " .. mp.get_script_name() .. "/" .. want.name then
            return true
        end
        if want.cmd and (cmd == want.cmd or cmd:sub(1, #want.cmd + 1) == want.cmd .. " ") then
            return true
        end
    end
    return false
end

local function keys_for(best, wants)
    local found = {}
    for key, binding in pairs(best) do
        if runs(binding.cmd, wants) then
            found[#found + 1] = key
        end
    end
    return found
end

-- The keys an element takes while shown that run `fn`.
local function context_keys(table_, fn)
    local found = {}
    for key, handler in pairs(table_ or {}) do
        if handler == fn then
            found[#found + 1] = key
        end
    end
    return found
end

-- Pretty, de-duplicated, best first, at most a few.
local function present(keys)
    table.sort(keys, function(a, b)
        if rank(a) ~= rank(b) then
            return rank(a) < rank(b)
        end
        return a < b
    end)
    local plain = {}
    local has_keyboard = false
    for _, key in ipairs(keys) do
        plain[key] = true
        has_keyboard = has_keyboard or rank(key) < 6
    end
    local out, seen = {}, {}
    for _, key in ipairs(keys) do
        local name = M.pretty(key)
        -- Media keys only when nothing else does it.
        if rank(key) == 6 and has_keyboard then
            name = nil
        end
        -- s and S both bound: "S" says it; "Shift+S" beside it is noise.
        if #key == 1 and key:match("%u") and plain[key:lower()] then
            name = nil
        end
        -- Keypad Enter is Enter; the wheel's two directions are one entry.
        if name == "Keypad Enter" then
            name = "Enter"
        end
        if name and not seen[name] and #out < MAX_KEYS then
            seen[name] = true
            out[#out + 1] = name
        end
    end
    return out
end

-- The current key for one of our bindings, for the menu's hint.
function M.key_for(name)
    local keys = present(keys_for(effective(), { { name = name } }))
    return keys[1]
end

---------------------------------------------------------------------------------------------
-- What is listed
---------------------------------------------------------------------------------------------

-- Each row: a label, what it runs, and (for keys that exist only while something is on
-- screen) the element's table and handler.
local function layout()
    return {
        {
            { heading = "Playing" },
            { "Play or pause", { { cmd = "cycle pause" }, { name = "pause-click" } } },
            { "Back or forward 5 s", { { cmd = "seek -5" }, { cmd = "seek 5" } } },
            { "Volume", { { cmd = "add volume" }, { name = "volume-up" }, { name = "volume-down" } } },
            { "Mute", { { cmd = "cycle mute" } } },
            { "Fullscreen", { { cmd = "cycle fullscreen" } } },
            { heading = "Episodes" },
            { "Next episode", { { name = "next" } } },
            { "Previous episode", { { name = "previous" } } },
            { "Play the next one now", { { name = "play-next" } }, up_next.KEYS, up_next.play_now },
            { "Stay on this episode", { { name = "cancel-next" } }, up_next.KEYS, up_next.cancel },
        },
        {
            { heading = "Skipping" },
            { "Skip the opening or ending", { { name = "skip" } }, skip.KEYS, skip.take },
            { "Undo an automatic skip", { { name = "undo-skip" } }, skipped.KEYS, skipped.undo },
            { heading = "Menus" },
            { "Menu", { { name = "menu" }, { name = "menu-pointer" } } },
            { "Episodes", { { name = "episodes" } } },
            { "Subtitles", { { name = "subtitles" } } },
            { "Audio", { { name = "audio" } } },
            { "These keys", { { name = "keys" } } },
            { heading = "Tools" },
            { "Mini player", { { name = "mini" } } },
            { "Sync subtitles", { { name = "sub-sync" } } },
        },
    }
end

-- Resolve every row against the bindings as they are now. Rows with no key at all are left
-- out, and so is a heading left with nothing under it.
local function build()
    local best = effective()
    local result = {}
    for _, column in ipairs(layout()) do
        local rows = {}
        local pending_heading = nil
        for _, spec in ipairs(column) do
            if spec.heading then
                pending_heading = { heading = spec.heading }
            else
                local always = keys_for(best, spec[2])
                local contextual = spec[3] and context_keys(spec[3], spec[4]) or {}
                local keys = present(#always > 0 and always or contextual)
                local note = (#always == 0 and #contextual > 0) and "while shown" or nil
                if #keys > 0 then
                    if pending_heading then
                        rows[#rows + 1] = pending_heading
                        pending_heading = nil
                    end
                    rows[#rows + 1] = { label = spec[1], keys = keys, note = note }
                end
            end
        end
        if #rows > 0 then
            result[#result + 1] = rows
        end
    end
    -- mpv's own complete list, for anything not here: stats page 4.
    local stats = present(keys_for(best, { { cmd = "script-binding stats/display-stats-toggle" } }))
    footer = stats[1] and ("All of mpv's keys: " .. stats[1] .. ", then 4") or nil
    return result
end

---------------------------------------------------------------------------------------------
-- Open and close
---------------------------------------------------------------------------------------------

M.before_open = function() end

function M.open()
    M.before_open()
    columns = build()
    open = true
    app.changed()
end

function M.close()
    if open then
        open = false
        app.changed()
    end
end

function M.toggle()
    if open then
        M.close()
    else
        M.open()
    end
end

function M.is_open()
    return open
end

function M.wants()
    return open
end

local KEYS = { ESC = M.close }

-- While open it holds only its close key; ? (its own binding) closes it too.
function M.keys()
    return KEYS
end

---------------------------------------------------------------------------------------------
-- Drawing
---------------------------------------------------------------------------------------------

local function fonts(ctx)
    local label = ctx:font(config.TYPE.body)
    label.color = config.color.text
    local quiet = ctx:font(config.TYPE.hint)
    quiet.color = config.color.dim
    -- Keys: silk, one step down, tracked out a little so they read as names, not words.
    local key = ctx:font(config.TYPE.time - 1, false, 0.6)
    key.color = config.color.text
    return label, quiet, key
end

local function row_width(ctx, row, label_font, quiet_font, key_font)
    if row.heading then
        return ctx:width(row.heading, quiet_font)
    end
    local w = ctx:width(row.label, label_font) + ctx:px(KEY_GAP) + ctx:width(table.concat(row.keys, "  "), key_font)
    if row.note then
        w = w + ctx:px(8) + ctx:width(row.note, quiet_font)
    end
    return w
end

local function column_height(ctx, rows)
    local h = 0
    for i, row in ipairs(rows) do
        if row.heading then
            h = h + GROUP + (i > 1 and GROUP_GAP or 0)
        else
            h = h + ROW
        end
    end
    return ctx:px(h)
end

local function geometry(ctx, cols)
    local label_font, quiet_font, key_font = fonts(ctx)
    local widths, height = {}, 0
    for i, rows in ipairs(cols) do
        local w = 0
        for _, row in ipairs(rows) do
            w = math.max(w, row_width(ctx, row, label_font, quiet_font, key_font))
        end
        widths[i] = w
        height = math.max(height, column_height(ctx, rows))
    end
    local total = ctx:px(PAD_X * 2) + ctx:px(COLUMN_GAP) * (#cols - 1)
    for _, w in ipairs(widths) do
        total = total + w
    end
    local footer_h = footer and ctx:px(ROW + 6) or 0
    return widths, total, ctx:px(HEADER + PAD_Y * 2) + height + footer_h
end

function M.measure(ctx)
    shown = columns and { columns = columns, footer = footer } or shown
    if not shown then
        return 0, 0
    end
    local _, w, h = geometry(ctx, shown.columns)
    return math.min(w, ctx.w - ctx:px(32)), h
end

function M.draw(ctx, x, y, w, h)
    if not shown then
        return
    end
    local label_font, quiet_font, key_font = fonts(ctx)
    local widths = geometry(ctx, shown.columns)

    -- A click anywhere but the panel closes it; a click on the panel does nothing.
    ctx:zone("help-outside", 0, 0, ctx.w, ctx.h, M.close)
    ctx:panel(x, y, x + w, y + h, 0.94)
    ctx:zone("help-panel", x, y, x + w, y + h, function() end)

    local hy = y + ctx:px(PAD_Y + HEADER / 2)
    ctx:text(x + ctx:px(PAD_X), hy, 4, "Keys", label_font)
    ctx:text(x + w - ctx:px(PAD_X), hy, 6, "Esc closes", quiet_font)

    local cx = x + ctx:px(PAD_X)
    local top = y + ctx:px(PAD_Y + HEADER)
    for i, rows in ipairs(shown.columns) do
        local cy = top
        local right = cx + widths[i]
        for j, row in ipairs(rows) do
            if row.heading then
                cy = cy + (j > 1 and ctx:px(GROUP_GAP) or 0)
                ctx:text(cx, cy + ctx:px(GROUP) / 2, 4, row.heading, quiet_font)
                cy = cy + ctx:px(GROUP)
            else
                local mid = cy + ctx:px(ROW) / 2
                ctx:text(cx, mid, 4, row.label, label_font)
                if row.note then
                    ctx:text(cx + ctx:width(row.label, label_font) + ctx:px(8), mid, 4, row.note, quiet_font)
                end
                ctx:text(right, mid, 6, table.concat(row.keys, "  "), key_font)
                cy = cy + ctx:px(ROW)
            end
        end
        cx = right + ctx:px(COLUMN_GAP)
    end

    if shown.footer then
        ctx:text(x + ctx:px(PAD_X), y + h - ctx:px(PAD_Y + ROW / 2), 4, shown.footer, quiet_font)
    end
end

-- What tests read: the rows as listed, `label -> "Key  Key"`.
function M.listed()
    local out = {}
    for _, rows in ipairs(columns or {}) do
        for _, row in ipairs(rows) do
            if row.label then
                out[row.label] = table.concat(row.keys, "  ")
            end
        end
    end
    return out
end

return M
