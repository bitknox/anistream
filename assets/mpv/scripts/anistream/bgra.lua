-- Raw BGRA bitmaps, as `overlay-add` takes them: four bytes a pixel, rows packed, alpha
-- premultiplied.
--
-- Only resampling, for mpv before 0.38, whose overlay-add shows a bitmap at its own size and
-- cannot scale it. Pure, so crates/anistream-player/tests/lua/bgra_test.lua checks it alone.

local M = {}

-- For each output pixel along one axis, the run of source pixels it covers: where it starts
-- and how many. A pixel blown up rather than shrunk covers one, which is nearest-neighbour.
local function spans(out, src)
    local first, count = {}, {}
    for i = 0, out - 1 do
        local a = math.min(src - 1, math.floor(i * src / out))
        local b = math.min(src, math.max(a + 1, math.floor((i + 1) * src / out)))
        first[i], count[i] = a, b - a
    end
    return first, count
end

-- `data`, `sw` x `sh`, resampled to `w` x `h` by a box filter: each pixel is the mean of the
-- source pixels under it. Its cost is the source's size whatever the target's, about a quarter
-- of a million reads for the Up Next still. nil when the data is shorter than its size says.
function M.resample(data, sw, sh, w, h)
    if w < 1 or h < 1 or #data < sw * sh * 4 then
        return nil
    end
    local xs, xn = spans(w, sw)
    local ys, yn = spans(h, sh)
    local byte, char, floor = string.byte, string.char, math.floor
    local stride = sw * 4
    local rows = {}
    for y = 0, h - 1 do
        local top, tall = ys[y] * stride + 1, yn[y]
        local row = {}
        for x = 0, w - 1 do
            local left, wide = xs[x] * 4, xn[x]
            local b, g, r, a = 0, 0, 0, 0
            for line = 0, tall - 1 do
                local at = top + line * stride + left
                for _ = 1, wide do
                    local pb, pg, pr, pa = byte(data, at, at + 3)
                    b, g, r, a = b + pb, g + pg, r + pr, a + pa
                    at = at + 4
                end
            end
            local n = wide * tall
            row[x + 1] = char(floor(b / n + 0.5), floor(g / n + 0.5), floor(r / n + 0.5), floor(a / n + 0.5))
        end
        rows[y + 1] = table.concat(row)
    end
    return table.concat(rows)
end

return M
