//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_box_shadow.c`.
//!
//! Integer box blur based shadow with the `LV_DRAW_SW_SHADOW_CACHE_SIZE` cache.
//!
//! Upstream reinterprets the 16-bit up-scaled corner buffer as `lv_opa_t`
//! in place once the blur is done. Without `unsafe` the 8-bit result is
//! left in the low byte of each `u16` slot by [`shadow_draw_corner_buf`]
//! (the in-place C conversion never reads a slot it has already
//! overwritten, so the values are identical) and [`box_shadow`] narrows it
//! into a `Vec<u8>`.
//!
//! `masks[]` (a `NULL`-terminated `void *` array) is a `Vec<MaskParam>` with
//! zero or one entry, `sh_buf_tmp` pointer arithmetic is an index into
//! `sh_buf`, and the circle cache of `lv_global_t` is
//! [`SwDrawUnit::circle_cache`].

use alloc::vec;
use alloc::vec::Vec;

use ferrovgl_core::draw::rect::BoxShadowDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask, Layer};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::{Color, ColorFormat, OPA_COVER, OPA_MAX, Opa, opa_mix2};
use ferrovgl_core::misc::style::BlendMode;

use crate::SwDrawUnit;
use crate::blend::{self, BlendDsc, CustomBlendHandler};
use crate::mask::{self, CircleCache, MaskParam, MaskRadiusParam, MaskRes};

/// `SHADOW_UPSCALE_SHIFT`
const SHADOW_UPSCALE_SHIFT: i32 = 6;

/// `LV_DRAW_SW_SHADOW_CACHE_SIZE` of the parity configuration
/// (`tests/configs/full.defconfig`: `CONFIG_LV_DRAW_SW_SHADOW_CACHE_SIZE=8`).
pub const SHADOW_CACHE_SIZE: u32 = 8;

/// `sizeof(cache->cache)`: `LV_DRAW_SW_SHADOW_CACHE_SIZE ^ 2` bytes.
const SHADOW_CACHE_BYTES: u32 = SHADOW_CACHE_SIZE * SHADOW_CACHE_SIZE;

/// `lv_draw_sw_shadow_cache_t`
#[derive(Clone, Debug)]
pub struct ShadowCache {
    /// `cache`: the cached corner (`cache_size * cache_size` bytes).
    pub cache: Vec<u8>,
    /// `cache_size`
    pub size: i32,
    /// `cache_r`
    pub r: i32,
}

impl Default for ShadowCache {
    /// The state `lv_init()` sets up: `cache_size = -1` and `cache_r = -1`,
    /// so the first `lv_draw_sw_box_shadow()` call always misses the cache.
    fn default() -> Self {
        Self {
            cache: Vec::new(),
            size: -1,
            r: -1,
        }
    }
}

