//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_argb8888.c`.
//!
//! Blend a colour or an image into a straight-alpha ARGB8888 destination.
//! The destination buffer is accessed byte-wise (`[b, g, r, a]` per pixel,
//! the in-memory layout of `lv_color32_t`). The optional
//! `LV_DRAW_SW_*_BLEND_*_TO_ARGB8888` accelerator hooks are not ported: they
//! default to `LV_RESULT_INVALID`, so the software loops always run.

#[cfg(any(feature = "cf-rgb565", feature = "cf-rgb565-swapped"))]
use ferrovgl_core::misc::color::Color16;
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

/// `ARGB8888_MASK_FILL_PX(dest_px, mask_val)`: one masked pixel: leave it
/// alone, store the colour, or mix.
#[inline(always)]
fn mask_fill_px(
    dest: &mut [u8],
    i: usize,
    mask_val: u32,
    color_argb: &mut Color32,
    cache: &mut ColorMixAlphaCache,
) {
    let a = mask_val;
    if a > OPA_MIN as u32 {
        color_argb.alpha = a as u8;
        let d = get_px(dest, i);
        if a >= OPA_MAX as u32 || d.alpha <= OPA_MIN {
            set_px(dest, i, *color_argb);
        } else {
            set_px(dest, i, color_32_32_mix_mid(*color_argb, d, cache));
        }
    }
}

