-- The one visual primitive: a compact flat panel on the dusk ground, a thin amber obi on the
-- left edge when it holds the primary action, and rows of text in two sizes.
--
-- The skip button, the Undo note, Up Next, the stall prompt, toasts and the subtitle-sync guide
-- are all this, with different rows. Sharing the layout is what keeps their radius, padding and
-- type scale identical rather than merely similar.
--
-- A card is described as data:
--
--   { obi = true | <ass colour> | nil,  -- marker on the left edge
--     action = { id, fn },              -- the whole card is one button (skip, Undo, a toast)
--     width = <base px> | nil,          -- fixed width; text marked `fit` is truncated to it
--     max_width = <base px> | nil,      -- or as wide as the content, up to this
--     media = { w, h, place } | nil,    -- a picture at the left (base px); `place(ctx, x, y,
--                                       -- w, h)` is told where it goes, in screen pixels
--     rows = {
--       { left = { item, ... }, right = { item, ... } },
--     } }
--
-- where an item is `{ parts = { {text, style, fit?}, ... }, id?, fn? }`: an item with `fn` is
-- a button inside the card (Play now, Cancel), with its own hover and click area.

local config = require("config")

local M = {}

-- Base metrics, at scale 1.
local PAD_X = 14 -- left and right padding inside the panel
local PAD_Y = 8 -- top and bottom padding
local LINE = 28 -- one row of text
-- A row holding buttons is taller, so each button is a comfortable target (32 px at scale 1)
-- while its drawn label stays the size of any other text.
local BUTTON_LINE = 36
local PART_GAP = 8 -- between a label and its key hint
local ITEM_GAP = 20 -- between two buttons in a row
local SIDE_GAP = 24 -- minimum gap between a row's left and right groups

local function row_height(row)
    if row.height then
        return row.height
    end
    for _, item in ipairs(row.left or {}) do
        if item.fn then
            return BUTTON_LINE
        end
    end
    for _, item in ipairs(row.right or {}) do
        if item.fn then
            return BUTTON_LINE
        end
    end
    return LINE
end

-- Two sizes, and bold only for the primary label.
local STYLES = {
    primary = { size = config.TYPE.body, bold = true, color = "text" },
    body = { size = config.TYPE.body, bold = false, color = "text" },
    quiet = { size = config.TYPE.body, bold = false, color = "dim" },
    hint = { size = config.TYPE.hint, bold = false, color = "dim" },
}

local function font(ctx, style)
    local s = STYLES[style] or STYLES.body
    local f = ctx:font(s.size, s.bold)
    f.color = config.color[s.color]
    return f
end

local function part_width(ctx, part)
    return ctx:width(part[1], font(ctx, part[2]))
end

local function item_width(ctx, item)
    local w = 0
    for i, part in ipairs(item.parts) do
        w = w + part_width(ctx, part) + (i > 1 and ctx:px(PART_GAP) or 0)
    end
    return w
end

local function group_width(ctx, items)
    local w = 0
    for i, item in ipairs(items or {}) do
        w = w + item_width(ctx, item) + (i > 1 and ctx:px(ITEM_GAP) or 0)
    end
    return w
end

local function row_width(ctx, row)
    local left, right = group_width(ctx, row.left), group_width(ctx, row.right)
    return left + right + ((left > 0 and right > 0) and ctx:px(SIDE_GAP) or 0)
end

local function rows_height(card)
    local h = 0
    for _, row in ipairs(card.rows) do
        h = h + row_height(row)
    end
    return h
end

-- Room the picture takes from the text: its width and a gap.
local function media_width(card)
    return card.media and card.media.w + PAD_X or 0
end

