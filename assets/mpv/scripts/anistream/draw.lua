-- ASS drawing primitives.
--
-- Everything is drawn in OSD pixels (the overlay's resolution is set to `osd-dimensions`), so a
-- size here is a size on screen, multiplied by the UI scale the caller passes in. Every event
-- carries `\bord0\shad0\blur0`: the panels are flat, and libass adds an outline by default.

local config = require("config")
local util = require("util")

local M = {}

local FLAT = "\\bord0\\shad0\\blur0"

local function round(v)
    return math.floor(v + 0.5)
end
M.round = round

-- ASS alpha is transparency, &H00& opaque to &HFF& invisible.
function M.alpha(opacity)
    opacity = math.max(0, math.min(1, opacity))
    return string.format("&H%02X&", round((1 - opacity) * 255))
end

local has_escape_ass = nil

-- `escape-ass` arrived in mpv 0.38; the fallback covers what can break an event: braces open
-- override blocks and a backslash before n/N/h is a line break or hard space.
function M.escape(text)
    text = tostring(text or "")
    if has_escape_ass ~= false then
        local escaped = mp.command_native({ "escape-ass", text })
        if type(escaped) == "string" then
            has_escape_ass = true
            return escaped
        end
        has_escape_ass = false
    end
    text = text:gsub("\\", "\\\226\129\160"):gsub("{", "\\{"):gsub("\n", " ")
    return text
end

local function num(v)
    return (string.format("%.2f", v):gsub("%.?0+$", ""))
end

-- The outline of a rounded rectangle as drawing commands, clockwise, or anticlockwise when
-- `reverse` is set (an inner contour drawn that way cuts a hole under either fill rule).
local function rounded_path(x0, y0, x1, y1, r, corners, reverse)
    local c = corners or { true, true, true, true }
    local rtl, rtr, rbr, rbl = c[1] and r or 0, c[2] and r or 0, c[3] and r or 0, c[4] and r or 0
    -- Bezier handle offset for a quarter circle (1 - 0.5523).
    local k = 0.4477
    local p
    if not reverse then
        p = {
            "m", x0 + rtl, y0,
            "l", x1 - rtr, y0,
            "b", x1 - rtr * k, y0, x1, y0 + rtr * k, x1, y0 + rtr,
            "l", x1, y1 - rbr,
            "b", x1, y1 - rbr * k, x1 - rbr * k, y1, x1 - rbr, y1,
            "l", x0 + rbl, y1,
            "b", x0 + rbl * k, y1, x0, y1 - rbl * k, x0, y1 - rbl,
            "l", x0, y0 + rtl,
            "b", x0, y0 + rtl * k, x0 + rtl * k, y0, x0 + rtl, y0,
        }
    else
        p = {
            "m", x0 + rtl, y0,
            "b", x0 + rtl * k, y0, x0, y0 + rtl * k, x0, y0 + rtl,
            "l", x0, y1 - rbl,
            "b", x0, y1 - rbl * k, x0 + rbl * k, y1, x0 + rbl, y1,
            "l", x1 - rbr, y1,
            "b", x1 - rbr * k, y1, x1, y1 - rbr * k, x1, y1 - rbr,
            "l", x1, y0 + rtr,
            "b", x1, y0 + rtr * k, x1 - rtr * k, y0, x1 - rtr, y0,
            "l", x0 + rtl, y0,
        }
    end
    for i, v in ipairs(p) do
        if type(v) == "number" then
            p[i] = num(v)
        end
    end
    return table.concat(p, " ")
end

local function shape(out, path, color, opacity, clip)
    local clip_tag = clip
            and string.format("\\clip(%d,%d,%d,%d)", round(clip[1]), round(clip[2]), round(clip[3]), round(clip[4]))
        or ""
    out[#out + 1] = string.format(
        "{\\an7\\pos(0,0)%s%s\\1c%s\\1a%s\\p1}%s{\\p0}",
        FLAT, clip_tag, color, M.alpha(opacity), path
    )
end

-- A rectangle with optionally rounded corners. `corners` picks which are rounded, as
-- `{tl, tr, br, bl}`; nil rounds all four. `clip` is `{x0, y0, x1, y1}`.
function M.rect(out, x0, y0, x1, y1, color, opacity, radius, corners, clip)
    if opacity <= 0 or x1 <= x0 or y1 <= y0 then
        return
    end
    x0, y0, x1, y1 = round(x0), round(y0), round(x1), round(y1)
    local r = math.min(radius or 0, (x1 - x0) / 2, (y1 - y0) / 2)
    shape(out, rounded_path(x0, y0, x1, y1, r, corners), color, opacity, clip)
end

-- A hairline just inside a rounded rectangle's edge. Drawn as its own ring rather than as an
-- ASS border, which would sit half under a translucent fill and show through it.
function M.ring(out, x0, y0, x1, y1, color, opacity, radius, width)
    if opacity <= 0 or x1 <= x0 or y1 <= y0 then
        return
    end
    x0, y0, x1, y1 = round(x0), round(y0), round(x1), round(y1)
    local r = math.min(radius or 0, (x1 - x0) / 2, (y1 - y0) / 2)
    local inner = math.max(0, r - width)
    local path = rounded_path(x0, y0, x1, y1, r) .. " "
        .. rounded_path(x0 + width, y0 + width, x1 - width, y1 - width, inner, nil, true)
    shape(out, path, color, opacity)
end

-- A filled circle.
function M.circle(out, cx, cy, radius, color, opacity)
    M.rect(out, cx - radius, cy - radius, cx + radius, cy + radius, color, opacity, radius)
end

-- Text style shared by drawing and measuring, so a measured width is the drawn width.
local function style(size, bold, spacing, font)
    return string.format(
        "\\fn%s\\fs%d\\b%d\\fsp%s\\q2%s",
        font or config.font, round(size), bold and 1 or 0, spacing or 0, FLAT
    )
end

-- `an` is the ASS numpad alignment of (x, y): 4 = left-middle, 6 = right-middle, 5 = centre.
-- `opts.outline` (pixels) gives text that sits straight on the video a dark hairline, so it
-- stays legible on a white frame; text on a panel never needs one.
function M.text(out, x, y, an, text, opts)
    local opacity = opts.opacity or 1
    if opacity <= 0 or text == nil or text == "" then
        return
    end
    local outline = ""
    if opts.outline and opts.outline > 0 then
        outline = string.format(
            "\\bord%s\\3c%s\\3a%s", num(opts.outline), config.color.ground, M.alpha(0.9 * opacity)
        )
    end
    out[#out + 1] = string.format(
        "{\\an%d\\pos(%d,%d)%s\\1c%s\\1a%s%s}%s",
        an, round(x), round(y),
        style(opts.size, opts.bold, opts.spacing, opts.font),
        opts.color or config.color.text, M.alpha(opacity), outline, M.escape(text)
    )
end

---------------------------------------------------------------------------------------------
-- Measuring
--
-- libass knows the real advance of every glyph, so text is measured by laying it out in a
-- hidden overlay with `compute_bounds`. One synchronous round trip per string, cached: the set
-- of strings on screen is small and changes rarely.
---------------------------------------------------------------------------------------------

local probe = mp.create_osd_overlay("ass-events")
probe.compute_bounds = true
probe.hidden = true
probe.res_x, probe.res_y = 1920, 1080
local widths = {}
local cached = 0

function M.width(text, opts)
    if text == nil or text == "" then
        return 0
    end
    local key = string.format("%s|%d|%s|%s", text, round(opts.size), tostring(opts.bold), tostring(opts.spacing))
    local hit = widths[key]
    if hit then
        return hit
    end
    probe.data = string.format("{\\an7\\pos(0,0)%s}%s", style(opts.size, opts.bold, opts.spacing, opts.font), M.escape(text))
    local bounds = probe:update()
    local width
    if type(bounds) == "table" and bounds.x1 and bounds.x0 then
        width = bounds.x1 - bounds.x0
    else
        -- No OSD to lay out on (a headless run): an estimate keeps layout sane.
        width = #util.chars(text) * opts.size * 0.56
    end
    if cached > 2000 then
        widths, cached = {}, 0
    end
    widths[key] = width
    cached = cached + 1
    return width
end

-- The longest prefix of `text` that fits `max_width`, with an ellipsis when cut.
function M.fit(text, opts, max_width)
    if text == nil or text == "" or M.width(text, opts) <= max_width then
        return text
    end
    local chars = util.chars(text)
    local lo, hi = 0, #chars
    while lo < hi do
        local mid = math.floor((lo + hi + 1) / 2)
        if M.width(table.concat(chars, "", 1, mid) .. "…", opts) <= max_width then
            lo = mid
        else
            hi = mid - 1
        end
    end
    return (table.concat(chars, "", 1, lo):gsub("%s+$", "")) .. "…"
end

return M
