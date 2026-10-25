//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_img.c`.
//!
//! Notes on the port:
//!
//! - `MAX_BUF_SIZE` is `4 * hor_res * color_format_size` of the refreshing
//!   display upstream. The draw context cannot see the displays, so the
//!   value of the parity configuration (800 px wide XRGB8888) is used. It
//!   only decides how many rows are transformed / recoloured per chunk;
//!   every row is computed independently, so the pixels do not depend on it.
//! - The `LV_DRAW_SW_IMAGE` / `LV_DRAW_SW_*_RECOLOR` acceleration hooks are
//!   not defined (`LV_USE_DRAW_SW_ASM == NONE`), i.e. they always return
//!   `LV_RESULT_INVALID` and the C fallbacks run.
//! - `LV_USE_LAYER_DEBUG` / `LV_USE_PARALLEL_DRAW_DEBUG` are off.
//! - `lv_draw_sw_blend(t, &blend_dsc)` blends into `t.target_layer` with the
//!   unit's custom blend handlers. The blend descriptors borrow the source
//!   and mask buffers, so they are rebuilt for every row / chunk where C
//!   keeps one descriptor and moves its pointers.

use alloc::vec;
use alloc::vec::Vec;

use ferrovgl_core::draw::image::{DecodedImage, DrawImageDsc, DrawImageSup, with_decoded_image};
use ferrovgl_core::draw::{DrawCtx, DrawTask, DrawTaskRef, DrawTaskState};
use ferrovgl_core::misc::area::{Align, Area};
use ferrovgl_core::misc::color::{Color, ColorFormat, OPA_MAX, OPA_MIN, opa_mix2};
use ferrovgl_core::misc::style::{ImageSrc, SCALE_NONE};

use crate::SwDrawUnit;
use crate::blend::{self, BlendDsc, MaskRes};
#[cfg(feature = "complex")]
use crate::mask::{self, MaskParam, MaskRadiusParam};

/// `MAX_BUF_SIZE`: `4 * lv_display_get_horizontal_resolution() *
/// lv_color_format_get_size(lv_display_get_color_format())` for the parity
/// display (800 x XRGB8888), see the module docs.
const MAX_BUF_SIZE: u32 = 4 * 800 * 4;

/// A zeroed `lv_draw_sw_blend_dsc_t` (`lv_memzero`).
fn blend_dsc_zeroed<'a>() -> BlendDsc<'a> {
    BlendDsc {
        blend_area: Area::default(),
        src_buf: None,
        src_stride: 0,
        src_color_format: ColorFormat::Unknown,
        src_area: None,
        opa: 0,
        color: Color::make(0, 0, 0),
        mask_buf: None,
        mask_res: MaskRes::Transp,
        mask_area: None,
        mask_stride: 0,
        blend_mode: Default::default(),
    }
}

/// `lv_draw_sw_blend(t, dsc)`
fn sw_blend(unit: &SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &BlendDsc<'_>) {
    if let Some(layer) = ctx.layer_mut(t.target_layer) {
        blend::blend(t, layer, dsc, &unit.blend_handlers);
    }
}

/// `buf + ofs` as a slice (`None` past the end).
#[inline]
fn tail(buf: &[u8], ofs: u32) -> Option<&[u8]> {
    buf.get(ofs as usize..)
}

/// The handle of the task being rendered: `unit.task_act` when it is set,
/// else the `IN_PROGRESS` task of the target layer matching `t`.
fn current_task_ref(unit: &SwDrawUnit, ctx: &DrawCtx, t: &DrawTask) -> Option<DrawTaskRef> {
    if let Some(r) = unit.task_act
        && ctx.task(r).is_some()
    {
        return Some(r);
    }
    let layer = ctx.layer(t.target_layer)?;
    let same = |x: &DrawTask| x.ty == t.ty && x.area == t.area && x.clip_area == t.clip_area;
    let index = layer
        .tasks
        .iter()
        .position(|x| x.state == DrawTaskState::InProgress && same(x))
        .or_else(|| layer.tasks.iter().position(same))?;
    Some(DrawTaskRef {
        layer: t.target_layer,
        index,
    })
}

/// `lv_draw_sw_layer()`: draw a finished sub-layer (`draw_dsc.src` is
/// [`ImageSrc::Layer`]) as an image, applying the bitmap mask first.
///
/// The source stays the layer: the decode helpers use its `draw_buf`,
/// which is what upstream's `new_draw_dsc.src = layer_to_draw->draw_buf`
/// does.
pub fn layer(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, draw_dsc: &DrawImageDsc, coords: &Area) {
    let ImageSrc::Layer(layer_to_draw) = draw_dsc.src else {
        return;
    };

    /*It can happen that nothing was draw on a layer and therefore its buffer is not allocated.
     *In this case just return. */
    if ctx
        .layer(layer_to_draw)
        .and_then(|l| l.draw_buf.as_ref())
        .is_none()
    {
        return;
    }

    if !draw_dsc.bitmap_mask_src.is_none() {
        let visible = apply_mask(ctx, draw_dsc);
        if !visible {
            return;
        }
    }

    /*The source should be a draw_buf, not a layer*/
    image(unit, ctx, t, draw_dsc, coords);
}

/// `lv_draw_sw_image()`
pub fn image(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &DrawImageDsc, coords: &Area) {
    let Some(t_ref) = current_task_ref(unit, ctx, t) else {
        return;
    };
    let mut core = |c: &mut DrawCtx,
                    _: DrawTaskRef,
                    d: &DrawImageDsc,
                    decoded: &DecodedImage<'_>,
                    sup: Option<&DrawImageSup>,
                    img_coords: &Area,
                    clipped_img_area: &Area| {
        img_draw_core(unit, c, t, d, decoded, sup, img_coords, clipped_img_area);
    };
    if !dsc.tile {
        ctx.image_normal_helper(t_ref, dsc, coords, &mut core);
    } else {
        ctx.image_tiled_helper(t_ref, dsc, coords, &mut core);
    }
}