/// `lv_draw_sw_box_shadow()`
pub fn box_shadow(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &BoxShadowDsc, coords: &Area) {
    /*Calculate the rectangle which is blurred to get the shadow in `shadow_area`*/
    let core_area = Area {
        x1: coords.x1 + dsc.ofs_x - dsc.spread,
        x2: coords.x2 + dsc.ofs_x + dsc.spread,
        y1: coords.y1 + dsc.ofs_y - dsc.spread,
        y2: coords.y2 + dsc.ofs_y + dsc.spread,
    };

    /*Calculate the bounding box of the shadow*/
    let shadow_area = Area {
        x1: core_area.x1 - dsc.width / 2 - 1,
        x2: core_area.x2 + dsc.width / 2 + 1,
        y1: core_area.y1 - dsc.width / 2 - 1,
        y2: core_area.y2 + dsc.width / 2 + 1,
    };

    let mut opa = dsc.opa;
    if opa > OPA_MAX {
        opa = OPA_COVER;
    }

    /*Get clipped draw area which is the real draw area.
     *It is always the same or inside `shadow_area`*/
    if Area::intersect(&shadow_area, &t.clip_area).is_none() {
        return;
    }

    /*Consider 1 px smaller bg to be sure the edge will be covered by the shadow*/
    let mut bg_area = *coords;
    bg_area.increase(-1, -1);

    /*Get the clamped radius*/
    let mut r_bg = dsc.radius;
    let mut short_side = bg_area.width().min(bg_area.height());
    if r_bg > short_side >> 1 {
        r_bg = short_side >> 1;
    }

    /*Get the clamped radius*/
    let mut r_sh = dsc.radius;
    short_side = core_area.width().min(core_area.height());
    if r_sh > short_side >> 1 {
        r_sh = short_side >> 1;
    }

    /*Get how many pixels are affected by the blur on the corners*/
    let corner_size = dsc.width + r_sh;
    /* A negative size would be a huge `lv_malloc` upstream (fails). */
    if corner_size < 0 {
        return;
    }
    let cs = corner_size as usize;

    let mut sh_buf: Vec<u8>;
    let cache = &mut unit.shadow_cache;
    if cache.size == corner_size && cache.r == r_sh {
        /*Use the cache if available*/
        sh_buf = vec![0; cs * cs];
        let n = (cs * cs).min(cache.cache.len());
        sh_buf[..n].copy_from_slice(&cache.cache[..n]);
    } else {
        /*A larger buffer is required for calculation*/
        let mut ups = vec![0u16; cs * cs];
        draw_corner_buf(&mut unit.circle_cache, &core_area, &mut ups, dsc.width, r_sh);
        sh_buf = ups.iter().map(|&v| v as u8).collect();

        /*Cache the corner if it fits into the cache size*/
        if (corner_size as u32).wrapping_mul(corner_size as u32) < SHADOW_CACHE_BYTES {
            cache.cache.clear();
            cache.cache.extend_from_slice(&sh_buf);
            cache.size = corner_size;
            cache.r = r_sh;
        }
    }

    /*Skip a lot of masking if the background will cover the shadow that would be masked out*/
    let simple = dsc.bg_cover;

    /*Create a radius mask to clip remove shadow on the bg area*/
    let mut masks: Vec<MaskParam<'static>> = Vec::new();
    if !simple {
        let mut mask_rout_param = MaskRadiusParam::default();
        mask::radius_init(&mut unit.circle_cache, &mut mask_rout_param, &bg_area, r_bg, true);
        masks.push(MaskParam::Radius(mask_rout_param));
    }

    let Some(layer) = ctx.layer_mut(t.target_layer) else {
        free_masks(&mut unit.circle_cache, &mut masks);
        return;
    };

    let mut mask_buf: Vec<Opa> = vec![0; shadow_area.width().max(0) as usize];

    let mut b = Blender {
        t,
        layer,
        handlers: &unit.blend_handlers,
        color: dsc.color,
        opa: dsc.opa,
        mask_res: MaskRes::Transp,
    };
    let cache = &mut unit.circle_cache;

    let w_half = shadow_area.x1 + shadow_area.width() / 2;
    let h_half = shadow_area.y1 + shadow_area.height() / 2;

    /*Draw the corners if they are on the current clip area and not fully covered by the bg*/

    /*Top right corner*/
    let mut blend_area = Area {
        x2: shadow_area.x2,
        x1: shadow_area.x2 - corner_size + 1,
        y1: shadow_area.y1,
        y2: shadow_area.y1 + corner_size - 1,
    };
    /*Do not overdraw the other top corners*/
    blend_area.x1 = blend_area.x1.max(w_half);
    blend_area.y2 = blend_area.y2.min(h_half);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        let mut tmp = (clip_area_sub.y1 - shadow_area.y1) as usize * cs;
        tmp += (clip_area_sub.x1 - (shadow_area.x2 - corner_size + 1)) as usize;

        /*Do not mask if out of the bg*/
        let simple_sub = simple_sub(simple, &clip_area_sub, &bg_area, r_bg);
        if w > 0 {
            b.mask_res = MaskRes::Changed; /*In simple mode it won't be overwritten*/
            for y in clip_area_sub.y1..=clip_area_sub.y2 {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);
                corner_row(
                    &mut b,
                    cache,
                    &mut masks,
                    &mut mask_buf,
                    &sh_buf,
                    tmp,
                    cs,
                    simple_sub,
                    row,
                    w,
                );
                tmp += cs;
            }
        }
    }

    /*Bottom right corner.
     *Almost the same as top right just read the lines of `sh_buf` from then end*/
    blend_area = Area {
        x2: shadow_area.x2,
        x1: shadow_area.x2 - corner_size + 1,
        y1: shadow_area.y2 - corner_size + 1,
        y2: shadow_area.y2,
    };
    /*Do not overdraw the other corners*/
    blend_area.x1 = blend_area.x1.max(w_half);
    blend_area.y1 = blend_area.y1.max(h_half + 1);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        let mut tmp = (blend_area.y2 - clip_area_sub.y2) as usize * cs;
        tmp += (clip_area_sub.x1 - (shadow_area.x2 - corner_size + 1)) as usize;
        /*Do not mask if out of the bg*/
        let simple_sub = simple_sub(simple, &clip_area_sub, &bg_area, r_bg);

        if w > 0 {
            b.mask_res = MaskRes::Changed; /*In simple mode it won't be overwritten*/
            for y in (clip_area_sub.y1..=clip_area_sub.y2).rev() {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);
                corner_row(
                    &mut b,
                    cache,
                    &mut masks,
                    &mut mask_buf,
                    &sh_buf,
                    tmp,
                    cs,
                    simple_sub,
                    row,
                    w,
                );
                tmp += cs;
            }
        }
    }

    /*Top side*/
    blend_area = Area {
        x1: shadow_area.x1 + corner_size,
        x2: shadow_area.x2 - corner_size,
        y1: shadow_area.y1,
        y2: shadow_area.y1 + corner_size - 1,
    };
    blend_area.y2 = blend_area.y2.min(h_half);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        let mut tmp = (clip_area_sub.y1 - blend_area.y1) as usize * cs;

        /*Do not mask if out of the bg*/
        let simple_sub = simple_sub(simple, &clip_area_sub, &bg_area, r_bg);

        if w > 0 {
            for y in clip_area_sub.y1..=clip_area_sub.y2 {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);
                side_row(
                    &mut b,
                    cache,
                    &mut masks,
                    &mut mask_buf,
                    &sh_buf,
                    tmp,
                    simple_sub,
                    row,
                    w,
                    opa,
                    dsc.opa,
                );
                tmp += cs;
            }
        }
    }
    b.opa = dsc.opa; /*Restore*/

    /*Bottom side*/
    blend_area = Area {
        x1: shadow_area.x1 + corner_size,
        x2: shadow_area.x2 - corner_size,
        y1: shadow_area.y2 - corner_size + 1,
        y2: shadow_area.y2,
    };
    blend_area.y1 = blend_area.y1.max(h_half + 1);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        let mut tmp = (blend_area.y2 - clip_area_sub.y2) as usize * cs;
        if w > 0 {
            for y in (clip_area_sub.y1..=clip_area_sub.y2).rev() {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);

                /*Do not mask if out of the bg*/
                let simple_sub = simple_sub(simple, &clip_area_sub, &bg_area, r_bg);

                /* `(sh_buf_tmp[0] * dsc->opa) >> 8` is `LV_OPA_MIX2`. */
                side_row(
                    &mut b,
                    cache,
                    &mut masks,
                    &mut mask_buf,
                    &sh_buf,
                    tmp,
                    simple_sub,
                    row,
                    w,
                    opa,
                    dsc.opa,
                );
                tmp += cs;
            }
        }
    }

    b.opa = dsc.opa; /*Restore*/

    /*Right side*/
    blend_area = Area {
        x1: shadow_area.x2 - corner_size + 1,
        x2: shadow_area.x2,
        y1: shadow_area.y1 + corner_size,
        y2: shadow_area.y2 - corner_size,
    };
    /*Do not overdraw the other corners*/
    blend_area.y1 = blend_area.y1.min(h_half + 1);
    blend_area.y2 = blend_area.y2.max(h_half);
    blend_area.x1 = blend_area.x1.max(w_half);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        let mut tmp = cs.saturating_sub(1) * cs;
        tmp += (clip_area_sub.x1 - (shadow_area.x2 - corner_size + 1)) as usize;

        /*Do not mask if out of the bg*/
        let simple_sub = simple_sub(simple, &clip_area_sub, &bg_area, r_bg);
        if w > 0 {
            b.mask_res = MaskRes::Changed; /*In simple mode it won't be overwritten*/
            for y in clip_area_sub.y1..=clip_area_sub.y2 {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);
                edge_row(
                    &mut b,
                    cache,
                    &mut masks,
                    &mut mask_buf,
                    &sh_buf,
                    tmp,
                    simple_sub,
                    row,
                    w,
                );
            }
        }
    }

    /*Mirror the shadow corner buffer horizontally*/
    if cs > 0 {
        for line in sh_buf.chunks_exact_mut(cs) {
            line.reverse();
        }
    }

    /*Left side*/
    blend_area = Area {
        x1: shadow_area.x1,
        x2: shadow_area.x1 + corner_size - 1,
        y1: shadow_area.y1 + corner_size,
        y2: shadow_area.y2 - corner_size,
    };
    /*Do not overdraw the other corners*/
    blend_area.y1 = blend_area.y1.min(h_half + 1);
    blend_area.y2 = blend_area.y2.max(h_half);
    blend_area.x2 = blend_area.x2.min(w_half - 1);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        let mut tmp = cs.saturating_sub(1) * cs;
        tmp += (clip_area_sub.x1 - blend_area.x1) as usize;

        /*Do not mask if out of the bg*/
        let simple_sub = simple_sub(simple, &clip_area_sub, &bg_area, r_bg);
        if w > 0 {
            b.mask_res = MaskRes::Changed; /*In simple mode it won't be overwritten*/
            for y in clip_area_sub.y1..=clip_area_sub.y2 {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);
                edge_row(
                    &mut b,
                    cache,
                    &mut masks,
                    &mut mask_buf,
                    &sh_buf,
                    tmp,
                    simple_sub,
                    row,
                    w,
                );
            }
        }
    }

    /*Top left corner*/
    blend_area = Area {
        x1: shadow_area.x1,
        x2: shadow_area.x1 + corner_size - 1,
        y1: shadow_area.y1,
        y2: shadow_area.y1 + corner_size - 1,
    };
    /*Do not overdraw the other corners*/
    blend_area.x2 = blend_area.x2.min(w_half - 1);
    blend_area.y2 = blend_area.y2.min(h_half);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        let mut tmp = (clip_area_sub.y1 - blend_area.y1) as usize * cs;
        tmp += (clip_area_sub.x1 - blend_area.x1) as usize;

        /*Do not mask if out of the bg*/
        let simple_sub = simple_sub(simple, &clip_area_sub, &bg_area, r_bg);

        if w > 0 {
            b.mask_res = MaskRes::Changed; /*In simple mode it won't be overwritten*/
            for y in clip_area_sub.y1..=clip_area_sub.y2 {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);
                corner_row(
                    &mut b,
                    cache,
                    &mut masks,
                    &mut mask_buf,
                    &sh_buf,
                    tmp,
                    cs,
                    simple_sub,
                    row,
                    w,
                );
                tmp += cs;
            }
        }
    }

    /*Bottom left corner.
     *Almost the same as bottom right just read the lines of `sh_buf` from then end*/
    blend_area = Area {
        x1: shadow_area.x1,
        x2: shadow_area.x1 + corner_size - 1,
        y1: shadow_area.y2 - corner_size + 1,
        y2: shadow_area.y2,
    };
    /*Do not overdraw the other corners*/
    blend_area.y1 = blend_area.y1.max(h_half + 1);
    blend_area.x2 = blend_area.x2.min(w_half - 1);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        let mut tmp = (blend_area.y2 - clip_area_sub.y2) as usize * cs;
        tmp += (clip_area_sub.x1 - blend_area.x1) as usize;

        /*Do not mask if out of the bg*/
        let simple_sub = simple_sub(simple, &clip_area_sub, &bg_area, r_bg);
        if w > 0 {
            b.mask_res = MaskRes::Changed; /*In simple mode it won't be overwritten*/
            for y in (clip_area_sub.y1..=clip_area_sub.y2).rev() {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);
                corner_row(
                    &mut b,
                    cache,
                    &mut masks,
                    &mut mask_buf,
                    &sh_buf,
                    tmp,
                    cs,
                    simple_sub,
                    row,
                    w,
                );
                tmp += cs;
            }
        }
    }

    /*Draw the center rectangle.*/
    blend_area = Area {
        x1: shadow_area.x1 + corner_size,
        x2: shadow_area.x2 - corner_size,
        y1: shadow_area.y1 + corner_size,
        y2: shadow_area.y2 - corner_size,
    };
    blend_area.y1 = blend_area.y1.min(h_half + 1);
    blend_area.y2 = blend_area.y2.max(h_half);

    if let Some(clip_area_sub) = visible_part(&blend_area, &t.clip_area, &bg_area, r_bg) {
        let w = clip_area_sub.width();
        if w > 0 {
            for y in clip_area_sub.y1..=clip_area_sub.y2 {
                let row = Area::new(clip_area_sub.x1, y, clip_area_sub.x2, y);

                let line = &mut mask_buf[..w as usize];
                line.fill(0xff);
                b.mask_res = mask::apply(cache, &mut masks, line, clip_area_sub.x1, y, w);
                b.blend(row, Some(line));
            }
        }
    }

    free_masks(cache, &mut masks);
}

