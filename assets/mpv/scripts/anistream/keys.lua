-- Contextual key bindings: keys that belong to an element only while it is on screen.
--
-- S is mpv's screenshot key and Enter its playlist-next; both are taken with a forced binding
-- for exactly as long as the skip button (or a card) is showing and handed back the moment it
-- goes. Binding them for the whole session was the bug this replaces: S skipped nothing when
-- no skip was on offer, and screenshots stopped working.

local M = {}

local bound = {}

-- A handler that repeats while the key is held (arrow keys in a menu).
function M.repeating(fn)
    return { fn = fn, flags = { repeatable = true } }
end

local function handler(value)
    return type(value) == "table" and value.fn or value
end

-- `wanted` maps a key name to its handler (or `M.repeating(handler)`). Anything bound and no
-- longer wanted is released; a key whose handler changed (Enter moving from the skip button to
-- the Up Next card) is rebound.
function M.set(wanted)
    local changed = false
    for key, fn in pairs(bound) do
        if handler(wanted[key]) ~= fn then
            mp.remove_key_binding("ctx-" .. key)
            bound[key] = nil
            changed = true
        end
    end
    for key, value in pairs(wanted) do
        if not bound[key] then
            local fn = handler(value)
            mp.add_forced_key_binding(key, "ctx-" .. key, fn, type(value) == "table" and value.flags or nil)
            bound[key] = fn
            changed = true
        end
    end
    -- mpv applies binding changes when the script next goes idle; the published `ui` state
    -- follows this call, and must never say "shown" before the keys that go with it work.
    if changed then
        M.flush()
    end
end

function M.flush()
    if mp.flush_keybindings then
        mp.flush_keybindings()
    end
end

return M