/// `img_draw_core()`: blit / transform `decoded` (the `decoder_dsc`) into
/// the layer; the `lv_draw_image_core_cb` of this renderer.
#[allow(clippy::too_many_arguments)] // mirrors the C signature
pub fn img_draw_core(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    draw_dsc: &DrawImageDsc,
    decoded: &DecodedImage<'_>,
    sup: Option<&DrawImageSup>,
    img_coords: &Area,
    clipped_img_area: &Area,
) {
    let decoder_dsc = decoded;
    let transformed =
        draw_dsc.rotation != 0 || draw_dsc.scale_x != SCALE_NONE || draw_dsc.scale_y != SCALE_NONE;

    let radius = draw_dsc.clip_radius > 0;

    let decoded = decoder_dsc.decoded;
    let src_buf = decoded.data();
    let header = &decoded.header;
    let img_stride = u32::from(decoded.header.stride);
    let cf = decoded.header.cf;

    let mut blend_dsc = blend_dsc_zeroed();
    blend_dsc.opa = draw_dsc.opa;
    blend_dsc.blend_mode = draw_dsc.blend_mode;
    blend_dsc.src_stride = img_stride;

    if !transformed && !radius && cf == ColorFormat::A8 {
        if Area::intersect(img_coords, &t.clip_area).is_none() {
            return;
        }

        blend_dsc.mask_buf = Some(src_buf);
        blend_dsc.mask_area = Some(*img_coords);
        blend_dsc.mask_stride = img_stride as i32;
        blend_dsc.src_buf = None;
        blend_dsc.color = draw_dsc.recolor;
        blend_dsc.mask_res = MaskRes::Changed;

        blend_dsc.blend_area = *img_coords;
        sw_blend(unit, ctx, t, &blend_dsc);
    } else if !transformed
        && !radius
        && cf == ColorFormat::Rgb565A8
        && draw_dsc.recolor_opa <= OPA_MIN
        && draw_dsc.colorkey.is_none()
    {
        let src_h = img_coords.height();
        let src_w = img_coords.width();
        if header.w == 0 {
            return; /*Division by zero upstream*/
        }
        blend_dsc.src_area = Some(*img_coords);
        blend_dsc.src_buf = Some(src_buf);
        let mask_ofs = img_stride
            .wrapping_mul(src_w as u32)
            .wrapping_div(u32::from(header.w))
            .wrapping_mul(src_h as u32);
        let Some(mask_buf) = tail(src_buf, mask_ofs) else {
            return;
        };
        blend_dsc.mask_buf = Some(mask_buf);
        /*Note, for RGB565A8, lacking of stride parameter, we always use
         *always half of RGB map stride as alpha map stride. The image should
         *be generated in this way too.*/
        blend_dsc.mask_stride = (img_stride / 2) as i32;
        blend_dsc.blend_area = *img_coords;
        blend_dsc.mask_area = Some(*img_coords);
        blend_dsc.mask_res = MaskRes::Changed;
        blend_dsc.src_color_format = ColorFormat::Rgb565;
        sw_blend(unit, ctx, t, &blend_dsc);
    }
    /*Two upstream branches with identical bodies, merged (same order, same effect):
     *`!transformed && !radius && (L8 || AL88) && colorkey == NULL` and
     *the simplest case: just copy the pixels into the draw_buf. Blending will convert the colors if needed*/
    else if !transformed
        && !radius
        && ((cf == ColorFormat::L8 || cf == ColorFormat::Al88) || draw_dsc.recolor_opa <= OPA_MIN)
        && draw_dsc.colorkey.is_none()
    {
        blend_dsc.src_area = Some(*img_coords);
        blend_dsc.src_buf = Some(src_buf);
        blend_dsc.blend_area = *img_coords;
        blend_dsc.src_color_format = cf;
        sw_blend(unit, ctx, t, &blend_dsc);
    } else if !transformed && !radius && draw_dsc.recolor_opa > OPA_MIN && draw_dsc.colorkey.is_none() {
        recolor_only(unit, ctx, t, draw_dsc, decoder_dsc, img_coords, clipped_img_area);
    }
    /*Handle masked RGB565, RGB888, XRGB888, or ARGB8888 images*/
    else if cfg!(feature = "complex")
        && !transformed
        && radius
        && draw_dsc.recolor_opa <= OPA_MIN
        && draw_dsc.colorkey.is_none()
    {
        #[cfg(feature = "complex")]
        radius_only(unit, ctx, t, draw_dsc, decoder_dsc, img_coords, clipped_img_area);
    }
    /* check whether it is possible to accelerate the operation in synchronous mode:
     * LV_DRAW_SW_IMAGE(...) is LV_RESULT_INVALID without an ASM backend */
    else {
        /*In the other cases every pixel need to be checked one-by-one*/
        transform_and_recolor(
            unit,
            ctx,
            t,
            draw_dsc,
            decoder_dsc,
            sup,
            img_coords,
            clipped_img_area,
        );
    }
}

/// `radius_only()`
#[cfg(feature = "complex")]
fn radius_only(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    draw_dsc: &DrawImageDsc,
    decoder_dsc: &DecodedImage<'_>,
    img_coords: &Area,
    clipped_img_area: &Area,
) {
    let decoded = decoder_dsc.decoded;
    let img_stride = u32::from(decoded.header.stride);
    let mut cf = decoded.header.cf;
    let cf_ori = cf;

    if cf == ColorFormat::Rgb565A8 {
        cf = ColorFormat::Rgb565;
    }

    let data = decoded.data();

    let mut blend_area = *clipped_img_area;
    let y_last = blend_area.y2;
    blend_area.y2 = blend_area.y1;

    let blend_w = blend_area.width();
    let mut mask_buf: Vec<u8> = vec![0; blend_w.max(0) as usize];

    let is_a8 = cf == ColorFormat::A8;

    let mut mask_param = MaskRadiusParam::default();
    let _ = mask::radius_init(
        &mut unit.circle_cache,
        &mut mask_param,
        &draw_dsc.image_area,
        draw_dsc.clip_radius,
        false,
    );

    let mut masks = [MaskParam::Radius(mask_param)];

    let image_h = img_coords.height();
    while blend_area.y1 <= y_last {
        let y_ofs = blend_area.y1 - img_coords.y1;
        let x_ofs = blend_area.x1 - img_coords.x1;
        if cf_ori == ColorFormat::Rgb565A8 {
            let mask_start = img_stride.wrapping_mul(image_h as u32);
            let ofs = mask_start
                .wrapping_add((y_ofs as u32).wrapping_mul(img_stride) / 2)
                .wrapping_add(x_ofs as u32);
            copy_into(&mut mask_buf, data, ofs);
        } else if cf_ori == ColorFormat::A8 {
            let ofs = (y_ofs as u32).wrapping_mul(img_stride).wrapping_add(x_ofs as u32);
            copy_into(&mut mask_buf, data, ofs);
        } else {
            mask_buf.fill(0xff);
        }

        let mut mask_res = mask::apply(
            &mut unit.circle_cache,
            &mut masks,
            &mut mask_buf,
            blend_area.x1,
            blend_area.y1,
            blend_w,
        );

        if cf_ori == ColorFormat::Rgb565A8 || cf_ori == ColorFormat::A8 {
            mask_res = MaskRes::Changed;
        }

        /*Blend*/
        let mut blend_dsc = blend_dsc_zeroed();
        blend_dsc.opa = draw_dsc.opa;
        blend_dsc.blend_mode = draw_dsc.blend_mode;
        blend_dsc.src_stride = img_stride;
        blend_dsc.src_area = Some(*img_coords);
        blend_dsc.src_buf = if is_a8 { None } else { Some(data) };
        blend_dsc.src_color_format = cf;
        if is_a8 {
            blend_dsc.color = draw_dsc.recolor;
        }
        blend_dsc.blend_area = blend_area;
        blend_dsc.mask_buf = Some(&mask_buf);
        blend_dsc.mask_area = Some(blend_area);
        blend_dsc.mask_stride = blend_w;
        blend_dsc.mask_res = mask_res;
        sw_blend(unit, ctx, t, &blend_dsc);

        /*Go to the next area*/
        blend_area.y1 += 1;
        blend_area.y2 += 1;
    }
    mask::free_param(&mut unit.circle_cache, &mut masks[0]);
}

/// `lv_memcpy(dst, src + ofs, dst.len())`, clamped to `src`.
#[cfg(feature = "complex")]
fn copy_into(dst: &mut [u8], src: &[u8], ofs: u32) {
    let s = tail(src, ofs).unwrap_or(&[]);
    let n = dst.len().min(s.len());
    dst[..n].copy_from_slice(&s[..n]);
}

