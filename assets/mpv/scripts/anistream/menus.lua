-- The anistream menu: episodes, sources, sub/dub, upscaling, tone mapping, night mode, and the
-- script's own mini player and subtitle sync.
--
-- One description, several renderings. In anistream mode this script draws it (menu_view.lua),
-- subtitle and audio tracks included, so every menu the controls open looks the same. In `mpv`
-- mode it fits the user's setup: a uosc menu when they run uosc (open-menu with a callback to
-- this script, kept current with update-menu while open), otherwise a chain of
-- `mp.input.select` pickers; and either way an "anistream" submenu in mpv's own context menu
-- (`menu-data`), appended beside whatever the user has there.
--
-- The host's `menus` key is the only source of truth for what is current. A pick is reported
-- as `choose` and nothing changes here until the host writes `menus` back.

local app = require("app")
local config = require("config")
local help = require("help")
local lang_names = require("lang")
local menu_view = require("menu_view")
local screen = require("screen")
local util = require("util")

local M = {}

-- Set by main.lua: the script-only features the menu can start.
M.actions = {}

local TYPE = "anistream"
local CONTEXT_TITLE = "anistream"

local input_ok, input = pcall(require, "mp.input")
if not input_ok or type(input) ~= "table" or type(input.select) ~= "function" then
    input = nil
end

---------------------------------------------------------------------------------------------
-- The description
---------------------------------------------------------------------------------------------

