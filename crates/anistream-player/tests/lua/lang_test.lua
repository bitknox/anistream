-- Run by tests/companion.rs inside mpv, which is where the companion's Lua runs: loads `lang`
-- from the script directory it is given and quits with 1 if any name is wrong.
local options = { dir = "" }
require("mp.options").read_options(options, "lang_test")
package.path = options.dir .. "/?.lua;" .. package.path
local lang = require("lang")

local cases = {
    { "eng", "English" },
    { "ger", "German" },
    { "deu", "German" },
    { "fre", "French" },
    { "jpn", "Japanese" },
    { "ja", "Japanese" },
    { "pt-BR", "Portuguese (Brazil)" },
    { "es-419", "Spanish (Latin America)" },
    { "zh-Hant", "Chinese (Traditional)" },
    { "zh_Hans_CN", "Chinese (Simplified)" },
    { "en-AU", "English (AU)" },
    { "und", "Unknown language" },
    { "xx", nil },
    { "", nil },
}
local failed = 0
for _, case in ipairs(cases) do
    local got = lang.name(case[1])
    if got ~= case[2] then
        mp.msg.error(string.format("%q: wanted %s, got %s", case[1], tostring(case[2]), tostring(got)))
        failed = failed + 1
    end
end
mp.commandv("quit", failed == 0 and "0" or "1")