/// `recolor_only()`
fn recolor_only(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    draw_dsc: &DrawImageDsc,
    decoder_dsc: &DecodedImage<'_>,
    img_coords: &Area,
    clipped_img_area: &Area,
) {
    let mut blend_area = *clipped_img_area;

    let decoded = decoder_dsc.decoded;
    let img_stride = u32::from(decoded.header.stride);
    let cf = decoded.header.cf;
    let px_size = u32::from(cf.size());
    let src_h = img_coords.height();
    let blend_w = blend_area.width();
    let blend_h = blend_area.height();
    let mut buf_stride = (blend_w as u32).wrapping_mul(px_size);
    if buf_stride == 0 {
        buf_stride = 1;
    }
    let buf_h = ((MAX_BUF_SIZE / buf_stride) as i32).max(1).min(blend_h);
    let mut tmp_buf: Vec<u8> = vec![0; buf_stride as usize * buf_h.max(0) as usize];

    let data = decoded.data();
    let mut src_color_format = cf;
    let mut mask: Option<(&[u8], Area, i32)> = None;
    if cf == ColorFormat::Rgb565A8 {
        let Some(m) = tail(data, img_stride.wrapping_mul(src_h as u32)) else {
            return;
        };
        mask = Some((m, *img_coords, (img_stride / 2) as i32));
        src_color_format = ColorFormat::Rgb565;
    }

    let y_last = blend_area.y2;
    blend_area.y2 = blend_area.y1 + buf_h - 1;
    while blend_area.y1 <= y_last {
        let mut relative_area = blend_area;
        relative_area.move_by(-img_coords.x1, -img_coords.y1);

        recolor(
            relative_area,
            Some(data),
            &mut tmp_buf,
            img_stride as i32,
            src_color_format,
            draw_dsc,
        );

        let mut blend_dsc = blend_dsc_zeroed();
        blend_dsc.opa = draw_dsc.opa;
        blend_dsc.blend_mode = draw_dsc.blend_mode;
        blend_dsc.src_stride = (blend_w as u32).wrapping_mul(px_size);
        blend_dsc.src_area = Some(blend_area);
        blend_dsc.blend_area = blend_area;
        blend_dsc.src_buf = Some(&tmp_buf);
        blend_dsc.src_color_format = src_color_format;
        if let Some((m, a, stride)) = mask {
            blend_dsc.mask_area = Some(a);
            blend_dsc.mask_buf = Some(m);
            blend_dsc.mask_res = MaskRes::Changed;
            blend_dsc.mask_stride = stride;
        }
        sw_blend(unit, ctx, t, &blend_dsc);

        /*Go to the next area*/
        blend_area.y1 = blend_area.y2 + 1;
        blend_area.y2 = blend_area.y1 + buf_h - 1;
        if blend_area.y2 > y_last {
            blend_area.y2 = y_last;
        }
    }
}

/// `transform_and_recolor()`
#[allow(clippy::too_many_arguments)] // mirrors the C signature
fn transform_and_recolor(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    draw_dsc: &DrawImageDsc,
    decoder_dsc: &DecodedImage<'_>,
    sup: Option<&DrawImageSup>,
    img_coords: &Area,
    clipped_img_area: &Area,
) {
    let decoded = decoder_dsc.decoded;
    let img_stride = u32::from(decoded.header.stride);
    let cf = decoded.header.cf;

    let mut blend_area = *clipped_img_area;

    let src_w = img_coords.width();
    let src_h = img_coords.height();
    let blend_w = blend_area.width();
    let blend_h = blend_area.height();

    let mut color = Color::make(0, 0, 0);
    let mut do_recolor = draw_dsc.recolor_opa > OPA_MIN;
    if cf == ColorFormat::L8 || cf == ColorFormat::A8 {
        color = draw_dsc.recolor;
        do_recolor = false;
    }

    let has_colorkey = draw_dsc.colorkey.is_some();

    let mut cf_final = cf;
    /*RGB888/XRGB8888 gain an alpha channel: the transform fades the edge pixels. That alpha
     *is straight, so these must not be flagged premultiplied.*/
    if cf_final == ColorFormat::Rgb888 || cf_final == ColorFormat::Xrgb8888 {
        cf_final = ColorFormat::Argb8888;
    } else if cf_final == ColorFormat::Rgb565 || cf_final == ColorFormat::Rgb565Swapped {
        cf_final = ColorFormat::Rgb565A8;
    } else if cf_final == ColorFormat::L8 {
        cf_final = ColorFormat::Al88;
    }
    /*Antialiasing an image that has an alpha channel is only correct in premultiplied space,
     *so that is what the filter works in and what it writes out. Without the antialiasing the
     *pixels are copied as they are and stay straight.*/
    else if cfg!(feature = "cf-argb8888-premultiplied")
        && cf_final == ColorFormat::Argb8888
        && draw_dsc.antialias
    {
        cf_final = ColorFormat::Argb8888Premultiplied;
    }

    let (buf_h, buf_len): (i32, usize) = if cf_final == ColorFormat::Rgb565A8 {
        let buf_stride = (blend_w as u32).wrapping_mul(3);
        if buf_stride == 0 {
            return; /*Division by zero upstream*/
        }
        let buf_h = ((MAX_BUF_SIZE / buf_stride) as i32).max(1).min(blend_h);
        (buf_h, buf_stride as usize * buf_h.max(0) as usize)
    } else if cf_final == ColorFormat::Al88 {
        let buf_stride = blend_w as u32;
        if buf_stride == 0 {
            return; /*Division by zero upstream*/
        }
        let buf_h = ((MAX_BUF_SIZE / buf_stride.wrapping_mul(2)) as i32)
            .max(1)
            .min(blend_h);
        (buf_h, buf_stride as usize * buf_h.max(0) as usize * 2)
    } else {
        let buf_stride = (blend_w as u32).wrapping_mul(u32::from(cf_final.size()));
        if buf_stride == 0 {
            return; /*Division by zero upstream*/
        }
        let buf_h = ((MAX_BUF_SIZE / buf_stride) as i32).max(1).min(blend_h);
        (buf_h, buf_stride as usize * buf_h.max(0) as usize)
    };
    let mut transformed_buf: Vec<u8> = vec![0; buf_len];

    let mut src_color_format = cf_final;
    let y_last = blend_area.y2;
    blend_area.y2 = blend_area.y1 + buf_h - 1;

    let src_buf = decoded.data();
    let src_stride: u32;
    let mut mask_ofs: Option<usize> = None;
    let mut mask_stride = 0;
    let mut src_is_null = false;
    if cf_final == ColorFormat::Rgb565A8 {
        /*RGB565A8 images will blended as RGB565 + mask
         *Therefore the stride can be different. */
        src_stride = (blend_w as u32).wrapping_mul(2);
        mask_ofs = Some(blend_w as usize * 2 * buf_h as usize);
        mask_stride = blend_w;
        src_color_format = ColorFormat::Rgb565;
    } else if cf_final == ColorFormat::Al88 {
        /*AL88 images will be blended as L8 + mask*/
        src_stride = blend_w as u32;
        mask_ofs = Some(blend_w as usize * buf_h as usize);
        mask_stride = blend_w;
        src_color_format = ColorFormat::L8;
    } else if cf_final == ColorFormat::A8 {
        src_stride = 0;
        mask_ofs = Some(0);
        mask_stride = blend_w;
        src_is_null = true;
    } else {
        src_stride = (blend_w as u32).wrapping_mul(u32::from(cf_final.size()));
    }

    while blend_area.y1 <= y_last {
        /*Apply transformations if any or separate the channels*/
        let mut relative_area = blend_area;
        relative_area.move_by(-img_coords.x1, -img_coords.y1);
        crate::transform::transform(
            &relative_area,
            src_buf,
            src_w,
            src_h,
            img_stride as i32,
            draw_dsc,
            sup,
            cf,
            &mut transformed_buf,
        );

        if do_recolor || has_colorkey {
            let mut relative_area2 = blend_area;
            relative_area2.move_by(-blend_area.x1, -blend_area.y1);
            if has_colorkey && cf_final != ColorFormat::Rgb565Swapped {
                colorkey_and_recolor(
                    relative_area2,
                    None,
                    &mut transformed_buf,
                    src_stride as i32,
                    cf_final,
                    draw_dsc,
                    mask_ofs,
                    mask_stride,
                );
            } else {
                recolor(
                    relative_area2,
                    None,
                    &mut transformed_buf,
                    src_stride as i32,
                    cf_final,
                    draw_dsc,
                );
            }
        }

        /*Blend*/
        let mut blend_dsc = blend_dsc_zeroed();
        blend_dsc.opa = draw_dsc.opa;
        blend_dsc.blend_mode = draw_dsc.blend_mode;
        blend_dsc.color = color;
        blend_dsc.blend_area = blend_area;
        blend_dsc.src_area = Some(blend_area);
        blend_dsc.src_stride = src_stride;
        blend_dsc.src_color_format = src_color_format;
        blend_dsc.src_buf = if src_is_null { None } else { Some(&transformed_buf) };
        if let Some(ofs) = mask_ofs {
            blend_dsc.mask_area = Some(blend_area);
            blend_dsc.mask_res = MaskRes::Changed;
            blend_dsc.mask_buf = Some(transformed_buf.get(ofs..).unwrap_or(&[]));
            blend_dsc.mask_stride = mask_stride;
        }
        sw_blend(unit, ctx, t, &blend_dsc);

        /*Go to the next area*/
        blend_area.y1 = blend_area.y2 + 1;
        blend_area.y2 = blend_area.y1 + buf_h - 1;
        if blend_area.y2 > y_last {
            blend_area.y2 = y_last;
            if cf_final == ColorFormat::Rgb565A8 {
                mask_ofs = Some(blend_w as usize * 2 * blend_area.height().max(0) as usize);
            } else if cf_final == ColorFormat::Al88 {
                mask_ofs = Some(blend_w as usize * blend_area.height().max(0) as usize);
            }
        }
    }
}