/// `shadow_draw_corner_buf()`: render the up-sampled 16-bit corner into
/// the caller-provided `sh_buf`, blur it in place with
/// [`shadow_blur_corner`] and down-convert to 8 bit inside (the cache
/// lookup stays in [`box_shadow`], as upstream).
///
/// `sh_buf` must hold at least `(sw + r)^2` elements. On return each of the
/// first `(sw + r)^2` elements holds the `lv_opa_t` result (`0..=255`) that
/// upstream writes into the same memory as bytes.
///
/// Upstream initialises the radius mask from the global circle cache; this
/// entry point uses a temporary cache (the circle data is the same, only
/// the cache bookkeeping differs). [`box_shadow`] uses
/// [`SwDrawUnit::circle_cache`].
pub fn shadow_draw_corner_buf(coords: &Area, sh_buf: &mut [u16], sw: i32, r: i32) {
    let mut cache = CircleCache::default();
    draw_corner_buf(&mut cache, coords, sh_buf, sw, r);
}

/// `shadow_blur_corner()`
///
/// `sh_ups_buf` must hold at least `size^2` elements.
pub fn shadow_blur_corner(size: i32, sw: i32, sh_ups_buf: &mut [u16]) {
    /* `sw <= 0` divides by zero / indexes out of bounds upstream; it is never
     * reached (`shadow_width == 0` is skipped by `lv_draw_box_shadow`). */
    if size <= 0 || sw <= 0 {
        return;
    }
    let n = size as usize;
    let sh_ups_buf = &mut sh_ups_buf[..n * n];

    let s_left_init = sw >> 1;
    let s_right = sw >> 1;
    let s_left = if (sw & 1) == 0 {
        s_left_init - 1
    } else {
        s_left_init
    };

    /*Horizontal blur*/
    let mut sh_ups_blur_buf: Vec<u16> = vec![0; n];

    for line in sh_ups_buf.chunks_exact_mut(n) {
        let mut v: i32 = (line[n - 1] as i32).wrapping_mul(sw);
        for x in (0..size).rev() {
            sh_ups_blur_buf[x as usize] = v as u16;

            /*Forget the right pixel*/
            let mut right_val: u32 = 0;
            if x + s_right < size {
                right_val = line[(x + s_right) as usize] as u32;
            }
            v = (v as u32).wrapping_sub(right_val) as i32;

            /*Add the left pixel*/
            let left_val: u32 = if x - s_left - 1 < 0 {
                line[0] as u32
            } else {
                line[(x - s_left - 1) as usize] as u32
            };
            v = (v as u32).wrapping_add(left_val) as i32;
        }
        line.copy_from_slice(&sh_ups_blur_buf);
    }

    /*Vertical blur*/
    let max_v: u32 = (OPA_COVER as u32) << SHADOW_UPSCALE_SHIFT;
    let max_v_div: u32 = max_v / sw as u32;
    for px in sh_ups_buf.iter_mut() {
        if *px == 0 {
            continue;
        } else if *px as u32 == max_v {
            *px = max_v_div as u16;
        } else {
            *px = (*px as i32 / sw) as u16;
        }
    }

    for x in 0..n {
        let mut v: i32 = (sh_ups_buf[x] as i32).wrapping_mul(sw);
        for y in 0..size {
            sh_ups_blur_buf[y as usize] = if v < 0 {
                0
            } else {
                (v >> SHADOW_UPSCALE_SHIFT) as u16
            };

            /*Forget the top pixel*/
            let top_val: u32 = if y - s_right <= 0 {
                /* `sh_ups_tmp_buf[0]`: the pixel of the current row */
                sh_ups_buf[y as usize * n + x] as u32
            } else {
                sh_ups_buf[(y - s_right) as usize * n + x] as u32
            };
            v = (v as u32).wrapping_sub(top_val) as i32;

            /*Add the bottom pixel*/
            let bottom_val: u32 = if y + s_left + 1 < size {
                sh_ups_buf[(y + s_left + 1) as usize * n + x] as u32
            } else {
                sh_ups_buf[(n - 1) * n + x] as u32
            };
            v = (v as u32).wrapping_add(bottom_val) as i32;
        }

        /*Write back the result into `sh_ups_buf`*/
        for y in 0..n {
            sh_ups_buf[y * n + x] = sh_ups_blur_buf[y];
        }
    }
}

