-- The loading card: what the window shows while the host finds or opens the next stream.
--
-- Mostly empty dusk ground, the small Nagare mark, one line saying what is happening and, when
-- the host has one, a quieter line of detail. In anistream mode the window between episodes is
-- painted dusk even with nothing to say, so the binge never cuts to a black frame.
--
-- Like the app's eyecatch, a wait that stops being instant admits it: after a few seconds the
-- elapsed time appears, because a still screen cannot otherwise tell "working" from "wedged".

local app = require("app")
local config = require("config")
local sprite = require("sprite")
local util = require("util")

local ADMIT_AFTER = 3

local M = { id = "loading", region = "backdrop" }

local status = nil -- { text, detail, since }
local last = { text = "" }
local ticker = nil

-- Text for the card, or nil when there is no load in progress.
local function current()
    local s = util.tbl(app.host.status)
    if not s or s.phase ~= "loading" then
        return nil
    end
    return s
end

function M.update()
    local s = current()
    if s then
        local text = util.str(s.text) or "Loading…"
        if not status or status.text ~= text then
            status = { text = text, since = status and status.since or mp.get_time() }
        end
        status.detail = util.str(s.detail)
        last = status
        if not ticker then
            ticker = mp.add_periodic_timer(1, app.changed)
        end
    else
        status = nil
        if ticker then
            ticker:kill()
            ticker = nil
        end
    end
end

-- The card itself, which is what the `ui` state reports.
function M.card_visible()
    return status ~= nil
end

function M.wants()
    if status then
        return true
    end
    -- Bare ground between episodes, in our own mode only: in `mpv` mode the idle window
    -- belongs to the user's setup.
    return config.opts.uosc and app.player.idle
end

function M.draw(ctx, x, y, w, h)
    local p = app.player
    -- Opaque when there is no picture to dim; a veil over a frozen frame otherwise.
    local opacity = (p.idle or not p.vo_configured) and 1 or 0.92
    ctx:rect(x, y, x + w, y + h, config.color.ground, opacity, 0)

    local shown = status or (ctx.alpha < 1 and last.text ~= "" and last) or nil
    if not shown then
        return
    end

    local text_font = ctx:font(config.TYPE.body)
    text_font.color = config.color.text
    local detail_font = ctx:font(config.TYPE.hint)
    detail_font.color = config.color.dim

    local detail = shown.detail
    local waited = math.floor(mp.get_time() - (shown.since or mp.get_time()))
    if waited >= ADMIT_AFTER then
        detail = (detail and detail .. "  ·  " or "") .. string.format("%d s", waited)
    end

    -- A whole number of screen pixels per sprite pixel, so every edge stays on the grid.
    local pixel = math.max(1, math.floor(ctx:px(1.5) + 0.5))
    local mark_w, mark_h = sprite.WIDTH * pixel, sprite.HEIGHT * pixel
    local line = ctx:px(28)
    local gap = ctx:px(20)
    local block = mark_h + gap + line + (detail and ctx:px(4) + line or 0)
    local top = y + (h - block) / 2
    -- Slightly above centre: optical centre, and clear of where subtitles would sit.
    top = top - h * 0.04

    sprite.draw(ctx.out, x + (w - mark_w) / 2, top, pixel, ctx.alpha)
    local cx = x + w / 2
    local ty = top + mark_h + gap + line / 2
    ctx:text(cx, ty, 5, ctx:fit(shown.text, text_font, w - ctx:px(64)), text_font)
    if detail then
        ctx:text(cx, ty + ctx:px(4) + line, 5, ctx:fit(detail, detail_font, w - ctx:px(64)), detail_font)
    end
end

function M.keys()
    return {}
end

return M