/*
 * Pixel access helpers for `recolor` / `colorkey_and_recolor`: `src` is
 * `None` when the source is the destination itself (`src_buf == dest_buf`
 * upstream). Out-of-range reads give 0 and writes are dropped.
 */

#[inline]
fn rd8(src: Option<&[u8]>, dest: &[u8], i: i64) -> u8 {
    let Ok(i) = usize::try_from(i) else {
        return 0;
    };
    match src {
        Some(s) => s.get(i),
        None => dest.get(i),
    }
    .copied()
    .unwrap_or(0)
}

#[inline]
fn rd16(src: Option<&[u8]>, dest: &[u8], i: i64) -> u16 {
    u16::from_le_bytes([rd8(src, dest, i), rd8(src, dest, i + 1)])
}

#[inline]
fn wr8(dest: &mut [u8], i: i64, v: u8) {
    if let Some(d) = usize::try_from(i).ok().and_then(|i| dest.get_mut(i)) {
        *d = v;
    }
}

#[inline]
fn wr16(dest: &mut [u8], i: i64, v: u16) {
    let b = v.to_le_bytes();
    wr8(dest, i, b[0]);
    wr8(dest, i + 1, b[1]);
}

/// `lv_color_swap_16()`
#[inline]
const fn color_swap_16(c: u16) -> u16 {
    c.rotate_left(8)
}

/// `lv_color16_to_color()`
#[inline]
const fn color16_to_color(c: u16) -> Color {
    let red = (c >> 11) as u8;
    let green = ((c >> 5) & 0x3F) as u8;
    let blue = (c & 0x1F) as u8;
    Color::make(red << 3, green << 2, blue << 3)
}

/// `lv_color_is_in_range()`
#[inline]
const fn color_is_in_range(color: Color, l_color: Color, h_color: Color) -> bool {
    color.red <= h_color.red
        && color.green <= h_color.green
        && color.blue <= h_color.blue
        && color.red >= l_color.red
        && color.green >= l_color.green
        && color.blue >= l_color.blue
}

