//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_argb8888_premultiplied.c`.
//!
//! Blend a colour or an image into an ARGB8888 destination whose colour
//! channels are premultiplied with the alpha. The destination buffer is
//! accessed byte-wise (`[b, g, r, a]` per pixel). The optional
//! `LV_DRAW_SW_*_BLEND_*_TO_ARGB8888_PREMULTIPLIED` accelerator hooks are not
//! ported: they default to `LV_RESULT_INVALID`, so the software loops always
//! run.

use ferrovgl_core::misc::color::{Color32, ColorFormat, OPA_MAX, OPA_MIN, Opa, opa_mix2, opa_mix3};
use ferrovgl_core::misc::style::BlendMode;

use crate::blend::{BlendFillDsc, BlendImageDsc};

/// `lv_color_mix_alpha_cache_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct ColorMixAlphaCache {
    fg_saved: Color32,
    bg_saved: Color32,
    res_saved: Color32,
    res_alpha_saved: Opa,
    ratio_saved: Opa,
}

/// Read the pixel at byte offset `i`.
#[inline(always)]
fn get_px(buf: &[u8], i: usize) -> Color32 {
    Color32 {
        blue: buf[i],
        green: buf[i + 1],
        red: buf[i + 2],
        alpha: buf[i + 3],
    }
}

/// Write the pixel at byte offset `i`.
#[inline(always)]
fn set_px(buf: &mut [u8], i: usize, c: Color32) {
    buf[i] = c.blue;
    buf[i + 1] = c.green;
    buf[i + 2] = c.red;
    buf[i + 3] = c.alpha;
}

/// Byte offset of pixel `x` in a row starting at `row`.
#[inline(always)]
const fn px_ofs(row: usize, x: i32) -> usize {
    row + (x as usize) * 4
}

/// Advance a mask row offset by `mask_stride` (`mask += mask_stride`).
#[inline(always)]
const fn mask_next_row(row: usize, mask_stride: i32) -> usize {
    row.wrapping_add_signed(mask_stride as isize)
}

/// `(c * a) >> 8` of the premultiplication steps, truncated to 8 bits.
#[inline(always)]
const fn premul_ch(c: u8, a: u8) -> u8 {
    ((c as u32 * a as u32) >> 8) as u8
}

