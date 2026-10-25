//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_rgb565.c`.
//!
//! Colour fills and image blending into an RGB565 destination buffer.
//!
//! The destination buffer is a byte slice holding little-endian `u16`
//! pixels (the reference platform's native order); pixels are accessed
//! with `from_le_bytes` / `to_le_bytes`. Rows advance by the byte strides
//! exactly like upstream `drawbuf_next_row()`.
//!
//! None of the `LV_DRAW_SW_*_TO_RGB565*` acceleration hooks exist here, so
//! every `LV_RESULT_INVALID == HOOK(dsc)` test of upstream is always true and
//! the software path always runs.
//!
//! Upstream reads the source of the non-normal blend modes of the L8, AL88
//! and I1 images with a pixel step of 4 (`src_x += 4`), which walks past the
//! end of the row. That quirk is kept for parity; reads that would fall
//! outside of the source slice return 0 instead of the neighbouring memory.

#[cfg(any(
    feature = "cf-al88",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
use ferrovgl_core::misc::color::opa_mix3;
use ferrovgl_core::misc::color::{Color16, ColorFormat, OPA_MAX, Opa, opa_mix2};
use ferrovgl_core::misc::style::BlendMode;

use crate::blend::{
    BlendFillDsc, BlendImageDsc, color_16_16_mix, color_24_to_16, color_mix_16_prepare,
    color_mix_16_to_16_prepared,
};

/*********************
 *  BYTE VIEW HELPERS
 *********************/

/// Read the RGB565 pixel at byte offset `off`.
#[inline(always)]
fn rd16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

/// Write the RGB565 pixel at byte offset `off`.
#[inline(always)]
fn wr16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

/// Write two RGB565 pixels at once (`lv_draw_sw_word_t::u32`).
#[inline(always)]
fn wr32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// The three bytes `[b, g, r]` at byte offset `off`.
#[cfg_attr(
    not(any(
        feature = "cf-rgb888",
        feature = "cf-xrgb8888",
        feature = "cf-argb8888",
        feature = "cf-argb8888-premultiplied"
    )),
    allow(dead_code)
)]
#[inline(always)]
fn px24(buf: &[u8], off: usize) -> [u8; 3] {
    [buf[off], buf[off + 1], buf[off + 2]]
}

/// A source byte that may lie past the end of the slice (the `src_x += 4`
/// reads of upstream); 0 outside.
#[cfg_attr(
    not(any(feature = "cf-i1", feature = "cf-al88", feature = "cf-l8")),
    allow(dead_code)
)]
#[inline(always)]
fn rd8_or0(buf: &[u8], off: usize) -> u8 {
    buf.get(off).copied().unwrap_or(0)
}

/// `drawbuf_next_row()`: advance a byte offset by `stride`.
#[inline(always)]
fn drawbuf_next_row(off: usize, stride: i32) -> usize {
    off.wrapping_add_signed(stride as isize)
}

/// The fields of an `lv_color16_t` bit-field view of a pixel, promoted to
/// `int` like C does on every use.
#[derive(Copy, Clone)]
struct Color16Fields {
    red: i32,
    green: i32,
    blue: i32,
}

/// Bit-field view of an RGB565 value (`(lv_color16_t *)` cast, and
/// `lv_color16_from_u16()` of upstream).
#[inline(always)]
fn color16_fields(raw: u16) -> Color16Fields {
    Color16Fields {
        red: ((raw >> 11) & 0x1F) as i32,
        green: ((raw >> 5) & 0x3F) as i32,
        blue: (raw & 0x1F) as i32,
    }
}

/// `res = r << 11; res += g << 5; res += b;` with `res` an `uint16_t`.
#[inline(always)]
fn pack_res(r: i32, g: i32, b: i32) -> u16 {
    let mut res: u16 = (r << 11) as u16;
    res = (res as i32 + (g << 5)) as u16;
    res = (res as i32 + b) as u16;
    res
}

/// `LV_ABS(x)`
#[inline(always)]
fn lv_abs(x: i32) -> i32 {
    if x > 0 { x } else { x.wrapping_neg() }
}

/**********************
 *   GLOBAL FUNCTIONS
 **********************/

/// `RGB565_MASK_FILL_PX(dest_px, mask_val)`: leave the pixel alone, store
/// the colour, or mix.
#[inline(always)]
fn rgb565_mask_fill_px(buf: &mut [u8], off: usize, mask_val: u8, color16: u16, fg_prep: u32) {
    let a = mask_val as u32;
    if a != 0 {
        if a >= OPA_MAX as u32 {
            wr16(buf, off, color16);
        } else {
            let d = rd16(buf, off);
            wr16(buf, off, color_mix_16_to_16_prepared(fg_prep, d, a as u8));
        }
    }
}

/// The cache of `RGB565_OPA_FILL_PX(dest_px)`: the last background and its
/// result, kept as a pair.
struct OpaFillCache {
    last_dest_color: u16,
    last_res32_color: u32,
}

impl OpaFillCache {
    /// `RGB565_OPA_FILL_PX(dest_px)`
    #[inline(always)]
    fn px(&mut self, buf: &mut [u8], off: usize, color16: u16, opa: Opa) {
        let d = rd16(buf, off);
        if self.last_dest_color != d {
            let res = Color16::mix(color16, d, opa);
            self.last_dest_color = d;
            self.last_res32_color = (res as u32) | ((res as u32) << 16);
        }
        wr16(buf, off, self.last_res32_color as u16);
    }
}

