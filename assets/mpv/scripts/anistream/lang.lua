-- Language names for tracks that carry only a code.
--
-- A release that titles its tracks says "English" or "Deutsch" itself; one that does not leaves
-- mpv with the bare tag, `eng` or `ger` or `pt-BR`. Those read as noise in a menu, so the common
-- ones are named here: ISO 639-1, and ISO 639-2 in both its bibliographic and terminological
-- forms, since releases use either (`ger` and `deu`, `chi` and `zho`, `fre` and `fra`).

local M = {}

local NAMES = {}
local function add(name, ...)
    for _, code in ipairs({ ... }) do
        NAMES[code] = name
    end
end

add("Afrikaans", "af", "afr")
add("Albanian", "sq", "alb", "sqi")
add("Arabic", "ar", "ara")
add("Armenian", "hy", "arm", "hye")
add("Basque", "eu", "baq", "eus")
add("Bengali", "bn", "ben")
add("Bosnian", "bs", "bos")
add("Bulgarian", "bg", "bul")
add("Burmese", "my", "bur", "mya")
add("Catalan", "ca", "cat")
add("Chinese", "zh", "chi", "zho")
add("Croatian", "hr", "hrv")
add("Czech", "cs", "cze", "ces")
add("Danish", "da", "dan")
add("Dutch", "nl", "dut", "nld")
add("English", "en", "eng")
add("Estonian", "et", "est")
add("Filipino", "fil", "tl", "tgl")
add("Finnish", "fi", "fin")
add("French", "fr", "fre", "fra")
add("Galician", "gl", "glg")
add("Georgian", "ka", "geo", "kat")
add("German", "de", "ger", "deu")
add("Greek", "el", "gre", "ell")
add("Hebrew", "he", "heb")
add("Hindi", "hi", "hin")
add("Hungarian", "hu", "hun")
add("Icelandic", "is", "ice", "isl")
add("Indonesian", "id", "ind")
add("Irish", "ga", "gle")
add("Italian", "it", "ita")
add("Japanese", "ja", "jpn")
add("Kannada", "kn", "kan")
add("Kazakh", "kk", "kaz")
add("Khmer", "km", "khm")
add("Korean", "ko", "kor")
add("Lao", "lo", "lao")
add("Latvian", "lv", "lav")
add("Lithuanian", "lt", "lit")
add("Macedonian", "mk", "mac", "mkd")
add("Malay", "ms", "may", "msa")
add("Malayalam", "ml", "mal")
add("Marathi", "mr", "mar")
add("Mongolian", "mn", "mon")
add("Nepali", "ne", "nep")
add("Norwegian", "no", "nor")
add("Norwegian Bokmål", "nb", "nob")
add("Norwegian Nynorsk", "nn", "nno")
add("Persian", "fa", "per", "fas")
add("Polish", "pl", "pol")
add("Portuguese", "pt", "por")
add("Romanian", "ro", "rum", "ron")
add("Russian", "ru", "rus")
add("Serbian", "sr", "srp")
add("Sinhala", "si", "sin")
add("Slovak", "sk", "slo", "slk")
add("Slovenian", "sl", "slv")
add("Spanish", "es", "spa")
add("Swahili", "sw", "swa")
add("Swedish", "sv", "swe")
add("Tamil", "ta", "tam")
add("Telugu", "te", "tel")
add("Thai", "th", "tha")
add("Turkish", "tr", "tur")
add("Ukrainian", "uk", "ukr")
add("Urdu", "ur", "urd")
add("Uzbek", "uz", "uzb")
add("Vietnamese", "vi", "vie")
add("Welsh", "cy", "wel", "cym")

-- Tags that say there is no single language rather than naming one.
add("Unknown language", "und")
add("Several languages", "mul")
add("No language", "zxx")

-- The regions a subtitle release actually distinguishes. Anything else is shown as the region
-- code it is, after the language's name.
local REGIONS = {
    ["419"] = "Latin America",
    la = "Latin America",
    br = "Brazil",
    pt = "Portugal",
    es = "Spain",
    mx = "Mexico",
    us = "US",
    gb = "UK",
    ca = "Canada",
    cn = "Simplified",
    sg = "Simplified",
    tw = "Traditional",
    hk = "Traditional",
    mo = "Traditional",
    hans = "Simplified",
    hant = "Traditional",
}

-- The name for a language tag such as `eng`, `pt-BR` or `zh_Hant`, or nil when it is unknown.
function M.name(tag)
    if type(tag) ~= "string" or tag == "" then
        return nil
    end
    local lower = tag:lower():gsub("_", "-")
    local base, rest = lower:match("^([a-z]+)%-?(.*)$")
    local name = base and NAMES[base]
    if not name then
        return nil
    end
    if rest == nil or rest == "" then
        return name
    end
    -- `zh-Hant-TW` names its script first; the first subtag that means something wins.
    for subtag in rest:gmatch("[^-]+") do
        local region = REGIONS[subtag]
        if region then
            return name .. " (" .. region .. ")"
        end
    end
    return name .. " (" .. rest:upper() .. ")"
end

return M