/// `lv_draw_sw_blend_color_to_argb8888()`: fill with a colour (+ opa / mask).
pub fn fill(dsc: &mut BlendFillDsc<'_>) {
    let mut w = dsc.dest_w;
    let mut h = dsc.dest_h;
    let opa = dsc.opa;
    let mask = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_stride = dsc.dest_stride;
    let dest = &mut *dsc.dest_buf;

    let mut cache = ColorMixAlphaCache::default();
    color_mix_with_alpha_cache_init(&mut cache);

    match mask {
        /*Simple fill*/
        None if opa >= OPA_MAX => {
            let color32 = dsc.color.to_u32().to_le_bytes();
            let mut row = 0usize;
            /* Treat the buffer as one long row when stride matches width*/
            if dest_stride == w.wrapping_mul(4) {
                w = w.wrapping_mul(h);
                h = 1;
            }
            for _y in 0..h {
                for x in 0..w {
                    let i = px_ofs(row, x);
                    dest[i..i + 4].copy_from_slice(&color32);
                }
                row = drawbuf_next_row(row, dest_stride);
            }
        }
        /*Opacity only*/
        None => {
            let color_argb = dsc.color.to_32(opa);
            let mut row = 0usize;
            for _y in 0..h {
                for x in 0..w {
                    let i = px_ofs(row, x);
                    let d = get_px(dest, i);
                    set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
                }
                row = drawbuf_next_row(row, dest_stride);
            }
        }
        /*Masked with full opacity*/
        Some(mask) if opa >= OPA_MAX => {
            let mut color_argb = dsc.color.to_32(0xff);
            let mut row = 0usize;
            let mut m = 0usize;
            /*Glyphs are a few pixels wide, where the alignment and tail handling of the
             *word based loop would cost more than it saves. Rounded corners and borders are
             *as wide as the widget and hold long runs of 0 and 255, where it pays off.*/
            if w < 32 {
                for _y in 0..h {
                    for x in 0..w {
                        let mv = mask[m + x as usize] as u32;
                        mask_fill_px(dest, px_ofs(row, x), mv, &mut color_argb, &mut cache);
                    }
                    row = drawbuf_next_row(row, dest_stride);
                    m = mask_next_row(m, mask_stride);
                }
            } else {
                let mut color_opaque = color_argb;
                color_opaque.alpha = 0xff;
                let base = mask.as_ptr() as usize;
                for _y in 0..h {
                    let mut x = 0i32;
                    /*Align the mask so that the word reads below are aligned too*/
                    while x < w && (base.wrapping_add(m + x as usize) & 0x3) != 0 {
                        let mv = mask[m + x as usize] as u32;
                        mask_fill_px(dest, px_ofs(row, x), mv, &mut color_argb, &mut cache);
                        x += 1;
                    }
                    while x <= w - 4 {
                        let mi = m + x as usize;
                        let mask32 = u32::from_ne_bytes([mask[mi], mask[mi + 1], mask[mi + 2], mask[mi + 3]]);
                        if mask32 == 0 {
                            /*Four transparent pixels*/
                            x += 4;
                            continue;
                        }
                        if mask32 == 0xFFFF_FFFF {
                            /*Four opaque pixels*/
                            for k in 0..4 {
                                set_px(dest, px_ofs(row, x + k), color_opaque);
                            }
                            x += 4;
                            continue;
                        }
                        /*Read the bytes again rather than split `mask32`, so the byte
                         *order doesn't matter*/
                        for k in 0..4 {
                            let mv = mask[mi + k as usize] as u32;
                            mask_fill_px(dest, px_ofs(row, x + k), mv, &mut color_argb, &mut cache);
                        }
                        x += 4;
                    }
                    while x < w {
                        let mv = mask[m + x as usize] as u32;
                        mask_fill_px(dest, px_ofs(row, x), mv, &mut color_argb, &mut cache);
                        x += 1;
                    }
                    row = drawbuf_next_row(row, dest_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
        }
        /*Masked with opacity*/
        Some(mask) => {
            let mut color_argb = dsc.color.to_32(opa);
            let mut row = 0usize;
            let mut m = 0usize;
            for _y in 0..h {
                for x in 0..w {
                    let mv = opa_mix2(mask[m + x as usize], opa) as u32;
                    mask_fill_px(dest, px_ofs(row, x), mv, &mut color_argb, &mut cache);
                }
                row = drawbuf_next_row(row, dest_stride);
                m = mask_next_row(m, mask_stride);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_argb8888()`: blend an image of any supported
/// source format.
pub fn image(dsc: &mut BlendImageDsc<'_>) {
    match dsc.src_color_format {
        #[cfg(feature = "cf-rgb565")]
        ColorFormat::Rgb565 => rgb565_image_blend(dsc),
        #[cfg(feature = "cf-rgb565-swapped")]
        ColorFormat::Rgb565Swapped => rgb565_swapped_image_blend(dsc),
        #[cfg(feature = "cf-rgb888")]
        ColorFormat::Rgb888 => rgb888_image_blend(dsc, 3),
        #[cfg(feature = "cf-xrgb8888")]
        ColorFormat::Xrgb8888 => rgb888_image_blend(dsc, 4),
        #[cfg(feature = "cf-argb8888-premultiplied")]
        ColorFormat::Argb8888Premultiplied => argb8888_premultiplied_image_blend(dsc),
        ColorFormat::Argb8888 => argb8888_image_blend(dsc),
        #[cfg(feature = "cf-l8")]
        ColorFormat::L8 => l8_image_blend(dsc),
        #[cfg(feature = "cf-al88")]
        ColorFormat::Al88 => al88_image_blend(dsc),
        #[cfg(feature = "cf-i1")]
        ColorFormat::I1 => i1_image_blend(dsc),
        /* LV_LOG_WARN("Not supported source color format") */
        _ => {}
    }
}

/// `i1_image_blend()`
#[cfg(feature = "cf-i1")]
fn i1_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let src = dsc.src_buf;
    let dest = &mut *dsc.dest_buf;

    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let chan_val = get_bit(&src[srow..], x).wrapping_mul(255);
                        set_px(
                            dest,
                            px_ofs(drow, x),
                            Color32::make(chan_val, chan_val, chan_val, chan_val),
                        );
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let chan_val = get_bit(&src[srow..], x).wrapping_mul(255);
                        color_8_32_mix(chan_val, dest, px_ofs(drow, x), opa);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let chan_val = get_bit(&src[srow..], x).wrapping_mul(255);
                        color_8_32_mix(chan_val, dest, px_ofs(drow, x), mask[m + x as usize]);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
            Some(mask) => {
                for _y in 0..h {
                    for x in 0..w {
                        let chan_val = get_bit(&src[srow..], x).wrapping_mul(255);
                        color_8_32_mix(
                            chan_val,
                            dest,
                            px_ofs(drow, x),
                            opa_mix2(mask[m + x as usize], opa),
                        );
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
        }
    } else {
        let mut cache = ColorMixAlphaCache::default();
        color_mix_with_alpha_cache_init(&mut cache);
        for _y in 0..h {
            for x in 0..w {
                let v = get_bit(&src[srow..], x).wrapping_mul(255);
                let alpha = match mask_buf {
                    None => opa,
                    Some(mask) => opa_mix2(mask[m + x as usize], opa),
                };
                let src_argb = Color32::make(v, v, v, alpha);
                blend_non_normal_pixel(dest, px_ofs(drow, x), src_argb, dsc.blend_mode, &mut cache);
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// `al88_image_blend()`
#[cfg(feature = "cf-al88")]
fn al88_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let src = dsc.src_buf;
    let dest = &mut *dsc.dest_buf;

    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;

    /* `lv_color16a_t` is `[lumi, alpha]` */
    let lumi = |srow: usize, x: i32| src[srow + x as usize * 2];
    let alpha = |srow: usize, x: i32| src[srow + x as usize * 2 + 1];

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_32_mix(lumi(srow, x), dest, px_ofs(drow, x), alpha(srow, x));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_32_mix(
                            lumi(srow, x),
                            dest,
                            px_ofs(drow, x),
                            opa_mix2(alpha(srow, x), opa),
                        );
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let mix = opa_mix2(alpha(srow, x), mask[m + x as usize]);
                        color_8_32_mix(lumi(srow, x), dest, px_ofs(drow, x), mix);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
            Some(mask) => {
                for _y in 0..h {
                    for x in 0..w {
                        let mix = opa_mix3(alpha(srow, x), mask[m + x as usize], opa);
                        color_8_32_mix(lumi(srow, x), dest, px_ofs(drow, x), mix);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
        }
    } else {
        let mut cache = ColorMixAlphaCache::default();
        color_mix_with_alpha_cache_init(&mut cache);
        for _y in 0..h {
            for x in 0..w {
                let l = lumi(srow, x);
                let a = match mask_buf {
                    None => opa_mix2(alpha(srow, x), opa),
                    Some(mask) => opa_mix3(alpha(srow, x), mask[m + x as usize], opa),
                };
                let src_argb = Color32::make(l, l, l, a);
                blend_non_normal_pixel(dest, px_ofs(drow, x), src_argb, dsc.blend_mode, &mut cache);
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// `l8_image_blend()`
#[cfg(feature = "cf-l8")]
fn l8_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let src = dsc.src_buf;
    let dest = &mut *dsc.dest_buf;

    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let l = src[srow + x as usize];
                        set_px(dest, px_ofs(drow, x), Color32::make(l, l, l, 0xff));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_32_mix(src[srow + x as usize], dest, px_ofs(drow, x), opa);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_32_mix(
                            src[srow + x as usize],
                            dest,
                            px_ofs(drow, x),
                            mask[m + x as usize],
                        );
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
            Some(mask) => {
                for _y in 0..h {
                    for x in 0..w {
                        let mix = opa_mix2(mask[m + x as usize], opa);
                        color_8_32_mix(src[srow + x as usize], dest, px_ofs(drow, x), mix);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
        }
    } else {
        let mut cache = ColorMixAlphaCache::default();
        color_mix_with_alpha_cache_init(&mut cache);
        for _y in 0..h {
            for x in 0..w {
                let l = src[srow + x as usize];
                let a = match mask_buf {
                    None => opa,
                    Some(mask) => opa_mix2(mask[m + x as usize], opa),
                };
                let src_argb = Color32::make(l, l, l, a);
                blend_non_normal_pixel(dest, px_ofs(drow, x), src_argb, dsc.blend_mode, &mut cache);
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// Read the native-endian `lv_color16_t` at pixel `x` of a row.
#[cfg(feature = "cf-rgb565")]
#[inline(always)]
fn get_c16(src: &[u8], row: usize, x: i32) -> Color16 {
    let i = row + x as usize * 2;
    Color16(u16::from_le_bytes([src[i], src[i + 1]]))
}

/// The rounded 5/6/5 to 8/8/8 channel expansion of the RGB565 paths
/// (`(c * 2106) >> 8` and `(c * 1037) >> 8`), into `c`.
#[cfg(any(feature = "cf-rgb565", feature = "cf-rgb565-swapped"))]
#[inline(always)]
fn c16_to_rgb(px: Color16, c: &mut Color32) {
    c.red = ((px.red() as u32 * 2106) >> 8) as u8; /*To make it rounded*/
    c.green = ((px.green() as u32 * 1037) >> 8) as u8;
    c.blue = ((px.blue() as u32 * 2106) >> 8) as u8;
}

/// `RGB565_TO_ARGB8888_MASK_PX(dest_px, src_px, mask_val)`: one masked pixel
/// of an RGB565 source. The colour conversion is only done once the pixel is
/// known to be visible.
#[cfg(feature = "cf-rgb565")]
#[inline(always)]
fn rgb565_to_argb8888_mask_px(
    dest: &mut [u8],
    i: usize,
    src_px: Color16,
    mask_val: u32,
    color_argb: &mut Color32,
    cache: &mut ColorMixAlphaCache,
) {
    let a = mask_val;
    if a > OPA_MIN as u32 {
        color_argb.alpha = a as u8;
        c16_to_rgb(src_px, color_argb);
        let d = get_px(dest, i);
        if a >= OPA_MAX as u32 || d.alpha <= OPA_MIN {
            set_px(dest, i, *color_argb);
        } else {
            set_px(dest, i, color_32_32_mix_mid(*color_argb, d, cache));
        }
    }
}

/// `rgb565_masked_rows()`: the masked rows of an RGB565 source.
#[cfg(feature = "cf-rgb565")]
fn rgb565_masked_rows(
    dest: &mut [u8],
    dest_stride: i32,
    src: &[u8],
    src_stride: i32,
    mask_buf: &[u8],
    mask_stride: i32,
    w: i32,
    h: i32,
    cache_p: &mut ColorMixAlphaCache,
) {
    let mut color_argb = Color32::default();
    let mut cache = *cache_p;
    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;
    let base = mask_buf.as_ptr() as usize;

    color_argb.alpha = 0xff;
    for _y in 0..h {
        /*The mask is the image's alpha plane, mostly 0 or 255. Wide rows test four mask bytes
         *with one word read.*/
        let mut x = 0i32;
        if w >= 32 {
            while x < w && (base.wrapping_add(m + x as usize) & 0x3) != 0 {
                let mv = mask_buf[m + x as usize] as u32;
                rgb565_to_argb8888_mask_px(
                    dest,
                    px_ofs(drow, x),
                    get_c16(src, srow, x),
                    mv,
                    &mut color_argb,
                    &mut cache,
                );
                x += 1;
            }
            while x <= w - 4 {
                let mi = m + x as usize;
                let m32 =
                    u32::from_ne_bytes([mask_buf[mi], mask_buf[mi + 1], mask_buf[mi + 2], mask_buf[mi + 3]]);
                if m32 == 0 {
                    x += 4;
                    continue;
                }
                for k in 0..4 {
                    let mv = mask_buf[mi + k as usize] as u32;
                    rgb565_to_argb8888_mask_px(
                        dest,
                        px_ofs(drow, x + k),
                        get_c16(src, srow, x + k),
                        mv,
                        &mut color_argb,
                        &mut cache,
                    );
                }
                x += 4;
            }
        }
        while x < w {
            let mv = mask_buf[m + x as usize] as u32;
            rgb565_to_argb8888_mask_px(
                dest,
                px_ofs(drow, x),
                get_c16(src, srow, x),
                mv,
                &mut color_argb,
                &mut cache,
            );
            x += 1;
        }
        drow = drawbuf_next_row(drow, dest_stride);
        srow = drawbuf_next_row(srow, src_stride);
        m = mask_next_row(m, mask_stride);
    }
    *cache_p = cache;
}

/// `rgb565_image_blend()`
#[cfg(feature = "cf-rgb565")]
fn rgb565_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let src = dsc.src_buf;
    let dest = &mut *dsc.dest_buf;

    let mut color_argb = Color32::default();
    let mut cache = ColorMixAlphaCache::default();
    color_mix_with_alpha_cache_init(&mut cache);

    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None => {
                color_argb.alpha = opa;
                if opa >= OPA_MAX {
                    /* Fast path, opaque foreground simply needs to be stored*/
                    for _y in 0..h {
                        for x in 0..w {
                            c16_to_rgb(get_c16(src, srow, x), &mut color_argb);
                            set_px(dest, px_ofs(drow, x), color_argb);
                        }
                        drow = drawbuf_next_row(drow, dest_stride);
                        srow = drawbuf_next_row(srow, src_stride);
                    }
                } else {
                    for _y in 0..h {
                        for x in 0..w {
                            c16_to_rgb(get_c16(src, srow, x), &mut color_argb);
                            let i = px_ofs(drow, x);
                            let d = get_px(dest, i);
                            set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
                        }
                        drow = drawbuf_next_row(drow, dest_stride);
                        srow = drawbuf_next_row(srow, src_stride);
                    }
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                rgb565_masked_rows(
                    dest,
                    dest_stride,
                    src,
                    src_stride,
                    mask,
                    mask_stride,
                    w,
                    h,
                    &mut cache,
                );
            }
            Some(mask) => {
                for _y in 0..h {
                    for x in 0..w {
                        color_argb.alpha = opa_mix2(mask[m + x as usize], opa);
                        c16_to_rgb(get_c16(src, srow, x), &mut color_argb);
                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
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
            for x in 0..w {
                c16_to_rgb(get_c16(src, srow, x), &mut src_argb);
                src_argb.alpha = match mask_buf {
                    None => opa,
                    Some(mask) => opa_mix2(mask[m + x as usize], opa),
                };
                blend_non_normal_pixel(dest, px_ofs(drow, x), src_argb, dsc.blend_mode, &mut cache);
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// `lv_color_swap_16(src_buf_u16[x])` + `lv_color16_from_u16()` of the
/// swapped source: the byte-swapped pixel `x` of a row.
#[cfg(feature = "cf-rgb565-swapped")]
#[inline(always)]
fn get_c16_swapped(src: &[u8], row: usize, x: i32) -> Color16 {
    let i = row + x as usize * 2;
    let raw = u16::from_le_bytes([src[i], src[i + 1]]).swap_bytes(); /* swap byte order */
    color16_from_u16(raw)
}

/// `rgb565_swapped_image_blend()`
#[cfg(feature = "cf-rgb565-swapped")]
fn rgb565_swapped_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let src = dsc.src_buf;
    let dest = &mut *dsc.dest_buf;

    let mut color_argb = Color32::default();
    let mut cache = ColorMixAlphaCache::default();
    color_mix_with_alpha_cache_init(&mut cache);

    let mut drow = 0usize;
    let mut srow = 0usize;
    let mut m = 0usize;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None => {
                color_argb.alpha = opa;
                if opa >= OPA_MAX {
                    /* Fast path, opaque foreground simply needs to be stored*/
                    for _y in 0..h {
                        for x in 0..w {
                            c16_to_rgb(get_c16_swapped(src, srow, x), &mut color_argb);
                            set_px(dest, px_ofs(drow, x), color_argb);
                        }
                        drow = drawbuf_next_row(drow, dest_stride);
                        srow = drawbuf_next_row(srow, src_stride);
                    }
                } else {
                    for _y in 0..h {
                        for x in 0..w {
                            c16_to_rgb(get_c16_swapped(src, srow, x), &mut color_argb);
                            let i = px_ofs(drow, x);
                            let d = get_px(dest, i);
                            set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
                        }
                        drow = drawbuf_next_row(drow, dest_stride);
                        srow = drawbuf_next_row(srow, src_stride);
                    }
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        color_argb.alpha = mask[m + x as usize];
                        c16_to_rgb(get_c16_swapped(src, srow, x), &mut color_argb);
                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
            Some(mask) => {
                for _y in 0..h {
                    for x in 0..w {
                        color_argb.alpha = opa_mix2(mask[m + x as usize], opa);
                        c16_to_rgb(get_c16_swapped(src, srow, x), &mut color_argb);
                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
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
            for x in 0..w {
                c16_to_rgb(get_c16_swapped(src, srow, x), &mut src_argb);
                src_argb.alpha = match mask_buf {
                    None => opa,
                    Some(mask) => opa_mix2(mask[m + x as usize], opa),
                };
                blend_non_normal_pixel(dest, px_ofs(drow, x), src_argb, dsc.blend_mode, &mut cache);
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
                    color_argb.red = src[s + 2];
                    color_argb.green = src[s + 1];
                    color_argb.blue = src[s];
                    let i = px_ofs(drow, dest_x);
                    let d = get_px(dest, i);
                    set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
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
                        color_argb.red = src[s + 2];
                        color_argb.green = src[s + 1];
                        color_argb.blue = src[s];
                        let i = px_ofs(drow, dest_x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
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
                        color_argb.alpha = ((opa as u32 * mask[m + dest_x as usize] as u32) >> 8) as u8;
                        color_argb.red = src[s + 2];
                        color_argb.green = src[s + 1];
                        color_argb.blue = src[s];
                        let i = px_ofs(drow, dest_x);
                        let d = get_px(dest, i);
                        set_px(dest, i, color_32_32_mix(color_argb, d, &mut cache));
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
                blend_non_normal_pixel(dest, px_ofs(drow, dest_x), src_argb, dsc.blend_mode, &mut cache);
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// The shared per-pixel step of the normal-mode `argb8888_image_blend()`
/// loops: "fast path opaque source or empty destination".
#[inline(always)]
fn argb8888_blend_px(dest: &mut [u8], i: usize, color_argb: Color32, cache: &mut ColorMixAlphaCache) {
    let d = get_px(dest, i);
    if color_argb.alpha >= OPA_MAX || d.alpha <= OPA_MIN {
        set_px(dest, i, color_argb);
    } else if color_argb.alpha > OPA_MIN {
        set_px(dest, i, color_32_32_mix_mid(color_argb, d, cache));
    }
}

/// `argb8888_image_blend()`
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
                        argb8888_blend_px(dest, px_ofs(drow, x), color_argb, &mut cache);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let mut color_argb = get_px(src, px_ofs(srow, x));
                        color_argb.alpha = opa_mix2(color_argb.alpha, opa);
                        argb8888_blend_px(dest, px_ofs(drow, x), color_argb, &mut cache);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let mut color_argb = get_px(src, px_ofs(srow, x));
                        color_argb.alpha = opa_mix2(color_argb.alpha, mask[m + x as usize]);
                        argb8888_blend_px(dest, px_ofs(drow, x), color_argb, &mut cache);
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
                        color_argb.alpha = opa_mix3(color_argb.alpha, opa, mask[m + x as usize]);
                        argb8888_blend_px(dest, px_ofs(drow, x), color_argb, &mut cache);
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
                color_argb.alpha = match mask_buf {
                    None => opa_mix2(color_argb.alpha, opa),
                    Some(mask) => opa_mix3(color_argb.alpha, mask[m + x as usize], opa),
                };
                blend_non_normal_pixel(dest, px_ofs(drow, x), color_argb, dsc.blend_mode, &mut cache);
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// `lv_color_32_32_mix_premult_scaled()`: blend a premultiplied foreground
/// pixel to a straight alpha background. `scale` is the extra opacity for the
/// channels, `ae` the matching effective alpha.
#[cfg(feature = "cf-argb8888-premultiplied")]
#[inline(always)]
fn color_32_32_mix_premult_scaled(
    mut fg_premult: Color32,
    mut bg: Color32,
    scale: u8,
    ae: u8,
    cache: &mut ColorMixAlphaCache,
) -> Color32 {
    if ae <= OPA_MIN {
        return bg;
    }

    if bg.alpha >= OPA_MAX {
        /*The premultiplied channels can be added as they are,
         *the background is weighted with the remaining alpha*/
        let ae_inv = 255u32 - ae as u32;
        if scale == 255 {
            bg.red = (fg_premult.red as u32).wrapping_add((bg.red as u32 * ae_inv) >> 8) as u8;
            bg.green = (fg_premult.green as u32).wrapping_add((bg.green as u32 * ae_inv) >> 8) as u8;
            bg.blue = (fg_premult.blue as u32).wrapping_add((bg.blue as u32 * ae_inv) >> 8) as u8;
        } else {
            bg.red =
                (opa_mix2(fg_premult.red, scale) as u32).wrapping_add((bg.red as u32 * ae_inv) >> 8) as u8;
            bg.green = (opa_mix2(fg_premult.green, scale) as u32)
                .wrapping_add((bg.green as u32 * ae_inv) >> 8) as u8;
            bg.blue =
                (opa_mix2(fg_premult.blue, scale) as u32).wrapping_add((bg.blue as u32 * ae_inv) >> 8) as u8;
        }
        return bg; /*The alpha of the background is kept*/
    }

    /*Semi-transparent background: unpremultiply and use the generic mix (rare case).
     *`fg_premult.alpha` can't be 0 here, the effective alpha was checked above.*/
    let reciprocal = ((255u32 * 256) / fg_premult.alpha as u32) as u16 as u32;
    fg_premult.red = ((fg_premult.red as u32 * reciprocal) >> 8) as u8;
    fg_premult.green = ((fg_premult.green as u32 * reciprocal) >> 8) as u8;
    fg_premult.blue = ((fg_premult.blue as u32 * reciprocal) >> 8) as u8;
    fg_premult.alpha = ae;
    color_32_32_mix(fg_premult, bg, cache)
}

/// `argb8888_premultiplied_image_blend()`
#[cfg(feature = "cf-argb8888-premultiplied")]
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

    /* Process the normal blend mode (for premultiplied alpha) */
    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let s = get_px(src, px_ofs(srow, x));
                        if s.alpha != 0 {
                            let i = px_ofs(drow, x);
                            let d = get_px(dest, i);
                            set_px(
                                dest,
                                i,
                                color_32_32_mix_premult_scaled(s, d, 255, s.alpha, &mut cache),
                            );
                        }
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let s = get_px(src, px_ofs(srow, x));
                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        let r = color_32_32_mix_premult_scaled(s, d, opa, opa_mix2(s.alpha, opa), &mut cache);
                        set_px(dest, i, r);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                }
            }
            Some(mask) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let s = get_px(src, px_ofs(srow, x));
                        let mv = mask[m + x as usize];
                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        let r = color_32_32_mix_premult_scaled(s, d, mv, opa_mix2(s.alpha, mv), &mut cache);
                        set_px(dest, i, r);
                    }
                    drow = drawbuf_next_row(drow, dest_stride);
                    srow = drawbuf_next_row(srow, src_stride);
                    m = mask_next_row(m, mask_stride);
                }
            }
            Some(mask) => {
                for _y in 0..h {
                    for x in 0..w {
                        let s = get_px(src, px_ofs(srow, x));
                        let mv = mask[m + x as usize];
                        let i = px_ofs(drow, x);
                        let d = get_px(dest, i);
                        let r = color_32_32_mix_premult_scaled(
                            s,
                            d,
                            opa_mix2(opa, mv),
                            opa_mix3(s.alpha, mv, opa),
                            &mut cache,
                        );
                        set_px(dest, i, r);
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
                /* Unpremultiply the source color by using the reciprocal of the alpha */
                let mut color_argb = get_px(src, px_ofs(srow, x));
                if color_argb.alpha != 0 {
                    let reciprocal_alpha = ((255u32 * 256) / color_argb.alpha as u32) as u16 as u32;
                    color_argb.red = ((color_argb.red as u32 * reciprocal_alpha) >> 8) as u8;
                    color_argb.green = ((color_argb.green as u32 * reciprocal_alpha) >> 8) as u8;
                    color_argb.blue = ((color_argb.blue as u32 * reciprocal_alpha) >> 8) as u8;
                }
                color_argb.alpha = match mask_buf {
                    None => opa_mix2(color_argb.alpha, opa),
                    Some(mask) => opa_mix3(color_argb.alpha, mask[m + x as usize], opa),
                };

                /* Blend with destination using non-normal blend mode */
                blend_non_normal_pixel(dest, px_ofs(drow, x), color_argb, dsc.blend_mode, &mut cache);
            }
            if mask_buf.is_some() {
                m = mask_next_row(m, mask_stride);
            }
            drow = drawbuf_next_row(drow, dest_stride);
            srow = drawbuf_next_row(srow, src_stride);
        }
    }
}

/// `lv_color_8_32_mix()`: mix a grey level into the pixel at byte offset `i`
/// (the result is always opaque).
#[cfg(any(feature = "cf-i1", feature = "cf-al88", feature = "cf-l8"))]
#[inline(always)]
fn color_8_32_mix(src: u8, dest: &mut [u8], i: usize, mix: u8) {
    if mix == 0 {
        return;
    }

    let mut d = get_px(dest, i);
    d.alpha = 255;
    if mix >= OPA_MAX {
        d.red = src;
        d.green = src;
        d.blue = src;
    } else {
        let mix_inv = (255 - mix) as u32;
        let s = src as u32 * mix as u32;
        d.red = ((s + d.red as u32 * mix_inv) >> 8) as u8;
        d.green = ((s + d.green as u32 * mix_inv) >> 8) as u8;
        d.blue = ((s + d.blue as u32 * mix_inv) >> 8) as u8;
    }
    set_px(dest, i, d);
}

/// `lv_color_32_32_mix_semi_transparent_bg()`: the alpha compositing case of
/// [`color_32_32_mix`], for a background that is partly transparent.
#[inline(never)]
fn color_32_32_mix_semi_transparent_bg(
    mut fg: Color32,
    bg: Color32,
    cache: &mut ColorMixAlphaCache,
) -> Color32 {
    /*Save the parameters and the result. If they will be asked again don't compute again*/

    /*Update the ratio and the result alpha value if the input alpha values change*/
    if bg.alpha != cache.bg_saved.alpha || fg.alpha != cache.fg_saved.alpha {
        /*Info:
         * https://en.wikipedia.org/wiki/Alpha_compositing#Analytical_derivation_of_the_over_operator*/
        cache.res_alpha_saved = 255 - opa_mix2(255 - fg.alpha, 255 - bg.alpha);
        debug_assert!(cache.res_alpha_saved != 0);
        cache.ratio_saved = ((fg.alpha as u32 * 255) / cache.res_alpha_saved as u32) as Opa;
    }

    if !Color32::eq(bg, cache.bg_saved) || !Color32::eq(fg, cache.fg_saved) {
        cache.fg_saved = fg;
        cache.bg_saved = bg;
        fg.alpha = cache.ratio_saved;
        cache.res_saved = Color32::mix32(fg, bg);
        cache.res_saved.alpha = cache.res_alpha_saved;
    }

    cache.res_saved
}

/// `lv_color_32_32_mix_mid()`: [`color_32_32_mix`] without its two leading
/// shortcuts, for the callers that already checked them.
#[inline(always)]
fn color_32_32_mix_mid(fg: Color32, bg: Color32, cache: &mut ColorMixAlphaCache) -> Color32 {
    /*Opaque background: use simple mix*/
    if bg.alpha >= OPA_MAX {
        return Color32::mix32(fg, bg);
    }

    /*Both colors have alpha: slow path*/
    color_32_32_mix_semi_transparent_bg(fg, bg, cache)
}

/// `lv_color_32_32_mix()`: "over" compositing of a straight-alpha `fg` onto a
/// straight-alpha `bg`.
#[inline(always)]
fn color_32_32_mix(fg: Color32, bg: Color32, cache: &mut ColorMixAlphaCache) -> Color32 {
    /*Pick the foreground if it's fully opaque or the Background is fully transparent*/
    if fg.alpha >= OPA_MAX || bg.alpha <= OPA_MIN {
        fg
    }
    /*Transparent foreground: use the Background*/
    else if fg.alpha <= OPA_MIN {
        bg
    }
    /*Opaque background: use simple mix*/
    else if bg.alpha >= OPA_MAX {
        Color32::mix32(fg, bg)
    }
    /*Both colors have alpha: slow path*/
    else {
        color_32_32_mix_semi_transparent_bg(fg, bg, cache)
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

/// `get_bit()`: bit `bit_idx` of an I1 row (MSB first).
#[cfg(feature = "cf-i1")]
#[inline(always)]
fn get_bit(buf: &[u8], bit_idx: i32) -> u8 {
    (buf[(bit_idx / 8) as usize] >> (7 - (bit_idx % 8))) & 1
}

/// `blend_non_normal_pixel()`: blend `src` into the pixel at byte offset `i`
/// with an additive / subtractive / multiply / difference blend mode.
#[inline(always)]
fn blend_non_normal_pixel(
    dest: &mut [u8],
    i: usize,
    src: Color32,
    mode: BlendMode,
    cache: &mut ColorMixAlphaCache,
) {
    let d = get_px(dest, i);
    let mut res = Color32::default();
    let (dr, dg, db) = (d.red as i32, d.green as i32, d.blue as i32);
    let (sr, sg, sb) = (src.red as i32, src.green as i32, src.blue as i32);
    match mode {
        BlendMode::Additive => {
            res.red = (dr + sr).min(255) as u8;
            res.green = (dg + sg).min(255) as u8;
            res.blue = (db + sb).min(255) as u8;
        }
        BlendMode::Subtractive => {
            res.red = (dr - sr).max(0) as u8;
            res.green = (dg - sg).max(0) as u8;
            res.blue = (db - sb).max(0) as u8;
        }
        BlendMode::Multiply => {
            res.red = ((dr * sr) >> 8) as u8;
            res.green = ((dg * sg) >> 8) as u8;
            res.blue = ((db * sb) >> 8) as u8;
        }
        BlendMode::Difference => {
            res.red = (dr - sr).unsigned_abs() as u8;
            res.green = (dg - sg).unsigned_abs() as u8;
            res.blue = (db - sb).unsigned_abs() as u8;
        }
        /* LV_LOG_WARN("Not supported blend mode") */
        BlendMode::Normal => return,
    }
    res.alpha = src.alpha;
    set_px(dest, i, color_32_32_mix(res, d, cache));
}

/// `drawbuf_next_row()`: advance a row offset by `stride` bytes.
#[inline(always)]
const fn drawbuf_next_row(row: usize, stride: i32) -> usize {
    row.wrapping_add(stride as u32 as usize)
}

/// `lv_color16_from_u16()`
#[cfg(feature = "cf-rgb565-swapped")]
#[inline(always)]
const fn color16_from_u16(raw: u16) -> Color16 {
    Color16::new(
        ((raw >> 11) & 0x1F) as u8,
        ((raw >> 5) & 0x3F) as u8,
        (raw & 0x1F) as u8,
    )
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

    fn fill_dsc<'a>(
        dest: &'a mut [u8],
        w: i32,
        h: i32,
        dest_stride: i32,
        mask: Option<&'a [u8]>,
        mask_stride: i32,
        color: Color,
        opa: Opa,
    ) -> BlendFillDsc<'a> {
        BlendFillDsc {
            dest_buf: dest,
            dest_w: w,
            dest_h: h,
            dest_stride,
            mask_buf: mask,
            mask_stride,
            color,
            opa,
            relative_area: Area::default(),
        }
    }

    fn img_dsc<'a>(
        dest: &'a mut [u8],
        w: i32,
        h: i32,
        src: &'a [u8],
        src_stride: i32,
        cf: ColorFormat,
        opa: Opa,
        mask: Option<&'a [u8]>,
        mode: BlendMode,
    ) -> BlendImageDsc<'a> {
        BlendImageDsc {
            dest_buf: dest,
            dest_w: w,
            dest_h: h,
            dest_stride: w * 4,
            mask_buf: mask,
            mask_stride: w,
            src_buf: src,
            src_stride,
            src_color_format: cf,
            opa,
            blend_mode: mode,
            relative_area: Area::default(),
            src_area: Area::default(),
        }
    }

    fn new_cache() -> ColorMixAlphaCache {
        let mut c = ColorMixAlphaCache::default();
        color_mix_with_alpha_cache_init(&mut c);
        c
    }

    #[test]
    fn mix_32_32_semantics() {
        let mut cache = new_cache();
        assert_eq!(cache.res_alpha_saved, 255);
        assert_eq!(cache.ratio_saved, 255);
        let blue = c32(0, 0, 255, 255);
        // Cover source wins.
        assert_eq!(
            color_32_32_mix(c32(1, 2, 3, 255), blue, &mut cache),
            c32(1, 2, 3, 255)
        );
        assert_eq!(
            color_32_32_mix(c32(1, 2, 3, OPA_MAX), blue, &mut cache),
            c32(1, 2, 3, OPA_MAX)
        );
        // Transparent destination: the foreground is stored as is.
        assert_eq!(
            color_32_32_mix(c32(9, 8, 7, 100), c32(0, 0, 255, 0), &mut cache),
            c32(9, 8, 7, 100)
        );
        // Transparent foreground keeps the background.
        assert_eq!(color_32_32_mix(c32(9, 8, 7, OPA_MIN), blue, &mut cache), blue);
        // Half opa over an opaque background: LV_UDIV255 mix, background alpha kept.
        assert_eq!(
            color_32_32_mix(c32(255, 0, 0, 128), blue, &mut cache),
            c32(128, 0, 127, 255)
        );
        // Both semi-transparent: res_alpha = 255 - MIX2(127, 127) = 192,
        // ratio = 128 * 255 / 192 = 170.
        let r = color_32_32_mix(c32(255, 0, 0, 128), c32(0, 0, 255, 128), &mut cache);
        assert_eq!(r, c32(170, 0, 85, 192));
        assert_eq!((cache.res_alpha_saved, cache.ratio_saved), (192, 170));
        // Cached result returned for the same inputs, recomputed for a new colour.
        let r2 = color_32_32_mix(c32(255, 0, 0, 128), c32(0, 0, 255, 128), &mut cache);
        assert_eq!(r2, r);
        let r3 = color_32_32_mix(c32(0, 255, 0, 128), c32(0, 0, 255, 128), &mut cache);
        assert_eq!(r3, c32(0, 170, 85, 192));
        // `mix_mid` skips the leading shortcuts only.
        assert_eq!(
            color_32_32_mix_mid(c32(255, 0, 0, 128), blue, &mut cache),
            c32(128, 0, 127, 255)
        );
    }

    #[test]
    fn fill_simple_with_padding_and_collapsed_rows() {
        // Stride of 4 pixels, width 3: the padding pixel must stay untouched.
        let pad = c32(0x11, 0x11, 0x11, 0x11);
        let mut buf = buf_of(&[pad; 8]);
        fill(&mut fill_dsc(&mut buf, 3, 2, 16, None, 0, RED, 255));
        let p = pxs_of(&buf);
        for row in 0..2 {
            for x in 0..3 {
                assert_eq!(p[row * 4 + x], c32(255, 0, 0, 255));
            }
            assert_eq!(p[row * 4 + 3], pad);
        }
        // stride == w * 4: treated as one long row (more than 16 pixels).
        let mut buf = vec![0u8; 5 * 4 * 4];
        fill(&mut fill_dsc(
            &mut buf,
            5,
            4,
            20,
            None,
            0,
            Color::make(1, 2, 3),
            OPA_MAX,
        ));
        assert!(pxs_of(&buf).iter().all(|&c| c == c32(1, 2, 3, 255)));
    }

    #[test]
    fn fill_opa_only() {
        let mut buf = buf_of(&[c32(0, 0, 255, 255), c32(0, 0, 255, 0), c32(0, 0, 255, 128)]);
        fill(&mut fill_dsc(&mut buf, 3, 1, 12, None, 0, RED, 128));
        let p = pxs_of(&buf);
        assert_eq!(p[0], c32(128, 0, 127, 255)); // opaque dest
        assert_eq!(p[1], c32(255, 0, 0, 128)); // transparent dest
        assert_eq!(p[2], c32(170, 0, 85, 192)); // semi-transparent dest
    }

    /// The per-pixel reference of the masked fill (the `w < 32` loop).
    fn masked_fill_reference(dest: &mut [u8], w: i32, h: i32, mask: &[u8], color: Color) {
        let mut cache = new_cache();
        let mut color_argb = color.to_32(0xff);
        for y in 0..h {
            for x in 0..w {
                let mv = mask[(y * w + x) as usize] as u32;
                mask_fill_px(dest, ((y * w + x) * 4) as usize, mv, &mut color_argb, &mut cache);
            }
        }
    }

    #[test]
    fn fill_mask_word_path_matches_per_pixel() {
        let w = 41;
        let h = 3;
        let n = (w * h) as usize;
        // Runs of 0 and 255 plus gradients, at several offsets to test the alignment loop.
        let mut storage = vec![0u8; n + 4];
        for i in 0..n + 3 {
            storage[i + 1] = match (i / 8) % 4 {
                0 => 0,
                1 => 255,
                2 => (i * 37 % 256) as u8,
                _ => (i % 3) as u8,
            };
        }
        let dest_init: Vec<Color32> = (0..n)
            .map(|i| {
                c32(
                    (i * 7) as u8,
                    (i * 13) as u8,
                    (i * 3) as u8,
                    [255, 0, 128, 1][i % 4],
                )
            })
            .collect();
        let color = Color::make(10, 200, 30);
        for offset in 0..4 {
            let mask = &storage[offset..offset + n];
            let mut expect = buf_of(&dest_init);
            masked_fill_reference(&mut expect, w, h, mask, color);
            let mut got = buf_of(&dest_init);
            fill(&mut fill_dsc(&mut got, w, h, w * 4, Some(mask), w, color, 255));
            assert_eq!(pxs_of(&got), pxs_of(&expect));
        }
        // Spot checks of the semantics.
        let mut d = buf_of(&[c32(0, 0, 255, 255); 3]);
        let mask = [0u8, 255, 128];
        fill(&mut fill_dsc(&mut d, 3, 1, 12, Some(&mask), 3, RED, 255));
        let p = pxs_of(&d);
        assert_eq!(p[0], c32(0, 0, 255, 255));
        assert_eq!(p[1], c32(255, 0, 0, 255));
        assert_eq!(p[2], c32(128, 0, 127, 255));
    }

    #[test]
    fn fill_mask_and_opa() {
        let mut d = buf_of(&[c32(0, 0, 255, 255), c32(0, 0, 255, 255), c32(0, 0, 0, 0)]);
        let mask = [255u8, 4, 255];
        fill(&mut fill_dsc(&mut d, 3, 1, 12, Some(&mask), 3, RED, 128));
        let p = pxs_of(&d);
        // MIX2(255, 128) = 127
        assert_eq!(p[0], c32(127, 0, 128, 255));
        // MIX2(4, 128) = 2 <= LV_OPA_MIN: untouched
        assert_eq!(p[1], c32(0, 0, 255, 255));
        // Transparent destination: stored with the effective alpha
        assert_eq!(p[2], c32(255, 0, 0, 127));
    }

    #[test]
    fn image_argb8888_normal() {
        let src = buf_of(&[
            c32(255, 0, 0, 255),
            c32(255, 0, 0, 128),
            c32(255, 0, 0, 0),
            c32(255, 0, 0, 128),
        ]);
        let dst0 = [
            c32(0, 0, 255, 255),
            c32(0, 0, 255, 255),
            c32(0, 0, 255, 255),
            c32(0, 0, 255, 0),
        ];
        let mut d = buf_of(&dst0);
        image(&mut img_dsc(
            &mut d,
            4,
            1,
            &src,
            16,
            ColorFormat::Argb8888,
            255,
            None,
            BlendMode::Normal,
        ));
        let p = pxs_of(&d);
        assert_eq!(p[0], c32(255, 0, 0, 255)); // cover source
        assert_eq!(p[1], c32(128, 0, 127, 255)); // half alpha
        assert_eq!(p[2], c32(0, 0, 255, 255)); // transparent source
        assert_eq!(p[3], c32(255, 0, 0, 128)); // transparent destination

        // Opacity: MIX2(255, 128) = 127
        let mut d = buf_of(&dst0);
        image(&mut img_dsc(
            &mut d,
            4,
            1,
            &src,
            16,
            ColorFormat::Argb8888,
            128,
            None,
            BlendMode::Normal,
        ));
        assert_eq!(pxs_of(&d)[0], c32(127, 0, 128, 255));

        // Mask: MIX2(255, 128) = 127. An empty destination takes the source even
        // with a zero effective alpha (upstream's fast path).
        let mask = [128u8, 255, 255, 0];
        let mut d = buf_of(&dst0);
        image(&mut img_dsc(
            &mut d,
            4,
            1,
            &src,
            16,
            ColorFormat::Argb8888,
            255,
            Some(&mask),
            BlendMode::Normal,
        ));
        let p = pxs_of(&d);
        assert_eq!(p[0], c32(127, 0, 128, 255));
        assert_eq!(p[3], c32(255, 0, 0, 0));

        // Mask and opa: MIX3(255, 128, 128) = 63
        let mut d = buf_of(&dst0);
        image(&mut img_dsc(
            &mut d,
            4,
            1,
            &src,
            16,
            ColorFormat::Argb8888,
            128,
            Some(&mask),
            BlendMode::Normal,
        ));
        assert_eq!(pxs_of(&d)[0], c32(63, 0, 192, 255));
    }

    #[cfg(feature = "cf-rgb565")]
    #[test]
    fn image_rgb565() {
        let src: Vec<u8> = [0xFFFFu16, 0xF800, 0x07E0, 0x001F]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut d = vec![0u8; 16];
        image(&mut img_dsc(
            &mut d,
            4,
            1,
            &src,
            8,
            ColorFormat::Rgb565,
            255,
            None,
            BlendMode::Normal,
        ));
        let p = pxs_of(&d);
        assert_eq!(
            p,
            [
                c32(255, 255, 255, 255),
                c32(255, 0, 0, 255),
                c32(0, 255, 0, 255),
                c32(0, 0, 255, 255)
            ]
        );

        // Masked rows, wide enough for the word path.
        let w = 40;
        let src: Vec<u8> = (0..w)
            .flat_map(|i| (0xF800u16 | (i as u16 & 0x1F)).to_le_bytes())
            .collect();
        let mask: Vec<u8> = (0..w).map(|i| [0u8, 255, 128, 3][(i / 4) % 4]).collect();
        let dst0 = vec![c32(0, 0, 255, 255); w];
        let mut d = buf_of(&dst0);
        let wi = w as i32;
        image(&mut img_dsc(
            &mut d,
            wi,
            1,
            &src,
            2 * wi,
            ColorFormat::Rgb565,
            255,
            Some(&mask),
            BlendMode::Normal,
        ));
        let p = pxs_of(&d);
        for i in 0..w {
            let s = c32(255, 0, ((((i as u32) & 0x1F) * 2106) >> 8) as u8, mask[i]);
            let expect = if mask[i] <= OPA_MIN {
                dst0[i]
            } else if mask[i] >= OPA_MAX {
                s
            } else {
                Color32::mix32(s, dst0[i])
            };
            assert_eq!(p[i], expect, "px {i}");
        }
    }

    #[cfg(all(feature = "cf-rgb565", feature = "cf-rgb565-swapped"))]
    #[test]
    fn image_rgb565_swapped_matches_rgb565() {
        let vals = [0x1234u16, 0xF81F, 0x07E0, 0xABCD];
        let le: Vec<u8> = vals.iter().flat_map(|v| v.to_le_bytes()).collect();
        let be: Vec<u8> = vals.iter().flat_map(|v| v.to_be_bytes()).collect();
        for opa in [255u8, 100] {
            let mut a = buf_of(&[c32(5, 6, 7, 200); 4]);
            let mut b = a.clone();
            image(&mut img_dsc(
                &mut a,
                4,
                1,
                &le,
                8,
                ColorFormat::Rgb565,
                opa,
                None,
                BlendMode::Normal,
            ));
            image(&mut img_dsc(
                &mut b,
                4,
                1,
                &be,
                8,
                ColorFormat::Rgb565Swapped,
                opa,
                None,
                BlendMode::Normal,
            ));
            assert_eq!(a, b);
        }
    }

    #[cfg(all(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
    #[test]
    fn image_rgb888_and_xrgb8888() {
        let src3 = [1u8, 2, 3, 4, 5, 6];
        let mut d = vec![0u8; 8];
        image(&mut img_dsc(
            &mut d,
            2,
            1,
            &src3,
            6,
            ColorFormat::Rgb888,
            255,
            None,
            BlendMode::Normal,
        ));
        assert_eq!(pxs_of(&d), [c32(3, 2, 1, 255), c32(6, 5, 4, 255)]);
        // XRGB8888 opaque: plain row copy (the X byte included).
        let src4 = [1u8, 2, 3, 0x77, 4, 5, 6, 0x88];
        let mut d = vec![0u8; 8];
        image(&mut img_dsc(
            &mut d,
            2,
            1,
            &src4,
            8,
            ColorFormat::Xrgb8888,
            255,
            None,
            BlendMode::Normal,
        ));
        assert_eq!(d, src4);
        // With opacity over an opaque background.
        let src = [0u8, 0, 255, 0xff];
        let mut d = buf_of(&[c32(0, 0, 255, 255)]);
        image(&mut img_dsc(
            &mut d,
            1,
            1,
            &src,
            4,
            ColorFormat::Xrgb8888,
            128,
            None,
            BlendMode::Normal,
        ));
        assert_eq!(pxs_of(&d)[0], c32(128, 0, 127, 255));
        // Mask and opa: (opa * mask) >> 8 = 127
        let mask = [255u8];
        let mut d = buf_of(&[c32(0, 0, 255, 255)]);
        image(&mut img_dsc(
            &mut d,
            1,
            1,
            &src[..3],
            3,
            ColorFormat::Rgb888,
            128,
            Some(&mask),
            BlendMode::Normal,
        ));
        assert_eq!(pxs_of(&d)[0], c32(127, 0, 128, 255));
    }

    #[cfg(all(feature = "cf-l8", feature = "cf-al88", feature = "cf-i1"))]
    #[test]
    fn image_grey_sources() {
        // L8 opaque: stored, alpha 255.
        let mut d = buf_of(&[c32(0, 0, 0, 0); 2]);
        image(&mut img_dsc(
            &mut d,
            2,
            1,
            &[10, 200],
            2,
            ColorFormat::L8,
            255,
            None,
            BlendMode::Normal,
        ));
        assert_eq!(pxs_of(&d), [c32(10, 10, 10, 255), c32(200, 200, 200, 255)]);
        // L8 with opa: (src * mix + dest * (255 - mix)) >> 8, alpha forced to 255.
        let mut d = buf_of(&[c32(0, 0, 100, 7)]);
        image(&mut img_dsc(
            &mut d,
            1,
            1,
            &[200],
            1,
            ColorFormat::L8,
            128,
            None,
            BlendMode::Normal,
        ));
        let b = ((200u32 * 128 + 100 * 127) >> 8) as u8;
        assert_eq!(pxs_of(&d)[0], c32(100, 100, b, 255));
        // AL88: alpha 0 leaves the destination alone.
        let mut d = buf_of(&[c32(1, 2, 3, 4), c32(1, 2, 3, 4)]);
        image(&mut img_dsc(
            &mut d,
            2,
            1,
            &[50, 0, 60, 255],
            4,
            ColorFormat::Al88,
            255,
            None,
            BlendMode::Normal,
        ));
        assert_eq!(pxs_of(&d), [c32(1, 2, 3, 4), c32(60, 60, 60, 255)]);
        // I1: MSB first, 0 or 255 in every channel incl. alpha.
        let mut d = vec![0u8; 9 * 4];
        let src = [0b1010_0000u8, 0b1000_0000];
        image(&mut img_dsc(
            &mut d,
            9,
            1,
            &src,
            2,
            ColorFormat::I1,
            255,
            None,
            BlendMode::Normal,
        ));
        let p = pxs_of(&d);
        assert_eq!(p[0], c32(255, 255, 255, 255));
        assert_eq!(p[1], c32(0, 0, 0, 0));
        assert_eq!(p[2], c32(255, 255, 255, 255));
        assert_eq!(p[8], c32(255, 255, 255, 255));
    }

    #[test]
    fn blend_modes() {
        let dst = c32(100, 200, 50, 255);
        let src = buf_of(&[c32(200, 100, 60, 255)]);
        let run = |mode| {
            let mut d = buf_of(&[dst]);
            image(&mut img_dsc(
                &mut d,
                1,
                1,
                &src,
                4,
                ColorFormat::Argb8888,
                255,
                None,
                mode,
            ));
            pxs_of(&d)[0]
        };
        // MIX2(255, 255) = 254 >= LV_OPA_MAX: the result is stored with that alpha.
        assert_eq!(run(BlendMode::Additive), c32(255, 255, 110, 254));
        assert_eq!(run(BlendMode::Subtractive), c32(0, 100, 0, 254));
        assert_eq!(run(BlendMode::Multiply), c32(78, 78, 11, 254));
        assert_eq!(run(BlendMode::Difference), c32(100, 100, 10, 254));
        // A transparent source alpha keeps the destination.
        let src0 = buf_of(&[c32(200, 100, 60, 255)]);
        let mut d = buf_of(&[dst]);
        image(&mut img_dsc(
            &mut d,
            1,
            1,
            &src0,
            4,
            ColorFormat::Argb8888,
            0,
            None,
            BlendMode::Additive,
        ));
        assert_eq!(pxs_of(&d)[0], dst);
    }

    #[cfg(feature = "cf-argb8888-premultiplied")]
    #[test]
    fn image_premultiplied_source() {
        let src = buf_of(&[c32(128, 0, 0, 128), c32(128, 0, 0, 128), c32(9, 9, 9, 0)]);
        let mut d = buf_of(&[c32(0, 0, 255, 255), c32(0, 0, 255, 0), c32(1, 2, 3, 4)]);
        let cf = ColorFormat::Argb8888Premultiplied;
        image(&mut img_dsc(
            &mut d,
            3,
            1,
            &src,
            12,
            cf,
            255,
            None,
            BlendMode::Normal,
        ));
        let p = pxs_of(&d);
        // Opaque background: fg + (bg * (255 - ae)) >> 8
        assert_eq!(p[0], c32(128, 0, 126, 255));
        // Transparent background: unpremultiplied (128 * 510) >> 8 = 255
        assert_eq!(p[1], c32(255, 0, 0, 128));
        // Zero alpha source: skipped
        assert_eq!(p[2], c32(1, 2, 3, 4));
        // With opa 128 on an opaque background: MIX2(128, 128) + (255 * (255 - 64)) >> 8
        let mut d = buf_of(&[c32(0, 0, 255, 255)]);
        image(&mut img_dsc(
            &mut d,
            1,
            1,
            &src[..4],
            4,
            cf,
            128,
            None,
            BlendMode::Normal,
        ));
        assert_eq!(pxs_of(&d)[0], c32(64, 0, 190, 255));
    }
}
