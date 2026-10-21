-- The compositor: one `ass-events` overlay for everything this script draws.
--
-- Elements register with a screen region and the compositor places them: the bottom-right
-- corner (skip button, Undo note, stall prompt, Up Next card) sits just above the player bar,
-- notices hang from the top, and the loading card covers the window. A corner shows one element
-- at a time — main.lua decides which — so the only overlap is the brief crossfade when one
-- hands the corner to another, anchored at the same edge.
--
-- Rendering is on demand. A change requests one frame; only a running fade keeps frames coming,
-- at 60 a second, and stops when the last fade lands.

local config = require("config")
local draw = require("draw")
local screen = require("screen")
local app = require("app")

local M = {}

-- Fade length, in seconds. Fades only — nothing here moves, scales or bounces.
local FADE = 0.15

local overlay = mp.create_osd_overlay("ass-events")
-- Under uosc's own overlay (z 0): its menus, curtain and window controls draw over ours.
overlay.z = -1

local elements = {}
local zones = {}
local hover_id = nil
local mouse = { x = -1, y = -1, inside = false }
local pending = false
local frame_timer = nil
local last = { data = nil, w = 0, h = 0 }

---------------------------------------------------------------------------------------------
-- Drawing context handed to elements
---------------------------------------------------------------------------------------------

local Ctx = {}
Ctx.__index = Ctx

-- A base size (designed at scale 1) in pixels at the current scale.
function Ctx:px(value)
    return value * self.s
end

function Ctx:rect(x0, y0, x1, y1, color, opacity, radius, corners, clip)
    draw.rect(self.out, x0, y0, x1, y1, color, opacity * self.alpha, radius, corners, clip)
end

-- Panels: flat ground with a small radius, the base of every element, edged with a one-pixel
-- hairline in the rule colour. On a bright frame the ground already separates the panel from
-- the picture; on a dark one the hairline is what still shows where it ends.
function Ctx:panel(x0, y0, x1, y1, opacity)
    opacity = opacity or config.PANEL_OPACITY
    self:rect(x0, y0, x1, y1, config.color.ground, opacity, self:px(4))
    self:ring(x0, y0, x1, y1, config.color.rule, opacity * 0.7)
end

function Ctx:ring(x0, y0, x1, y1, color, opacity)
    draw.ring(self.out, x0, y0, x1, y1, color, opacity * self.alpha, self:px(4),
        math.max(1, draw.round(self:px(1))))
end

function Ctx:circle(cx, cy, radius, color, opacity)
    draw.circle(self.out, cx, cy, radius, color, opacity * self.alpha)
end

-- The obi: a thin amber bar on the left edge of whatever is primary.
-- Drawn as the panel's own rounded left edge clipped to a few pixels, so the bar follows the
-- panel's corner curve exactly instead of poking past it.
function Ctx:obi(x0, y0, y1, color, opacity)
    local width = math.max(2, draw.round(self:px(3)))
    local radius = self:px(4)
    self:rect(x0, y0, x0 + radius * 2 + width, y1, color or config.color.obi, opacity or 1, radius,
        nil, { x0, y0, x0 + width, y1 })
end

-- `opts.size` is a base size; it is scaled here so callers never mix scaled and unscaled.
function Ctx:font(size, bold, spacing)
    return { size = size * self.s, bold = bold, spacing = spacing and spacing * self.s or 0 }
end

function Ctx:text(x, y, an, text, opts)
    local o = {}
    for k, v in pairs(opts) do
        o[k] = v
    end
    o.opacity = (opts.opacity or 1) * self.alpha
    draw.text(self.out, x, y, an, text, o)
end

function Ctx:width(text, opts)
    return draw.width(text, opts)
end

function Ctx:fit(text, opts, max_width)
    return draw.fit(text, opts, max_width)
end

