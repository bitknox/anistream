# Shaders: provenance and licences

Every file in this directory is someone else's work, vendored unmodified (apart from the
line endings noted below) and embedded into the anistream binary by
`crates/anistream-engine/src/playback/shaders.rs`. At runtime they are written, with these licence files,
to `<cache>/shaders/<digest>/` and handed to mpv as `glsl-shaders`.

The licences are not all the same. Read the table before redistributing.

| Licence | Files | Text |
|---|---|---|
| MIT | `Anime4K_*.glsl` | `LICENSE-Anime4K` |
| MIT | `ArtCNN_C4F16.glsl` | `LICENSE-ArtCNN` |
| **CC BY-NC 4.0** (model weights) + MIT (ArtCNN architecture) | `AniSD_ArtCNN_C4F32_i4.glsl` | `LICENSE-Upscale-Hub`, `LICENSE-ArtCNN` |
| LGPL-3.0-or-later | `KrigBilateral.glsl`, `SSimDownscaler.glsl` | `LICENSE-LGPL-3.0` (which incorporates `LICENSE-GPL-3.0`) |

## The non-commercial files

`AniSD_ArtCNN_C4F32_i4.glsl` is **not MIT**. It is a model trained by Sirosky on the ArtCNN architecture and distributed under Creative Commons
Attribution-NonCommercial 4.0 International. anistream bundles them because it is free,
open source and non-commercial, which that licence allows, with attribution. The
attribution header at the top of each file is the author's and stays intact.

Anyone distributing anistream, or a fork of it, in a way that is primarily intended for
commercial advantage or monetary compensation must remove these two files (and their
entries in `shaders.rs`); the chooser then needs another model for SD and 720p sources.
The rest of this directory carries no such restriction.

## Files

Hashes are SHA-256 of the file as vendored here.

### Anime4K v4.0.1 — bloc97/Anime4K, MIT

Source: `https://github.com/bloc97/Anime4K/releases/download/v4.0.1/Anime4K_v4.0.zip`
(tag `v4.0.1`, commit `4029bf701ecaa15f163cdc49cffe5501c1acf410`; archive SHA-256
`139cd282086457c5adc79caf7b75b8b825091d71c9b54958c18745fea62d7ed7`). Chains follow
`md/GLSL_Instructions_Advanced.md` and `md/Template/GLSL_Mac_Linux_{High,Low}-end/input.conf`.

| File | SHA-256 |
|---|---|
| `Anime4K_AutoDownscalePre_x2.glsl` ¹ | `8c58291740146bd766a4d73f132775a797fe80f7d07919b5d767e27a5dc85656` |
| `Anime4K_AutoDownscalePre_x4.glsl` ¹ | `5af62d8cd844916dc1126613e13bad3beab195787f93a71200b47c6ec78f2e41` |
| `Anime4K_Clamp_Highlights.glsl` ¹ | `a2a9bf7fbc1d75d09660ca2e701e4d7fb0cf5457b94da47e1825032fa2b3671a` |
| `Anime4K_Restore_CNN_M.glsl` | `67ea3ed26539e8de3b7d307688535d2ff17e8d147e11dda0247da7770dbecf41` |
| `Anime4K_Restore_CNN_VL.glsl` | `35036722733305cd4d4e57660b883bbe2569ba2914033c254327107d7b77e35e` |
| `Anime4K_Restore_CNN_Soft_M.glsl` | `a78a2c76898e08e09e442a9628c64208c26e8e15789649b8755223f009794c02` |
| `Anime4K_Restore_CNN_Soft_VL.glsl` | `094334b0e20c1a201fe4941c7c68de72451e5aee9efb5524d7fb82b12dca64b9` |
| `Anime4K_Upscale_CNN_x2_S.glsl` | `4c53ec2e287908f7ee7bcb266b0170421626d663576468b7d7dafc62962649a4` |
| `Anime4K_Upscale_CNN_x2_M.glsl` | `716e02098a68f0d648761f2b96b4dd139e1cb09b174bb369fca3aa34328fff7e` |
| `Anime4K_Upscale_CNN_x2_VL.glsl` | `5638fe31c37c151a3443fea3451a3ef91af073f4dbb9615f6c0d1e29db11493d` |
| `Anime4K_Upscale_Denoise_CNN_x2_M.glsl` | `8c72b042e2301fe66a45c3089720459148e2504cd72af16f9c0d5017ff14181e` |
| `Anime4K_Upscale_Denoise_CNN_x2_VL.glsl` | `359c48fe5a317fbc6b706ce368401eef496e84ed98abac7a43efebca2b65d79b` |