function M.measure(ctx, card)
    local h = ctx:px(PAD_Y * 2 + math.max(rows_height(card), card.media and card.media.h or 0))
    if card.width then
        return ctx:px(card.width), h
    end
    local w = 0
    for _, row in ipairs(card.rows) do
        w = math.max(w, row_width(ctx, row))
    end
    w = w + ctx:px(PAD_X * 2 + media_width(card))
    if card.max_width then
        w = math.min(w, ctx:px(card.max_width))
    end
    return w, h
end

-- Truncate any part marked `fit` so its row fits the card's inner width.
local function fit_row(ctx, row, inner)
    local over = row_width(ctx, row) - inner
    if over <= 0 then
        return
    end
    for _, item in ipairs(row.left or {}) do
        for _, part in ipairs(item.parts) do
            if part.fit then
                local f = font(ctx, part[2])
                part[1] = ctx:fit(part[1], f, math.max(0, ctx:width(part[1], f) - over))
                return
            end
        end
    end
end

local function draw_item(ctx, item, x, cy, row_top, row_bottom, zone_bottom)
    local w = item_width(ctx, item)
    if item.fn then
        local pad = ctx:px(8)
        local x0, x1 = x - pad, x + w + pad
        if ctx.hover == item.id then
            ctx:rect(x0, row_top + ctx:px(2), x1, row_bottom - ctx:px(2), config.color.hover, 1, ctx:px(3))
        end
        ctx:zone(item.id, x0, row_top, x1, zone_bottom, item.fn)
    end
    for _, part in ipairs(item.parts) do
        ctx:text(x, cy, 4, part[1], font(ctx, part[2]))
        x = x + part_width(ctx, part) + ctx:px(PART_GAP)
    end
    return w
end

function M.draw(ctx, card, x, y, w, h)
    local hovered = card.action and ctx.hover == card.action.id
    ctx:panel(x, y, x + w, y + h, card.quiet and 0.6 or nil)
    if hovered then
        -- The whole card is the button: hovering raises it a step rather than adding a frame.
        ctx:rect(x, y, x + w, y + h, config.color.hover, 0.55, ctx:px(4))
    end
    if card.obi then
        ctx:obi(x, y, y + h, card.obi ~= true and card.obi or nil, card.quiet and 0.6 or 1)
    end
    if card.action then
        ctx:zone(card.action.id, x, y, x + w, y + h, card.action.fn)
    end

    if card.media then
        local m = card.media
        local mx, my = x + ctx:px(PAD_X), y + (h - ctx:px(m.h)) / 2
        m.place(ctx, mx, my, ctx:px(m.w), ctx:px(m.h))
    end

    local left = x + ctx:px(PAD_X + media_width(card))
    local inner = x + w - ctx:px(PAD_X) - left
    -- Text rows sit centred against a taller picture.
    local row_top = y + (h - ctx:px(rows_height(card))) / 2
    for i, row in ipairs(card.rows) do
        fit_row(ctx, row, inner)
        local row_bottom = row_top + ctx:px(row_height(row))
        -- The last row's buttons reach down through the padding to the card's edge.
        local zone_bottom = i == #card.rows and y + h or row_bottom
        local cy = (row_top + row_bottom) / 2
        local cx = left
        for _, item in ipairs(row.left or {}) do
            cx = cx + draw_item(ctx, item, cx, cy, row_top, row_bottom, zone_bottom) + ctx:px(ITEM_GAP)
        end
        local rw = group_width(ctx, row.right)
        cx = x + w - ctx:px(PAD_X) - rw
        for _, item in ipairs(row.right or {}) do
            cx = cx + draw_item(ctx, item, cx, cy, row_top, row_bottom, zone_bottom) + ctx:px(ITEM_GAP)
        end
        row_top = row_bottom
    end
end

-- An element whose whole look is one card, built fresh from its state on every frame.
function M.element(region, build)
    return {
        region = region,
        measure = function(ctx)
            return M.measure(ctx, build(ctx))
        end,
        draw = function(ctx, x, y, w, h)
            M.draw(ctx, build(ctx), x, y, w, h)
        end,
    }
end

return M