-- A clickable area. Elements that are fading out draw but take no clicks. `fn` runs on
-- release; `drag`, if given, is told about the press, every move while held and the release
-- as `drag(x, y, phase)` with phase "down", "move" or "up" (the timeline's scrubbing).
function Ctx:zone(id, x0, y0, x1, y1, fn, drag)
    if self.interactive then
        zones[#zones + 1] = { id = id, x0 = x0, y0 = y0, x1 = x1, y1 = y1, fn = fn, drag = drag }
    end
end

---------------------------------------------------------------------------------------------
-- Elements and their fades
---------------------------------------------------------------------------------------------

-- `el` provides `region` ("backdrop" and "canvas" draw on the whole window; "corner",
-- "top-left", "top-center" and "center" are placed), `draw(ctx, x, y, w, h)` and, for placed
-- regions, `measure(ctx) -> w, h`. Registration order is drawing order.
function M.register(el)
    el.fade = { value = 0, from = 0, target = 0, t0 = 0 }
    elements[#elements + 1] = el
    return el
end

local function fade_value(el, now)
    local f = el.fade
    local t = (now - f.t0) / FADE
    if t >= 1 then
        return f.target
    end
    -- Ease out: the change is visible from the first frame and settles softly.
    t = 1 - (1 - t) * (1 - t)
    return f.from + (f.target - f.from) * t
end

-- Told when the fades under way have all landed, on the frame that lands the last of them.
local settle_listeners = {}
-- An element set off towards a new state since the screen was last still.
local settle_due = false

function M.show(el, visible)
    local target = visible and 1 or 0
    if el.fade.target == target then
        return
    end
    local now = mp.get_time()
    el.fade.from = fade_value(el, now)
    el.fade.target = target
    el.fade.t0 = now
    settle_due = true
    M.request()
end

function M.visible(el)
    return el.fade.target == 1
end

-- Whether any element is still on its way to where it was sent, as the last frame drew it.
function M.fading()
    for _, el in ipairs(elements) do
        if el.fade.value ~= el.fade.target then
            return true
        end
    end
    return false
end

-- Call `fn` each time the screen comes to rest: every fade landed, nothing more to draw for it.
-- What is published from a frame (the volume's form) is then the last frame's, for good.
function M.on_settle(fn)
    settle_listeners[#settle_listeners + 1] = fn
end

-- Things that move with the overlay's fades without being drawn by it — the subtitles lifting
-- with the seek bar — read the fade values here, on the same frames the overlay draws.
local frame_listeners = {}

function M.on_frame(fn)
    frame_listeners[#frame_listeners + 1] = fn
end

---------------------------------------------------------------------------------------------
-- Pointer
--
-- The left button is only bound while the pointer is over one of our buttons, so a click
-- anywhere else still pauses, drags the window or does whatever the user's input.conf says.
---------------------------------------------------------------------------------------------

local click_enabled = false
local pressed = nil -- the zone a drag started in, held until release

local function zone_at(x, y)
    for i = #zones, 1, -1 do
        local z = zones[i]
        if x >= z.x0 and x <= z.x1 and y >= z.y0 and y <= z.y1 then
            return z
        end
    end
    return nil
end

-- The button event can overtake the position update that goes with it (a click that moves the
-- pointer arrives before `mouse-pos` is observed to change), so it is read afresh.
local function sync_mouse()
    local pos = mp.get_property_native("mouse-pos")
    if type(pos) == "table" then
        mouse.x, mouse.y = pos.x or mouse.x, pos.y or mouse.y
    end
end

local update_hover

local function on_down()
    sync_mouse()
    local z = zone_at(mouse.x, mouse.y)
    if z and z.drag then
        pressed = z
        z.drag(mouse.x, mouse.y, "down")
    end
end

local function on_up()
    sync_mouse()
    if pressed then
        local z = pressed
        pressed = nil
        z.drag(mouse.x, mouse.y, "up")
        update_hover()
        return
    end
    -- Buttons act on release, like every other button: a press that slides off does nothing.
    -- A click that arrives as one event (mpv's `mouse x y 0 single`) only reaches here, so
    -- a drag zone takes it as a press and release on the spot.
    local z = zone_at(mouse.x, mouse.y)
    if z and z.drag then
        z.drag(mouse.x, mouse.y, "down")
        z.drag(mouse.x, mouse.y, "up")
    elseif z and z.fn then
        z.fn()
    end
end

-- Its own section rather than a forced binding: those allow window dragging, and dragging the
-- seek bar must scrub, not move the window. The cursor may still hide.
mp.set_key_bindings({
    { "mbtn_left", on_up, on_down },
    -- Two quick clicks on a button are two clicks, not a fullscreen toggle.
    { "mbtn_left_dbl", function() end },
}, "anistream-pointer", "force")

update_hover = function()
    local z = mouse.inside and zone_at(mouse.x, mouse.y) or nil
    local id = z and z.id or nil
    if (z or pressed) and not click_enabled then
        mp.enable_key_bindings("anistream-pointer", "allow-hide-cursor")
        click_enabled = true
    elseif not z and not pressed and click_enabled then
        mp.disable_key_bindings("anistream-pointer")
        click_enabled = false
    end
    if id ~= hover_id then
        hover_id = id
        M.request()
    end
end

-- Where the pointer is, in OSD pixels, and whether it is over the window.
function M.mouse()
    return mouse
end

function M.hovered()
    return hover_id
end

local mouse_listeners = {}

function M.on_mouse_move(fn)
    mouse_listeners[#mouse_listeners + 1] = fn
end

mp.observe_property("mouse-pos", "native", function(_, value)
    if type(value) ~= "table" then
        return
    end
    mouse.x, mouse.y, mouse.inside = value.x or -1, value.y or -1, value.hover ~= false
    if pressed then
        pressed.drag(mouse.x, mouse.y, "move")
    end
    update_hover()
    for _, fn in ipairs(mouse_listeners) do
        fn(mouse)
    end
end)

---------------------------------------------------------------------------------------------
-- Rendering
---------------------------------------------------------------------------------------------

function M.render()
    pending = false
    local p = app.player
    local w, h = p.osd_w, p.osd_h
    if w <= 0 or h <= 0 then
        -- No OSD to draw on (a headless run, or the window is not up yet). State, keys and
        -- the published `ui` property do not depend on drawing.
        w, h = 1280, 720
    end

    local s = screen.scale()
    local insets = screen.insets(s)
    local now = mp.get_time()
    local out = {}
    zones = {}

    local ctx = setmetatable({ out = out, s = s, w = w, h = h, hover = hover_id }, Ctx)
    local edge = 16 * s
    local corner_right = w - (insets.align_right or math.max(insets.right, edge))
    local corner_bottom = h - insets.bottom - 12 * s
    local top_y = insets.top + edge
    local animating = false

    for _, el in ipairs(elements) do
        local alpha = fade_value(el, now)
        if alpha ~= el.fade.target then
            animating = true
        end
        el.fade.value = alpha
        if alpha > 0 then
            ctx.alpha = alpha
            ctx.interactive = el.fade.target == 1
            if el.region == "backdrop" or el.region == "canvas" then
                el.draw(ctx, 0, 0, w, h)
            else
                local ew, eh = el.measure(ctx)
                if el.region == "corner" then
                    el.draw(ctx, corner_right - ew, corner_bottom - eh, ew, eh)
                elseif el.region == "top-left" then
                    el.draw(ctx, edge, top_y, ew, eh)
                elseif el.region == "top-center" then
                    el.draw(ctx, (w - ew) / 2, top_y, ew, eh)
                elseif el.region == "center" then
                    el.draw(ctx, (w - ew) / 2, (h - eh) / 2, ew, eh)
                end
            end
        end
    end

    for _, fn in ipairs(frame_listeners) do
        fn()
    end

    local data = table.concat(out, "\n")
    if data ~= last.data or w ~= last.w or h ~= last.h then
        overlay.res_x, overlay.res_y = w, h
        overlay.data = data
        if data == "" then
            overlay:remove()
        else
            overlay:update()
        end
        last.data, last.w, last.h = data, w, h
    end

    update_hover()

    -- At rest, after something was set moving: said, even when the move took no frame at all
    -- because the first frame came after its whole fade.
    if not animating and settle_due then
        settle_due = false
        for _, fn in ipairs(settle_listeners) do
            fn()
        end
    end

    if animating and not frame_timer then
        frame_timer = mp.add_timeout(1 / 60, function()
            frame_timer = nil
            M.render()
        end)
    end
end

function M.request()
    if not pending then
        pending = true
        mp.add_timeout(0, M.render)
    end
end

return M
