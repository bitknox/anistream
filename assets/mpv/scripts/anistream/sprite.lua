-- Nagare, the mascot, as the same hand-authored pixel sprite the website draws
-- (website/src/scripts/landing-motion.ts). Rendered at a whole number of screen pixels per
-- sprite pixel so every edge lands on the pixel grid and stays crisp.

local draw = require("draw")

local M = {}

local WIDTH = 20
local ROWS = {
    "........hH..........",
    ".........Hh.........",
    "........hH..........",
    "......hHHHHh........",
    ".....hHHHHHHh.......",
    "....hHHHHHHHHh......",
    "....HHHHHHHHHHh.....",
    "...hHFFFFhHHHHh.....",
    "...HFFFFFFhHHHh.....",
    "...HFEFFFFhHHHh.....",
    "...hFFFFFfhHHh......",
    ".ww.fFFFfhHHh.......",
    ".wWWAAAAAAHHh.......",
    ".wWaAAAAAAa..aAA....",
    "..w.aAAAa....aAAa...",
    "..SRCCCCCCc.........",
    "..SRCCCCCCCc........",
    "..SRCCCCCCCc........",
    "..SfFCCCCCCCc.......",
    "..SRCCCCCCCCc.......",
    "..SRCCCCCCCCc.......",
    "..SRCCCCCCCCcc......",
    "..SRCCCCCCCCCc......",
    "..SRCCCCCCCCCc......",
    "..SRCCCCCCCCCcc.....",
    "..SRCCCCCCCCCCc.....",
    "..SRCCCCCCCCCCc.....",
    "..SRCCCCCCCCCCcc....",
    "..SRCCCCCCCCCCCc....",
    "..S.BB......BB......",
    "....BB......BB......",
    "...BBB.....BBB......",
}
local COLORS = {
    H = { 232, 226, 212 },
    h = { 198, 190, 169 },
    F = { 239, 207, 172 },
    f = { 208, 168, 131 },
    E = { 35, 40, 66 },
    C = { 70, 78, 124 },
    c = { 46, 52, 92 },
    R = { 104, 114, 158 },
    A = { 242, 166, 75 },
    a = { 197, 124, 44 },
    S = { 138, 104, 68 },
    W = { 127, 199, 217 },
    w = { 223, 245, 251 },
    B = { 35, 40, 66 },
}

M.WIDTH = WIDTH
M.HEIGHT = #ROWS

-- Horizontal runs of one colour, gathered per colour so the whole sprite is one drawing event
-- per colour rather than one per pixel.
local runs = {}
for y, row in ipairs(ROWS) do
    local x = 1
    while x <= WIDTH do
        local char = row:sub(x, x)
        local start = x
        while x <= WIDTH and row:sub(x, x) == char do
            x = x + 1
        end
        if COLORS[char] then
            runs[char] = runs[char] or {}
            table.insert(runs[char], { start - 1, y - 1, x - start })
        end
    end
end

-- Draw with its top-left at (x, y), `pixel` screen pixels per sprite pixel.
function M.draw(out, x, y, pixel, opacity)
    x, y, pixel = draw.round(x), draw.round(y), math.max(1, draw.round(pixel))
    for char, list in pairs(runs) do
        local rgb = COLORS[char]
        local path = {}
        for _, run in ipairs(list) do
            local x0, y0 = x + run[1] * pixel, y + run[2] * pixel
            local x1, y1 = x0 + run[3] * pixel, y0 + pixel
            path[#path + 1] = string.format("m %d %d l %d %d l %d %d l %d %d", x0, y0, x1, y0, x1, y1, x0, y1)
        end
        out[#out + 1] = string.format(
            "{\\an7\\pos(0,0)\\bord0\\shad0\\blur0\\1c&H%02X%02X%02X&\\1a%s\\p1}%s{\\p0}",
            rgb[3], rgb[2], rgb[1], draw.alpha(opacity), table.concat(path, " ")
        )
    end
end

return M
