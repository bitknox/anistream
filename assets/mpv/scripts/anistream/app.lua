-- Shared state and the one "something changed" hook.
--
-- Modules write what they know into `app.host` (the protocol keys) and `app.player` (mpv
-- properties several modules need), then call `app.changed()`. main.lua sets that hook to
-- redraw, re-derive the contextual key bindings and republish `user-data/anistream/ui` — the
-- three things that must follow any visible change, kept in one place so none is forgotten.

local M = {
    -- `user-data/anistream/<key>` as last written by the host; any may be nil.
    host = {},
    player = {
        idle = mp.get_property_native("idle-active") == true,
        duration = nil,
        time_pos = nil,
        speed = 1,
        osd_w = 0,
        osd_h = 0,
        hidpi = 1,
        fullscreen = false,
        maximized = false,
        vo_configured = false,
        -- A file without sound gets no volume control at all.
        has_audio = false,
    },
    -- True while another script's modal UI (a uosc menu, mpv's console or context menu) owns
    -- the keyboard; our contextual keys stand down so Enter still means "select this item".
    modal = false,
}

function M.changed() end

return M