/// `colorkey_and_recolor()`: `src_buf == None` means in place. The mask
/// written for keyed RGB565A8 pixels is `blend_dsc->mask_buf`, given here as
/// `mask_ofs` into `dest_buf` (it is the A8 plane of the transform buffer).
#[allow(clippy::too_many_arguments)] // mirrors the C signature
fn colorkey_and_recolor(
    relative_area: Area,
    src_buf: Option<&[u8]>,
    dest_buf: &mut [u8],
    src_stride: i32,
    cf: ColorFormat,
    draw_dsc: &DrawImageDsc,
    mask_ofs: Option<usize>,
    mask_stride: i32,
) {
    let w = relative_area.width();
    let h = relative_area.height();

    /* Recolor parameters */
    let recolor = draw_dsc.recolor;
    let mix = draw_dsc.recolor_opa;
    let mix_inv = 255 - mix;
    let mix32 = u32::from(mix);
    let mix_inv32 = u32::from(mix_inv);

    /* Colorkey parameters */
    let Some(colorkey) = draw_dsc.colorkey else {
        return;
    };
    let colorkey_low = colorkey.low;
    let colorkey_high = colorkey.high;
    if cf == ColorFormat::Rgb565A8 || cf == ColorFormat::Rgb565 {
        let mut src_row =
            i64::from(src_stride) * i64::from(relative_area.y1) + i64::from(relative_area.x1) * 2;
        let img_stride_px = src_stride / 2;

        let mut dest = 0i64;
        let recolor16 = recolor.to_u16();

        let c_mult = [
            u32::from(recolor.blue >> 3) * mix32,
            u32::from(recolor.green >> 2) * mix32,
            u32::from(recolor.red >> 3) * mix32,
        ];
        let mut mask_row = mask_ofs.map(|m| m as i64);
        for _y in 0..h {
            for x in 0..i64::from(w) {
                let src16 = rd16(src_buf, dest_buf, src_row + x * 2);
                let src_color = color16_to_color(src16);

                /* Check colorkey */
                if color_is_in_range(src_color, colorkey_low, colorkey_high) {
                    if let (ColorFormat::Rgb565A8, Some(m)) = (cf, mask_row) {
                        wr8(dest_buf, m + x, 0);
                    } else {
                        wr16(dest_buf, dest * 2, 0x0000); // Transparent
                    }
                }
                /* Apply recolor */
                else if mix >= OPA_MAX {
                    wr16(dest_buf, dest * 2, recolor16);
                } else if mix > OPA_MIN {
                    let s = u32::from(src16);
                    let v = (((c_mult[2] + ((s >> 11) & 0x1F) * mix_inv32) << 3) & 0xF800)
                        + (((c_mult[1] + ((s >> 5) & 0x3F) * mix_inv32) >> 3) & 0x07E0)
                        + ((c_mult[0] + (s & 0x1F) * mix_inv32) >> 8);
                    wr16(dest_buf, dest * 2, v as u16);
                } else {
                    wr16(dest_buf, dest * 2, src16); // No change
                }
                dest += 1;
            }
            src_row += i64::from(img_stride_px) * 2;
            mask_row = mask_row.map(|m| m + i64::from(mask_stride));
        }
    } else if cf == ColorFormat::Rgb888 || cf == ColorFormat::Xrgb8888 || cf == ColorFormat::Argb8888 {
        let px_size = i64::from(cf.size());
        let mut s =
            i64::from(src_stride) * i64::from(relative_area.y1) + i64::from(relative_area.x1) * px_size;
        let mut d = 0i64;

        let c_mult = [
            u32::from(recolor.blue) * mix32,
            u32::from(recolor.green) * mix32,
            u32::from(recolor.red) * mix32,
        ];

        for _y in 0..h {
            for _x in 0..w {
                let s0 = rd8(src_buf, dest_buf, s);
                let s1 = rd8(src_buf, dest_buf, s + 1);
                let s2 = rd8(src_buf, dest_buf, s + 2);
                let s3 = rd8(src_buf, dest_buf, s + 3);
                let src_color = Color {
                    blue: s0,
                    green: s1,
                    red: s2,
                };

                let alpha = if color_is_in_range(src_color, colorkey_low, colorkey_high) {
                    wr8(dest_buf, d, 0);
                    wr8(dest_buf, d + 1, 0);
                    wr8(dest_buf, d + 2, 0);
                    0
                } else if mix >= OPA_MAX {
                    wr8(dest_buf, d, recolor.blue);
                    wr8(dest_buf, d + 1, recolor.green);
                    wr8(dest_buf, d + 2, recolor.red);
                    s3
                } else if mix > OPA_MIN {
                    wr8(dest_buf, d, ((c_mult[0] + u32::from(s0) * mix_inv32) >> 8) as u8);
                    wr8(
                        dest_buf,
                        d + 1,
                        ((c_mult[1] + u32::from(s1) * mix_inv32) >> 8) as u8,
                    );
                    wr8(
                        dest_buf,
                        d + 2,
                        ((c_mult[2] + u32::from(s2) * mix_inv32) >> 8) as u8,
                    );
                    s3
                } else {
                    wr8(dest_buf, d, s0);
                    wr8(dest_buf, d + 1, s1);
                    wr8(dest_buf, d + 2, s2);
                    s3
                };
                if cf == ColorFormat::Argb8888 {
                    wr8(dest_buf, d + 3, alpha);
                } else if cf == ColorFormat::Xrgb8888 {
                    wr8(dest_buf, d + 3, 0xff);
                }

                s += px_size;
                d += px_size;
            }
            s += i64::from(src_stride) - i64::from(w) * px_size;
        }
    } else if cf == ColorFormat::Argb8888Premultiplied {
        let px_size = 4i64;
        let mut s =
            i64::from(src_stride) * i64::from(relative_area.y1) + i64::from(relative_area.x1) * px_size;
        let mut d = 0i64;

        let c_mult = [
            u32::from(recolor.blue) * mix32,
            u32::from(recolor.green) * mix32,
            u32::from(recolor.red) * mix32,
        ];

        for _y in 0..h {
            for _x in 0..w {
                let alpha = rd8(src_buf, dest_buf, s + 3);
                let s0 = rd8(src_buf, dest_buf, s);
                let s1 = rd8(src_buf, dest_buf, s + 1);
                let s2 = rd8(src_buf, dest_buf, s + 2);

                // Only process if pixel is visible
                if alpha > 0 {
                    /* Unpremultiply for color comparison and recolor */
                    let reciprocal = ((255 * 256) / u32::from(alpha)) as u16;
                    let mut r = ((u32::from(s2) * u32::from(reciprocal)) >> 8) as u8;
                    let mut g = ((u32::from(s1) * u32::from(reciprocal)) >> 8) as u8;
                    let mut b = ((u32::from(s0) * u32::from(reciprocal)) >> 8) as u8;

                    let src_color = Color::make(r, g, b);

                    /* Check colorkey */
                    if color_is_in_range(src_color, colorkey_low, colorkey_high) {
                        // Set entire pixel to transparent
                        wr8(dest_buf, d, 0);
                        wr8(dest_buf, d + 1, 0);
                        wr8(dest_buf, d + 2, 0);
                        wr8(dest_buf, d + 3, 0);
                    }
                    /* Apply recolor */
                    else {
                        if mix >= OPA_MAX {
                            r = recolor.red;
                            g = recolor.green;
                            b = recolor.blue;
                        } else if mix > OPA_MIN {
                            r = ((c_mult[2] + u32::from(r) * mix_inv32) >> 8) as u8;
                            g = ((c_mult[1] + u32::from(g) * mix_inv32) >> 8) as u8;
                            b = ((c_mult[0] + u32::from(b) * mix_inv32) >> 8) as u8;
                        }
                        // Else keep original colors

                        /* Premultiply again */
                        wr8(dest_buf, d, ((u32::from(b) * u32::from(alpha)) >> 8) as u8);
                        wr8(dest_buf, d + 1, ((u32::from(g) * u32::from(alpha)) >> 8) as u8);
                        wr8(dest_buf, d + 2, ((u32::from(r) * u32::from(alpha)) >> 8) as u8);
                        wr8(dest_buf, d + 3, alpha); // Keep original alpha
                    }
                } else {
                    // Already transparent, copy as-is
                    wr8(dest_buf, d, s0);
                    wr8(dest_buf, d + 1, s1);
                    wr8(dest_buf, d + 2, s2);
                    wr8(dest_buf, d + 3, 0); // Ensure alpha is 0
                }

                s += px_size;
                d += px_size;
            }
            s += i64::from(src_stride) - i64::from(w) * px_size;
        }
    }
}