-- The tracks of one type as menu entries, for the drawn menu. Subtitles get an "Off".
local function track_entries(kind)
    local property = kind == "sub" and "sid" or "aid"
    local selected = mp.get_property_native(property)
    local list = {}
    if kind == "sub" then
        list[1] = { title = "Off", current = selected == false or selected == "no", command = "set sid no" }
    end
    for _, track in ipairs(mp.get_property_native("track-list") or {}) do
        if track.type == kind then
            local notes = {}
            local lang = util.str(track.lang)
            local title = util.str(track.title)
            -- A release that titles its tracks has named them; one that does not left a code,
            -- which reads better as the language it stands for.
            local named = title or lang_names.name(lang)
            if lang and named then
                notes[#notes + 1] = lang
            end
            if track.forced then
                notes[#notes + 1] = "forced"
            end
            if track.external then
                notes[#notes + 1] = "external"
            end
            list[#list + 1] = {
                title = named or lang or ("Track " .. tostring(track.id)),
                hint = table.concat(notes, " · "),
                current = track.selected == true,
                command = string.format("set %s %d", property, track.id),
            }
        end
    end
    return list
end

-- Entries are { title, hint, current, items | choose | action | command }. `style` is how the
-- menu will be shown ("drawn", "uosc", "select" or "context"); only the track menus differ.
local function entries(style)
    local m = util.tbl(app.host.menus) or {}
    local list = {}

    local function current_of(items)
        for _, item in ipairs(items) do
            if item.current then
                return item
            end
        end
        return nil
    end

    local episodes = {}
    for _, ep in ipairs(util.tbl(m.episodes) or {}) do
        local number = util.str(ep.number)
        if number then
            local label = util.str(ep.label) or ("Episode " .. number)
            local title = util.str(ep.title)
            local notes = {}
            if ep.filler == true then
                notes[#notes + 1] = "filler"
            end
            if ep.watched == true then
                notes[#notes + 1] = "watched"
            end
            episodes[#episodes + 1] = {
                title = title and (label .. "  " .. title) or label,
                short = label,
                hint = table.concat(notes, " · "),
                current = ep.current == true,
                choose = { menu = "episode", value = number },
            }
        end
    end
    if #episodes > 0 then
        local now = current_of(episodes)
        list[#list + 1] = { id = "episodes", title = "Episodes", hint = now and now.short, items = episodes }
    end

    local sources = {}
    for _, source in ipairs(util.tbl(m.sources) or {}) do
        local id = util.str(source.id)
        if id then
            sources[#sources + 1] = {
                title = util.str(source.label) or id,
                hint = util.str(source.detail),
                current = source.current == true,
                choose = { menu = "source", value = id },
            }
        end
    end
    if #sources > 0 then
        local now = current_of(sources)
        list[#list + 1] = { id = "sources", title = "Sources", hint = now and now.title, items = sources }
    end

    local translation = util.tbl(m.translation)
    if translation and util.tbl(translation.options) and #translation.options > 1 then
        local now = util.str(translation.current)
        local options = {}
        for _, option in ipairs(translation.options) do
            local value = util.str(option)
            if value then
                options[#options + 1] = {
                    title = util.capitalize(value),
                    current = value == now,
                    choose = { menu = "translation", value = value },
                }
            end
        end
        list[#list + 1] = {
            id = "translation",
            title = "Sub / Dub",
            hint = now and util.capitalize(now),
            items = options,
        }
    end

    local function option_menu(key, menu_name, id, title)
        local block = util.tbl(m[key])
        if not block or not util.tbl(block.options) then
            return
        end
        local now = util.str(block.current)
        local options = {}
        local now_label = nil
        for _, option in ipairs(block.options) do
            local value = util.str(option.id)
            if value then
                local label = util.str(option.label) or value
                if value == now then
                    now_label = label
                end
                options[#options + 1] = {
                    title = label,
                    hint = util.str(option.hint),
                    current = value == now,
                    choose = { menu = menu_name, value = value },
                }
            end
        end
        if #options > 0 then
            list[#list + 1] = { id = id, title = title, hint = now_label, items = options }
        end
    end
    option_menu("upscaling", "upscaling", "upscaling", "Upscaling")
    -- Only offered for HDR sources; the host sends null otherwise.
    option_menu("tone_mapping", "tone_mapping", "tone_mapping", "HDR tone mapping")

    if m.night_mode ~= nil then
        local on = m.night_mode == true
        list[#list + 1] = {
            title = "Night mode",
            hint = on and "On" or "Off",
            choose = { menu = "night_mode", value = not on },
            keep_open = true,
        }
    end

    -- A rule between what the host offers and what the player itself does.
    if #list > 0 then
        list[#list].separator = true
    end
    if style == "drawn" then
        list[#list + 1] = { id = "subtitles", title = "Subtitles", items = track_entries("sub") }
        list[#list + 1] = { id = "audio", title = "Audio", items = track_entries("audio") }
    elseif style == "uosc" then
        list[#list + 1] = { title = "Subtitles", command = "script-binding uosc/subtitles" }
        list[#list + 1] = { title = "Audio", command = "script-binding uosc/audio" }
    else
        list[#list + 1] = { title = "Subtitles", command = "script-binding select/select-sid" }
        list[#list + 1] = { title = "Audio", command = "script-binding select/select-aid" }
    end
    list[#list + 1] = { title = "Sync subtitles", action = "sub-sync" }
    list[#list + 1] = { title = "Mini player", hint = M.mini_on and M.mini_on() and "On" or nil, action = "mini" }
    -- The hint is the key as bound right now, so a rebind shows here too.
    list[#list + 1] = { title = "Keys", hint = help.key_for("keys"), action = "keys" }
    return list
end

-- What the video is: HDR format and upscaler. Shown in the menu rather than on the video.
local function media_note()
    local media = util.tbl(app.host.media) or {}
    local parts = {}
    if util.str(media.hdr) then
        parts[#parts + 1] = util.str(media.hdr)
    end
    if util.str(media.upscaler) then
        parts[#parts + 1] = util.str(media.upscaler)
    end
    return #parts > 0 and table.concat(parts, " · ") or nil
end

local function menu_title()
    local episode = util.tbl(app.host.episode) or {}
    return util.str(episode.show) or "anistream"
end

---------------------------------------------------------------------------------------------
-- Acting on a pick
---------------------------------------------------------------------------------------------

local function activate(entry)
    if entry.choose then
        util.emit("choose", entry.choose)
    elseif entry.action and M.actions[entry.action] then
        M.actions[entry.action]()
    elseif entry.command then
        mp.command(entry.command)
    end
end

---------------------------------------------------------------------------------------------
-- uosc
---------------------------------------------------------------------------------------------

local open_view = nil -- "root", or the id of a submenu opened on its own

local function uosc_item(entry, path)
    local item = {
        title = entry.title,
        hint = entry.hint ~= "" and entry.hint or nil,
        separator = entry.separator,
        keep_open = entry.keep_open,
    }
    if entry.items then
        item.id = entry.id
        item.items = {}
        for i, child in ipairs(entry.items) do
            item.items[i] = uosc_item(child, { path[1], i })
        end
    else
        -- The current pick carries a check rather than uosc's `active` fill, which would be a
        -- block of amber in a menu where amber marks only the focused row.
        item.icon = entry.current and "check" or nil
        item.value = path
    end
    return item
end

-- The menu as uosc data. Values are paths into `entries()` so a pick is resolved against the
-- menu as it is when picked, not as it was when opened.
local function uosc_data(view)
    local list = entries("uosc")
    local data = {
        type = TYPE,
        title = menu_title(),
        footnote = media_note(),
        callback = { mp.get_script_name(), "menu-event" },
        items = {},
    }
    if view ~= "root" then
        for i, entry in ipairs(list) do
            if entry.id == view and entry.items then
                data.title = entry.title
                for j, child in ipairs(entry.items) do
                    data.items[j] = uosc_item(child, { i, j })
                    if child.current then
                        data.selected_index = j
                    end
                end
                return data
            end
        end
        return nil
    end
    for i, entry in ipairs(list) do
        data.items[i] = uosc_item(entry, { i })
    end
    return data
end

local function resolve(path)
    if type(path) ~= "table" then
        return nil
    end
    local entry = entries("uosc")[tonumber(path[1])]
    if entry and path[2] then
        entry = entry.items and entry.items[tonumber(path[2])]
    end
    return entry
end

local uosc_menu_type = nil
mp.observe_property("user-data/uosc/menu/type", "native", function(_, value)
    uosc_menu_type = value
    if value ~= TYPE then
        open_view = nil
    end
end)

local function uosc_open(view)
    local data = uosc_data(view)
    if not data then
        return false
    end
    open_view = view
    screen.to_uosc("open-menu", util.format_json(data))
    return true
end

local function uosc_close()
    screen.to_uosc("close-menu", TYPE)
end

mp.register_script_message("menu-event", function(json)
    local event = util.parse_json(json)
    if event.type == "activate" then
        local entry = resolve(event.value)
        if not entry then
            return
        end
        if not entry.keep_open then
            uosc_close()
        end
        activate(entry)
    elseif event.type == "back" then
        uosc_close()
    end
end)

---------------------------------------------------------------------------------------------
-- mpv's own pickers
---------------------------------------------------------------------------------------------

local function select_open(view)
    if not input then
        mp.osd_message("This mpv has no menu support (mp.input needs mpv 0.39 or newer)")
        return
    end
    local list = entries("select")
    local prompt = menu_title()
    if view ~= "root" then
        for _, entry in ipairs(list) do
            if entry.id == view and entry.items then
                list, prompt = entry.items, entry.title
                break
            end
        end
    end
    local labels, default = {}, nil
    for i, entry in ipairs(list) do
        local label = entry.title
        if entry.items then
            label = label .. (entry.hint and ("  ·  " .. entry.hint) or "") .. "  ›"
        elseif entry.hint and entry.hint ~= "" then
            label = label .. "  ·  " .. entry.hint
        end
        if entry.current then
            label = label .. "  ✓"
            default = i
        end
        labels[i] = label
    end
    input.select({
        prompt = prompt .. ":",
        items = labels,
        default_item = default,
        submit = function(index)
            local entry = list[index]
            if not entry then
                return
            end
            if entry.items then
                -- A nested select replaces this one; defer so this one has closed first.
                mp.add_timeout(0, function()
                    select_open(entry.id)
                end)
            else
                activate(entry)
            end
        end,
    })
end

---------------------------------------------------------------------------------------------
-- mpv's context menu (`menu-data`, mpv 0.40+)
---------------------------------------------------------------------------------------------

-- A command-line argument mpv will read back verbatim.
local function quote(value)
    return '"' .. tostring(value):gsub("\\", "\\\\"):gsub('"', '\\"') .. '"'
end

local function context_items(list)
    local items = {}
    for _, entry in ipairs(list) do
        local item = { title = entry.title }
        if entry.hint and entry.hint ~= "" and not entry.items then
            item.title = entry.title .. "  ·  " .. entry.hint
        end
        if entry.items then
            item.type = "submenu"
            item.submenu = context_items(entry.items)
        elseif entry.choose then
            item.cmd = string.format(
                "script-message-to %s pick %s %s",
                mp.get_script_name(), entry.choose.menu, quote(tostring(entry.choose.value))
            )
        elseif entry.action then
            item.cmd = "script-binding " .. mp.get_script_name() .. "/" .. entry.action
        elseif entry.command then
            item.cmd = entry.command
        end
        if entry.current then
            item.state = { "checked" }
        end
        items[#items + 1] = item
    end
    return items
end

local function is_ours(item)
    return type(item) == "table" and item.title == CONTEXT_TITLE and item.type == "submenu"
end

local syncing = false

function M.sync_context_menu()
    if config.opts.uosc or syncing then
        return
    end
    local data = mp.get_property_native("menu-data")
    if type(data) ~= "table" then
        return -- older mpv: no context menu to join
    end
    local kept = {}
    for _, item in ipairs(data) do
        if not is_ours(item) then
            kept[#kept + 1] = item
        end
    end
    kept[#kept + 1] = { title = CONTEXT_TITLE, type = "submenu", submenu = context_items(entries("context")) }
    if util.format_json(kept) ~= util.format_json(data) then
        syncing = true
        mp.set_property_native("menu-data", kept)
        syncing = false
    end
end

-- Another script rewriting `menu-data` wholesale would drop our entry; put it back.
mp.observe_property("menu-data", "native", function(_, value)
    if type(value) ~= "table" or config.opts.uosc then
        return
    end
    for _, item in ipairs(value) do
        if is_ours(item) then
            return
        end
    end
    M.sync_context_menu()
end)

-- Picks from the context menu arrive here as `pick <menu> <value>`.
mp.register_script_message("pick", function(menu, value)
    if not menu or value == nil then
        return
    end
    local parsed = value
    if menu == "night_mode" then
        parsed = value == "true"
    end
    util.emit("choose", { menu = menu, value = parsed })
end)

---------------------------------------------------------------------------------------------
-- Public
---------------------------------------------------------------------------------------------

menu_view.entries = function()
    return entries("drawn")
end
menu_view.activate = activate
menu_view.title = menu_title
menu_view.note = media_note

function M.is_open()
    return menu_view.is_open() or uosc_menu_type == TYPE
end

-- Open the whole menu, or one of its lists (`episodes`, `sources`, `subtitles`, `audio`) on
-- its own. Asking for what is already open closes it, so one key toggles.
function M.open(view)
    view = view or "root"
    if config.opts.uosc then
        if menu_view.is_open() then
            menu_view.close()
        else
            menu_view.open(view)
        end
        return
    end
    -- Outside our own mode the track lists belong to the user's player UI.
    if view == "subtitles" or view == "audio" then
        local which = view == "subtitles" and "sid" or "aid"
        mp.command(screen.uosc() and ("script-binding uosc/" .. (view == "audio" and "audio" or "subtitles"))
            or ("script-binding select/select-" .. which))
        return
    end
    if screen.uosc() then
        if uosc_menu_type == TYPE and open_view == view then
            uosc_close()
        else
            uosc_open(view)
        end
    else
        select_open(view)
    end
end

-- The host rewrote `menus` (or the episode, or the media): refresh whatever is showing it.
function M.refresh()
    menu_view.refresh()
    if uosc_menu_type == TYPE and open_view then
        local data = uosc_data(open_view)
        if data then
            screen.to_uosc("update-menu", util.format_json(data))
        end
    end
    M.sync_context_menu()
end

-- Track changes reach the drawn menu's Subtitles and Audio lists while they are open.
mp.observe_property("track-list", "native", function()
    if menu_view.is_open() then
        menu_view.refresh()
    end
end)

return M
