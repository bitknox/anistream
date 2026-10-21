/*
 * This file is part of mpv.
 *
 * mpv is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2.1 of the License, or (at your option) any later version.
 *
 * mpv is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with mpv.  If not, see <http://www.gnu.org/licenses/>.
 */

/*
 * MPVKit's gpu-context=moltenvk (Sources/BuildScripts/patch/libmpv/
 * 0001-player-add-moltenvk-context.patch at 1.1.0-n9.0.2), with resizing: the original reads
 * the layer's drawableSize only at reconfig, so a window resize, full screen or a rotation
 * left mpv drawing into the old rectangle (mpvkit/MPVKit#3). Here VOCTRL_CHECK_EVENTS, which
 * the VO polls every frame, compares the drawable with the size mpv last used and reports
 * VO_EVENT_RESIZE when it differs, as edde746/MPVKit@e6b129f does.
 *
 * The anistream player spike compiles this against mpv v0.41.0's headers and swaps it for
 * MPVKit's object when it re-links Mpv.framework (scripts/relink.sh).
 */

#include <CoreGraphics/CoreGraphics.h>
#include <QuartzCore/CAMetalLayer.h>
#include <MoltenVK/mvk_vulkan.h>

#include "common.h"
#include "context.h"
#include "utils.h"

struct priv {
    struct mpvk_ctx vk;
    CAMetalLayer *layer;
    int last_width;
    int last_height;
};

static void moltenvk_uninit(struct ra_ctx *ctx)
{
    struct priv *p = ctx->priv;
    ra_vk_ctx_uninit(ctx);
    mpvk_uninit(&p->vk);
}

// The layer's drawable is the source of truth: embedded through wid, mpv has no window of
// its own to ask.
static bool resize(struct ra_ctx *ctx)
{
    struct priv *p = ctx->priv;
    CGSize s = p->layer.drawableSize;
    int w = (int)s.width, h = (int)s.height;
    if (w <= 1 || h <= 1)
        return false;
    // vo_reconfig sets dwidth/dheight to the video's size before calling us, so set them
    // every time, not only when the swapchain changes.
    ctx->vo->dwidth = w;
    ctx->vo->dheight = h;
    if (w == p->last_width && h == p->last_height)
        return true;
    p->last_width = w;
    p->last_height = h;
    return ra_vk_ctx_resize(ctx, w, h);
}

static bool moltenvk_init(struct ra_ctx *ctx)
{
    struct priv *p = ctx->priv = talloc_zero(ctx, struct priv);
    struct mpvk_ctx *vk = &p->vk;
    int msgl = ctx->opts.probing ? MSGL_V : MSGL_ERR;

    if (ctx->vo->opts->WinID == -1) {
        MP_MSG(ctx, msgl, "WinID missing\n");
        goto fail;
    }

    if (!mpvk_init(vk, ctx, VK_EXT_METAL_SURFACE_EXTENSION_NAME))
        goto fail;

    p->layer = (__bridge CAMetalLayer *)(intptr_t)ctx->vo->opts->WinID;
    VkMetalSurfaceCreateInfoEXT info = {
         .sType = VK_STRUCTURE_TYPE_METAL_SURFACE_CREATE_INFO_EXT,
         .pLayer = p->layer,
    };

    struct ra_ctx_params params = {0};

    VkInstance inst = vk->vkinst->instance;
    VkResult res = vkCreateMetalSurfaceEXT(inst, &info, NULL, &vk->surface);
    if (res != VK_SUCCESS) {
        MP_MSG(ctx, msgl, "Failed creating MoltenVK surface\n");
        goto fail;
    }

    if (!ra_vk_ctx_init(ctx, vk, params, VK_PRESENT_MODE_FIFO_KHR))
        goto fail;

    return true;
fail:
    moltenvk_uninit(ctx);
    return false;
}

static bool moltenvk_reconfig(struct ra_ctx *ctx)
{
    struct priv *p = ctx->priv;
    p->last_width = p->last_height = 0;
    resize(ctx);
    return true;
}

static int moltenvk_control(struct ra_ctx *ctx, int *events, int request, void *arg)
{
    if (request != VOCTRL_CHECK_EVENTS)
        return VO_NOTIMPL;

    struct priv *p = ctx->priv;
    CGSize s = p->layer.drawableSize;
    int w = (int)s.width, h = (int)s.height;
    if (w > 1 && h > 1 && (w != p->last_width || h != p->last_height)) {
        if (!resize(ctx))
            return VO_ERROR;
        *events |= VO_EVENT_RESIZE;
    }
    return VO_TRUE;
}

const struct ra_ctx_fns ra_ctx_vulkan_moltenvk = {
    .type           = "vulkan",
    .name           = "moltenvk",
    .reconfig       = moltenvk_reconfig,
    .control        = moltenvk_control,
    .init           = moltenvk_init,
    .uninit         = moltenvk_uninit,
};