/// `recolor()`: `src_buf == None` means in place (`src_buf == dest_buf`).
/// The destination is packed (`w * px_size` bytes per row).
fn recolor(
    relative_area: Area,
    src_buf: Option<&[u8]>,
    dest_buf: &mut [u8],
    src_stride: i32,
    cf: ColorFormat,
    draw_dsc: &DrawImageDsc,
) {
    let w = relative_area.width();
    let h = relative_area.height();

    /*Apply recolor*/
    let color = draw_dsc.recolor;
    let mix = draw_dsc.recolor_opa;
    let mix_inv = 255 - mix;
    let mix32 = u32::from(mix);
    let mix_inv32 = u32::from(mix_inv);

    if cf == ColorFormat::Rgb565A8 || cf == ColorFormat::Rgb565 || cf == ColorFormat::Rgb565Swapped {
        /*LV_DRAW_SW_RGB565_RECOLOR / LV_DRAW_SW_RGB565_SWAPPED_RECOLOR: LV_RESULT_INVALID*/
        let swapped = cf == ColorFormat::Rgb565Swapped;
        let mut src_row =
            i64::from(src_stride) * i64::from(relative_area.y1) + i64::from(relative_area.x1) * 2;
        let img_stride_px = src_stride / 2;

        let mut dest = 0i64;
        let color16 = color.to_u16();
        if mix >= OPA_MAX {
            for _y in 0..h {
                for _x in 0..w {
                    wr16(dest_buf, dest * 2, color16);
                    dest += 1;
                }
                /*buf16_src += img_stride_px: never read in this branch*/
            }
        } else {
            let c_mult = [
                u32::from(color.blue >> 3) * mix32,
                u32::from(color.green >> 2) * mix32,
                u32::from(color.red >> 3) * mix32,
            ];

            for _y in 0..h {
                for x in 0..i64::from(w) {
                    let raw = rd16(src_buf, dest_buf, src_row + x * 2);
                    let s = u32::from(if swapped { color_swap_16(raw) } else { raw });
                    let v = ((((c_mult[2] + ((s >> 11) & 0x1F) * mix_inv32) << 3) & 0xF800)
                        + (((c_mult[1] + ((s >> 5) & 0x3F) * mix_inv32) >> 3) & 0x07E0)
                        + ((c_mult[0] + (s & 0x1F) * mix_inv32) >> 8)) as u16;
                    wr16(dest_buf, dest * 2, if swapped { color_swap_16(v) } else { v });
                    dest += 1;
                }
                src_row += i64::from(img_stride_px) * 2;
            }
        }
    } else if cf == ColorFormat::Rgb888 || cf == ColorFormat::Xrgb8888 || cf == ColorFormat::Argb8888 {
        /*LV_DRAW_SW_RGB888_RECOLOR: LV_RESULT_INVALID*/
        let px_size = i64::from(cf.size());
        let mut s =
            i64::from(src_stride) * i64::from(relative_area.y1) + i64::from(relative_area.x1) * px_size;
        let mut d = 0i64;
        if mix >= OPA_MAX {
            for _y in 0..h {
                for _x in 0..w {
                    let s3 = rd8(src_buf, dest_buf, s + 3);
                    wr8(dest_buf, d, color.blue);
                    wr8(dest_buf, d + 1, color.green);
                    wr8(dest_buf, d + 2, color.red);
                    if cf == ColorFormat::Argb8888 {
                        wr8(dest_buf, d + 3, s3);
                    } else if cf == ColorFormat::Xrgb8888 {
                        wr8(dest_buf, d + 3, 0xff);
                    }
                    s += px_size;
                    d += px_size;
                }
                s += i64::from(src_stride) - i64::from(w) * px_size;
            }
        } else {
            let c_mult = [
                u32::from(color.blue) * mix32,
                u32::from(color.green) * mix32,
                u32::from(color.red) * mix32,
            ];
            for _y in 0..h {
                for _x in 0..w {
                    let s0 = rd8(src_buf, dest_buf, s);
                    let s1 = rd8(src_buf, dest_buf, s + 1);
                    let s2 = rd8(src_buf, dest_buf, s + 2);
                    let s3 = rd8(src_buf, dest_buf, s + 3);
                    wr8(dest_buf, d, ((c_mult[0] + u32::from(s0) * mix_inv32) >> 8) as u8);
                    wr8(
                        dest_buf,
                        d + 1,
                        ((c_mult[1] + u32::from(s1) * mix_inv32) >> 8) as u8,
                    );
                    wr8(
                        dest_buf,
                        d + 2,
                        ((c_mult[2] + u32::from(s2) * mix_inv32) >> 8) as u8,
                    );
                    if cf == ColorFormat::Argb8888 {
                        wr8(dest_buf, d + 3, s3);
                    } else if cf == ColorFormat::Xrgb8888 {
                        wr8(dest_buf, d + 3, 0xff);
                    }
                    s += px_size;
                    d += px_size;
                }
                s += i64::from(src_stride) - i64::from(w) * px_size;
            }
        }
    } else if cf == ColorFormat::Argb8888Premultiplied {
        /*LV_DRAW_SW_ARGB8888_PREMULTIPLIED_RECOLOR: LV_RESULT_INVALID*/
        let px_size = i64::from(cf.size());
        let mut s =
            i64::from(src_stride) * i64::from(relative_area.y1) + i64::from(relative_area.x1) * px_size;
        let mut d = 0i64;

        let c_mult = [
            u32::from(color.blue) * mix32,
            u32::from(color.green) * mix32,
            u32::from(color.red) * mix32,
        ];

        for _y in 0..h {
            for _x in 0..w {
                let alpha = rd8(src_buf, dest_buf, s + 3);
                let s0 = rd8(src_buf, dest_buf, s);
                let s1 = rd8(src_buf, dest_buf, s + 1);
                let s2 = rd8(src_buf, dest_buf, s + 2);

                if alpha > 0 {
                    /* Step 1: Unpremultiply (convert to non-premultiplied RGB) */
                    let reciprocal = ((255 * 256) / u32::from(alpha)) as u16;
                    let mut r = ((u32::from(s2) * u32::from(reciprocal)) >> 8) as u8;
                    let mut g = ((u32::from(s1) * u32::from(reciprocal)) >> 8) as u8;
                    let mut b = ((u32::from(s0) * u32::from(reciprocal)) >> 8) as u8;

                    /* Step 2: Apply recoloring */
                    r = ((c_mult[2] + u32::from(r) * mix_inv32) >> 8) as u8;
                    g = ((c_mult[1] + u32::from(g) * mix_inv32) >> 8) as u8;
                    b = ((c_mult[0] + u32::from(b) * mix_inv32) >> 8) as u8;

                    /* Step 3: Premultiply again */
                    wr8(dest_buf, d, ((u32::from(b) * u32::from(alpha)) >> 8) as u8);
                    wr8(dest_buf, d + 1, ((u32::from(g) * u32::from(alpha)) >> 8) as u8);
                    wr8(dest_buf, d + 2, ((u32::from(r) * u32::from(alpha)) >> 8) as u8);
                } else {
                    /* If alpha is 0, just copy the pixel as is */
                    wr8(dest_buf, d, s0);
                    wr8(dest_buf, d + 1, s1);
                    wr8(dest_buf, d + 2, s2);
                }

                wr8(dest_buf, d + 3, alpha); /* Keep original alpha*/

                s += px_size;
                d += px_size;
            }
            s += i64::from(src_stride) - i64::from(w) * px_size;
        }
    }
}