/// `lv_draw_sw_blend_color_to_argb8888_premultiplied()`: fill with a colour (+ opa / mask).
///
/// The input colour is converted to premultiplied alpha before blending.
pub fn fill(dsc: &mut BlendFillDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let mut opa = dsc.opa;
    let mask = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_stride = dsc.dest_stride;
    let dest = &mut *dsc.dest_buf;

    let mut cache = ColorMixAlphaCache::default();
    color_mix_with_alpha_cache_init(&mut cache);

    /* Convert source color to premultiplied */
    if opa >= OPA_MAX {
        opa = 0xff;
    }
    let color_argb = dsc.color.to_32(opa);
    let color_argb_premul = if opa == 0xff {
        color_argb
    } else {
        Color32 {
            alpha: opa,
            red: premul_ch(color_argb.red, opa),
            green: premul_ch(color_argb.green, opa),
            blue: premul_ch(color_argb.blue, opa),
        }
    };

    let mut row = 0usize;
    match mask {
        /* Simple fill */
        None if opa >= OPA_MAX => {
            let color32 = dsc.color.to_u32().to_le_bytes();
            for _y in 0..h {
                for x in 0..w {
                    let i = px_ofs(row, x);
                    dest[i..i + 4].copy_from_slice(&color32);
                }
                row = drawbuf_next_row(row, dest_stride);
            }
        }
        /* Opacity only */
        None => {
            for _y in 0..h {
                for x in 0..w {
                    let i = px_ofs(row, x);
                    let d = get_px(dest, i);
                    set_px(dest, i, color_32_32_mix_premul(color_argb_premul, d, &mut cache));
                }
                row = drawbuf_next_row(row, dest_stride);
            }
        }
        /* Masked fill */
        Some(mask) if opa >= OPA_MAX => {
            let mut m = 0usize;
            for _y in 0..h {
                for x in 0..w {
                    let mut color_premul = color_argb;
                    let mv = mask[m + x as usize];
                    let i = px_ofs(row, x);
                    if mv >= OPA_MAX {
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_premul, d, &mut cache));
                    } else if mv > OPA_MIN {
                        color_premul.alpha = mv;
                        color_premul.red = premul_ch(color_premul.red, color_premul.alpha);
                        color_premul.green = premul_ch(color_premul.green, color_premul.alpha);
                        color_premul.blue = premul_ch(color_premul.blue, color_premul.alpha);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_premul, d, &mut cache));
                    }
                }
                row = drawbuf_next_row(row, dest_stride);
                m = mask_next_row(m, mask_stride);
            }
        }
        /* Masked with opacity */
        Some(mask) => {
            let mut m = 0usize;
            for _y in 0..h {
                for x in 0..w {
                    let mut color_premul = color_argb;
                    let mv = mask[m + x as usize];
                    let alpha = opa_mix2(mv, opa);
                    let i = px_ofs(row, x);
                    if alpha >= OPA_MAX {
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_premul, d, &mut cache));
                    } else if mv > OPA_MIN {
                        color_premul.alpha = alpha;
                        color_premul.red = premul_ch(color_premul.red, color_premul.alpha);
                        color_premul.green = premul_ch(color_premul.green, color_premul.alpha);
                        color_premul.blue = premul_ch(color_premul.blue, color_premul.alpha);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_premul, d, &mut cache));
                    }
                }
                row = drawbuf_next_row(row, dest_stride);
                m = mask_next_row(m, mask_stride);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_argb8888_premultiplied()`: blend an image of any supported
/// source format.
pub fn image(dsc: &mut BlendImageDsc<'_>) {
    match dsc.src_color_format {
        #[cfg(feature = "cf-argb8888")]
        ColorFormat::Argb8888 => argb8888_image_blend(dsc),
        #[cfg(feature = "cf-rgb888")]
        ColorFormat::Rgb888 => rgb888_image_blend(dsc, 3),
        #[cfg(feature = "cf-xrgb8888")]
        ColorFormat::Xrgb8888 => rgb888_image_blend(dsc, 4),
        ColorFormat::Argb8888Premultiplied => argb8888_premultiplied_image_blend(dsc),
        /* LV_LOG_WARN("Not supported source color format") */
        _ => {}
    }
}

/// The shared per-pixel step of the normal-mode `argb8888_image_blend()`
/// loops. `alpha` is the effective alpha of the straight `color_argb` source.
#[cfg(feature = "cf-argb8888")]
#[inline(always)]
fn argb8888_premul_blend_px(
    dest: &mut [u8],
    i: usize,
    mut color_argb: Color32,
    alpha: Opa,
    cache: &mut ColorMixAlphaCache,
) {
    if alpha >= OPA_MAX {
        color_argb.alpha = 0xff;
        set_px(dest, i, color_argb);
    } else if alpha > OPA_MIN {
        /*Premultiplication can cause loss of precision which can result slightly
         *darker color when blending the same color to the background.*/
        let d = get_px(dest, i);
        if d.red != color_argb.red
            || d.green != color_argb.green
            || d.blue != color_argb.blue
            || d.alpha != color_argb.alpha
        {
            color_argb.alpha = alpha;
            color_argb.red = premul_ch(color_argb.red, color_argb.alpha);
            color_argb.green = premul_ch(color_argb.green, color_argb.alpha);
            color_argb.blue = premul_ch(color_argb.blue, color_argb.alpha);
            set_px(dest, i, color_32_32_mix_premul(color_argb, d, cache));
        }
    }
}

/// `argb8888_image_blend()`: straight-alpha ARGB8888 source.
#[cfg(feature = "cf-argb8888")]
fn argb8888_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let src = dsc.src_buf;
    let dest = &mut *dsc.dest_buf;

    let mut cache = ColorMixAlphaCache::default();
    color_mix_with_alpha_cache_init(&mut cache);

    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let color_argb = get_px(src, px_ofs(srow, x));
                        let alpha = color_argb.alpha;
                        argb8888_premul_blend_px(dest, px_ofs(drow, x), color_argb, alpha, &mut cache);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let color_argb = get_px(src, px_ofs(srow, x));
                        /* Apply global opacity */
                        let alpha = opa_mix2(color_argb.alpha, opa);
                        argb8888_premul_blend_px(dest, px_ofs(drow, x), color_argb, alpha, &mut cache);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let color_argb = get_px(src, px_ofs(srow, x));
                        /* Apply mask opacity */
                        let alpha = opa_mix2(color_argb.alpha, mask[m + x as usize]);
                        argb8888_premul_blend_px(dest, px_ofs(drow, x), color_argb, alpha, &mut cache);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
            Some(mask) => {
                for _y in 0..h {
                    for x in 0..w {
                        let color_argb = get_px(src, px_ofs(srow, x));
                        /* Apply both mask and global opacity */
                        let alpha = opa_mix3(color_argb.alpha, opa, mask[m + x as usize]);
                        argb8888_premul_blend_px(dest, px_ofs(drow, x), color_argb, alpha, &mut cache);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                let mut color_argb = get_px(src, px_ofs(srow, x));

                /* Apply mask and/or opacity */
                color_argb.alpha = match mask_buf {
                    None => opa_mix2(color_argb.alpha, opa),
                    Some(mask) => opa_mix3(color_argb.alpha, mask[m + x as usize], opa),
                };

                /* Premultiply alpha */
                color_argb.red = premul_ch(color_argb.red, color_argb.alpha);
                color_argb.green = premul_ch(color_argb.green, color_argb.alpha);
                color_argb.blue = premul_ch(color_argb.blue, color_argb.alpha);

                blend_non_normal_pixel_premultiplied(
                    dest,
                    px_ofs(drow, x),
                    color_argb,
                    dsc.blend_mode,
                    &mut cache,
                );
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// `rgb888_image_blend()`: RGB888 (`src_px_size == 3`) or XRGB8888
/// (`src_px_size == 4`) source.
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
fn rgb888_image_blend(dsc: &mut BlendImageDsc<'_>, src_px_size: u8) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let src = dsc.src_buf;
    let dest = &mut *dsc.dest_buf;
    let ps = src_px_size as usize;

    let mut color_argb = Color32::default();
    let mut cache = ColorMixAlphaCache::default();
    color_mix_with_alpha_cache_init(&mut cache);

    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;

    if dsc.blend_mode == BlendMode::Normal {
        /*Special case*/
        if mask_buf.is_none() && opa >= OPA_MAX {
            if src_px_size == 4 {
                let line_in_bytes = (w as u32).wrapping_mul(4) as usize;
                for _y in 0..h {
                    dest[drow..drow + line_in_bytes].copy_from_slice(&src[srow..srow + line_in_bytes]);
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            } else if src_px_size == 3 {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = srow + dest_x as usize * 3;
                        let c = Color32::make(src[s + 2], src[s + 1], src[s], 0xff);
                        set_px(dest, px_ofs(drow, dest_x), c);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
        }
        if mask_buf.is_none() && opa < OPA_MAX {
            color_argb.alpha = opa;
            for _y in 0..h {
                for dest_x in 0..w {
                    let s = srow + dest_x as usize * ps;
                    color_argb.red = premul_ch(src[s + 2], color_argb.alpha);
                    color_argb.green = premul_ch(src[s + 1], color_argb.alpha);
                    color_argb.blue = premul_ch(src[s], color_argb.alpha);
                    let i = px_ofs(drow, dest_x);
                    let d = get_px(dest, i);
                    set_px(dest, i, color_32_32_mix_premul(color_argb, d, &mut cache));
                }
                drow = drawbuf_next_row(drow, dest_stride);
                srow = drawbuf_next_row(srow, src_stride);
            }
        }
        if let Some(mask) = mask_buf {
            if opa >= OPA_MAX {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = srow + dest_x as usize * ps;
                        color_argb.alpha = mask[m + dest_x as usize];
                        color_argb.red = premul_ch(src[s + 2], color_argb.alpha);
                        color_argb.green = premul_ch(src[s + 1], color_argb.alpha);
                        color_argb.blue = premul_ch(src[s], color_argb.alpha);
                        let i = px_ofs(drow, dest_x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_argb, d, &mut cache));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
            if opa < OPA_MAX {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = srow + dest_x as usize * ps;
                        color_argb.alpha = premul_ch(opa, mask[m + dest_x as usize]);
                        color_argb.red = premul_ch(src[s + 2], color_argb.alpha);
                        color_argb.green = premul_ch(src[s + 1], color_argb.alpha);
                        color_argb.blue = premul_ch(src[s], color_argb.alpha);
                        let i = px_ofs(drow, dest_x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_argb, d, &mut cache));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
        }
    } else {
        let mut src_argb = Color32::default();
        for _y in 0..h {
            for dest_x in 0..w {
                let s = srow + dest_x as usize * ps;
                src_argb.red = src[s + 2];
                src_argb.green = src[s + 1];
                src_argb.blue = src[s];
                src_argb.alpha = match mask_buf {
                    None => opa,
                    Some(mask) => opa_mix2(mask[m + dest_x as usize], opa),
                };

                blend_non_normal_pixel_premultiplied(
                    dest,
                    px_ofs(drow, dest_x),
                    src_argb,
                    dsc.blend_mode,
                    &mut cache,
                );
            }

            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// `argb8888_premultiplied_image_blend()`: premultiplied ARGB8888 source.
fn argb8888_premultiplied_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let src = dsc.src_buf;
    let dest = &mut *dsc.dest_buf;

    let mut cache = ColorMixAlphaCache::default();
    color_mix_with_alpha_cache_init(&mut cache);

    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let s = get_px(src, px_ofs(srow, x));
                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(s, d, &mut cache));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let mut color_argb = get_px(src, px_ofs(srow, x));

                        /*Scaling the premultiplied channels by `opa` is equivalent to
                         *unpremultiplying, scaling the alpha, and premultiplying again*/
                        color_argb.red = opa_mix2(color_argb.red, opa);
                        color_argb.green = opa_mix2(color_argb.green, opa);
                        color_argb.blue = opa_mix2(color_argb.blue, opa);
                        color_argb.alpha = opa_mix2(color_argb.alpha, opa);

                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_argb, d, &mut cache));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let mut color_argb = get_px(src, px_ofs(srow, x));
                        let mv = mask[m + x as usize];

                        /*Scaling the premultiplied channels by the mask is equivalent to
                         *unpremultiplying, scaling the alpha, and premultiplying again*/
                        color_argb.red = opa_mix2(color_argb.red, mv);
                        color_argb.green = opa_mix2(color_argb.green, mv);
                        color_argb.blue = opa_mix2(color_argb.blue, mv);
                        color_argb.alpha = opa_mix2(color_argb.alpha, mv);

                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_argb, d, &mut cache));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
            Some(mask) => {
                for _y in 0..h {
                    for x in 0..w {
                        let mut color_argb = get_px(src, px_ofs(srow, x));
                        let mv = mask[m + x as usize];

                        /*Scaling the premultiplied channels by mask * opa is equivalent to
                         *unpremultiplying, scaling the alpha, and premultiplying again*/
                        let scale = opa_mix2(mv, opa);
                        color_argb.red = opa_mix2(color_argb.red, scale);
                        color_argb.green = opa_mix2(color_argb.green, scale);
                        color_argb.blue = opa_mix2(color_argb.blue, scale);
                        /*Combine the three factors in one step to keep the precision*/
                        color_argb.alpha = opa_mix3(color_argb.alpha, mv, opa);

                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix_premul(color_argb, d, &mut cache));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                let mut color_argb = get_px(src, px_ofs(srow, x));

                /* Unpremultiply the source color by using the reciprocal of the alpha */
                if color_argb.alpha != 0 {
                    let reciprocal_alpha = ((255u32 * 256) / color_argb.alpha as u32) as u16 as u32;
                    color_argb.red = ((color_argb.red as u32 * reciprocal_alpha) >> 8) as u8;
                    color_argb.green = ((color_argb.green as u32 * reciprocal_alpha) >> 8) as u8;
                    color_argb.blue = ((color_argb.blue as u32 * reciprocal_alpha) >> 8) as u8;
                }

                /* Adjust alpha if needed */
                color_argb.alpha = match mask_buf {
                    None => opa_mix2(color_argb.alpha, opa),
                    Some(mask) => opa_mix3(color_argb.alpha, mask[m + x as usize], opa),
                };

                /* Premultiply alpha */
                color_argb.red = premul_ch(color_argb.red, color_argb.alpha);
                color_argb.green = premul_ch(color_argb.green, color_argb.alpha);
                color_argb.blue = premul_ch(color_argb.blue, color_argb.alpha);

                blend_non_normal_pixel_premultiplied(
                    dest,
                    px_ofs(drow, x),
                    color_argb,
                    dsc.blend_mode,
                    &mut cache,
                );
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// `lv_color_32_32_mix_premul()`: mix a premultiplied foreground into the
/// background.
#[inline(always)]
fn color_32_32_mix_premul(fg: Color32, bg: Color32, cache: &mut ColorMixAlphaCache) -> Color32 {
    /*Pick the foreground if it's fully opaque or the background is fully transparent*/
    if fg.alpha >= OPA_MAX || bg.alpha <= OPA_MIN {
        fg
    }
    /* Transparent foreground: use the background */
    else if fg.alpha <= OPA_MIN {
        bg
    }
    /* Opaque background: use simple mix */
    else if bg.alpha >= OPA_MAX {
        Color32::mix32_premultiplied(fg, bg)
    } else {
        /* Check cache to avoid redundant calculations */
        if bg.alpha != cache.bg_saved.alpha || fg.alpha != cache.fg_saved.alpha {
            /* Compute final alpha value */
            cache.res_alpha_saved = 255 - opa_mix2(255 - fg.alpha, 255 - bg.alpha);
            debug_assert!(cache.res_alpha_saved != 0);

            /* Compute premultiplied blending ratio */
            cache.ratio_saved = ((fg.alpha as u32 * 255) / cache.res_alpha_saved as u32) as Opa;
        }

        /* Check if color blending is already cached */
        if !Color32::eq(bg, cache.bg_saved) || !Color32::eq(fg, cache.fg_saved) {
            cache.fg_saved = fg;
            cache.bg_saved = bg;

            /* Blend using premultiplied alpha */
            let inv_fg_alpha = 255u32 - fg.alpha as u32;
            cache.res_saved.red = (fg.red as u32).wrapping_add((bg.red as u32 * inv_fg_alpha) >> 8) as u8;
            cache.res_saved.green =
                (fg.green as u32).wrapping_add((bg.green as u32 * inv_fg_alpha) >> 8) as u8;
            cache.res_saved.blue = (fg.blue as u32).wrapping_add((bg.blue as u32 * inv_fg_alpha) >> 8) as u8;
            cache.res_saved.alpha = cache.res_alpha_saved;
        }

        cache.res_saved
    }
}

/// `lv_color_mix_with_alpha_cache_init()`
fn color_mix_with_alpha_cache_init(cache: &mut ColorMixAlphaCache) {
    cache.fg_saved = Color32::default();
    cache.bg_saved = Color32::default();
    cache.res_saved = Color32::default();
    cache.res_alpha_saved = 255;
    cache.ratio_saved = 255;
}

/// `drawbuf_next_row()`: advance a row offset by `stride` bytes.
#[inline(always)]
const fn drawbuf_next_row(row: usize, stride: i32) -> usize {
    row.wrapping_add(stride as u32 as usize)
}

/// `blend_non_normal_pixel_premultiplied()`: blend a premultiplied `src` into
/// the pixel at byte offset `i` with a non-normal blend mode.
#[inline(always)]
fn blend_non_normal_pixel_premultiplied(
    dest: &mut [u8],
    i: usize,
    src: Color32,
    mode: BlendMode,
    cache: &mut ColorMixAlphaCache,
) {
    let d = get_px(dest, i);
    let mut res = Color32::default();
    let src_alpha = src.alpha; /* Premultiplied alpha of source */
    let dest_alpha = d.alpha as i32;
    let (dr, dg, db) = (d.red as i32, d.green as i32, d.blue as i32);
    let (sr, sg, sb) = (src.red as i32, src.green as i32, src.blue as i32);

    match mode {
        BlendMode::Additive => {
            /* Ensure RGB remains premultiplied */
            res.red = (dr + sr).min(dest_alpha) as u8;
            res.green = (dg + sg).min(dest_alpha) as u8;
            res.blue = (db + sb).min(dest_alpha) as u8;
        }
        BlendMode::Subtractive => {
            /* Ensure RGB remains non-negative and premultiplied */
            res.red = (dr - sr).max(0) as u8;
            res.green = (dg - sg).max(0) as u8;
            res.blue = (db - sb).max(0) as u8;
        }
        BlendMode::Multiply => {
            /* Adjusted for premultiplied alpha: scale the result properly */
            let div = (src_alpha as i32).max(1);
            res.red = ((dr * sr) / div) as u8;
            res.green = ((dg * sg) / div) as u8;
            res.blue = ((db * sb) / div) as u8;
        }
        BlendMode::Difference => {
            res.red = (dr - sr).unsigned_abs() as u8;
            res.green = (dg - sg).unsigned_abs() as u8;
            res.blue = (db - sb).unsigned_abs() as u8;
        }
        /* LV_LOG_WARN("Not supported blend mode") */
        BlendMode::Normal => return,
    }

    res.alpha = src_alpha; /* Keep the alpha from premultiplied source */
    set_px(dest, i, color_32_32_mix_premul(res, d, cache));
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use ferrovgl_core::misc::area::Area;
    use ferrovgl_core::misc::color::Color;

    const RED: Color = Color::make(255, 0, 0);

    fn c32(r: u8, g: u8, b: u8, a: u8) -> Color32 {
        Color32::make(r, g, b, a)
    }

    fn buf_of(pxs: &[Color32]) -> Vec<u8> {
        let mut v = vec![0u8; pxs.len() * 4];
        for (i, p) in pxs.iter().enumerate() {
            set_px(&mut v, i * 4, *p);
        }
        v
    }

    fn pxs_of(buf: &[u8]) -> Vec<Color32> {
        (0..buf.len() / 4).map(|i| get_px(buf, i * 4)).collect()
    }

    fn fill_px(dest: &[Color32], mask: Option<&[u8]>, color: Color, opa: Opa) -> Vec<Color32> {
        let w = dest.len() as i32;
        let mut buf = buf_of(dest);
        fill(&mut BlendFillDsc {
            dest_buf: &mut buf,
            dest_w: w,
            dest_h: 1,
            dest_stride: w * 4,
            mask_buf: mask,
            mask_stride: w,
            color,
            opa,
            relative_area: Area::default(),
        });
        pxs_of(&buf)
    }

    fn image_px(
        dest: &[Color32],
        src: &[u8],
        cf: ColorFormat,
        opa: Opa,
        mask: Option<&[u8]>,
        mode: BlendMode,
    ) -> Vec<Color32> {
        let w = dest.len() as i32;
        let mut buf = buf_of(dest);
        image(&mut BlendImageDsc {
            dest_buf: &mut buf,
            dest_w: w,
            dest_h: 1,
            dest_stride: w * 4,
            mask_buf: mask,
            mask_stride: w,
            src_buf: src,
            src_stride: src.len() as i32,
            src_color_format: cf,
            opa,
            blend_mode: mode,
            relative_area: Area::default(),
            src_area: Area::default(),
        });
        pxs_of(&buf)
    }

    fn new_cache() -> ColorMixAlphaCache {
        let mut c = ColorMixAlphaCache::default();
        color_mix_with_alpha_cache_init(&mut c);
        c
    }

    const BLUE: Color32 = Color32::make(0, 0, 255, 255);
    const TRANSP: Color32 = Color32::make(0, 0, 0, 0);

    #[test]
    fn mix_premul_semantics() {
        let mut cache = new_cache();
        // Cover foreground and transparent background: the foreground.
        assert_eq!(
            color_32_32_mix_premul(c32(1, 2, 3, 255), BLUE, &mut cache),
            c32(1, 2, 3, 255)
        );
        assert_eq!(
            color_32_32_mix_premul(c32(1, 2, 3, 50), TRANSP, &mut cache),
            c32(1, 2, 3, 50)
        );
        // Transparent foreground: the background.
        assert_eq!(
            color_32_32_mix_premul(c32(1, 2, 3, OPA_MIN), BLUE, &mut cache),
            BLUE
        );
        // Opaque background: lv_color_mix32_premultiplied (LV_OPA_MAX - alpha weight).
        assert_eq!(
            color_32_32_mix_premul(c32(64, 0, 0, 128), BLUE, &mut cache),
            c32(64, 0, 124, 255)
        );
        // Both semi-transparent: fg + bg * (255 - fg.alpha) >> 8, alpha 255 - MIX2(127, 127).
        let r = color_32_32_mix_premul(c32(64, 0, 0, 128), c32(0, 0, 128, 128), &mut cache);
        assert_eq!(r, c32(64, 0, 63, 192));
        assert_eq!((cache.res_alpha_saved, cache.ratio_saved), (192, 170));
        assert_eq!(
            color_32_32_mix_premul(c32(64, 0, 0, 128), c32(0, 0, 128, 128), &mut cache),
            r
        );
    }

    #[test]
    fn fill_cover_and_stride() {
        let pad = c32(0x11, 0x11, 0x11, 0x11);
        let mut buf = buf_of(&[pad; 6]);
        fill(&mut BlendFillDsc {
            dest_buf: &mut buf,
            dest_w: 2,
            dest_h: 2,
            dest_stride: 12,
            mask_buf: None,
            mask_stride: 0,
            color: RED,
            opa: OPA_MAX,
            relative_area: Area::default(),
        });
        let p = pxs_of(&buf);
        assert_eq!(p[0], c32(255, 0, 0, 255));
        assert_eq!(p[1], c32(255, 0, 0, 255));
        assert_eq!(p[2], pad);
        assert_eq!(p[3], c32(255, 0, 0, 255));
        assert_eq!(p[5], pad);
    }

    #[test]
    fn fill_half_opa() {
        // Premultiplied colour (127, 0, 0, 128).
        let p = fill_px(&[TRANSP, BLUE], None, RED, 128);
        assert_eq!(p[0], c32(127, 0, 0, 128));
        assert_eq!(p[1], c32(127, 0, 124, 255));
    }

    #[test]
    fn fill_mask() {
        let p = fill_px(&[TRANSP, TRANSP, BLUE], Some(&[255, 128, 1]), RED, 255);
        assert_eq!(p[0], c32(255, 0, 0, 255));
        assert_eq!(p[1], c32(127, 0, 0, 128));
        assert_eq!(p[2], BLUE);
        // Mask and opa: alpha MIX2(255, 128) = 127, red (255 * 127) >> 8 = 126.
        // The test is on the mask, not the effective alpha: mask 3 still blends
        // (alpha 1), which keeps an opaque background.
        let p = fill_px(&[TRANSP, BLUE, BLUE], Some(&[255, 2, 3]), RED, 128);
        assert_eq!(p[0], c32(126, 0, 0, 127));
        assert_eq!(p[1], BLUE);
        assert_eq!(p[2], BLUE);
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn image_straight_argb8888() {
        let src = buf_of(&[
            c32(255, 0, 0, 255),
            c32(255, 0, 0, 128),
            c32(255, 0, 0, 128),
            c32(255, 0, 0, 1),
        ]);
        let dest = [TRANSP, TRANSP, c32(255, 0, 0, 128), BLUE];
        let p = image_px(&dest, &src, ColorFormat::Argb8888, 255, None, BlendMode::Normal);
        assert_eq!(p[0], c32(255, 0, 0, 255));
        assert_eq!(p[1], c32(127, 0, 0, 128));
        // The same colour is not blended again.
        assert_eq!(p[2], c32(255, 0, 0, 128));
        assert_eq!(p[3], BLUE);
        // opa: MIX2(255, 128) = 127
        let p = image_px(
            &[TRANSP],
            &src[..4],
            ColorFormat::Argb8888,
            128,
            None,
            BlendMode::Normal,
        );
        assert_eq!(p[0], c32(126, 0, 0, 127));
        // mask: MIX2(255, 128) = 127
        let p = image_px(
            &[TRANSP],
            &src[..4],
            ColorFormat::Argb8888,
            255,
            Some(&[128]),
            BlendMode::Normal,
        );
        assert_eq!(p[0], c32(126, 0, 0, 127));
        // mask + opa: MIX3(255, 128, 255) = 127
        let p = image_px(
            &[TRANSP],
            &src[..4],
            ColorFormat::Argb8888,
            128,
            Some(&[255]),
            BlendMode::Normal,
        );
        assert_eq!(p[0], c32(126, 0, 0, 127));
    }

    #[test]
    fn image_premultiplied_argb8888() {
        let src = buf_of(&[c32(128, 0, 0, 128)]);
        let cf = ColorFormat::Argb8888Premultiplied;
        assert_eq!(
            image_px(&[BLUE], &src, cf, 255, None, BlendMode::Normal)[0],
            c32(128, 0, 124, 255)
        );
        assert_eq!(
            image_px(&[TRANSP], &src, cf, 255, None, BlendMode::Normal)[0],
            c32(128, 0, 0, 128)
        );
        // Scaled channels: (64, 0, 0, 64)
        assert_eq!(
            image_px(&[BLUE], &src, cf, 128, None, BlendMode::Normal)[0],
            c32(64, 0, 188, 255)
        );
        assert_eq!(
            image_px(&[BLUE], &src, cf, 255, Some(&[128]), BlendMode::Normal)[0],
            c32(64, 0, 188, 255)
        );
        // scale MIX2(128, 128) = 64 -> red 32, alpha MIX3(128, 128, 128) = 32
        assert_eq!(
            image_px(&[BLUE], &src, cf, 128, Some(&[128]), BlendMode::Normal)[0],
            c32(32, 0, 220, 255)
        );
    }

    #[cfg(all(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
    #[test]
    fn image_rgb888_xrgb8888() {
        let p = image_px(
            &[TRANSP, TRANSP],
            &[1, 2, 3, 4, 5, 6],
            ColorFormat::Rgb888,
            255,
            None,
            BlendMode::Normal,
        );
        assert_eq!(p, [c32(3, 2, 1, 255), c32(6, 5, 4, 255)]);
        let src4 = [1u8, 2, 3, 0x77];
        let p = image_px(
            &[TRANSP],
            &src4,
            ColorFormat::Xrgb8888,
            255,
            None,
            BlendMode::Normal,
        );
        assert_eq!(p[0], c32(3, 2, 1, 0x77));
        // opa 128: premultiplied (127, 0, 0, 128)
        let p = image_px(
            &[BLUE],
            &[0, 0, 255],
            ColorFormat::Rgb888,
            128,
            None,
            BlendMode::Normal,
        );
        assert_eq!(p[0], c32(127, 0, 124, 255));
        let p = image_px(
            &[TRANSP],
            &[0, 0, 255, 0],
            ColorFormat::Xrgb8888,
            255,
            Some(&[128]),
            BlendMode::Normal,
        );
        assert_eq!(p[0], c32(127, 0, 0, 128));
        // mask + opa: (opa * mask) >> 8 = 127
        let p = image_px(
            &[TRANSP],
            &[0, 0, 255],
            ColorFormat::Rgb888,
            128,
            Some(&[255]),
            BlendMode::Normal,
        );
        assert_eq!(p[0], c32(126, 0, 0, 127));
    }

    #[test]
    fn blend_modes_premultiplied() {
        // Unpremultiplied (255, 127, 63), alpha MIX2(128, 255) = 127,
        // premultiplied again (126, 63, 31, 127).
        let src = buf_of(&[c32(128, 64, 32, 128)]);
        let dst = c32(100, 200, 50, 255);
        let cf = ColorFormat::Argb8888Premultiplied;
        // Multiply: d * s / alpha, then fg + (bg * 126) >> 8
        assert_eq!(
            image_px(&[dst], &src, cf, 255, None, BlendMode::Multiply)[0],
            c32(148, 197, 36, 255)
        );
        assert_eq!(
            image_px(&[dst], &src, cf, 255, None, BlendMode::Difference)[0],
            c32(75, 235, 43, 255)
        );
        assert_eq!(
            image_px(&[dst], &src, cf, 255, None, BlendMode::Subtractive)[0],
            c32(49, 235, 43, 255)
        );
        // Additive is clamped to the destination alpha.
        assert_eq!(
            image_px(&[TRANSP], &src, cf, 255, None, BlendMode::Additive)[0],
            c32(0, 0, 0, 127)
        );
    }
}