/*********************
 *   STATIC FUNCTIONS
 *********************/

/// The body of `shadow_draw_corner_buf()` with an explicit circle cache.
fn draw_corner_buf(cache: &mut CircleCache, coords: &Area, sh_buf: &mut [u16], sw: i32, r: i32) {
    let sw_ori = sw;
    let size = sw_ori + r;
    if size <= 0 {
        return;
    }
    let n = size as usize;
    let sh_buf = &mut sh_buf[..n * n];

    let mut sh_area = *coords;
    sh_area.x2 = sw / 2 + r - 1 - if (sw & 1) != 0 { 0 } else { 1 };
    sh_area.y1 = sw / 2 + 1;

    sh_area.x1 = sh_area.x2 - coords.width();
    sh_area.y2 = sh_area.y1 + coords.height();

    let mut mask_param = MaskRadiusParam::default();
    mask::radius_init(cache, &mut mask_param, &sh_area, r, false);

    /*SHADOW_ENHANCE: Set half shadow width because blur will be repeated*/
    let mut sw = if sw_ori == 1 { 1 } else { sw_ori >> 1 };

    /* `sw == 0` (shadow width 0) divides by zero upstream; it is never
     * reached because `lv_draw_box_shadow` skips zero width shadows. */
    if sw == 0 {
        sh_buf.fill(0);
        mask::free_param(cache, &mut MaskParam::Radius(mask_param));
        return;
    }

    let mut mask_line: Vec<Opa> = vec![0; n];
    for (y, line) in sh_buf.chunks_exact_mut(n).enumerate() {
        mask_line.fill(0xff);
        let mask_res = mask::mask_radius(cache, &mut mask_line, 0, y as i32, size, &mut mask_param);
        if mask_res == MaskRes::Transp {
            line.fill(0);
        } else {
            line[0] = (((mask_line[0] as i32) << SHADOW_UPSCALE_SHIFT) / sw) as u16;
            for i in 1..n {
                if mask_line[i] == mask_line[i - 1] {
                    line[i] = line[i - 1];
                } else {
                    line[i] = (((mask_line[i] as i32) << SHADOW_UPSCALE_SHIFT) / sw) as u16;
                }
            }
        }
    }
    drop(mask_line);

    mask::free_param(cache, &mut MaskParam::Radius(mask_param));

    if sw == 1 {
        for px in sh_buf.iter_mut() {
            *px = ((*px >> SHADOW_UPSCALE_SHIFT) as u8) as u16;
        }
        return;
    }

    shadow_blur_corner(size, sw, sh_buf);

    /*SHADOW_ENHANCE == 1*/
    sw += sw_ori & 1;
    if sw > 1 {
        let max_v_div: u32 = (((OPA_COVER as i32) << SHADOW_UPSCALE_SHIFT) / sw) as u32;
        for px in sh_buf.iter_mut() {
            if *px == 0 {
                continue;
            } else if *px == OPA_COVER as u16 {
                *px = max_v_div as u16;
            } else {
                *px = (((*px as i32) << SHADOW_UPSCALE_SHIFT) / sw) as u16;
            }
        }

        shadow_blur_corner(size, sw, sh_buf);
    }
    /* `res_buf[x] = (lv_opa_t) sh_buf[x]` */
    for px in sh_buf.iter_mut() {
        *px = (*px as u8) as u16;
    }
}

