-- anistream's companion script for mpv.
--
-- The host (the anistream TUI) drives playback over JSON IPC and owns every decision; this
-- script draws the in-player parts of the app and reports what the viewer does. The contract
-- is protocol v1:
--
--   host -> script  state     user-data/anistream/{protocol,episode,next,status,menus,media}
--   host -> script  commands  script-message-to anistream <skip-offer|skip-clear|skipped|toast|
--                                                           stall|stall-clear|hello> <json>
--   script -> host  events    script-message anistream <event> [json]
--                             ready, next, previous, skip, undo-skip, play-next, cancel-next,
--                             choose, stall-switch, stall-wait
--   script -> tests           user-data/anistream/ui (what is visible right now)
--
-- Key bindings. Every one is named, so `<key> script-binding anistream/<name>` in input.conf
-- rebinds it. "While shown" keys are forced bindings that exist only while their element is on
-- screen; the rest of the time the key does whatever mpv or the user's input.conf says (S takes
-- screenshots, Enter steps the playlist, Esc leaves fullscreen).
--
--   name           default key          when
--   next           N                    always: next episode
--   previous       P                    always: previous episode (replaces mpv's show-progress)
--   menu           M, right click*      always: the anistream menu  (*anistream mode only)
--   episodes       -                    always: the episode list on its own
--   sources        -                    always: the source list on its own
--   subtitles      -                    always: subtitle tracks (anistream mode: our menu)
--   audio          -                    always: audio tracks (anistream mode: our menu)
--   mini           Alt+m                always: mini player on/off
--   sub-sync       Alt+z                always: start (or stop) two-point subtitle sync
--   skip           S, Enter             while the skip button is shown
--   undo-skip      u                    while "Skipped … · Undo" is shown
--   play-next      Enter                while Up Next is shown
--   cancel-next    Esc                  while Up Next is shown
--   stall-switch   Enter                while the stall prompt is shown
--   stall-wait     Esc                  while the stall prompt is shown
--   (sync)         Enter mark, Esc stop while subtitle sync is running
--   (menu)         arrows, PgUp/PgDn,   while our menu is open (anistream mode): move, open,
--                  Home/End, Enter,     back; a click outside closes it
--                  Esc/BS, wheel
--   volume-up      wheel up*            always: volume +2  (*anistream mode only)
--   volume-down    wheel down*          always: volume -2  (*anistream mode only)
--   pause-click    left click*          on the picture: pause/resume after the double-click
--                                       time; a double click stays mpv's fullscreen toggle
--
-- Buttons also take a left click; the left button is bound only while the pointer is over one,
-- in a section that keeps the window from being dragged instead. None of the "while shown" keys
-- are held while uosc's menu or mpv's console owns the keyboard.

local config = require("config")
local util = require("util")
local app = require("app")
local overlay = require("overlay")
local keys = require("keys")
local screen = require("screen")
local menus = require("menus")
local menu_view = require("menu_view")
local mini = require("mini")
local subsync = require("subsync")
local timeline = require("timeline")
local volume = require("volume")
local loading = require("elements/loading")
local skip = require("elements/skip")
local skipped = require("elements/skipped")
local stall = require("elements/stall")
local up_next = require("elements/up_next")
local toast = require("elements/toast")
local pause_flash = require("elements/pause_flash")
local host = require("host")
local subs = require("subs")

local PROTOCOL = 1

-- Drawing order, back to front.
overlay.register(loading)
overlay.register(timeline)
overlay.register(volume)
overlay.register(skip)
overlay.register(skipped)
overlay.register(stall)
overlay.register(up_next)
overlay.register(toast)
overlay.register(subsync.element)
overlay.register(pause_flash)
overlay.register(menu_view)

-- The bottom-right corner holds one element at a time, the first of these that has something
-- to show. A stall outranks everything (nothing else matters if the video is not moving); Up
-- Next outranks an ending's skip button, since playing on is the better way out of an ending.
local CORNER = { stall, up_next, skipped, skip }

---------------------------------------------------------------------------------------------
-- The one refresh: visibility, keys, published state
---------------------------------------------------------------------------------------------

local last_ui = nil

local function publish()
    local ui = {
        skip = { visible = overlay.visible(skip), label = skip.label() },
        skipped = { visible = overlay.visible(skipped) },
        up_next = {
            visible = overlay.visible(up_next),
            countdown = overlay.visible(up_next) and up_next.countdown() or nil,
            -- Beyond the brief's shape: whether the next episode's still is on screen.
            still = up_next.still(),
        },
        loading = { visible = overlay.visible(loading) and loading.card_visible() },
        stall = { visible = overlay.visible(stall) },
        toast = toast.text(),
        mini = mini.active(),
        -- Not in the brief's shape: the subtitle-sync step (1-4) while it runs.
        sync = subsync.step(),
        -- Also beyond the brief: our own menu and seek bar, in anistream mode.
        menu = menu_view.view(),
        timeline = overlay.visible(timeline),
        -- Which form the volume control last drew, "row" or "pill": what a fade showed.
        volume = volume.drawn(),
    }
    local json = util.format_json(ui)
    if json ~= last_ui then
        last_ui = json
        mp.set_property_native("user-data/anistream/ui", ui)
    end
end

-- In `mpv` mode the loading card replaces whatever the user's player UI shows on an idle
-- window (uosc's idle indicator, the stock OSC's logo) for as long as it is up, and gives it
-- back after. In anistream mode uosc-opts.conf already turns uosc's off.
local covering = false
local logo_was_on = false
local function cover_idle_indicators(on)
    if on == covering or config.opts.uosc then
        return
    end
    covering = on
    if screen.uosc() then
        screen.to_uosc("disable-elements", mp.get_script_name(), on and "idle_indicator" or "")
    end
    if mp.get_property_native("osc") == true then
        if on then
            logo_was_on = mp.get_property_native("user-data/osc/idlescreen") ~= false
            if logo_was_on then
                mp.commandv("script-message-to", "osc", "osc-idlescreen", "no", "no-osd")
            end
        elseif logo_was_on then
            mp.commandv("script-message-to", "osc", "osc-idlescreen", "yes", "no-osd")
        end
    end
end

local function refresh()
    loading.update()
    overlay.show(loading, loading.wants())
    cover_idle_indicators(loading.card_visible())

    -- A loading card covers the video, and nothing in the corner is about that video any more.
    local winner = nil
    if not loading.card_visible() then
        for _, el in ipairs(CORNER) do
            if el.wants() then
                winner = el
                break
            end
        end
    end
    for _, el in ipairs(CORNER) do
        overlay.show(el, el == winner)
    end
    up_next.sync_still(winner == up_next)
    overlay.show(toast, toast.wants())
    overlay.show(subsync.element, subsync.element.wants())
    overlay.show(menu_view, menu_view.wants())
    local seekbar = timeline.wants() and not menu_view.is_open() and not loading.card_visible()
    overlay.show(timeline, seekbar)
    timeline.observe(seekbar)
    -- Subtitles step out of the way of the bar while it is up.
    subs.follow(seekbar)
    overlay.show(volume, volume.wants() and not menu_view.is_open() and not loading.card_visible())
    overlay.show(pause_flash, pause_flash.wants() and not menu_view.is_open()
        and not loading.card_visible())

    -- The open menu owns the keyboard; then a running sync (Enter marks, Esc stops); then
    -- whatever holds the corner.
    local wanted = {}
    if not app.modal then
        for _, el in ipairs({ menu_view, subsync.element, winner }) do
            if overlay.visible(el) then
                for key, fn in pairs(el.keys()) do
                    if wanted[key] == nil then
                        wanted[key] = fn
                    end
                end
            end
        end
    end
    keys.set(wanted)

    publish()
    overlay.request()
end

app.changed = refresh

---------------------------------------------------------------------------------------------
-- Host state
---------------------------------------------------------------------------------------------

local HOST_KEYS = { "protocol", "episode", "next", "status", "menus", "media" }

for _, key in ipairs(HOST_KEYS) do
    mp.observe_property("user-data/anistream/" .. key, "native", function(_, value)
        app.host[key] = value
        if key == "next" then
            up_next.set(value)
        elseif key == "menus" or key == "episode" or key == "media" then
            menus.refresh()
        end
        refresh()
    end)
end

---------------------------------------------------------------------------------------------
-- Host commands
---------------------------------------------------------------------------------------------

mp.register_script_message("skip-offer", function(json)
    skip.offer(util.parse_json(json))
end)

mp.register_script_message("skip-clear", function()
    skip.clear()
end)

mp.register_script_message("skipped", function(json)
    skipped.show(util.parse_json(json))
end)

mp.register_script_message("toast", function(json)
    local payload = util.parse_json(json)
    toast.show(payload.text, util.str(payload.level), util.num(payload.duration))
end)

mp.register_script_message("stall", function(json)
    stall.show(util.parse_json(json))
end)

mp.register_script_message("stall-clear", function()
    stall.clear()
end)

---------------------------------------------------------------------------------------------
-- Player state
---------------------------------------------------------------------------------------------

local p = app.player

mp.observe_property("idle-active", "bool", function(_, value)
    p.idle = value == true
    up_next.evaluate()
    refresh()
end)

mp.observe_property("duration", "number", function(_, value)
    p.duration = value
    up_next.evaluate()
end)

mp.observe_property("speed", "number", function(_, value)
    p.speed = value or 1
    up_next.evaluate()
end)

mp.observe_property("osd-dimensions", "native", function(_, value)
    if type(value) == "table" then
        p.osd_w, p.osd_h = value.w or 0, value.h or 0
        overlay.request()
    end
end)

mp.observe_property("display-hidpi-scale", "number", function(_, value)
    p.hidpi = value or 1
    overlay.request()
end)

mp.observe_property("fullscreen", "bool", function(_, value)
    p.fullscreen = value == true
    overlay.request()
end)

mp.observe_property("window-maximized", "bool", function(_, value)
    p.maximized = value == true
    overlay.request()
end)

mp.observe_property("vo-configured", "bool", function(_, value)
    p.vo_configured = value == true
    overlay.request()
end)

mp.observe_property("current-tracks/audio", "native", function(_, value)
    p.has_audio = value ~= nil
    refresh()
end)

-- Another script's modal UI owns the keyboard while it is open.
local modal = {}
local function watch_modal(property)
    mp.observe_property(property, "native", function(_, value)
        modal[property] = value ~= nil and value ~= false
        local any = false
        for _, open in pairs(modal) do
            any = any or open
        end
        if any ~= app.modal then
            app.modal = any
            refresh()
        end
    end)
end
watch_modal("user-data/uosc/menu/type")
watch_modal("user-data/mpv/console/open")
watch_modal("user-data/mpv/context-menu/open")

-- Per-file state does not outlive its file: an offer, a stall or a cancelled Up Next belonged
-- to the episode that just ended.
local function new_file()
    skip.clear()
    skipped.clear()
    stall.clear()
    up_next.reset()
    if subsync.step() then
        subsync.cancel()
    end
end
mp.register_event("start-file", new_file)
mp.register_event("end-file", function()
    skip.clear()
    stall.clear()
end)

---------------------------------------------------------------------------------------------
-- Bringing the controls up
--
-- In anistream mode the resting state is just the video. uosc's control row and our seek bar
-- come up when the pointer moves (anywhere, not only near the bottom), the video pauses or a
-- seek lands, and fade out once mpv hides the cursor again. uosc's `flash-elements` shows its
-- row for its `flash_duration`, which uosc-opts.conf sets to mpv's default cursor-hide delay.
---------------------------------------------------------------------------------------------

if config.opts.uosc then
    local last_flash = -math.huge
    local function wake()
        if p.idle or menu_view.is_open() or app.modal or loading.card_visible() then
            return
        end
        timeline.poke()
        local now = mp.get_time()
        if now - last_flash > 0.25 then
            last_flash = now
            screen.to_uosc("flash-elements", "controls")
        end
    end
    local seen_pointer = false
    overlay.on_mouse_move(function(mouse)
        -- The first report is where the pointer already was, not a movement.
        if seen_pointer and mouse.inside then
            wake()
        end
        seen_pointer = true
    end)
    -- Compared with the value at start-up rather than "skip the first report": a pause toggled
    -- before the first report arrives would otherwise be taken for the starting state.
    local was_paused = mp.get_property_native("pause") == true
    mp.observe_property("pause", "bool", function(_, value)
        if (value == true) ~= was_paused then
            was_paused = value == true
            wake()
        end
    end)
    mp.register_event("seek", wake)
end

---------------------------------------------------------------------------------------------
-- Bindings
---------------------------------------------------------------------------------------------

subsync.notify = function(text, alert)
    toast.show(text, alert and "alert" or "info", 4)
end
menus.actions = { mini = mini.toggle, ["sub-sync"] = subsync.start }
menus.mini_on = mini.active

mp.add_key_binding("N", "next", function()
    util.emit("next")
    toast.show("Next episode…", "info", 1.5)
end)
mp.add_key_binding("P", "previous", function()
    util.emit("previous")
    toast.show("Previous episode…", "info", 1.5)
end)
mp.add_key_binding("M", "menu", function()
    menus.open("root")
end)
-- Right click is mpv's pause toggle by default, and the wheel seeks. In our own mode right
-- click opens the menu and the wheel sets the volume, as in most players; in `mpv` mode the
-- user's setup keeps both.
if config.opts.uosc then
    mp.add_key_binding("MBTN_RIGHT", "menu-pointer", function()
        menus.open("root")
    end)
    mp.add_key_binding("WHEEL_UP", "volume-up", function()
        volume.step(2)
    end)
    mp.add_key_binding("WHEEL_DOWN", "volume-down", function()
        volume.step(-2)
    end)

    -- A click on the picture pauses, a double click goes fullscreen (mpv's own MBTN_LEFT_DBL),
    -- so the single click waits out the double-click time before acting: otherwise every
    -- double click would also pause and unpause underneath the fullscreen change. A press that
    -- turns into a window drag never sends its release here, so dragging does not pause.
    local pending = nil
    local down_at = nil
    mp.add_key_binding("MBTN_LEFT", "pause-click", function(event)
        local pos = mp.get_property_native("mouse-pos") or {}
        if event.event == "down" then
            down_at = { x = pos.x or 0, y = pos.y or 0 }
            return
        end
        if event.event ~= "up" and event.event ~= "press" then
            return
        end
        if down_at and math.abs((pos.x or 0) - down_at.x) + math.abs((pos.y or 0) - down_at.y) > 8 then
            down_at = nil
            return
        end
        down_at = nil
        if pending then
            -- The second click of a double click: fullscreen is mpv's, the pause is cancelled.
            pending:kill()
            pending = nil
            return
        end
        local wait = (mp.get_property_number("input-doubleclick-time") or 300) / 1000
        pending = mp.add_timeout(wait, function()
            pending = nil
            mp.commandv("cycle", "pause")
        end)
    end, { complex = true })
end
mp.add_key_binding(nil, "episodes", function()
    menus.open("episodes")
end)
mp.add_key_binding(nil, "sources", function()
    menus.open("sources")
end)
mp.add_key_binding(nil, "subtitles", function()
    menus.open("subtitles")
end)
mp.add_key_binding(nil, "audio", function()
    menus.open("audio")
end)
mp.add_key_binding("Alt+m", "mini", mini.toggle)
mp.add_key_binding("Alt+z", "sub-sync", subsync.start)

mp.add_key_binding(nil, "skip", skip.take)
mp.add_key_binding(nil, "undo-skip", skipped.undo)
mp.add_key_binding(nil, "play-next", up_next.play_now)
mp.add_key_binding(nil, "cancel-next", up_next.cancel)
mp.add_key_binding(nil, "stall-switch", stall.switch)
mp.add_key_binding(nil, "stall-wait", stall.wait)

---------------------------------------------------------------------------------------------
-- Hello
---------------------------------------------------------------------------------------------

local function ready()
    util.emit("ready", {
        protocol = PROTOCOL,
        uosc = screen.uosc(),
        mpv = mp.get_property("mpv-version") or "",
    })
end

-- `ready` goes out once at start-up, which can be before the host's IPC connection exists:
-- mpv does not hold client messages for clients that connect later. A host that connected
-- late asks again with `script-message-to anistream hello`.
mp.register_script_message("hello", ready)

refresh()
menus.sync_context_menu()
host.start()
-- Every binding above is live before the host hears we are ready.
keys.flush()
ready()