/// `apply_mask()`: multiply the alpha of the layer to draw with the A8 / L8
/// `bitmap_mask_src` aligned to the centre of `image_area`. Returns `false`
/// when nothing remains visible.
fn apply_mask(ctx: &mut DrawCtx, draw_dsc: &DrawImageDsc) -> bool {
    let ImageSrc::Layer(layer_to_draw) = draw_dsc.src else {
        return true;
    };

    with_decoded_image(ctx, &draw_dsc.bitmap_mask_src, |ctx, mask_decoder_dsc| {
        let Some(mask_decoder_dsc) = mask_decoder_dsc else {
            /*"Could open the mask. The mask is not applied."*/
            return true;
        };
        let mask_draw_buf = mask_decoder_dsc.decoded;

        if mask_draw_buf.header.cf != ColorFormat::A8 && mask_draw_buf.header.cf != ColorFormat::L8 {
            /*"The mask image is not A8/L8 format. The mask is not applied."*/
            return true;
        }

        let mask_stride = mask_draw_buf.header.stride as usize;

        /*Align the mask to the center*/
        let image_area = draw_dsc.image_area; /*Use the whole image area for the alignment*/
        let mut mask_area = Area::default();
        mask_area.set(
            0,
            0,
            i32::from(mask_draw_buf.header.w) - 1,
            i32::from(mask_draw_buf.header.h) - 1,
        );
        Area::align(&image_area, &mut mask_area, Align::Center, 0, 0);

        let Some(layer) = ctx.layer_mut(layer_to_draw) else {
            return true;
        };
        /*The image can be smaller if only a part was rendered. Use this are during rendering*/
        let image_area = layer.buf_area;

        /*Only the intersection of the mask and image needs to be rendered
         *If if there is no intersection there is nothing to render as the image is out of the mask.*/
        let Some(masked_area) = Area::intersect(&mask_area, &image_area) else {
            return false;
        };

        let Some(image_draw_buf) = layer.draw_buf.as_mut() else {
            return true;
        };

        /*Clear the sides if any*/
        /*Top*/
        let mut side_area = Area::new(
            0,
            0,
            i32::from(image_draw_buf.header.w) - 1,
            masked_area.y1 - 1 - image_area.y1,
        );
        image_draw_buf.clear(Some(&side_area));

        /*Bottom*/
        side_area.y1 = masked_area.y2 + 1 - image_area.y1;
        side_area.y2 = i32::from(image_draw_buf.header.h) - 1;
        image_draw_buf.clear(Some(&side_area));

        /*Left*/
        side_area.y1 = 0;
        side_area.x1 = 0;
        side_area.x2 = masked_area.x1 - 1 - image_area.x1;
        image_draw_buf.clear(Some(&side_area));

        /*Right*/
        side_area.x1 = masked_area.x2 + 1 - image_area.x1;
        side_area.x2 = i32::from(image_draw_buf.header.w) - 1;
        image_draw_buf.clear(Some(&side_area));

        /*Seek to the first of the image and mask on the masked area*/
        let mut img_start = image_draw_buf.goto_xy(
            (masked_area.x1 - image_area.x1) as u32,
            (masked_area.y1 - image_area.y1) as u32,
        );
        let mut mask_start = mask_draw_buf.goto_xy(
            (masked_area.x1 - mask_area.x1) as u32,
            (masked_area.y1 - mask_area.y1) as u32,
        );

        let h = masked_area.height();
        let w = masked_area.width() as usize;
        let img_stride = image_draw_buf.header.stride as usize;
        let mask_data = mask_draw_buf.data();
        let Some(img_data) = image_draw_buf.data_mut() else {
            return true;
        };

        for _y in 0..h {
            for x in 0..w {
                let m = mask_data.get(mask_start + x).copied().unwrap_or(0);
                if let Some(a) = img_data.get_mut(img_start + x * 4 + 3) {
                    *a = opa_mix2(m, *a);
                }
            }
            img_start += img_stride;
            mask_start += mask_stride;
        }

        true
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "complex")]
    use alloc::rc::Rc;
    #[cfg(feature = "complex")]
    use ferrovgl_core::draw::draw_buf::{DrawBuf, STRIDE_AUTO};
    use ferrovgl_core::draw::draw_buf::{ImageDsc, ImageHeader};
    use ferrovgl_core::draw::{DrawTaskType, LayerId};
    use ferrovgl_core::misc::color::{Color32, OPA_COVER};
    use ferrovgl_core::misc::style::ImageColorkey;

    /// A root ARGB8888 layer with a cleared buffer and a task drawing an
    /// image over `area`; the task is marked `IN_PROGRESS` (the sw unit
    /// does that before executing it).
    fn setup(buf_area: Area, cf: ColorFormat) -> (DrawCtx, LayerId) {
        let mut ctx = DrawCtx::new();
        let layer = ctx.layer_create(None, cf, &buf_area);
        assert!(ctx.layer_alloc_buf(layer));
        ctx.layer_mut(layer).unwrap().clip_area = buf_area;
        (ctx, layer)
    }

    fn add_task(ctx: &mut DrawCtx, layer: LayerId, area: Area) -> (DrawTaskRef, DrawTask) {
        let t = ctx.add_task(layer, &area, DrawTaskType::Image);
        ctx.task_mut(t).unwrap().state = DrawTaskState::InProgress;
        let task = ctx.task(t).unwrap().clone();
        (t, task)
    }

    fn px(ctx: &DrawCtx, layer: LayerId, x: u32, y: u32) -> [u8; 4] {
        let buf = ctx.layer(layer).unwrap().draw_buf.as_ref().unwrap();
        let o = buf.goto_xy(x, y);
        let d = buf.data();
        [d[o], d[o + 1], d[o + 2], d[o + 3]]
    }

    static PX_XRGB: [u8; 8] = [0x40, 0x80, 0xC0, 0xFF, 0x10, 0x20, 0x30, 0xFF];
    static IMG_XRGB: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::Xrgb8888, 2, 1, 8), &PX_XRGB);

    static PX_A8: [u8; 4] = [0xFF, 0x00, 0x80, 0xFF];
    static IMG_A8: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::A8, 2, 2, 2), &PX_A8);

    #[test]
    fn plain_copy_to_argb8888() {
        let (mut ctx, layer) = setup(Area::new(0, 0, 9, 9), ColorFormat::Argb8888);
        let (_, task) = add_task(&mut ctx, layer, Area::new(1, 1, 2, 1));
        let mut unit = SwDrawUnit::new(0);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_XRGB);
        image(&mut unit, &mut ctx, &task, &dsc, &Area::new(1, 1, 2, 1));
        assert_eq!(px(&ctx, layer, 1, 1), [0x40, 0x80, 0xC0, 0xFF]);
        assert_eq!(px(&ctx, layer, 2, 1), [0x10, 0x20, 0x30, 0xFF]);
        assert_eq!(px(&ctx, layer, 0, 1), [0, 0, 0, 0]);
    }

    #[test]
    fn a8_is_drawn_with_the_recolor() {
        let (mut ctx, layer) = setup(Area::new(0, 0, 9, 9), ColorFormat::Argb8888);
        let (_, task) = add_task(&mut ctx, layer, Area::new(0, 0, 1, 1));
        let mut unit = SwDrawUnit::new(0);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_A8);
        dsc.recolor = Color::make(0xFF, 0, 0);
        image(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 1, 1));
        assert_eq!(px(&ctx, layer, 0, 0), [0, 0, 0xFF, 0xFF]);
        assert_eq!(px(&ctx, layer, 1, 0)[3], 0);
        assert_eq!(px(&ctx, layer, 0, 1)[3], 0x80);
    }

    /* test_image_recolor_xrgb8888.c: recolouring an XRGB8888 image keeps
     * the X byte opaque */
    #[test]
    fn recolor_xrgb8888_preserves_opaque_x_byte() {
        let (mut ctx, layer) = setup(Area::new(0, 0, 9, 9), ColorFormat::Xrgb8888);
        let (_, task) = add_task(&mut ctx, layer, Area::new(0, 0, 1, 0));
        let mut unit = SwDrawUnit::new(0);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_XRGB);
        dsc.recolor = Color::make(0xF4, 0x43, 0x36);
        dsc.recolor_opa = 178; /*LV_OPA_70*/
        image(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 1, 0));
        for x in 0..2 {
            assert_eq!(px(&ctx, layer, x, 0)[3], 0xff);
        }
        /*(c_mult + src * mix_inv) >> 8 per channel*/
        let mix = 178u32;
        let inv = 255 - mix;
        let b = ((0x36 * mix + 0x40 * inv) >> 8) as u8;
        let g = ((0x43 * mix + 0x80 * inv) >> 8) as u8;
        let r = ((0xF4 * mix + 0xC0 * inv) >> 8) as u8;
        assert_eq!(&px(&ctx, layer, 0, 0)[..3], &[b, g, r]);
    }

    #[test]
    fn recolor_rgb565_formula() {
        let mut d = DrawImageDsc::new();
        d.recolor = Color::make(0xFF, 0x00, 0x00);
        d.recolor_opa = 128;
        let src = 0x001Fu16.to_le_bytes(); /*pure blue*/
        let mut out = [0u8; 2];
        recolor(
            Area::new(0, 0, 0, 0),
            Some(&src),
            &mut out,
            2,
            ColorFormat::Rgb565,
            &d,
        );
        let v = u16::from_le_bytes(out);
        let (mix, inv) = (128u32, 127u32);
        let expect = ((((31 * mix) << 3) & 0xF800) + (((0) >> 3) & 0x07E0) + ((31 * inv) >> 8)) as u16;
        assert_eq!(v, expect);

        /*Full recolor writes the colour, swapped variant keeps it unswapped like upstream*/
        d.recolor_opa = OPA_COVER;
        recolor(
            Area::new(0, 0, 0, 0),
            Some(&src),
            &mut out,
            2,
            ColorFormat::Rgb565Swapped,
            &d,
        );
        assert_eq!(u16::from_le_bytes(out), 0xF800);
    }

    #[test]
    fn recolor_premultiplied_keeps_alpha() {
        let mut d = DrawImageDsc::new();
        d.recolor = Color::make(0, 0, 0);
        d.recolor_opa = 128;
        let mut buf = [100u8, 100, 100, 200, 5, 6, 7, 0];
        recolor(
            Area::new(0, 0, 1, 0),
            None,
            &mut buf,
            8,
            ColorFormat::Argb8888Premultiplied,
            &d,
        );
        assert_eq!(buf[3], 200);
        let rec = (255u32 * 256 / 200) as u16;
        let un = ((100 * u32::from(rec)) >> 8) as u8;
        let mixed = ((u32::from(un) * 127) >> 8) as u8;
        let pre = ((u32::from(mixed) * 200) >> 8) as u8;
        assert_eq!(&buf[..3], &[pre, pre, pre]);
        /*Transparent pixels are copied*/
        assert_eq!(&buf[4..], &[5, 6, 7, 0]);
    }

    #[test]
    fn colorkey_rgb565a8_clears_the_mask() {
        let mut d = DrawImageDsc::new();
        d.colorkey = Some(ImageColorkey {
            low: Color::make(0xF0, 0, 0),
            high: Color::make(0xFF, 0x10, 0x10),
        });
        /*2 RGB565 pixels (red, green) followed by their A8 mask*/
        let mut buf = vec![0u8; 6];
        buf[..2].copy_from_slice(&0xF800u16.to_le_bytes());
        buf[2..4].copy_from_slice(&0x07E0u16.to_le_bytes());
        buf[4] = 0xFF;
        buf[5] = 0xFF;
        colorkey_and_recolor(
            Area::new(0, 0, 1, 0),
            None,
            &mut buf,
            4,
            ColorFormat::Rgb565A8,
            &d,
            Some(4),
            2,
        );
        assert_eq!(buf[4], 0);
        assert_eq!(buf[5], 0xFF);
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 0x07E0);
    }

    #[test]
    fn colorkey_argb8888_makes_keyed_pixels_transparent() {
        let mut d = DrawImageDsc::new();
        d.colorkey = Some(ImageColorkey {
            low: Color::make(0, 0, 0),
            high: Color::make(0x10, 0x10, 0x10),
        });
        let mut buf = [1u8, 2, 3, 255, 200, 200, 200, 255];
        colorkey_and_recolor(
            Area::new(0, 0, 1, 0),
            None,
            &mut buf,
            8,
            ColorFormat::Argb8888,
            &d,
            None,
            0,
        );
        assert_eq!(buf, [0, 0, 0, 0, 200, 200, 200, 255]);
    }

    #[test]
    fn color_helpers() {
        assert_eq!(color_swap_16(0x1234), 0x3412);
        assert_eq!(color16_to_color(0xFFFF), Color::make(0xF8, 0xFC, 0xF8));
        assert!(color_is_in_range(
            Color::make(5, 5, 5),
            Color::make(0, 0, 0),
            Color::make(5, 5, 5)
        ));
        assert!(!color_is_in_range(
            Color::make(6, 5, 5),
            Color::make(0, 0, 0),
            Color::make(5, 5, 5)
        ));
    }

    #[cfg(feature = "complex")]
    #[test]
    fn radius_clips_the_corners() {
        let (mut ctx, layer) = setup(Area::new(0, 0, 19, 19), ColorFormat::Argb8888);
        let (_, task) = add_task(&mut ctx, layer, Area::new(0, 0, 9, 9));
        let mut unit = SwDrawUnit::new(0);
        let mut src = DrawBuf::create(10, 10, ColorFormat::Xrgb8888, STRIDE_AUTO).unwrap();
        src.data_mut().unwrap().fill(0xFF);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::DrawBuf(Rc::new(src));
        dsc.clip_radius = 5;
        dsc.image_area = Area::new(0, 0, 9, 9);
        image(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 9, 9));
        assert_eq!(px(&ctx, layer, 0, 0)[3], 0);
        assert_eq!(px(&ctx, layer, 5, 5), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(px(&ctx, layer, 5, 0)[3], 0xFF);
    }

    #[test]
    fn layer_source_with_bitmap_mask() {
        let (mut ctx, root) = setup(Area::new(0, 0, 9, 9), ColorFormat::Argb8888);
        let child = ctx.layer_create(Some(root), ColorFormat::Argb8888, &Area::new(0, 0, 3, 0));
        assert!(ctx.layer_alloc_buf(child));
        {
            let buf = ctx.layer_mut(child).unwrap().draw_buf.as_mut().unwrap();
            let d = buf.data_mut().unwrap();
            for p in d.chunks_mut(4).take(4) {
                p.copy_from_slice(&[0x10, 0x20, 0x30, 0xFF]);
            }
        }
        static MASK: [u8; 2] = [0xFF, 0x80];
        static MASK_IMG: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::A8, 2, 1, 2), &MASK);

        let t = ctx.add_task(root, &Area::new(0, 0, 3, 0), DrawTaskType::Layer);
        ctx.task_mut(t).unwrap().state = DrawTaskState::InProgress;
        let task = ctx.task(t).unwrap().clone();
        let mut unit = SwDrawUnit::new(0);
        unit.task_act = Some(t);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Layer(child);
        dsc.image_area = Area::new(0, 0, 3, 0);
        dsc.bitmap_mask_src = ImageSrc::Static(&MASK_IMG);
        layer(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 3, 0));

        /*The 2 px mask is centred on the 4 px image: x = 1..2*/
        assert_eq!(px(&ctx, root, 0, 0), [0, 0, 0, 0]);
        let a1 = px(&ctx, root, 1, 0)[3];
        let a2 = px(&ctx, root, 2, 0)[3];
        assert!(a1 > a2 && a2 > 0);
        assert_eq!(px(&ctx, root, 3, 0), [0, 0, 0, 0]);
        /*apply_mask() multiplied the alpha of the child itself*/
        let child_buf = ctx.layer(child).unwrap().draw_buf.as_ref().unwrap();
        assert_eq!(child_buf.data()[3], 0);
        assert_eq!(child_buf.data()[7], opa_mix2(0xFF, 0xFF));
        assert_eq!(child_buf.data()[11], opa_mix2(0x80, 0xFF));
        assert_eq!(child_buf.data()[15], 0);
        /*A layer without a buffer draws nothing*/
        ctx.layer_mut(child).unwrap().draw_buf = None;
        layer(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 3, 0));
    }

    #[test]
    fn indexed_image_is_drawn_row_by_row() {
        static PX_I1: [u8; 10] = [0, 0, 0xFF, 255, 0, 0xFF, 0, 255, 0b1000_0000, 0b0100_0000];
        static IMG_I1: ImageDsc = ImageDsc::new(ImageHeader::new(ColorFormat::I1, 2, 2, 1), &PX_I1);
        let (mut ctx, layer) = setup(Area::new(0, 0, 9, 9), ColorFormat::Argb8888);
        let (_, task) = add_task(&mut ctx, layer, Area::new(0, 0, 1, 1));
        let mut unit = SwDrawUnit::new(0);
        let mut dsc = DrawImageDsc::new();
        dsc.src = ImageSrc::Static(&IMG_I1);
        image(&mut unit, &mut ctx, &task, &dsc, &Area::new(0, 0, 1, 1));
        let red = Color32::make(0xFF, 0, 0, 0xFF);
        let green = Color32::make(0, 0xFF, 0, 0xFF);
        let c = |p: [u8; 4]| Color32 {
            blue: p[0],
            green: p[1],
            red: p[2],
            alpha: p[3],
        };
        assert_eq!(c(px(&ctx, layer, 0, 0)), green);
        assert_eq!(c(px(&ctx, layer, 1, 0)), red);
        assert_eq!(c(px(&ctx, layer, 0, 1)), red);
        assert_eq!(c(px(&ctx, layer, 1, 1)), green);
    }
}
