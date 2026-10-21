-- Run by tests/companion.rs inside mpv, which is where the companion's Lua runs: loads `bgra`
-- from the script directory it is given and quits with 1 if any resample is wrong.
local options = { dir = "" }
require("mp.options").read_options(options, "bgra_test")
package.path = options.dir .. "/?.lua;" .. package.path
local bgra = require("bgra")

-- A bitmap from a list of {b, g, r, a} pixels, row by row.
local function bitmap(pixels)
    local out = {}
    for i, p in ipairs(pixels) do
        out[i] = string.char(p[1], p[2], p[3], p[4])
    end
    return table.concat(out)
end

local failed = 0
local function check(name, got, wanted)
    if got ~= wanted then
        local function hex(s)
            return s and (s:gsub(".", function(c)
                return string.format("%02x", c:byte())
            end)) or "nil"
        end
        mp.msg.error(string.format("%s: wanted %s, got %s", name, hex(wanted), hex(got)))
        failed = failed + 1
    end
end

-- 4x2 to 2x1: each pixel is the mean of the 2x2 block under it, rounded.
local source = bitmap({
    { 0, 10, 20, 255 }, { 4, 10, 20, 255 }, { 100, 1, 0, 255 }, { 200, 0, 0, 255 },
    { 8, 10, 21, 255 }, { 12, 10, 21, 255 }, { 0, 0, 0, 0 }, { 0, 0, 0, 0 },
})
check("halved", bgra.resample(source, 4, 2, 2, 1), bitmap({ { 6, 10, 21, 255 }, { 75, 0, 0, 128 } }))

-- At its own size the bitmap comes back as it went in.
check("same size", bgra.resample(source, 4, 2, 4, 2), source)

-- 2x1 to 4x2: blown up, each pixel is repeated rather than smeared.
local pair = bitmap({ { 1, 2, 3, 4 }, { 5, 6, 7, 8 } })
check("doubled", bgra.resample(pair, 2, 1, 4, 2), bitmap({
    { 1, 2, 3, 4 }, { 1, 2, 3, 4 }, { 5, 6, 7, 8 }, { 5, 6, 7, 8 },
    { 1, 2, 3, 4 }, { 1, 2, 3, 4 }, { 5, 6, 7, 8 }, { 5, 6, 7, 8 },
}))

-- 640x360 to 160x90, the host's still into the card's slot at scale 1: uniform stays uniform.
local grey = string.rep(string.char(0x80, 0x80, 0x80, 0xff), 640 * 360)
check("the still", bgra.resample(grey, 640, 360, 160, 90), string.rep(string.char(0x80, 0x80, 0x80, 0xff), 160 * 90))

-- A file shorter than its stated size is refused rather than read past.
check("short", bgra.resample(source:sub(1, 20), 4, 2, 2, 1), nil)

mp.commandv("quit", failed == 0 and "0" or "1")