/// `lv_draw_sw_blend_color_to_rgb565()`: fill an area with a colour.
///
/// Supports normal fill, fill with opacity, fill with mask, and fill with
/// mask and opacity. The background (`dest_buf`) cannot have an alpha
/// channel.
pub fn fill(dsc: &mut BlendFillDsc<'_>) {
    let mut w = dsc.dest_w;
    let mut h = dsc.dest_h;
    let color16 = dsc.color.to_u16();
    let opa = dsc.opa;
    let mask = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_stride = dsc.dest_stride;
    let buf: &mut [u8] = &mut *dsc.dest_buf;
    let mut dest: usize = 0;

    /*The fill color is the same for the whole area, so expand it once*/
    let fg_prep = color_mix_16_prepare(color16);

    match mask {
        /*Simple fill*/
        None if opa >= OPA_MAX => {
            /* Treat the buffer as one long row when stride matches width*/
            if dest_stride == w.wrapping_mul(2) {
                w = w.wrapping_mul(h);
                h = 1;
            }

            for _y in 0..h {
                /*Upstream aligns to a word, stores 8 words unrolled, then
                 *word pairs, then the last pixel; every store is `color16`.*/
                for x in 0..w {
                    wr16(buf, dest + 2 * x as usize, color16);
                }
                dest = drawbuf_next_row(dest, dest_stride);
            }
        }
        /*Opacity only*/
        None => {
            /*Flat areas are common, so cache the last background and its result: repeated
             *pixels then cost a compare instead of a mix. Seeded one below the first pixel so
             *the first compare always misses and the entry is valid from then on.*/
            let first = if buf.len() >= 2 { rd16(buf, 0) } else { 0 };
            let mut cache = OpaFillCache {
                last_dest_color: first.wrapping_sub(1),
                last_res32_color: 0,
            };

            for _y in 0..h {
                let mut x: i32 = 0;
                if (buf.as_ptr() as usize).wrapping_add(dest) & 0x3 != 0 {
                    cache.px(buf, dest, color16, opa);
                    x = 1;
                }

                while x < w.wrapping_sub(2) {
                    let p = dest + 2 * x as usize;
                    if rd16(buf, p) == rd16(buf, p + 2) {
                        /*Both pixels are equal so use a u32 write*/
                        cache.px(buf, p, color16, opa);
                        wr32(buf, p, cache.last_res32_color);
                    } else {
                        cache.px(buf, p, color16, opa);
                        cache.px(buf, p + 2, color16, opa);
                    }
                    x += 2;
                }

                while x < w {
                    cache.px(buf, dest + 2 * x as usize, color16, opa);
                    x += 1;
                }
                dest = drawbuf_next_row(dest, dest_stride);
            }
        }
        /*Masked with full opacity*/
        Some(mask) if opa >= OPA_MAX => {
            let mut moff: usize = 0;
            /*Masks are mostly 0 or 255, so both are decided before the mix. Narrow rows go
             *per pixel; wide ones have long runs, worth testing four mask bytes at once.*/
            if w < 32 {
                for _y in 0..h {
                    /*Upstream counts up to zero from -w; the pixel order is the same*/
                    let mut i: i32 = 0;
                    if w & 0x1 != 0 {
                        let iu = i as usize;
                        rgb565_mask_fill_px(buf, dest + 2 * iu, mask[moff + iu], color16, fg_prep);
                        i += 1;
                    }
                    while i < w {
                        let iu = i as usize;
                        rgb565_mask_fill_px(buf, dest + 2 * iu, mask[moff + iu], color16, fg_prep);
                        rgb565_mask_fill_px(buf, dest + 2 * (iu + 1), mask[moff + iu + 1], color16, fg_prep);
                        i += 2;
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            } else {
                for _y in 0..h {
                    /*Align the mask so that the word reads below are aligned too*/
                    let mut x: i32 = 0;
                    while x < w && (mask.as_ptr() as usize).wrapping_add(moff + x as usize) & 0x3 != 0 {
                        let xu = x as usize;
                        rgb565_mask_fill_px(buf, dest + 2 * xu, mask[moff + xu], color16, fg_prep);
                        x += 1;
                    }
                    while x <= w - 4 {
                        let xu = x as usize;
                        x += 4;
                        let m = moff + xu;
                        let mask32 = u32::from_ne_bytes([mask[m], mask[m + 1], mask[m + 2], mask[m + 3]]);
                        if mask32 == 0 {
                            continue; /*Four transparent pixels*/
                        }
                        if mask32 == 0xFFFF_FFFF {
                            /*Four opaque pixels*/
                            wr16(buf, dest + 2 * xu, color16);
                            wr16(buf, dest + 2 * (xu + 1), color16);
                            wr16(buf, dest + 2 * (xu + 2), color16);
                            wr16(buf, dest + 2 * (xu + 3), color16);
                            continue;
                        }
                        /*Read the bytes again rather than split `mask32`, so the byte
                         *order doesn't matter*/
                        rgb565_mask_fill_px(buf, dest + 2 * xu, mask[m], color16, fg_prep);
                        rgb565_mask_fill_px(buf, dest + 2 * (xu + 1), mask[m + 1], color16, fg_prep);
                        rgb565_mask_fill_px(buf, dest + 2 * (xu + 2), mask[m + 2], color16, fg_prep);
                        rgb565_mask_fill_px(buf, dest + 2 * (xu + 3), mask[m + 3], color16, fg_prep);
                    }
                    while x < w {
                        let xu = x as usize;
                        rgb565_mask_fill_px(buf, dest + 2 * xu, mask[moff + xu], color16, fg_prep);
                        x += 1;
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
        /*Masked with opacity*/
        Some(mask) => {
            let mut moff: usize = 0;
            for _y in 0..h {
                for x in 0..w {
                    let xu = x as usize;
                    /*The product can't reach 255, so there is no opaque shortcut here*/
                    let a = opa_mix2(mask[moff + xu], opa) as u32;
                    if a == 0 {
                        continue;
                    }
                    let p = dest + 2 * xu;
                    let d = rd16(buf, p);
                    wr16(buf, p, color_mix_16_to_16_prepared(fg_prep, d, a as u8));
                }
                dest = drawbuf_next_row(dest, dest_stride);
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_rgb565()`: blend an image of any supported
/// source format into an RGB565 buffer. Unsupported formats are ignored
/// (upstream only logs a warning).
pub fn image(dsc: &mut BlendImageDsc<'_>) {
    match dsc.src_color_format {
        ColorFormat::Rgb565 => rgb565_image_blend(dsc),
        #[cfg(feature = "cf-rgb565-swapped")]
        ColorFormat::Rgb565Swapped => rgb565_swapped_image_blend(dsc),
        #[cfg(feature = "cf-rgb888")]
        ColorFormat::Rgb888 => rgb888_image_blend(dsc, 3),
        #[cfg(feature = "cf-xrgb8888")]
        ColorFormat::Xrgb8888 => rgb888_image_blend(dsc, 4),
        #[cfg(feature = "cf-argb8888")]
        ColorFormat::Argb8888 => argb8888_image_blend(dsc),
        #[cfg(feature = "cf-argb8888-premultiplied")]
        ColorFormat::Argb8888Premultiplied => argb8888_premultiplied_image_blend(dsc),
        #[cfg(feature = "cf-l8")]
        ColorFormat::L8 => l8_image_blend(dsc),
        #[cfg(feature = "cf-al88")]
        ColorFormat::Al88 => al88_image_blend(dsc),
        #[cfg(feature = "cf-i1")]
        ColorFormat::I1 => i1_image_blend(dsc),
        _ => { /*LV_LOG_WARN("Not supported source color format")*/ }
    }
}

/**********************
 *   STATIC FUNCTIONS
 **********************/

/// The `switch(dsc->blend_mode)` of the L8 / AL88 images (`rb = lumi >> 3`,
/// `g = lumi >> 2` against the destination bit-fields). `None` is the
/// `default:` case that returns from the blend.
#[cfg(any(feature = "cf-l8", feature = "cf-al88"))]
#[inline(always)]
fn blend_mode_lumi(mode: BlendMode, d: u16, lumi: u8) -> Option<u16> {
    let dc = color16_fields(d);
    let rb = (lumi >> 3) as i32;
    let g = (lumi >> 2) as i32;
    Some(match mode {
        BlendMode::Additive => pack_res(
            (dc.red + rb).min(31),
            (dc.green + g).min(63),
            (dc.blue + rb).min(31),
        ),
        BlendMode::Subtractive => {
            pack_res((dc.red - rb).max(0), (dc.green - g).max(0), (dc.blue - rb).max(0))
        }
        BlendMode::Multiply => pack_res((dc.red * rb) >> 5, (dc.green * g) >> 6, (dc.blue * rb) >> 5),
        BlendMode::Difference => pack_res(lv_abs(dc.red - rb), lv_abs(dc.green - g), lv_abs(dc.blue - rb)),
        BlendMode::Normal => return None,
    })
}

/// The `switch(dsc->blend_mode)` of the RGB888 / XRGB8888 / ARGB8888
/// images: `s` is `[b, g, r]`. `None` is the `default:` case.
#[cfg(any(
    feature = "cf-rgb888",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
#[inline(always)]
fn blend_mode_24(mode: BlendMode, d: u16, s: [u8; 3]) -> Option<u16> {
    let dc = color16_fields(d);
    let sr = (s[2] >> 3) as i32;
    let sg = (s[1] >> 2) as i32;
    let sb = (s[0] >> 3) as i32;
    Some(match mode {
        BlendMode::Additive => pack_res(
            (dc.red + sr).min(31),
            (dc.green + sg).min(63),
            (dc.blue + sb).min(31),
        ),
        BlendMode::Subtractive => pack_res(
            (dc.red - sr).max(0),
            (dc.green - sg).max(0),
            (dc.blue - sb).max(0),
        ),
        BlendMode::Multiply => pack_res((dc.red * sr) >> 5, (dc.green * sg) >> 6, (dc.blue * sb) >> 5),
        BlendMode::Difference => pack_res(lv_abs(dc.red - sr), lv_abs(dc.green - sg), lv_abs(dc.blue - sb)),
        BlendMode::Normal => return None,
    })
}

/// Outcome of one pixel of the `switch(dsc->blend_mode)` of the RGB565
/// images.
enum ModeRes {
    /// `res` was computed.
    Res(u16),
    /// `continue`: the pixel is left alone.
    Skip,
    /// `default:`: the whole blend returns.
    Return,
}

/// The `switch(dsc->blend_mode)` of the RGB565 / RGB565_SWAPPED images.
/// `raw` is the source value as stored (used for the black / white skips)
/// and `px` its decoded colour.
#[inline(always)]
fn blend_mode_565(mode: BlendMode, d: u16, raw: u16, px: Color16Fields) -> ModeRes {
    let dc = color16_fields(d);
    match mode {
        BlendMode::Additive => {
            if raw == 0x0000 {
                return ModeRes::Skip; /*Do not add pure black*/
            }
            ModeRes::Res(pack_res(
                (dc.red + px.red).min(31),
                (dc.green + px.green).min(63),
                (dc.blue + px.blue).min(31),
            ))
        }
        BlendMode::Subtractive => {
            if raw == 0x0000 {
                return ModeRes::Skip; /*Do not subtract pure black*/
            }
            ModeRes::Res(pack_res(
                (dc.red - px.red).max(0),
                (dc.green - px.green).max(0),
                (dc.blue - px.blue).max(0),
            ))
        }
        BlendMode::Multiply => {
            if raw == 0xffff {
                return ModeRes::Skip; /*Do not multiply with pure white (considered as 1)*/
            }
            ModeRes::Res(pack_res(
                (dc.red * px.red) >> 5,
                (dc.green * px.green) >> 6,
                (dc.blue * px.blue) >> 5,
            ))
        }
        BlendMode::Difference => ModeRes::Res(pack_res(
            lv_abs(dc.red - px.red),
            lv_abs(dc.green - px.green),
            lv_abs(dc.blue - px.blue),
        )),
        BlendMode::Normal => ModeRes::Return,
    }
}

/// `i1_image_blend()`
#[cfg(feature = "cf-i1")]
fn i1_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_buf: &mut [u8] = &mut *dsc.dest_buf;
    let dest_stride = dsc.dest_stride;
    let src_buf_i1 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;

    let mut dest: usize = 0;
    let mut src: usize = 0;
    let mut moff: usize = 0;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let src_x = dest_x;
                        let chan_val = get_bit(src_buf_i1, src, src_x).wrapping_mul(255);
                        wr16(dest_buf, dest + 2 * dest_x as usize, l8_to_rgb565(chan_val));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let src_x = dest_x;
                        let chan_val = get_bit(src_buf_i1, src, src_x).wrapping_mul(255);
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(dest_buf, p, lv_color_8_16_mix(chan_val, d, opa));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let src_x = dest_x;
                        /*Images and masks are mostly fully transparent or fully opaque*/
                        let a = mask_buf[moff + dest_x as usize] as u32;
                        if a == 0 {
                            continue;
                        }
                        let chan_val = get_bit(src_buf_i1, src, src_x).wrapping_mul(255);
                        let p = dest + 2 * dest_x as usize;
                        if a >= OPA_MAX as u32 {
                            wr16(dest_buf, p, l8_to_rgb565(chan_val));
                        } else {
                            let d = rd16(dest_buf, p);
                            wr16(dest_buf, p, lv_color_8_16_mix(chan_val, d, a as u8));
                        }
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let src_x = dest_x;
                        let a = opa_mix2(mask_buf[moff + dest_x as usize], opa) as u32;
                        if a == 0 {
                            continue;
                        }
                        let chan_val = get_bit(src_buf_i1, src, src_x).wrapping_mul(255);
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(dest_buf, p, lv_color_8_16_mix(chan_val, d, a as u8));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            let mut src_x: i32 = 0;
            for dest_x in 0..w {
                let p = dest + 2 * dest_x as usize;
                let d = rd16(dest_buf, p);
                let chan_val = get_bit_or0(src_buf_i1, src, src_x).wrapping_mul(255);
                let c = l8_to_rgb565(chan_val) as i32;
                let res: u16 = match dsc.blend_mode {
                    /*Additive blending mode*/
                    BlendMode::Additive => (d as i32 + c).min(0xFFFF) as u16,
                    /*Subtractive blending mode*/
                    BlendMode::Subtractive => (d as i32 - c).max(0) as u16,
                    /*Multiply blending mode*/
                    BlendMode::Multiply => {
                        (((((d >> 11) as i32 * (c >> 3)) & 0x1F) << 11)
                            | (((((d >> 5) & 0x3F) as i32 * ((c >> 2) & 0x3F)) >> 6) << 5)
                            | (((d & 0x1F) as i32 * (c & 0x1F)) >> 5)) as u16
                    }
                    /*Difference blending mode*/
                    BlendMode::Difference => lv_abs(d as i32 - c) as u16,
                    BlendMode::Normal => {
                        /*LV_LOG_WARN("Not supported blend mode")*/
                        return;
                    }
                };

                match mask_buf {
                    None if opa >= OPA_MAX => wr16(dest_buf, p, res),
                    None => wr16(dest_buf, p, color_16_16_mix(res, d, opa)),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_16_16_mix(res, d, m));
                        } else {
                            wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix2(m, opa)));
                        }
                    }
                }
                src_x = src_x.wrapping_add(4);
            }

            dest = drawbuf_next_row(dest, dest_stride);
            src = drawbuf_next_row(src, src_stride);
            if mask_buf.is_some() {
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `al88_image_blend()`: the source holds `lv_color16a_t` `{lumi, alpha}`
/// byte pairs.
#[cfg(feature = "cf-al88")]
fn al88_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_buf: &mut [u8] = &mut *dsc.dest_buf;
    let dest_stride = dsc.dest_stride;
    let src_buf_al88 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;

    let mut dest: usize = 0;
    let mut src: usize = 0;
    let mut moff: usize = 0;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 2 * dest_x as usize;
                        /*Images and masks are mostly fully transparent or fully opaque*/
                        let a = src_buf_al88[s + 1] as u32;
                        if a == 0 {
                            continue;
                        }
                        let lumi = src_buf_al88[s];
                        let p = dest + 2 * dest_x as usize;
                        if a >= OPA_MAX as u32 {
                            wr16(dest_buf, p, l8_to_rgb565(lumi));
                        } else {
                            let d = rd16(dest_buf, p);
                            wr16(dest_buf, p, lv_color_8_16_mix(lumi, d, a as u8));
                        }
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 2 * dest_x as usize;
                        let alpha = src_buf_al88[s + 1];
                        if alpha == 0 {
                            continue;
                        }
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(
                            dest_buf,
                            p,
                            lv_color_8_16_mix(src_buf_al88[s], d, opa_mix2(alpha, opa)),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 2 * dest_x as usize;
                        let alpha = src_buf_al88[s + 1];
                        let m = mask_buf[moff + dest_x as usize];
                        if alpha == 0 || m == 0 {
                            continue;
                        }
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(
                            dest_buf,
                            p,
                            lv_color_8_16_mix(src_buf_al88[s], d, opa_mix2(alpha, m)),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 2 * dest_x as usize;
                        let alpha = src_buf_al88[s + 1];
                        let m = mask_buf[moff + dest_x as usize];
                        if alpha == 0 || m == 0 {
                            continue;
                        }
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(
                            dest_buf,
                            p,
                            lv_color_8_16_mix(src_buf_al88[s], d, opa_mix3(alpha, m, opa)),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            let mut src_x: usize = 0;
            for dest_x in 0..w {
                let p = dest + 2 * dest_x as usize;
                let d = rd16(dest_buf, p);
                let s = src.wrapping_add(2 * src_x);
                let lumi = rd8_or0(src_buf_al88, s);
                let alpha = rd8_or0(src_buf_al88, s.wrapping_add(1));
                let Some(res) = blend_mode_lumi(dsc.blend_mode, d, lumi) else {
                    /*LV_LOG_WARN("Not supported blend mode")*/
                    return;
                };
                match mask_buf {
                    None if opa >= OPA_MAX => wr16(dest_buf, p, color_16_16_mix(res, d, alpha)),
                    None => wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix2(opa, alpha))),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_16_16_mix(res, d, m));
                        } else {
                            wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix3(m, opa, alpha)));
                        }
                    }
                }
                src_x = src_x.wrapping_add(4);
            }

            dest = drawbuf_next_row(dest, dest_stride);
            src = drawbuf_next_row(src, src_stride);
            if mask_buf.is_some() {
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `l8_image_blend()`
#[cfg(feature = "cf-l8")]
fn l8_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_buf: &mut [u8] = &mut *dsc.dest_buf;
    let dest_stride = dsc.dest_stride;
    let src_buf_l8 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;

    let mut dest: usize = 0;
    let mut src: usize = 0;
    let mut moff: usize = 0;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src_buf_l8[src + dest_x as usize];
                        wr16(dest_buf, dest + 2 * dest_x as usize, l8_to_rgb565(s));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src_buf_l8[src + dest_x as usize];
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(dest_buf, p, lv_color_8_16_mix(s, d, opa));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        /*Images and masks are mostly fully transparent or fully opaque*/
                        let a = mask_buf[moff + dest_x as usize] as u32;
                        if a == 0 {
                            continue;
                        }
                        let s = src_buf_l8[src + dest_x as usize];
                        let p = dest + 2 * dest_x as usize;
                        if a >= OPA_MAX as u32 {
                            wr16(dest_buf, p, l8_to_rgb565(s));
                        } else {
                            let d = rd16(dest_buf, p);
                            wr16(dest_buf, p, lv_color_8_16_mix(s, d, a as u8));
                        }
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let a = opa_mix2(mask_buf[moff + dest_x as usize], opa) as u32;
                        if a == 0 {
                            continue;
                        }
                        let s = src_buf_l8[src + dest_x as usize];
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(dest_buf, p, lv_color_8_16_mix(s, d, a as u8));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            let mut src_x: usize = 0;
            for dest_x in 0..w {
                let p = dest + 2 * dest_x as usize;
                let d = rd16(dest_buf, p);
                let lumi = rd8_or0(src_buf_l8, src.wrapping_add(src_x));
                let Some(res) = blend_mode_lumi(dsc.blend_mode, d, lumi) else {
                    /*LV_LOG_WARN("Not supported blend mode")*/
                    return;
                };

                match mask_buf {
                    None if opa >= OPA_MAX => wr16(dest_buf, p, res),
                    None => wr16(dest_buf, p, color_16_16_mix(res, d, opa)),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_16_16_mix(res, d, m));
                        } else {
                            wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix2(m, opa)));
                        }
                    }
                }
                src_x = src_x.wrapping_add(4);
            }

            dest = drawbuf_next_row(dest, dest_stride);
            src = drawbuf_next_row(src, src_stride);
            if mask_buf.is_some() {
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// One pixel of the masked, fully opaque RGB565 image blend.
#[inline(always)]
fn rgb565_mask_px(dest_buf: &mut [u8], p: usize, s: u16, a: u32) {
    if a >= OPA_MAX as u32 {
        wr16(dest_buf, p, s);
    } else if a != 0 {
        let d = rd16(dest_buf, p);
        wr16(dest_buf, p, color_16_16_mix(s, d, a as u8));
    }
}

/// `rgb565_image_blend()`
fn rgb565_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_buf: &mut [u8] = &mut *dsc.dest_buf;
    let dest_stride = dsc.dest_stride;
    let src_buf_u16 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;

    let mut dest: usize = 0;
    let mut src: usize = 0;
    let mut moff: usize = 0;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                let line_in_bytes = (w as u32).wrapping_mul(2) as usize;
                for _y in 0..h {
                    dest_buf[dest..dest + line_in_bytes]
                        .copy_from_slice(&src_buf_u16[src..src + line_in_bytes]);
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let xu = x as usize;
                        let p = dest + 2 * xu;
                        let s = rd16(src_buf_u16, src + 2 * xu);
                        let d = rd16(dest_buf, p);
                        wr16(dest_buf, p, color_16_16_mix(s, d, opa));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    /*The mask here is the image's own alpha plane, which like a glyph mask is
                     *mostly 0 or 255, so both are decided before the mix. Wide rows test four
                     *mask bytes with one word read.*/
                    let mut x: i32 = 0;
                    if w >= 32 {
                        while x < w && (mask_buf.as_ptr() as usize).wrapping_add(moff + x as usize) & 0x3 != 0
                        {
                            let xu = x as usize;
                            x += 1;
                            let a = mask_buf[moff + xu] as u32;
                            if a == 0 {
                                continue;
                            }
                            rgb565_mask_px(dest_buf, dest + 2 * xu, rd16(src_buf_u16, src + 2 * xu), a);
                        }
                        while x <= w - 4 {
                            let xu = x as usize;
                            x += 4;
                            let m = moff + xu;
                            let m32 = u32::from_ne_bytes([
                                mask_buf[m],
                                mask_buf[m + 1],
                                mask_buf[m + 2],
                                mask_buf[m + 3],
                            ]);
                            if m32 == 0 {
                                continue;
                            }
                            for k in 0..4 {
                                let a = mask_buf[m + k] as u32;
                                rgb565_mask_px(
                                    dest_buf,
                                    dest + 2 * (xu + k),
                                    rd16(src_buf_u16, src + 2 * (xu + k)),
                                    a,
                                );
                            }
                        }
                    }
                    while x < w {
                        let xu = x as usize;
                        x += 1;
                        let a = mask_buf[moff + xu] as u32;
                        if a == 0 {
                            continue;
                        }
                        rgb565_mask_px(dest_buf, dest + 2 * xu, rd16(src_buf_u16, src + 2 * xu), a);
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        let xu = x as usize;
                        let p = dest + 2 * xu;
                        let s = rd16(src_buf_u16, src + 2 * xu);
                        let d = rd16(dest_buf, p);
                        wr16(
                            dest_buf,
                            p,
                            color_16_16_mix(s, d, opa_mix2(mask_buf[moff + xu], opa)),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                let xu = x as usize;
                let p = dest + 2 * xu;
                let d = rd16(dest_buf, p);
                let s = rd16(src_buf_u16, src + 2 * xu);
                let res = match blend_mode_565(dsc.blend_mode, d, s, color16_fields(s)) {
                    ModeRes::Res(res) => res,
                    ModeRes::Skip => continue,
                    ModeRes::Return => {
                        /*LV_LOG_WARN("Not supported blend mode")*/
                        return;
                    }
                };

                match mask_buf {
                    None => wr16(dest_buf, p, color_16_16_mix(res, d, opa)),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + xu];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_16_16_mix(res, d, m));
                        } else {
                            wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix2(m, opa)));
                        }
                    }
                }
            }

            dest = drawbuf_next_row(dest, dest_stride);
            src = drawbuf_next_row(src, src_stride);
            if mask_buf.is_some() {
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `lv_color_swap_16()`
#[cfg(feature = "cf-rgb565-swapped")]
#[inline(always)]
const fn color_swap_16(c: u16) -> u16 {
    c.rotate_left(8)
}

/// `rgb565_swapped_image_blend()`
#[cfg(feature = "cf-rgb565-swapped")]
fn rgb565_swapped_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_buf: &mut [u8] = &mut *dsc.dest_buf;
    let dest_stride = dsc.dest_stride;
    let src_buf_u16 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;

    let mut dest: usize = 0;
    let mut src: usize = 0;
    let mut moff: usize = 0;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                let line_in_bytes = (w as u32).wrapping_mul(2) as usize;
                for _y in 0..h {
                    dest_buf[dest..dest + line_in_bytes]
                        .copy_from_slice(&src_buf_u16[src..src + line_in_bytes]);
                    ferrovgl_core::draw::utils::rgb565_swap(
                        &mut dest_buf[dest..dest + line_in_bytes],
                        w as u32,
                    );
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let xu = x as usize;
                        let p = dest + 2 * xu;
                        let s = color_swap_16(rd16(src_buf_u16, src + 2 * xu));
                        let d = rd16(dest_buf, p);
                        wr16(dest_buf, p, color_16_16_mix(s, d, opa));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let xu = x as usize;
                        let p = dest + 2 * xu;
                        let s = color_swap_16(rd16(src_buf_u16, src + 2 * xu));
                        let d = rd16(dest_buf, p);
                        wr16(dest_buf, p, color_16_16_mix(s, d, mask_buf[moff + xu]));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        let xu = x as usize;
                        let p = dest + 2 * xu;
                        let s = color_swap_16(rd16(src_buf_u16, src + 2 * xu));
                        let d = rd16(dest_buf, p);
                        wr16(
                            dest_buf,
                            p,
                            color_16_16_mix(s, d, opa_mix2(mask_buf[moff + xu], opa)),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                let xu = x as usize;
                let p = dest + 2 * xu;
                let d = rd16(dest_buf, p);
                let stored = rd16(src_buf_u16, src + 2 * xu);
                let raw = color_swap_16(stored); /* swap byte order */
                let px = color16_fields(raw);

                let res = match blend_mode_565(dsc.blend_mode, d, stored, px) {
                    ModeRes::Res(res) => res,
                    ModeRes::Skip => continue,
                    ModeRes::Return => {
                        /*LV_LOG_WARN("Not supported blend mode")*/
                        return;
                    }
                };

                match mask_buf {
                    None => wr16(dest_buf, p, color_16_16_mix(res, d, opa)),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + xu];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_16_16_mix(res, d, m));
                        } else {
                            wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix2(m, opa)));
                        }
                    }
                }
            }

            dest = drawbuf_next_row(dest, dest_stride);
            src = drawbuf_next_row(src, src_stride);
            if mask_buf.is_some() {
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `rgb888_image_blend()`: RGB888 (`src_px_size = 3`) or XRGB8888
/// (`src_px_size = 4`) source.
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
fn rgb888_image_blend(dsc: &mut BlendImageDsc<'_>, src_px_size: u8) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_buf: &mut [u8] = &mut *dsc.dest_buf;
    let dest_stride = dsc.dest_stride;
    let src_buf_u8 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let px_size = src_px_size as usize;

    let mut dest: usize = 0;
    let mut src: usize = 0;
    let mut moff: usize = 0;

    if dsc.blend_mode == BlendMode::Normal {
        if mask_buf.is_none() && opa >= OPA_MAX {
            for _y in 0..h {
                for dest_x in 0..w {
                    let s = src + dest_x as usize * px_size;
                    wr16(
                        dest_buf,
                        dest + 2 * dest_x as usize,
                        color_24_to_16(px24(src_buf_u8, s)),
                    );
                }
                dest = drawbuf_next_row(dest, dest_stride);
                src = drawbuf_next_row(src, src_stride);
            }
        } else if mask_buf.is_none() && opa < OPA_MAX {
            for _y in 0..h {
                for dest_x in 0..w {
                    let s = src + dest_x as usize * px_size;
                    let p = dest + 2 * dest_x as usize;
                    let d = rd16(dest_buf, p);
                    wr16(dest_buf, p, lv_color_24_16_mix(px24(src_buf_u8, s), d, opa));
                }
                dest = drawbuf_next_row(dest, dest_stride);
                src = drawbuf_next_row(src, src_stride);
            }
        }
        /*Upstream starts a new `if` here (not `else if`); the conditions exclude
         *each other anyway*/
        if let Some(mask_buf) = mask_buf
            && opa >= OPA_MAX
        {
            for _y in 0..h {
                for dest_x in 0..w {
                    /*Images and masks are mostly fully transparent or fully opaque*/
                    let a = mask_buf[moff + dest_x as usize] as u32;
                    if a == 0 {
                        continue;
                    }
                    let s = src + dest_x as usize * px_size;
                    let p = dest + 2 * dest_x as usize;
                    if a >= OPA_MAX as u32 {
                        wr16(dest_buf, p, color_24_to_16(px24(src_buf_u8, s)));
                    } else {
                        let d = rd16(dest_buf, p);
                        wr16(dest_buf, p, lv_color_24_16_mix(px24(src_buf_u8, s), d, a as u8));
                    }
                }
                dest = drawbuf_next_row(dest, dest_stride);
                src = drawbuf_next_row(src, src_stride);
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
        if let Some(mask_buf) = mask_buf
            && opa < OPA_MAX
        {
            for _y in 0..h {
                for dest_x in 0..w {
                    let a = opa_mix2(mask_buf[moff + dest_x as usize], opa) as u32;
                    if a == 0 {
                        continue;
                    }
                    let s = src + dest_x as usize * px_size;
                    let p = dest + 2 * dest_x as usize;
                    let d = rd16(dest_buf, p);
                    wr16(dest_buf, p, lv_color_24_16_mix(px24(src_buf_u8, s), d, a as u8));
                }
                dest = drawbuf_next_row(dest, dest_stride);
                src = drawbuf_next_row(src, src_stride);
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            for dest_x in 0..w {
                let s = src + dest_x as usize * px_size;
                let p = dest + 2 * dest_x as usize;
                let d = rd16(dest_buf, p);
                let Some(res) = blend_mode_24(dsc.blend_mode, d, px24(src_buf_u8, s)) else {
                    /*LV_LOG_WARN("Not supported blend mode")*/
                    return;
                };

                match mask_buf {
                    None => wr16(dest_buf, p, color_16_16_mix(res, d, opa)),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_16_16_mix(res, d, m));
                        } else {
                            wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix2(m, opa)));
                        }
                    }
                }
            }
            dest = drawbuf_next_row(dest, dest_stride);
            src = drawbuf_next_row(src, src_stride);
            if mask_buf.is_some() {
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `argb8888_image_blend()`
#[cfg(feature = "cf-argb8888")]
fn argb8888_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_buf: &mut [u8] = &mut *dsc.dest_buf;
    let dest_stride = dsc.dest_stride;
    let src_buf_u8 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;

    let mut dest: usize = 0;
    let mut src: usize = 0;
    let mut moff: usize = 0;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 4 * dest_x as usize;
                        /*Images are mostly fully transparent or fully opaque*/
                        let a = src_buf_u8[s + 3] as u32;
                        if a == 0 {
                            continue;
                        }
                        let p = dest + 2 * dest_x as usize;
                        if a >= OPA_MAX as u32 {
                            wr16(dest_buf, p, color_24_to_16(px24(src_buf_u8, s)));
                        } else {
                            let d = rd16(dest_buf, p);
                            wr16(dest_buf, p, lv_color_24_16_mix(px24(src_buf_u8, s), d, a as u8));
                        }
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 4 * dest_x as usize;
                        /*Opa can't make a transparent pixel visible, nor an opaque one
                         *fully opaque, so only the transparent case is worth checking*/
                        if src_buf_u8[s + 3] == 0 {
                            continue;
                        }
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(
                            dest_buf,
                            p,
                            lv_color_24_16_mix(px24(src_buf_u8, s), d, opa_mix2(src_buf_u8[s + 3], opa)),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 4 * dest_x as usize;
                        let m = mask_buf[moff + dest_x as usize];
                        if src_buf_u8[s + 3] == 0 || m == 0 {
                            continue;
                        }
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(
                            dest_buf,
                            p,
                            lv_color_24_16_mix(px24(src_buf_u8, s), d, opa_mix2(src_buf_u8[s + 3], m)),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 4 * dest_x as usize;
                        let m = mask_buf[moff + dest_x as usize];
                        if src_buf_u8[s + 3] == 0 || m == 0 {
                            continue;
                        }
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        wr16(
                            dest_buf,
                            p,
                            lv_color_24_16_mix(px24(src_buf_u8, s), d, opa_mix3(src_buf_u8[s + 3], m, opa)),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for dest_x in 0..w {
                let s = src + 4 * dest_x as usize;
                let p = dest + 2 * dest_x as usize;
                let d = rd16(dest_buf, p);
                let Some(res) = blend_mode_24(dsc.blend_mode, d, px24(src_buf_u8, s)) else {
                    /*LV_LOG_WARN("Not supported blend mode")*/
                    return;
                };
                let alpha = src_buf_u8[s + 3];

                match mask_buf {
                    None if opa >= OPA_MAX => wr16(dest_buf, p, color_16_16_mix(res, d, alpha)),
                    None => wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix2(opa, alpha))),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_16_16_mix(res, d, m));
                        } else {
                            wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix3(m, opa, alpha)));
                        }
                    }
                }
            }

            dest = drawbuf_next_row(dest, dest_stride);
            src = drawbuf_next_row(src, src_stride);
            if mask_buf.is_some() {
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `lv_color_24_16_mix_premult()`: `c1` is a premultiplied `[b, g, r]`
/// pixel, `mix` its (scaled) alpha.
#[cfg(feature = "cf-argb8888-premultiplied")]
#[inline(always)]
fn lv_color_24_16_mix_premult(c1: [u8; 3], c2: u16, mix: u8) -> u16 {
    if mix == 0 {
        c2
    } else if mix == 255 {
        color_24_to_16(c1)
    } else {
        let mix_inv: Opa = 255 - mix;

        let r = ((c1[2] >> 3) as u32 + ((((c2 as u32 >> 11) & 0x1F) * mix_inv as u32) >> 8)) as u8;
        let g = ((c1[1] >> 2) as u32 + ((((c2 as u32 >> 5) & 0x3F) * mix_inv as u32) >> 8)) as u8;
        let b = ((c1[0] >> 3) as u32 + ((((c2 as u32) & 0x1F) * mix_inv as u32) >> 8)) as u8;

        (((r as u32) << 11) + ((g as u32) << 5) + (b as u32)) as u16
    }
}

/// `argb8888_premultiplied_image_blend()`
#[cfg(feature = "cf-argb8888-premultiplied")]
fn argb8888_premultiplied_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_buf: &mut [u8] = &mut *dsc.dest_buf;
    let dest_stride = dsc.dest_stride;
    let src_buf_u8 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;

    let mut dest: usize = 0;
    let mut src: usize = 0;
    let mut moff: usize = 0;

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 4 * dest_x as usize;
                        let p = dest + 2 * dest_x as usize;
                        let d = rd16(dest_buf, p);
                        /*For the trivial case use the premultiplied image as it is.
                         *For the other cases unpremultiply as another alpha also needs to be applied.*/
                        wr16(
                            dest_buf,
                            p,
                            lv_color_24_16_mix_premult(px24(src_buf_u8, s), d, src_buf_u8[s + 3]),
                        );
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 4 * dest_x as usize;
                        if src_buf_u8[s + 3] > 0 {
                            /*No need to unpremultiply: scale the premultiplied channels by `opa` and
                             *blend the background with the remaining (255 - alpha * opa) weight*/
                            let src_scaled = [
                                opa_mix2(src_buf_u8[s], opa),
                                opa_mix2(src_buf_u8[s + 1], opa),
                                opa_mix2(src_buf_u8[s + 2], opa),
                            ];
                            let p = dest + 2 * dest_x as usize;
                            let d = rd16(dest_buf, p);
                            wr16(
                                dest_buf,
                                p,
                                lv_color_24_16_mix_premult(src_scaled, d, opa_mix2(src_buf_u8[s + 3], opa)),
                            );
                        }
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 4 * dest_x as usize;
                        if src_buf_u8[s + 3] > 0 {
                            /*No need to unpremultiply: scale the premultiplied channels by the mask and
                             *blend the background with the remaining (255 - alpha * mask) weight*/
                            let m = mask_buf[moff + dest_x as usize];
                            let src_scaled = [
                                opa_mix2(src_buf_u8[s], m),
                                opa_mix2(src_buf_u8[s + 1], m),
                                opa_mix2(src_buf_u8[s + 2], m),
                            ];
                            let p = dest + 2 * dest_x as usize;
                            let d = rd16(dest_buf, p);
                            wr16(
                                dest_buf,
                                p,
                                lv_color_24_16_mix_premult(src_scaled, d, opa_mix2(src_buf_u8[s + 3], m)),
                            );
                        }
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let s = src + 4 * dest_x as usize;
                        if src_buf_u8[s + 3] > 0 {
                            /*No need to unpremultiply: scale the premultiplied channels by mask * opa and
                             *blend the background with the remaining (255 - alpha * mask * opa) weight*/
                            let m = mask_buf[moff + dest_x as usize];
                            let scale = opa_mix2(m, opa);
                            let src_scaled = [
                                opa_mix2(src_buf_u8[s], scale),
                                opa_mix2(src_buf_u8[s + 1], scale),
                                opa_mix2(src_buf_u8[s + 2], scale),
                            ];
                            let p = dest + 2 * dest_x as usize;
                            let d = rd16(dest_buf, p);
                            /*Combine the three factors in one step to keep the precision*/
                            wr16(
                                dest_buf,
                                p,
                                lv_color_24_16_mix_premult(
                                    src_scaled,
                                    d,
                                    opa_mix3(src_buf_u8[s + 3], m, opa),
                                ),
                            );
                        }
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for dest_x in 0..w {
                let s = src + 4 * dest_x as usize;
                let p = dest + 2 * dest_x as usize;
                let d = rd16(dest_buf, p);
                let Some(res) = blend_mode_24(dsc.blend_mode, d, px24(src_buf_u8, s)) else {
                    /*LV_LOG_WARN("Not supported blend mode")*/
                    return;
                };
                let alpha = src_buf_u8[s + 3];

                match mask_buf {
                    /* Blending premultiplied ARGB8888 to RGB565 with no mask and full opacity */
                    None if opa >= OPA_MAX => wr16(dest_buf, p, color_16_16_mix(res, d, alpha)),
                    /* Blending premultiplied ARGB8888 to RGB565 with no mask and partial opacity */
                    None => wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix2(opa, alpha))),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_16_16_mix(res, d, m));
                        } else {
                            wr16(dest_buf, p, color_16_16_mix(res, d, opa_mix3(m, opa, alpha)));
                        }
                    }
                }
            }

            dest = drawbuf_next_row(dest, dest_stride);
            src = drawbuf_next_row(src, src_stride);
            if mask_buf.is_some() {
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `l8_to_rgb565()`
#[cfg_attr(
    not(any(feature = "cf-i1", feature = "cf-al88", feature = "cf-l8")),
    allow(dead_code)
)]
#[inline(always)]
fn l8_to_rgb565(c1: u8) -> u16 {
    let c1 = c1 as u32;
    (((c1 & 0xF8) << 8) + ((c1 & 0xFC) << 3) + ((c1 & 0xF8) >> 3)) as u16
}

/// `lv_color_8_16_mix()`
///
/// No shortcuts for a fully transparent, fully opaque or equal foreground:
/// the callers pick those pixels off before calling. The maths is correct
/// for 0 and 255 anyway (`mix5` becomes 0 or 32).
#[cfg_attr(
    not(any(feature = "cf-i1", feature = "cf-al88", feature = "cf-l8")),
    allow(dead_code)
)]
#[inline(always)]
fn lv_color_8_16_mix(c1: u8, c2: u16, mix: u8) -> u16 {
    let c1_16 = l8_to_rgb565(c1);

    /*Spelled out instead of calling lv_color_16_16_mix_inlined()*/
    let mix5 = ((mix as u32) + 4) >> 3;
    let bg = color_mix_16_prepare(c2);
    let fg = color_mix_16_prepare(c1_16);
    let result = ((fg.wrapping_sub(bg).wrapping_mul(mix5) >> 5).wrapping_add(bg)) & 0x07E0_F81F;
    ((result >> 16) | result) as u16
}

/// `lv_color_24_16_mix()`: `c1` is `[b, g, r]`.
///
/// No shortcuts, see [`lv_color_8_16_mix`].
#[cfg_attr(
    not(any(feature = "cf-rgb888", feature = "cf-xrgb8888", feature = "cf-argb8888")),
    allow(dead_code)
)]
#[inline(always)]
fn lv_color_24_16_mix(c1: [u8; 3], c2: u16, mix: u8) -> u16 {
    let c1_16 = color_24_to_16(c1);

    /*Spelled out instead of calling lv_color_16_16_mix_inlined()*/
    let mix5 = ((mix as u32) + 4) >> 3;
    let bg = color_mix_16_prepare(c2);
    let fg = color_mix_16_prepare(c1_16);
    let result = ((fg.wrapping_sub(bg).wrapping_mul(mix5) >> 5).wrapping_add(bg)) & 0x07E0_F81F;
    ((result >> 16) | result) as u16
}

/// `get_bit()`: bit `bit_idx` (MSB first) of the row starting at byte
/// offset `row`.
#[cfg(feature = "cf-i1")]
#[inline(always)]
fn get_bit(buf: &[u8], row: usize, bit_idx: i32) -> u8 {
    (buf[row + (bit_idx / 8) as usize] >> (7 - (bit_idx % 8))) & 1
}

/// [`get_bit`] for the `src_x += 4` reads that may run past the slice.
#[cfg(feature = "cf-i1")]
#[inline(always)]
fn get_bit_or0(buf: &[u8], row: usize, bit_idx: i32) -> u8 {
    (rd8_or0(buf, row.wrapping_add((bit_idx / 8) as usize)) >> (7 - (bit_idx % 8))) & 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use ferrovgl_core::misc::area::Area;
    use ferrovgl_core::misc::color::Color;

    /// `lv_color_16_16_mix()` (the public, non-inlined one) as the scalar
    /// reference.
    fn mix(c1: u16, c2: u16, m: u8) -> u16 {
        Color16::mix(c1, c2, m)
    }

    /// Deterministic pseudo random bytes.
    struct Lcg(u32);
    impl Lcg {
        fn next(&mut self) -> u8 {
            self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (self.0 >> 16) as u8
        }
        /// Mask values biased towards the special cases.
        fn mask(&mut self) -> u8 {
            const SPECIAL: [u8; 8] = [0, 255, 254, 253, 252, 1, 128, 0];
            let r = self.next();
            if r & 1 == 0 {
                SPECIAL[(r >> 1) as usize & 7]
            } else {
                self.next()
            }
        }
    }

    const PAD: u8 = 0xA5;

    struct Bufs {
        w: i32,
        h: i32,
        dest: Vec<u8>,
        dest_stride: i32,
        src: Vec<u8>,
        src_stride: i32,
        mask: Vec<u8>,
        mask_stride: i32,
    }

    /// Random destination / source / mask with padded strides. `src_row_bytes`
    /// is the meaningful length of one source row.
    fn bufs(w: i32, h: i32, src_row_bytes: usize, seed: u32) -> Bufs {
        let mut rng = Lcg(seed);
        let dest_stride = w * 2 + 4;
        let src_stride = src_row_bytes as i32 + 3;
        let mask_stride = w + 2;
        let mut dest = vec![PAD; (dest_stride * h) as usize];
        let mut src = vec![PAD; (src_stride * h) as usize];
        let mut mask = vec![PAD; (mask_stride * h) as usize];
        for y in 0..h as usize {
            for x in 0..(w * 2) as usize {
                dest[y * dest_stride as usize + x] = rng.next();
            }
            for x in 0..src_row_bytes {
                src[y * src_stride as usize + x] = rng.next();
            }
            for x in 0..w as usize {
                mask[y * mask_stride as usize + x] = rng.mask();
            }
        }
        Bufs {
            w,
            h,
            dest,
            dest_stride,
            src,
            src_stride,
            mask,
            mask_stride,
        }
    }

    fn px(buf: &[u8], stride: i32, x: usize, y: usize) -> u16 {
        rd16(buf, y * stride as usize + 2 * x)
    }

    type Expect<'e> = &'e dyn Fn(u16, &[u8], usize, Option<u8>) -> u16;

    /// Check every pixel against `expect(d, src_row, x, m)` and that the
    /// padding is untouched.
    fn check(before: &Bufs, after: &[u8], expect: Expect<'_>, masked: bool) {
        for y in 0..before.h as usize {
            let src_row = &before.src[y * before.src_stride as usize..];
            for x in 0..before.w as usize {
                let d = px(&before.dest, before.dest_stride, x, y);
                let m = masked.then(|| before.mask[y * before.mask_stride as usize + x]);
                let e = expect(d, src_row, x, m);
                let got = px(after, before.dest_stride, x, y);
                assert_eq!(got, e, "pixel ({x}, {y}): d={d:#06x} m={m:?}");
            }
            let row = y * before.dest_stride as usize;
            for b in (before.w * 2) as usize..before.dest_stride as usize {
                assert_eq!(after[row + b], PAD, "padding touched at row {y}");
            }
        }
    }

    const SIZES: [(i32, i32); 5] = [(1, 1), (2, 3), (7, 2), (33, 3), (40, 2)];
    const OPAS: [u8; 5] = [255, 254, 253, 128, 1];

    fn run_fill(color: Color, opa: u8, masked: bool) {
        let color16 = color.to_u16();
        for (i, &(w, h)) in SIZES.iter().enumerate() {
            let b = bufs(w, h, 0, 7 + i as u32);
            let mut dest = b.dest.clone();
            let mut dsc = BlendFillDsc {
                dest_buf: &mut dest,
                dest_w: w,
                dest_h: h,
                dest_stride: b.dest_stride,
                mask_buf: masked.then_some(&b.mask[..]),
                mask_stride: b.mask_stride,
                color,
                opa,
                relative_area: Area::default(),
            };
            fill(&mut dsc);
            let expect = |d: u16, _: &[u8], _: usize, m: Option<u8>| match m {
                None if opa >= OPA_MAX => color16,
                None => mix(color16, d, opa),
                Some(m) if opa >= OPA_MAX => {
                    if m == 0 {
                        d
                    } else if m >= OPA_MAX {
                        color16
                    } else {
                        mix(color16, d, m)
                    }
                }
                Some(m) => {
                    let a = opa_mix2(m, opa);
                    if a == 0 { d } else { mix(color16, d, a) }
                }
            };
            check(&b, &dest, &expect, masked);
        }
    }

    #[test]
    fn fill_all_paths_match_scalar_mix() {
        for color in [
            Color::make(0x12, 0x9A, 0xF3),
            Color::make(0, 0, 0),
            Color::make(255, 255, 255),
        ] {
            for opa in OPAS {
                run_fill(color, opa, false);
                run_fill(color, opa, true);
            }
        }
    }

    #[test]
    fn fill_simple_contiguous_and_opa_cache() {
        // stride == w * 2: upstream treats the area as one long row.
        let c = Color::make(0xF8, 0, 0xF8);
        let mut dest = vec![0u8; 3 * 2 * 4];
        let mut dsc = BlendFillDsc {
            dest_buf: &mut dest,
            dest_w: 3,
            dest_h: 4,
            dest_stride: 6,
            mask_buf: None,
            mask_stride: 0,
            color: c,
            opa: 255,
            relative_area: Area::default(),
        };
        fill(&mut dsc);
        assert!(
            dest.as_chunks::<2>()
                .0
                .iter()
                .all(|p| u16::from_le_bytes(*p) == 0xF81F)
        );

        // Opacity fill over runs of equal and differing pixels (exercises the
        // cached pair writes).
        let bg = [
            0x0000u16, 0x0000, 0x0000, 0x1234, 0x1234, 0xFFFF, 0x0000, 0x0000, 0x0000,
        ];
        let mut dest: Vec<u8> = bg.iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut dsc = BlendFillDsc {
            dest_buf: &mut dest,
            dest_w: bg.len() as i32,
            dest_h: 1,
            dest_stride: (bg.len() * 2) as i32,
            mask_buf: None,
            mask_stride: 0,
            color: c,
            opa: 100,
            relative_area: Area::default(),
        };
        fill(&mut dsc);
        for (i, &d) in bg.iter().enumerate() {
            assert_eq!(rd16(&dest, 2 * i), mix(0xF81F, d, 100));
        }
    }

    type ImgExpect<'e> = &'e dyn Fn(u16, &[u8], usize, Option<u8>, u8) -> u16;

    /// Run `image()` for `cf` with every mask / opa combination and compare
    /// with `expect(d, src_row, x, m, opa)`.
    fn run_image(cf: ColorFormat, src_row_bytes: &dyn Fn(i32) -> usize, expect: ImgExpect<'_>) {
        for (i, &(w, h)) in SIZES.iter().enumerate() {
            for opa in OPAS {
                for masked in [false, true] {
                    let b = bufs(w, h, src_row_bytes(w), 100 + i as u32 + opa as u32);
                    let mut dest = b.dest.clone();
                    let mut dsc = BlendImageDsc {
                        dest_buf: &mut dest,
                        dest_w: w,
                        dest_h: h,
                        dest_stride: b.dest_stride,
                        mask_buf: masked.then_some(&b.mask[..]),
                        mask_stride: b.mask_stride,
                        src_buf: &b.src,
                        src_stride: b.src_stride,
                        src_color_format: cf,
                        opa,
                        blend_mode: BlendMode::Normal,
                        relative_area: Area::default(),
                        src_area: Area::default(),
                    };
                    image(&mut dsc);
                    check(&b, &dest, &|d, s, x, m| expect(d, s, x, m, opa), masked);
                }
            }
        }
    }

    /// Shared semantics of the "opaque colour" sources (RGB888, XRGB8888,
    /// L8, I1): skip on 0, store on >= `LV_OPA_MAX` with a full-opacity mask.
    fn opaque_src(fg: u16, d: u16, m: Option<u8>, opa: u8) -> u16 {
        match m {
            None if opa >= OPA_MAX => fg,
            None => mix(fg, d, opa),
            Some(m) if opa >= OPA_MAX => {
                if m == 0 {
                    d
                } else if m >= OPA_MAX {
                    fg
                } else {
                    mix(fg, d, m)
                }
            }
            Some(m) => {
                let a = opa_mix2(m, opa);
                if a == 0 { d } else { mix(fg, d, a) }
            }
        }
    }

    /// Shared semantics of the sources with an alpha channel (ARGB8888, AL88).
    fn alpha_src(fg: u16, alpha: u8, d: u16, m: Option<u8>, opa: u8) -> u16 {
        match m {
            None if opa >= OPA_MAX => {
                if alpha == 0 {
                    d
                } else if alpha >= OPA_MAX {
                    fg
                } else {
                    mix(fg, d, alpha)
                }
            }
            None => {
                if alpha == 0 {
                    d
                } else {
                    mix(fg, d, opa_mix2(alpha, opa))
                }
            }
            Some(m) => {
                if alpha == 0 || m == 0 {
                    d
                } else if opa >= OPA_MAX {
                    mix(fg, d, opa_mix2(alpha, m))
                } else {
                    mix(fg, d, opa_mix3(alpha, m, opa))
                }
            }
        }
    }

    fn l8_ref(l: u8) -> u16 {
        let l = l as u16;
        ((l & 0xF8) << 8) | ((l & 0xFC) << 3) | (l >> 3)
    }

    #[test]
    fn image_rgb565() {
        run_image(ColorFormat::Rgb565, &|w| (w * 2) as usize, &|d, s, x, m, opa| {
            let s = rd16(s, 2 * x);
            match m {
                None if opa >= OPA_MAX => s,
                None => mix(s, d, opa),
                Some(m) if opa >= OPA_MAX => {
                    if m == 0 {
                        d
                    } else if m >= OPA_MAX {
                        s
                    } else {
                        mix(s, d, m)
                    }
                }
                Some(m) => mix(s, d, opa_mix2(m, opa)),
            }
        });
    }

    #[test]
    fn image_rgb565_swapped() {
        run_image(
            ColorFormat::Rgb565Swapped,
            &|w| (w * 2) as usize,
            &|d, s, x, m, opa| {
                let s = rd16(s, 2 * x).swap_bytes();
                match m {
                    None if opa >= OPA_MAX => s,
                    None => mix(s, d, opa),
                    // No 0 / opaque shortcut here: the mix itself handles them.
                    Some(m) if opa >= OPA_MAX => mix(s, d, m),
                    Some(m) => mix(s, d, opa_mix2(m, opa)),
                }
            },
        );
    }

    #[test]
    fn image_rgb888_and_xrgb8888() {
        for (cf, px) in [(ColorFormat::Rgb888, 3usize), (ColorFormat::Xrgb8888, 4)] {
            run_image(cf, &|w| w as usize * px, &|d, s, x, m, opa| {
                let fg = color_24_to_16([s[x * px], s[x * px + 1], s[x * px + 2]]);
                opaque_src(fg, d, m, opa)
            });
        }
    }

    #[test]
    fn image_argb8888() {
        run_image(ColorFormat::Argb8888, &|w| w as usize * 4, &|d, s, x, m, opa| {
            let p = &s[x * 4..x * 4 + 4];
            alpha_src(color_24_to_16([p[0], p[1], p[2]]), p[3], d, m, opa)
        });
    }

    #[test]
    fn image_l8() {
        run_image(ColorFormat::L8, &|w| w as usize, &|d, s, x, m, opa| {
            opaque_src(l8_ref(s[x]), d, m, opa)
        });
    }

    #[test]
    fn image_al88() {
        run_image(ColorFormat::Al88, &|w| w as usize * 2, &|d, s, x, m, opa| {
            alpha_src(l8_ref(s[2 * x]), s[2 * x + 1], d, m, opa)
        });
    }

    #[test]
    fn image_i1() {
        run_image(
            ColorFormat::I1,
            &|w| (w as usize).div_ceil(8),
            &|d, s, x, m, opa| {
                let bit = (s[x / 8] >> (7 - (x % 8))) & 1;
                opaque_src(if bit == 1 { 0xFFFF } else { 0 }, d, m, opa)
            },
        );
    }

    #[test]
    fn image_argb8888_premultiplied() {
        // Independent restatement of `lv_color_24_16_mix_premult()`: the
        // background keeps `255 - k` of its weight, the source is added as is.
        fn premult(c: [u8; 3], d: u16, k: u8) -> u16 {
            match k {
                0 => d,
                255 => color_24_to_16(c),
                _ => {
                    let inv = 255 - k as u32;
                    let r = (c[2] as u32 >> 3) + ((((d >> 11) & 0x1F) as u32 * inv) >> 8);
                    let g = (c[1] as u32 >> 2) + ((((d >> 5) & 0x3F) as u32 * inv) >> 8);
                    let b = (c[0] as u32 >> 3) + (((d & 0x1F) as u32 * inv) >> 8);
                    ((r << 11) + (g << 5) + b) as u16
                }
            }
        }
        run_image(
            ColorFormat::Argb8888Premultiplied,
            &|w| w as usize * 4,
            &|d, s, x, m, opa| {
                let p = &s[x * 4..x * 4 + 4];
                let c = [p[0], p[1], p[2]];
                let scale = |f: u8| [opa_mix2(c[0], f), opa_mix2(c[1], f), opa_mix2(c[2], f)];
                match m {
                    None if opa >= OPA_MAX => premult(c, d, p[3]),
                    _ if p[3] == 0 => d,
                    None => premult(scale(opa), d, opa_mix2(p[3], opa)),
                    Some(m) if opa >= OPA_MAX => premult(scale(m), d, opa_mix2(p[3], m)),
                    Some(m) => premult(scale(opa_mix2(m, opa)), d, opa_mix3(p[3], m, opa)),
                }
            },
        );

        // A well formed premultiplied pixel: 50% red over white.
        let out = blend_one(
            ColorFormat::Argb8888Premultiplied,
            BlendMode::Normal,
            0xFFFF,
            &[0, 0, 128, 128],
            255,
            None,
        );
        // r = 16 + (31 * 127 >> 8) = 31, g = 0 + (63 * 127 >> 8) = 31, b = 15
        assert_eq!(rd16(&out, 0), (31 << 11) | (31 << 5) | 15);
    }

    /// Blend one row (one pixel, or `mask.len()` pixels) over a constant `d`.
    fn blend_one(
        cf: ColorFormat,
        mode: BlendMode,
        d: u16,
        src: &[u8],
        opa: u8,
        mask: Option<&[u8]>,
    ) -> Vec<u8> {
        let n = mask.map_or(1, |m| m.len());
        let mut dest: Vec<u8> = (0..n).flat_map(|_| d.to_le_bytes()).collect();
        let mut dsc = BlendImageDsc {
            dest_buf: &mut dest,
            dest_w: n as i32,
            dest_h: 1,
            dest_stride: (n * 2) as i32,
            mask_buf: mask,
            mask_stride: n as i32,
            src_buf: src,
            src_stride: src.len() as i32,
            src_color_format: cf,
            opa,
            blend_mode: mode,
            relative_area: Area::default(),
            src_area: Area::default(),
        };
        image(&mut dsc);
        dest
    }

    #[test]
    fn rgb565_blend_modes() {
        let d: u16 = (16 << 11) | (32 << 5) | 16;
        let s: u16 = (1 << 11) | (2 << 5) | 1;
        let src = s.to_le_bytes();
        let one = |mode| rd16(&blend_one(ColorFormat::Rgb565, mode, d, &src, 255, None), 0);
        assert_eq!(one(BlendMode::Additive), (17 << 11) | (34 << 5) | 17);
        assert_eq!(one(BlendMode::Subtractive), (15 << 11) | (30 << 5) | 15);
        assert_eq!(one(BlendMode::Multiply), 1 << 5);
        assert_eq!(one(BlendMode::Difference), (15 << 11) | (30 << 5) | 15);

        // Saturation, and the black / white skips.
        let white = 0xFFFFu16.to_le_bytes();
        let black = 0u16.to_le_bytes();
        let add = blend_one(ColorFormat::Rgb565, BlendMode::Additive, d, &white, 255, None);
        assert_eq!(rd16(&add, 0), 0xFFFF);
        let add = blend_one(ColorFormat::Rgb565, BlendMode::Additive, d, &black, 255, None);
        assert_eq!(rd16(&add, 0), d);
        let mul = blend_one(ColorFormat::Rgb565, BlendMode::Multiply, d, &white, 255, None);
        assert_eq!(rd16(&mul, 0), d);

        // With opa (no mask) the result is mixed even at 253 / 254.
        let r = (17u16 << 11) | (34 << 5) | 17;
        let add = blend_one(ColorFormat::Rgb565, BlendMode::Additive, d, &src, 254, None);
        assert_eq!(rd16(&add, 0), mix(r, d, 254));

        // Masked.
        let src2 = [src[0], src[1], src[0], src[1]];
        let mask = [0u8, 128];
        let out = blend_one(
            ColorFormat::Rgb565,
            BlendMode::Additive,
            d,
            &src2,
            255,
            Some(&mask),
        );
        assert_eq!(rd16(&out, 0), d);
        assert_eq!(rd16(&out, 2), mix(r, d, 128));
        let out = blend_one(
            ColorFormat::Rgb565,
            BlendMode::Additive,
            d,
            &src2,
            128,
            Some(&mask),
        );
        assert_eq!(rd16(&out, 2), mix(r, d, opa_mix2(128, 128)));

        // The swapped source decodes the swapped value but skips on the raw one.
        let swapped = s.swap_bytes().to_le_bytes();
        let one = |mode| {
            rd16(
                &blend_one(ColorFormat::Rgb565Swapped, mode, d, &swapped, 255, None),
                0,
            )
        };
        assert_eq!(one(BlendMode::Additive), (17 << 11) | (34 << 5) | 17);
        assert_eq!(one(BlendMode::Multiply), 1 << 5);
    }

    #[test]
    fn rgb888_argb8888_blend_modes() {
        let d: u16 = (16 << 11) | (32 << 5) | 16;
        // [b, g, r] = channels 1, 2, 1 after the >> 3 / >> 2
        let px = [8u8, 8, 8];
        let r = rd16(
            &blend_one(ColorFormat::Rgb888, BlendMode::Additive, d, &px, 255, None),
            0,
        );
        assert_eq!(r, (17 << 11) | (34 << 5) | 17);
        let r = rd16(
            &blend_one(
                ColorFormat::Xrgb8888,
                BlendMode::Subtractive,
                d,
                &[8, 8, 8, 0],
                255,
                None,
            ),
            0,
        );
        assert_eq!(r, (15 << 11) | (30 << 5) | 15);
        let r = rd16(
            &blend_one(ColorFormat::Rgb888, BlendMode::Difference, d, &px, 255, None),
            0,
        );
        assert_eq!(r, (15 << 11) | (30 << 5) | 15);

        // ARGB8888 weighs the result with the pixel alpha.
        let argb = [8u8, 8, 8, 128];
        let res = (17u16 << 11) | (34 << 5) | 17;
        let r = rd16(
            &blend_one(ColorFormat::Argb8888, BlendMode::Additive, d, &argb, 255, None),
            0,
        );
        assert_eq!(r, mix(res, d, 128));
        let r = rd16(
            &blend_one(ColorFormat::Argb8888, BlendMode::Additive, d, &argb, 100, None),
            0,
        );
        assert_eq!(r, mix(res, d, opa_mix2(100, 128)));
        let r = rd16(
            &blend_one(
                ColorFormat::Argb8888Premultiplied,
                BlendMode::Multiply,
                d,
                &argb,
                255,
                None,
            ),
            0,
        );
        assert_eq!(r, mix(1 << 5, d, 128));
    }

    #[test]
    fn lumi_blend_modes_step_by_four() {
        // Upstream steps the source by 4 pixels in the non-normal modes:
        // dest pixel 1 reads source pixel 4.
        let d: u16 = (16 << 11) | (32 << 5) | 16;
        let src = [8u8, 1, 2, 3, 16, 5, 6, 7];
        let mask = [255u8, 255];
        let out = blend_one(ColorFormat::L8, BlendMode::Additive, d, &src, 255, Some(&mask));
        // lumi 8: rb = 1, g = 2; lumi 16: rb = 2, g = 4
        assert_eq!(rd16(&out, 0), (17 << 11) | (34 << 5) | 17);
        assert_eq!(rd16(&out, 2), (18 << 11) | (36 << 5) | 18);

        // AL88: element 4 (byte 8) is past the end of the slice and reads as 0.
        let al = [8u8, 255, 0, 0];
        let out = blend_one(ColorFormat::Al88, BlendMode::Additive, d, &al, 255, Some(&mask));
        assert_eq!(rd16(&out, 0), (17 << 11) | (34 << 5) | 17);
        assert_eq!(rd16(&out, 2), d);

        // I1: dest pixel 1 reads bit 4.
        let i1 = [0b0000_1000u8];
        let out = blend_one(
            ColorFormat::I1,
            BlendMode::Additive,
            0x0001,
            &i1,
            255,
            Some(&mask),
        );
        assert_eq!(rd16(&out, 0), 0x0001);
        assert_eq!(rd16(&out, 2), 0xFFFF);
        let out = blend_one(
            ColorFormat::I1,
            BlendMode::Subtractive,
            0x0001,
            &[0x80],
            255,
            None,
        );
        assert_eq!(rd16(&out, 0), 0);
        let out = blend_one(ColorFormat::I1, BlendMode::Difference, 0x0001, &[0x80], 255, None);
        assert_eq!(rd16(&out, 0), 0xFFFE);
        let out = blend_one(ColorFormat::I1, BlendMode::Multiply, 0x8410, &[0x80], 255, None);
        // (16 * 31) & 0x1F = 16, (32 * 63) >> 6 = 31, (16 * 31) >> 5 = 15
        assert_eq!(rd16(&out, 0), (16 << 11) | (31 << 5) | 15);
    }

    #[test]
    fn unsupported_format_is_ignored() {
        let out = blend_one(ColorFormat::A8, BlendMode::Normal, 0x1234, &[255], 255, None);
        assert_eq!(rd16(&out, 0), 0x1234);
    }

    #[test]
    fn mix_helpers_match_reference() {
        for c in [0u8, 1, 7, 8, 127, 200, 255] {
            assert_eq!(l8_to_rgb565(c), color_24_to_16([c, c, c]));
            assert_eq!(l8_to_rgb565(c), l8_ref(c));
            for m in [0u8, 1, 4, 127, 252, 253, 254, 255] {
                for bg in [0u16, 0x1234, 0xFFFF] {
                    assert_eq!(lv_color_8_16_mix(c, bg, m), mix(l8_to_rgb565(c), bg, m));
                    let c3 = [c, c.wrapping_mul(3), 255 - c];
                    assert_eq!(lv_color_24_16_mix(c3, bg, m), mix(color_24_to_16(c3), bg, m));
                }
            }
        }
    }
}
