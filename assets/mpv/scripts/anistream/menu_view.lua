-- The anistream menu as this script draws it, in anistream mode.
--
-- uosc's menu could be recoloured but not reshaped: a heavy slab with a header rule, a divider
-- under every row and a fill on the current item. This one is the same primitive as every
-- other element: a flat dusk panel, rows separated by spacing alone, the focused row raised a
-- step with the amber obi on its left edge, the current choice marked by a small silk dot.
--
-- Keys while open: Up/Down (held repeats), PgUp/PgDn, Home/End, Enter or Right to open or
-- pick, Left or Backspace to go back, Esc to go back or close, the wheel to move, a right
-- click to go back. A click outside closes it.

local app = require("app")
local config = require("config")
local keys = require("keys")
local overlay = require("overlay")

local M = { id = "menu", region = "center" }

-- Base metrics, at scale 1.
local ROW = 36 -- also the click target
local HEADER = 44
local PAD_X = 16
local PAD_Y = 8
local SEPARATOR = 10 -- extra space after a row that ends a group
local MIN_WIDTH = 300
local MAX_WIDTH = 520

-- Set by menus.lua: the current description and what to do with a pick.
M.entries = function()
    return {}
end
M.activate = function() end
M.title = function()
    return "anistream"
end
M.note = function()
    return nil
end

local stack = nil -- views, root first: { id, title, hint, items, focus, scroll }
-- The view last drawn and how deep it was, so the panel can fade out after it is closed.
local shown = nil
local shown_depth = 1
local visible_rows = 8 -- from the last frame
local last_hover = nil
local last_mouse = { x = -1, y = -1 }