/// The blend descriptor state shared by the passes of [`box_shadow`]
/// (`blend_dsc` upstream; `blend_area` and `mask_buf` are per call).
struct Blender<'a> {
    t: &'a DrawTask,
    layer: &'a mut Layer,
    handlers: &'a [CustomBlendHandler],
    color: Color,
    opa: Opa,
    mask_res: MaskRes,
}

impl Blender<'_> {
    /// `lv_draw_sw_blend(t, &blend_dsc)` with `blend_area == mask_area == row`.
    fn blend(&mut self, row: Area, mask_buf: Option<&[Opa]>) {
        let dsc = BlendDsc {
            blend_area: row,
            src_buf: None,
            src_stride: 0,
            src_color_format: ColorFormat::Unknown,
            src_area: None,
            opa: self.opa,
            color: self.color,
            mask_buf,
            mask_res: self.mask_res,
            mask_area: Some(row),
            mask_stride: 0,
            blend_mode: BlendMode::Normal,
        };
        blend::blend(self.t, self.layer, &dsc, self.handlers);
    }
}

/// `lv_area_intersect(&clip_area_sub, blend_area, &t->clip_area) &&
/// !lv_area_is_in(&clip_area_sub, bg_area, r_bg)`
fn visible_part(blend_area: &Area, clip_area: &Area, bg_area: &Area, r_bg: i32) -> Option<Area> {
    let clip_area_sub = Area::intersect(blend_area, clip_area)?;
    if clip_area_sub.is_in(bg_area, r_bg) {
        return None;
    }
    Some(clip_area_sub)
}

