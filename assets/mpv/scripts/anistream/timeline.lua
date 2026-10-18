-- The seek bar, in anistream mode.
--
-- uosc's own timeline is switched off there (uosc-opts.conf): it draws exactly as thick as it
-- can be grabbed, one colour drives both its fill and the control icons, and it prints the time
-- inside the bar, so a thin line comes with unreadable numerals. This one separates the three:
--
--   - what you can grab is a generous band (36 px at scale 1) just above uosc's control row;
--   - what is drawn inside it is a 3 px line: a quiet silk track, buffered stretches a little
--     brighter, openings and endings tinted water, played time in amber;
--   - under the pointer or while dragging the line thickens to 6 px, and a small time label
--     (with the chapter's name) and a thumbnail from thumbfast sit above the pointer.
--
-- It shows with uosc's controls — on pointer movement, a pause or a seek — and fades out after
-- the same idle time mpv uses to hide the cursor. Nothing of it stays on screen at rest.

local app = require("app")
local config = require("config")
local draw = require("draw")
local overlay = require("overlay")
local screen = require("screen")

local M = { id = "timeline", region = "canvas" }

local LINE = 3
local LINE_ACTIVE = 6

-- The shade behind the bar and uosc's control row: how far above the bar it starts, at scale 1,
-- and how dark it gets. It reaches that depth at the bar, so the bar and the icons all sit on
-- the same ground, and is drawn as thin bands so it falls off above rather than ending in an
-- edge.
local SHADE_RISE = 110
local SHADE_DEPTH = 0.72
local SHADE_BANDS = 40

local shown_until = 0
local hide_timer = nil
local dragging = false
local position = nil
local ranges = {} -- { {from, to}, ... } openings and endings
local buffered = {} -- { {from, to}, ... }
local thumb = nil -- thumbfast-info
local thumb_sent = nil
local geometry = nil -- from the last frame, for turning a pointer x into a time
local last_seek = 0
local clear_thumb -- defined with the seeking helpers below

local function enabled()
    return config.opts.uosc
end

-- How long the bar stays after the last activity: mpv's own cursor-hide delay, so the cursor,
-- uosc's controls and this all leave together.
local function hide_delay()
    local value = mp.get_property_native("cursor-autohide")
    if type(value) == "number" and value > 0 then
        return value / 1000
    end
    return 1
end

-- Something happened that should bring the bar up: pointer movement, pause, seek.
function M.poke()
    if not enabled() then
        return
    end
    local delay = hide_delay()
    shown_until = mp.get_time() + delay
    if hide_timer then
        hide_timer:kill()
    end
    hide_timer = mp.add_timeout(delay + 0.01, app.changed)
    app.changed()
end

-- Whether the controls are up for activity, file or no file (the volume shares this).
function M.awake()
    return enabled() and (dragging or mp.get_time() < shown_until)
end

function M.wants()
    local p = app.player
    return enabled() and not p.idle and (p.duration or 0) > 0
        and (dragging or mp.get_time() < shown_until)
end

---------------------------------------------------------------------------------------------
-- What the bar shows
---------------------------------------------------------------------------------------------

local function on_position(_, value)
    position = value
    overlay.request()
end

local function on_cache(_, value)
    buffered = {}
    if type(value) == "table" and type(value["seekable-ranges"]) == "table" then
        for _, range in ipairs(value["seekable-ranges"]) do
            if range.start and range["end"] then
                buffered[#buffered + 1] = { range.start, range["end"] }
            end
        end
    end
    overlay.request()
end

local observing = false

-- The playhead and the cache are only watched while the bar is on screen.
function M.observe(visible)
    if visible and not observing then
        mp.observe_property("time-pos", "number", on_position)
        mp.observe_property("demuxer-cache-state", "native", on_cache)
        observing = true
    elseif not visible and observing then
        mp.unobserve_property(on_position)
        mp.unobserve_property(on_cache)
        observing = false
        clear_thumb()
    end
end

-- Which chapter titles start an opening or an ending: the ones the host writes from aniskip
-- (skip.rs `to_chapters`: Opening, Ending) and the ones release files carry (OP, ED, NCOP,
-- Intro, Outro, Credits, オープニング, エンディング). A range runs to the next chapter, so the
-- chapters that follow (Episode, Preview, 予告, Avant) end it without being tinted themselves.
local OPENING = { "^op$", "^op ", " op$", "^opening", " opening$", "^ncop", "^intro$", "オープニング" }
local ENDING = {
    "^ed$", "^ed ", " ed$", "^ending", " ending$", "^nced", "^outro$", "^credits", "エンディング",
}

local function matches(title, patterns)
    for _, pattern in ipairs(patterns) do
        if title:find(pattern) then
            return true
        end
    end
    return false
end

local function classify(title)
    local t = tostring(title or ""):lower():gsub("^%s+", ""):gsub("%s+$", "")
    if matches(t, OPENING) then
        return "opening"
    end
    if matches(t, ENDING) then
        return "ending"
    end
    return nil
end

local chapters = {}

mp.observe_property("chapter-list", "native", function(_, value)
    chapters = type(value) == "table" and value or {}
    ranges = {}
    for i, chapter in ipairs(chapters) do
        local kind = classify(chapter.title)
        if kind and chapter.time then
            local next_chapter = chapters[i + 1]
            -- An opening needs a following chapter to end it; an ending may run to the end.
            if next_chapter or kind == "ending" then
                ranges[#ranges + 1] = { chapter.time, next_chapter and next_chapter.time or math.huge }
            end
        end
    end
    overlay.request()
end)

local function chapter_at(time)
    local found = nil
    for _, chapter in ipairs(chapters) do
        if chapter.time and chapter.time <= time then
            found = chapter
        end
    end
    return found and found.title ~= "" and found.title or nil
end

mp.register_script_message("thumbfast-info", function(json)
    local info = require("util").parse_json(json)
    thumb = (info.width and info.height and not info.disabled) and info or nil
end)

-- Whole seconds rounded down, as the terminal's Now Playing counts them, so the two never show
-- the same runtime a second apart; and rounded down, the elapsed time never passes the total.
local function clock(seconds)
    seconds = math.max(0, math.floor(seconds))
    local h, m, s = math.floor(seconds / 3600), math.floor(seconds % 3600 / 60), seconds % 60
    if h > 0 then
        return string.format("%d:%02d:%02d", h, m, s)
    end
    return string.format("%d:%02d", m, s)
end

---------------------------------------------------------------------------------------------
-- Seeking
---------------------------------------------------------------------------------------------

local function time_at(x)
    local g = geometry
    local duration = app.player.duration
    if not g or not duration then
        return nil
    end
    local fraction = math.max(0, math.min(1, (x - g.x0) / (g.x1 - g.x0)))
    return fraction * duration
end

local function seek_to(x, exact)
    local time = time_at(x)
    if not time then
        return
    end
    -- While dragging, keyframe seeks at most every 50 ms keep up with the pointer; the press
    -- and the release land exactly.
    local now = mp.get_time()
    if not exact and now - last_seek < 0.05 then
        return
    end
    last_seek = now
    position = time
    mp.commandv("seek", tostring(time), exact and "absolute+exact" or "absolute+keyframes")
end

local function drag(x, _, phase)
    if phase == "down" then
        dragging = true
        seek_to(x, true)
    elseif phase == "move" then
        seek_to(x, false)
    else
        dragging = false
        seek_to(x, true)
        M.poke()
    end
    app.changed()
end

clear_thumb = function()
    if thumb_sent then
        mp.commandv("script-message-to", "thumbfast", "clear")
        thumb_sent = nil
    end
end

---------------------------------------------------------------------------------------------
-- Drawing
---------------------------------------------------------------------------------------------

function M.draw(ctx, _, _, w, h)
    local s = ctx.s
    local g = screen.timeline_geometry(s, w, h)
    geometry = g
    local duration = app.player.duration or 0
    if duration <= 0 then
        return
    end
    local pos = position or mp.get_property_native("time-pos") or 0

    -- Shade first, under everything here and under uosc's icons: the time and the icons are
    -- light, and over a bright frame nothing else would hold them apart from the picture.
    local top = g.band_top - SHADE_RISE * s
    local full = (g.line_y - top) / (h - top)
    for i = 0, SHADE_BANDS - 1 do
        local y0 = top + (h - top) * i / SHADE_BANDS
        local y1 = top + (h - top) * (i + 1) / SHADE_BANDS
        local t = math.min(1, (i + 0.5) / SHADE_BANDS / full)
        ctx:rect(0, y0, w, y1, config.color.ground, SHADE_DEPTH * t * t * (3 - 2 * t), 0)
    end

    local mouse = overlay.mouse()
    local active = dragging or ctx.hover == "timeline"
    local thick = draw.round((active and LINE_ACTIVE or LINE) * s)
    local y0 = draw.round(g.line_y - thick / 2)
    local y1 = y0 + thick
    local radius = thick / 2
    local span = g.x1 - g.x0
    local function x_of(t)
        return g.x0 + span * math.max(0, math.min(1, t / duration))
    end

    -- The track: a dark wash under a light one, so it shows on white frames and black ones.
    ctx:rect(g.x0, y0, g.x1, y1, config.color.ground, 0.3, radius)
    ctx:rect(g.x0, y0, g.x1, y1, config.color.text, 0.25, radius)
    for _, range in ipairs(buffered) do
        ctx:rect(x_of(range[1]), y0, x_of(range[2]), y1, config.color.text, 0.22, radius)
    end
    for _, range in ipairs(ranges) do
        ctx:rect(x_of(range[1]), y0, x_of(math.min(range[2], duration)), y1, config.color.state, 0.7, 0)
    end
    ctx:rect(g.x0, y0, x_of(pos), y1, config.color.obi, 1, radius)

    -- Elapsed and total beside play/pause, at reading size. Drawn like uosc's icons (silk
    -- with a one-pixel dark edge), so it reads on a white frame as on a black one.
    local time_font = ctx:font(config.TYPE.time)
    time_font.color = config.color.text
    time_font.opacity = 1
    time_font.outline = math.max(1, s)
    if g.time_x then
        ctx:text(g.time_x, g.time_y, 4, clock(pos) .. " / " .. clock(duration), time_font)
    end

    ctx:zone("timeline", g.x0 - ctx:px(6), g.band_top, g.x1 + ctx:px(6), g.band_bottom, nil, drag)

    if active and mouse.inside then
        local hx = math.max(g.x0, math.min(g.x1, mouse.x))
        local t = time_at(hx) or 0
        local label = clock(t)
        local chapter = chapter_at(t)
        local label_font = ctx:font(config.TYPE.time)
        label_font.color = config.color.text
        local chapter_font = ctx:font(config.TYPE.hint)
        chapter_font.color = config.color.dim
        local tw = ctx:width(label, label_font)
        local cw = chapter and ctx:width(chapter, chapter_font) or 0
        local pad = ctx:px(8)
        local pw = tw + (chapter and ctx:px(8) + cw or 0) + pad * 2
        local ph = ctx:px(30)
        local px0 = math.max(0, math.min(w - pw, hx - pw / 2))
        local py1 = y0 - ctx:px(8)
        local py0 = py1 - ph
        ctx:panel(px0, py0, px0 + pw, py1)
        ctx:text(px0 + pad, (py0 + py1) / 2, 4, label, label_font)
        if chapter then
            ctx:text(px0 + pad + tw + ctx:px(8), (py0 + py1) / 2, 4, chapter, chapter_font)
        end

        if thumb then
            local tx = draw.round(math.max(0, math.min(w - thumb.width, hx - thumb.width / 2)))
            local ty = draw.round(py0 - ctx:px(6) - thumb.height)
            -- No frame, only a dark hairline so the picture's edge holds against a bright frame.
            local hair = math.max(1, draw.round(s))
            draw.ring(ctx.out, tx - hair, ty - hair, tx + thumb.width + hair, ty + thumb.height + hair,
                config.color.ground, 0.6 * ctx.alpha, 0, hair)
            local key = string.format("%.1f %d %d", t, tx, ty)
            if key ~= thumb_sent then
                mp.commandv("script-message-to", "thumbfast", "thumb", tostring(t), tostring(tx), tostring(ty))
                thumb_sent = key
            end
        end
    else
        clear_thumb()
    end
end

-- The hover label and thumbnail follow the pointer.
overlay.on_mouse_move(function()
    if overlay.hovered() == "timeline" or dragging then
        overlay.request()
    elseif thumb_sent then
        clear_thumb()
    end
end)

return M