local function current()
    return stack and stack[#stack] or nil
end

local function drawn()
    return current() or shown
end

local function depth()
    return stack and #stack or shown_depth
end

local function index_of_current(items)
    for i, item in ipairs(items) do
        if item.current then
            return i
        end
    end
    return 1
end

local function submenu(items, id)
    for _, entry in ipairs(items) do
        if entry.id == id and entry.items then
            return entry
        end
    end
    return nil
end

local function push(entry)
    stack[#stack + 1] = {
        id = entry.id,
        title = entry.title,
        items = entry.items,
        focus = index_of_current(entry.items),
        scroll = 0,
    }
end

function M.open(view)
    -- Where the pointer already is does not count as pointing at a row.
    local mouse = overlay.mouse()
    last_mouse.x, last_mouse.y = mouse.x, mouse.y
    local items = M.entries()
    stack = { { id = "root", title = M.title(), hint = M.note(), items = items, focus = 1, scroll = 0 } }
    if view and view ~= "root" then
        local entry = submenu(items, view)
        if entry then
            push(entry)
        end
    end
    app.changed()
end

function M.close()
    if stack then
        stack = nil
        app.changed()
    end
end

function M.is_open()
    return stack ~= nil
end

function M.view()
    return stack and stack[#stack].id or nil
end

-- The host rewrote what the menu shows: rebuild each open level from the new description,
-- keeping where the viewer is.
function M.refresh()
    if not stack then
        return
    end
    local items = M.entries()
    local root = stack[1]
    root.items, root.title, root.hint = items, M.title(), M.note()
    for i = 2, #stack do
        local entry = submenu(stack[i - 1].items, stack[i].id)
        if not entry then
            for j = #stack, i, -1 do
                stack[j] = nil
            end
            break
        end
        stack[i].items = entry.items
    end
    for _, view in ipairs(stack) do
        view.focus = math.max(1, math.min(view.focus, #view.items))
    end
    app.changed()
end

---------------------------------------------------------------------------------------------
-- Navigation
---------------------------------------------------------------------------------------------

local function move(delta)
    local view = current()
    if not view or #view.items == 0 then
        return
    end
    view.focus = math.max(1, math.min(#view.items, view.focus + delta))
    app.changed()
end

local function back()
    if not stack then
        return
    end
    if #stack > 1 then
        stack[#stack] = nil
        app.changed()
    else
        M.close()
    end
end

local function activate(index)
    local view = current()
    local entry = view and view.items[index or view.focus]
    if not entry then
        return
    end
    view.focus = index or view.focus
    if entry.items then
        push(entry)
        app.changed()
        return
    end
    if not entry.keep_open then
        M.close()
    end
    M.activate(entry)
end

local bindings = {
    UP = keys.repeating(function()
        move(-1)
    end),
    DOWN = keys.repeating(function()
        move(1)
    end),
    WHEEL_UP = function()
        move(-1)
    end,
    WHEEL_DOWN = function()
        move(1)
    end,
    PGUP = function()
        move(-visible_rows)
    end,
    PGDWN = function()
        move(visible_rows)
    end,
    HOME = function()
        move(-math.huge)
    end,
    END = function()
        move(math.huge)
    end,
    ENTER = function()
        activate()
    end,
    KP_ENTER = function()
        activate()
    end,
    RIGHT = function()
        local view = current()
        local entry = view and view.items[view.focus]
        if entry and entry.items then
            activate()
        end
    end,
    LEFT = back,
    BS = back,
    ESC = back,
    MBTN_RIGHT = back,
}

function M.wants()
    return stack ~= nil
end

function M.keys()
    return bindings
end

---------------------------------------------------------------------------------------------
-- Drawing
---------------------------------------------------------------------------------------------

local function fonts(ctx)
    local title = ctx:font(config.TYPE.body)
    title.color = config.color.text
    local quiet = ctx:font(config.TYPE.body)
    quiet.color = config.color.dim
    local hint = ctx:font(config.TYPE.hint)
    hint.color = config.color.dim
    return title, quiet, hint
end

local function row_heights(ctx, view)
    local heights = {}
    for i, entry in ipairs(view.items) do
        heights[i] = ctx:px(ROW + ((entry.separator and i < #view.items) and SEPARATOR or 0))
    end
    return heights
end

function M.measure(ctx)
    local view = drawn()
    if not view then
        return 0, 0
    end
    shown, shown_depth = view, depth()
    local title_font, _, hint_font = fonts(ctx)
    local widest = ctx:width(view.title or "", title_font) + ctx:px(48)
        + (view.hint and ctx:width(view.hint, hint_font) or 0)
    for _, entry in ipairs(view.items) do
        local w = ctx:width(entry.title, title_font) + ctx:px(32)
            + (entry.hint and entry.hint ~= "" and ctx:width(entry.hint, hint_font) + ctx:px(10) or 0)
            + ctx:px(16)
        widest = math.max(widest, w)
    end
    local width = math.max(ctx:px(MIN_WIDTH), math.min(ctx:px(MAX_WIDTH), widest + ctx:px(PAD_X * 2)))
    width = math.min(width, ctx.w - ctx:px(32))

    -- As many rows as fit in four fifths of the window; the rest scroll.
    local heights = row_heights(ctx, view)
    local room = ctx.h * 0.8 - ctx:px(HEADER + PAD_Y * 2)
    local shown, used = 0, 0
    for i = 1, #heights do
        if used + heights[i] > room and shown > 0 then
            break
        end
        used = used + heights[i]
        shown = i
    end
    visible_rows = math.max(1, shown)
    -- Keep the focused row on screen.
    view.scroll = math.max(0, math.min(view.scroll, #view.items - visible_rows))
    if view.focus <= view.scroll then
        view.scroll = view.focus - 1
    elseif view.focus > view.scroll + visible_rows then
        view.scroll = view.focus - visible_rows
    end
    local body = 0
    for i = view.scroll + 1, math.min(#heights, view.scroll + visible_rows) do
        body = body + heights[i]
    end
    return width, ctx:px(HEADER + PAD_Y * 2) + body
end

function M.draw(ctx, x, y, w, h)
    local view = drawn()
    if not view or w <= 0 then
        return
    end
    local title_font, quiet_font, hint_font = fonts(ctx)
    local s = ctx.s

    -- Behind the panel, the whole window: a click anywhere else closes the menu.
    ctx:zone("menu-outside", 0, 0, ctx.w, ctx.h, M.close)

    ctx:panel(x, y, x + w, y + h, 0.94)
    ctx:zone("menu-panel", x, y, x + w, y + h, function() end)

    -- Header: where you are, and (at the root) what the video is.
    local hy = y + ctx:px(PAD_Y + HEADER / 2)
    local tx = x + ctx:px(PAD_X)
    -- At the root the header is context (the show), so it is quiet; inside a list it names the
    -- list and leads back out.
    local header_font = hint_font
    if depth() > 1 then
        header_font = title_font
        ctx:text(tx, hy, 4, "‹", quiet_font)
        ctx:zone("menu-back", x, y, x + ctx:px(PAD_X + 24) + ctx:width(view.title, title_font),
            y + ctx:px(PAD_Y + HEADER), back)
        tx = tx + ctx:px(18)
    end
    ctx:text(tx, hy, 4, ctx:fit(view.title or "", header_font, w - ctx:px(PAD_X * 2 + 120)), header_font)
    if view.hint then
        ctx:text(x + w - ctx:px(PAD_X), hy, 6, view.hint, hint_font)
    end

    -- The pointer moves focus as the keys do, but only when it moves onto another row: a
    -- pointer resting on the menu must not pull focus back after every key press.
    local mouse = overlay.mouse()
    local moved = mouse.x ~= last_mouse.x or mouse.y ~= last_mouse.y
    last_mouse.x, last_mouse.y = mouse.x, mouse.y
    if ctx.hover ~= last_hover and moved then
        local hovered = tonumber(tostring(ctx.hover):match("^menu%-row%-(%d+)$"))
        if hovered and view.items[hovered] then
            view.focus = hovered
        end
    end
    last_hover = ctx.hover

    local heights = row_heights(ctx, view)
    local ry = y + ctx:px(PAD_Y + HEADER)
    local last = math.min(#view.items, view.scroll + visible_rows)
    for i = view.scroll + 1, last do
        local entry = view.items[i]
        local row_h = ctx:px(ROW)
        local id = "menu-row-" .. i
        local focused = view.focus == i
        if focused then
            ctx:rect(x, ry, x + w, ry + row_h, config.color.hover, 0.9, 0)
            ctx:rect(x, ry, x + math.max(2, math.floor(3 * s + 0.5)), ry + row_h, config.color.obi, 1, 0)
        end
        local cy = ry + row_h / 2
        local right = x + w - ctx:px(PAD_X)
        if entry.items then
            ctx:text(right, cy, 6, "›", quiet_font)
            right = right - ctx:px(16)
        elseif entry.current then
            ctx:circle(right - ctx:px(3), cy, ctx:px(3), config.color.text, 1)
            right = right - ctx:px(16)
        end
        local hint_w = 0
        if entry.hint and entry.hint ~= "" then
            ctx:text(right, cy, 6, entry.hint, hint_font)
            hint_w = ctx:width(entry.hint, hint_font) + ctx:px(16)
        end
        local room = right - hint_w - (x + ctx:px(PAD_X))
        ctx:text(x + ctx:px(PAD_X), cy, 4, ctx:fit(entry.title, title_font, room), title_font)
        ctx:zone(id, x, ry, x + w, ry + row_h, function()
            activate(i)
        end)
        ry = ry + heights[i]
    end

    -- A hairline scroll thumb when the list is longer than the panel.
    if #view.items > visible_rows then
        local track_top = y + ctx:px(PAD_Y + HEADER)
        local track_h = (y + h - ctx:px(PAD_Y)) - track_top
        local thumb_h = math.max(ctx:px(16), track_h * visible_rows / #view.items)
        local thumb_y = track_top + (track_h - thumb_h) * view.scroll / (#view.items - visible_rows)
        local bx = x + w - ctx:px(5)
        ctx:rect(bx, thumb_y, bx + math.max(2, math.floor(2 * s + 0.5)), thumb_y + thumb_h, config.color.rule, 1, ctx:px(1))
    end
end

return M