/// `if(simple && lv_area_is_out(...)) simple_sub = true; else simple_sub = simple;`
fn simple_sub(simple: bool, clip_area_sub: &Area, bg_area: &Area, r_bg: i32) -> bool {
    if simple && clip_area_sub.is_out(bg_area, r_bg) {
        true
    } else {
        simple
    }
}

/// `sh_buf_tmp`: the tail of `sh_buf` at `ofs` (empty past the end).
fn tail(sh_buf: &[Opa], ofs: usize) -> &[Opa] {
    sh_buf.get(ofs..).unwrap_or(&[])
}

/// `lv_memcpy(mask_buf, sh_buf_tmp, len)`. Only the first `w` bytes are
/// used afterwards, so a copy truncated to the available source (upstream
/// reads past the 8-bit data on the last row) gives the same result.
fn copy_line(mask_buf: &mut [Opa], src: &[Opa], len: usize) {
    let n = len.min(src.len()).min(mask_buf.len());
    mask_buf[..n].copy_from_slice(&src[..n]);
}

/// One row of a corner pass: mask the `corner_size` wide corner line (or
/// use it directly as the mask in simple mode) and blend.
#[allow(clippy::too_many_arguments)] // mirrors the locals of the C loop body
fn corner_row(
    b: &mut Blender<'_>,
    cache: &mut CircleCache,
    masks: &mut [MaskParam<'_>],
    mask_buf: &mut [Opa],
    sh_buf: &[Opa],
    tmp: usize,
    corner_size: usize,
    simple_sub: bool,
    row: Area,
    w: i32,
) {
    if !simple_sub {
        copy_line(mask_buf, tail(sh_buf, tmp), corner_size);
        b.mask_res = mask::apply(cache, masks, mask_buf, row.x1, row.y1, w);
        if b.mask_res == MaskRes::FullCover {
            b.mask_res = MaskRes::Changed;
        }
        b.blend(row, Some(mask_buf));
    } else {
        b.blend(row, Some(tail(sh_buf, tmp)));
    }
}

/// One row of the top / bottom side: the opacity of the row is
/// `sh_buf_tmp[0]`, either as a memset mask or folded into `opa`.
#[allow(clippy::too_many_arguments)] // mirrors the locals of the C loop body
fn side_row(
    b: &mut Blender<'_>,
    cache: &mut CircleCache,
    masks: &mut [MaskParam<'_>],
    mask_buf: &mut [Opa],
    sh_buf: &[Opa],
    tmp: usize,
    simple_sub: bool,
    row: Area,
    w: i32,
    opa: Opa,
    dsc_opa: Opa,
) {
    let v = sh_buf.get(tmp).copied().unwrap_or(0);
    if !simple_sub {
        let line = &mut mask_buf[..w as usize];
        line.fill(v);
        b.mask_res = mask::apply(cache, masks, line, row.x1, row.y1, w);
        if b.mask_res == MaskRes::FullCover {
            b.mask_res = MaskRes::Changed;
        }
        b.blend(row, Some(line));
    } else {
        b.opa = if opa == OPA_COVER { v } else { opa_mix2(v, dsc_opa) };
        b.blend(row, None);
    }
}

/// One row of the left / right side: the last corner line (from `tmp`)
/// is the same for every row.
#[allow(clippy::too_many_arguments)] // mirrors the locals of the C loop body
fn edge_row(
    b: &mut Blender<'_>,
    cache: &mut CircleCache,
    masks: &mut [MaskParam<'_>],
    mask_buf: &mut [Opa],
    sh_buf: &[Opa],
    tmp: usize,
    simple_sub: bool,
    row: Area,
    w: i32,
) {
    if !simple_sub {
        copy_line(mask_buf, tail(sh_buf, tmp), w as usize);
        b.mask_res = mask::apply(cache, masks, mask_buf, row.x1, row.y1, w);
        if b.mask_res == MaskRes::FullCover {
            b.mask_res = MaskRes::Changed;
        }
        b.blend(row, Some(mask_buf));
    } else {
        b.blend(row, Some(tail(sh_buf, tmp)));
    }
}

/// `lv_draw_sw_mask_free_param()` for every mask of `masks`.
fn free_masks(cache: &mut CircleCache, masks: &mut [MaskParam<'_>]) {
    for m in masks.iter_mut() {
        mask::free_param(cache, m);
    }
}

#[cfg(test)]
mod tests {
    //! The expected hashes come from the upstream C code (lvgl v9.6.0, the
    //! static functions included into a harness, `LV_DRAW_SW_COMPLEX=1`,
    //! `LV_DRAW_SW_SHADOW_CACHE_SIZE=8`) run on the same inputs.

    use super::*;
    use ferrovgl_core::draw::{DrawBuf, DrawDsc, LayerId};

    fn fnv(bytes: impl IntoIterator<Item = u8>) -> u32 {
        let mut h: u32 = 2166136261;
        for b in bytes {
            h ^= b as u32;
            h = h.wrapping_mul(16777619);
        }
        h
    }

    fn corner(coords: Area, sw: i32, r: i32) -> (u32, Vec<u8>) {
        let size = (sw + r) as usize;
        let mut buf = vec![0u16; size * size];
        shadow_draw_corner_buf(&coords, &mut buf, sw, r);
        assert!(buf.iter().all(|&v| v <= 0xff));
        let res: Vec<u8> = buf.iter().map(|&v| v as u8).collect();
        let last_row = res[(size - 1) * size..].iter().copied().take(12).collect();
        (fnv(res.iter().copied()), last_row)
    }

    #[test]
    fn corner_buf_matches_upstream() {
        let a = Area::new(10, 10, 60, 40);
        let b = Area::new(0, 0, 80, 50);
        assert_eq!(corner(a, 1, 0), (0x050c5d1f, vec![0]));
        assert_eq!(corner(a, 2, 5), (0xf0ff1f16, vec![255, 255, 255, 255, 255, 0, 0]));
        assert_eq!(corner(a, 3, 4), (0x081a2f3a, vec![255, 255, 255, 255, 255, 0, 0]));
        assert_eq!(
            corner(b, 15, 10),
            (
                0xba67db38,
                vec![254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 249, 240]
            )
        );
        assert_eq!(
            corner(b, 20, 0),
            (
                0xf40332ae,
                vec![239, 232, 222, 209, 194, 177, 157, 135, 110, 88, 68, 51]
            )
        );
        assert_eq!(corner(Area::new(0, 0, 30, 30), 9, 15).0, 0x461ff3bc);
    }

    fn blur(size: i32, sw: i32) -> u32 {
        let n = (size * size) as usize;
        let mut buf: Vec<u16> = (0..n as u32).map(|i| ((i * 7919 + 13) % 16321) as u16).collect();
        shadow_blur_corner(size, sw, &mut buf);
        fnv(buf.iter().flat_map(|v| v.to_le_bytes()))
    }

    #[test]
    fn blur_corner_matches_upstream() {
        assert_eq!(blur(7, 3), 0x73eb63a8);
        assert_eq!(blur(12, 4), 0x391ca626);
        assert_eq!(blur(20, 9), 0x40a3fdc8);
        // Degenerate inputs are no-ops instead of a division by zero.
        let mut buf = vec![7u16; 4];
        shadow_blur_corner(2, 0, &mut buf);
        shadow_blur_corner(0, 3, &mut buf);
        assert_eq!(buf, vec![7; 4]);
    }

    #[test]
    fn blur_keeps_flat_buffers_flat() {
        // Identical rows stay identical: every column of the result is
        // uniform (the vertical pass only sees equal values), while the
        // horizontal pass treats the pixels past the right edge as 0.
        // An empty buffer stays empty.
        let size = 9;
        let max_v = (OPA_COVER as u16) << SHADOW_UPSCALE_SHIFT;
        let mut buf = vec![max_v / 3; 81];
        shadow_blur_corner(size, 3, &mut buf);
        for x in 0..9 {
            assert!((0..9).all(|y| buf[y * 9 + x] == buf[x]));
        }
        assert!(buf[8] < buf[0]);
        let mut zero = vec![0u16; 81];
        shadow_blur_corner(size, 4, &mut zero);
        assert!(zero.iter().all(|&v| v == 0));
    }

    /// A `w x h` layer at `(x, y)` filled with the `i * 31 + 7` byte pattern.
    fn make_ctx(cf: ColorFormat, w: u32, h: u32, x: i32, y: i32) -> (DrawCtx, LayerId) {
        let mut ctx = DrawCtx::new();
        let mut layer = Layer::new();
        let mut db = DrawBuf::create(w, h, cf, 0).unwrap();
        let len = db.header.stride as usize * h as usize;
        let data = db.data_mut().unwrap();
        for (i, b) in data[..len].iter_mut().enumerate() {
            *b = (i as u32).wrapping_mul(31).wrapping_add(7) as u8;
        }
        layer.draw_buf = Some(db);
        layer.color_format = cf;
        layer.buf_area = Area::new(x, y, x + w as i32 - 1, y + h as i32 - 1);
        let id = LayerId(ctx.layers.insert(layer));
        (ctx, id)
    }

    fn layer_hash(ctx: &DrawCtx, id: LayerId) -> u32 {
        let db = ctx.layer(id).unwrap().draw_buf.as_ref().unwrap();
        let len = db.header.stride as usize * db.header.h as usize;
        fnv(db.data()[..len].iter().copied())
    }

    fn task(layer: LayerId, clip_area: Area) -> DrawTask {
        DrawTask {
            ty: Default::default(),
            area: clip_area,
            real_area: clip_area,
            clip_area,
            target_layer: layer,
            draw_unit: None,
            state: Default::default(),
            dsc: DrawDsc::None,
            opa: OPA_COVER,
            preferred_draw_unit_id: 0,
            preference_score: 0,
        }
    }

    #[cfg(feature = "cf-xrgb8888")]
    #[allow(clippy::too_many_arguments)] // test case table
    fn shadow(
        unit: &mut SwDrawUnit,
        cf: ColorFormat,
        clip: Area,
        coords: Area,
        radius: i32,
        width: i32,
        spread: i32,
        ofs: (i32, i32),
        opa: Opa,
        bg_cover: bool,
    ) -> u32 {
        let (mut ctx, id) = make_ctx(cf, 120, 90, 5, 3);
        let t = task(id, clip);
        let mut dsc = BoxShadowDsc::new();
        dsc.radius = radius;
        dsc.color = Color::make(0x20, 0x40, 0xc0);
        dsc.width = width;
        dsc.spread = spread;
        dsc.ofs_x = ofs.0;
        dsc.ofs_y = ofs.1;
        dsc.opa = opa;
        dsc.bg_cover = bg_cover;
        box_shadow(unit, &mut ctx, &t, &dsc, &coords);
        layer_hash(&ctx, id)
    }

    #[cfg(all(feature = "cf-xrgb8888", feature = "cf-argb8888", feature = "cf-rgb565"))]
    #[test]
    fn box_shadow_matches_upstream() {
        use ColorFormat::{Argb8888, Rgb565, Xrgb8888};
        // One unit for the whole sequence, like the single upstream cache.
        let mut u = SwDrawUnit::new(0);
        let full = Area::new(0, 0, 200, 200);
        let part = Area::new(20, 15, 70, 50);
        let c = Area::new(30, 25, 80, 60);
        let s = Area::new(40, 30, 70, 55);
        assert_eq!(
            shadow(&mut u, Xrgb8888, full, c, 8, 12, 2, (3, 4), 255, false),
            0x23b6b1bb
        );
        assert_eq!(
            shadow(&mut u, Xrgb8888, full, c, 8, 12, 2, (3, 4), 255, true),
            0xddc4cdc8
        );
        assert_eq!(
            shadow(&mut u, Xrgb8888, full, c, 8, 12, 2, (3, 4), 150, true),
            0xa7699d14
        );
        assert_eq!(
            shadow(&mut u, Xrgb8888, full, c, 8, 12, 2, (3, 4), 150, false),
            0x9fee0cb7
        );
        assert_eq!(
            shadow(&mut u, Xrgb8888, part, c, 30, 20, 0, (-5, 6), 200, false),
            0xf185b9d5
        );
        assert_eq!(
            shadow(&mut u, Xrgb8888, part, c, 30, 20, 0, (-5, 6), 200, true),
            0x8805737b
        );
        assert_eq!(
            shadow(&mut u, Xrgb8888, full, s, 0, 1, 0, (0, 0), 255, false),
            0x44abcd31
        );
        // Corner size 5: computed and cached, then served from the cache.
        assert_eq!(
            shadow(&mut u, Xrgb8888, full, s, 2, 3, 1, (0, 0), 255, false),
            0xc5a73aee
        );
        assert_eq!((u.shadow_cache.size, u.shadow_cache.r), (5, 2));
        assert_eq!(
            shadow(&mut u, Xrgb8888, full, s, 2, 3, 1, (0, 0), 255, false),
            0xc5a73aee
        );
        assert_eq!(
            shadow(&mut u, Argb8888, full, c, 8, 12, 2, (3, 4), 255, false),
            0x4b0a0bc1
        );
        assert_eq!(
            shadow(&mut u, Rgb565, full, c, 32767, 25, 5, (0, 8), 255, false),
            0xd9531746
        );
        // Every circle taken by the radius masks was released.
        assert!(u.circle_cache.entries.iter().all(|e| e.used_cnt == 0));
    }

    #[cfg(feature = "cf-xrgb8888")]
    #[test]
    fn cache_only_keeps_small_corners() {
        let mut u = SwDrawUnit::new(0);
        let full = Area::new(0, 0, 200, 200);
        let c = Area::new(30, 25, 80, 60);
        // corner_size = 12 + 7: 361 bytes do not fit into 8 * 8.
        shadow(
            &mut u,
            ColorFormat::Xrgb8888,
            full,
            c,
            7,
            12,
            0,
            (0, 0),
            255,
            false,
        );
        assert_eq!((u.shadow_cache.size, u.shadow_cache.r), (-1, -1));
        assert!(u.shadow_cache.cache.is_empty());
        // corner_size = 4 + 3: 49 bytes fit.
        shadow(
            &mut u,
            ColorFormat::Xrgb8888,
            full,
            c,
            3,
            4,
            0,
            (0, 0),
            255,
            false,
        );
        assert_eq!((u.shadow_cache.size, u.shadow_cache.r), (7, 3));
        assert_eq!(u.shadow_cache.cache.len(), 49);
    }

    #[test]
    fn outside_of_clip_area_draws_nothing() {
        let (mut ctx, id) = make_ctx(ColorFormat::Xrgb8888, 120, 90, 5, 3);
        let before = layer_hash(&ctx, id);
        let t = task(id, Area::new(300, 300, 310, 310));
        let mut dsc = BoxShadowDsc::new();
        dsc.width = 10;
        dsc.opa = OPA_COVER;
        let mut u = SwDrawUnit::new(0);
        box_shadow(&mut u, &mut ctx, &t, &dsc, &Area::new(30, 25, 80, 60));
        assert_eq!(layer_hash(&ctx, id), before);
        assert_eq!(u.shadow_cache.size, -1);
    }

    /// `lv_init()`: `sw_shadow_cache.cache_size = -1; cache_r = -1;`
    #[test]
    fn fresh_cache_key_is_minus_one() {
        let c = ShadowCache::default();
        assert_eq!((c.size, c.r), (-1, -1));
        assert!(c.cache.is_empty());
        let u = SwDrawUnit::new(0);
        assert_eq!((u.shadow_cache.size, u.shadow_cache.r), (-1, -1));
    }
}