¹ Shipped upstream with CRLF line endings and vendored with LF; otherwise byte-identical.
Upstream SHA-256: `AutoDownscalePre_x2` `9141668ced0b26512253e6396e805820716f35b57c92950d9da489f8b96a7ba4`,
`AutoDownscalePre_x4` `dadb7b713cfa1d810c55b5deff616072f3390e546fed1e6a54f80ea555f7b95d`,
`Clamp_Highlights` `8c5fb67c76bed3021f8a27b050c3b97a6ac1b284f9ce91c04189015c354c0217`.

### ArtCNN v1.6.2 — Artoriuz/ArtCNN, MIT

Source: `https://raw.githubusercontent.com/Artoriuz/ArtCNN/f606e1f0ba7e6f0ab55049f33dac4d854819b00b/GLSL/<file>`
(tag `v1.6.2`, commit `f606e1f0ba7e6f0ab55049f33dac4d854819b00b`).

| File | SHA-256 |
|---|---|
| `ArtCNN_C4F16.glsl` | `03d0b3d31cb82c898a94a46663021a3e8f02c5a21d69c5cfdf0208de4bfd453e` |

`LICENSE-ArtCNN` is the repository's `LICENSE` at the same commit
(`285055c4b8b8bcb891685ca9cc6047baf25a8c1cf56e8ab2cbcdecaea0437c0b`).

### AniSD ArtCNN — Sirosky/Upscale-Hub, CC BY-NC 4.0

Release assets, which carry no separate commit; the repository's `main` was
`0498c4147df8f76d224b71a27b832c89934c1e70` when they were fetched.

| File | Source | SHA-256 |
|---|---|---|
| `AniSD_ArtCNN_C4F32_i4.glsl` | `https://github.com/Sirosky/Upscale-Hub/releases/download/AniSD-ArtCNN/AniSD_ArtCNN_C4F32_i4.glsl` (release `AniSD-ArtCNN`, 2024-06-29) | `7790bf1372250f9b429a198b8b603dd0b0b8a72edf7670003bb1df79fca3d46d` |

`LICENSE-Upscale-Hub` is `LICENSE.md` at `0498c4147df8f76d224b71a27b832c89934c1e70`
(`75aec2004e672b5677a971f641e8b6ea787eda444635bbbb19253d37c83080ec`), the full
CC BY-NC 4.0 legal code.

What the models are for, from the author's release notes and wiki: AniSD targets
standard-definition anime (DVD and SD web, roughly 2000s onward) and aims for faithful,
natural output; Ani4K v2 targets modern HD anime, from Blu-ray to heavily compressed web
releases, upscaled to 2K or 4K. The author's sample mpv auto-profile assigns 720p and
1080p to Ani4K v2 and 480/540/576 to AniSD. Ani4K v2 is not shipped: it only just held real
time on a 720p source on an M4 Pro, and ArtCNN C4F16 looked nearly the same at a fraction of
the cost (see `crates/anistream-engine/src/playback/shaders.rs`). The `i2`/`i4` suffixes are the author's
training-iteration tags; neither the releases nor the wiki define them further, and only
one of each is published. The `_CMP` compute variants were not taken: on this Mac (`gpu-next` over MoltenVK) they
cost about the same as the fragment versions, and the author notes they may only work
with Vulkan, while the fragment versions run on every mpv backend.

### KrigBilateral and SSimDownscaler — igv's gists, LGPL-3.0-or-later

| File | Source | SHA-256 |
|---|---|---|
| `KrigBilateral.glsl` | `https://gist.githubusercontent.com/igv/a015fc885d5c22e6891820ad89555637/raw/038064821c5f768dfc6c00261535018d5932cdd5/KrigBilateral.glsl` (gist revision `038064821c5f768dfc6c00261535018d5932cdd5`, 2023-10-27) | `8a4798abb77b83646fce0d71985ddfd14c9806e3be486e8fc7f5a5a8bd940cb0` |
| `SSimDownscaler.glsl` | `https://gist.githubusercontent.com/igv/36508af3ffc84410fe39761d6969be10/raw/38992bce7f9ff844f800820df0908692b65bb74a/SSimDownscaler.glsl` (gist revision `38992bce7f9ff844f800820df0908692b65bb74a`, 2024-09-05) | `f46f4710a162d17058b9d82ed8610588b0c04d7be07cef6bf2a8c4077828f804` |

KrigBilateral credits Shiandow in its header. `LICENSE-LGPL-3.0` and `LICENSE-GPL-3.0` are
`https://www.gnu.org/licenses/lgpl-3.0.txt` (`e3a994d82e644b03a792a930f574002658412f62407f5fee083f2555c5f23118`)
and `https://www.gnu.org/licenses/gpl-3.0.txt` (`3972dc9744f6499f0f9b2dbf76696f2ae7ad8af9b23dde66d6af86c9dfb36986`).
The shaders are separate files handed to mpv at runtime, unmodified; their source is this
directory.
