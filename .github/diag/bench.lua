-- Diagnosis only (to be dropped): how long bgra.resample takes in this mpv's Lua.
local options = { dir = "" }
require("mp.options").read_options(options, "bench")
package.path = options.dir .. "/?.lua;" .. package.path
local bgra = require("bgra")
local data = string.rep(string.char(1, 2, 3, 255), 640 * 360)
for _, size in ipairs({ { 160, 90 }, { 208, 117 }, { 320, 180 }, { 416, 234 } }) do
    local t = mp.get_time()
    local out = bgra.resample(data, 640, 360, size[1], size[2])
    mp.msg.warn(string.format("%s %dx%d: %.1f ms (%d bytes)", _VERSION, size[1], size[2], (mp.get_time() - t) * 1000, #out))
end
mp.commandv("quit")
