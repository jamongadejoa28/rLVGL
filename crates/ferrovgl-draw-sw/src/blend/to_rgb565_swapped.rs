//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_rgb565_swapped.c`.
//!
//! Colour fills and image blending into a byte-swapped RGB565 destination
//! (`LV_COLOR_FORMAT_RGB565_SWAPPED`, the big-endian order many SPI panels
//! expect).
//!
//! The destination buffer is a byte slice of `u16` pixels read and written
//! in the reference platform's native (little-endian) order, so a stored
//! value is the byte swap (`lv_color_swap_16()`) of the RGB565 colour.
//! Every mix swaps the pixel in, mixes in RGB565 and swaps it back out, in
//! the same places as upstream. Rows advance by the byte strides exactly
//! like upstream `drawbuf_next_row()`.
//!
//! None of the `LV_DRAW_SW_*_TO_RGB565_SWAPPED*` acceleration hooks exist
//! here, so every `LV_RESULT_INVALID == HOOK(dsc)` test of upstream is
//! always true and the software path always runs.
//!
//! Upstream quirks kept for parity:
//! - the non-normal blend modes of the L8, AL88 and I1 images read the
//!   source with a pixel step of 4 (`src_x += 4`), which walks past the end
//!   of the row; reads outside of the source slice return 0 instead of the
//!   neighbouring memory;
//! - the non-normal blend modes of the L8 image with full opacity and no
//!   mask store the result without swapping it back.

#[cfg(any(
    feature = "cf-al88",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
use ferrovgl_core::misc::color::opa_mix3;
use ferrovgl_core::misc::color::{ColorFormat, OPA_MAX, Opa, opa_mix2};
use ferrovgl_core::misc::style::BlendMode;

#[cfg(any(
    feature = "cf-rgb888",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
use crate::blend::color_24_to_16;
use crate::blend::{
    BlendFillDsc, BlendImageDsc, color_16_16_mix, color_mix_16_prepare, color_mix_16_to_16_prepared,
};

/*********************
 *  BYTE VIEW HELPERS
 *********************/

/// Read the stored `u16` at byte offset `off`.
#[inline(always)]
fn rd16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

/// Write the stored `u16` at byte offset `off`.
#[inline(always)]
fn wr16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

/// Write two pixels at once (`lv_draw_sw_word_t::u32`).
#[inline(always)]
fn wr32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// The three bytes `[b, g, r]` at byte offset `off`.
#[cfg(any(
    feature = "cf-rgb888",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
#[inline(always)]
fn px24(buf: &[u8], off: usize) -> [u8; 3] {
    [buf[off], buf[off + 1], buf[off + 2]]
}

/// A source byte that may lie past the end of the slice (the `src_x += 4`
/// reads of upstream); 0 outside.
#[cfg(any(feature = "cf-i1", feature = "cf-al88", feature = "cf-l8"))]
#[inline(always)]
fn rd8_or0(buf: &[u8], off: usize) -> u8 {
    buf.get(off).copied().unwrap_or(0)
}

/// `drawbuf_next_row()`: advance a byte offset by `stride`.
#[inline(always)]
fn drawbuf_next_row(off: usize, stride: i32) -> usize {
    off.wrapping_add_signed(stride as isize)
}

/// `lv_color_swap_16()`
#[inline(always)]
const fn color_swap_16(c: u16) -> u16 {
    c.rotate_left(8)
}

/// `lv_draw_rgb565_swap((uint8_t *)dest_buf_u16, w)`: swap the row starting
/// at byte offset `off` in place.
#[cfg(any(
    feature = "cf-l8",
    feature = "cf-al88",
    feature = "cf-rgb565",
    feature = "cf-rgb888",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
#[inline(always)]
fn swap_row(buf: &mut [u8], off: usize, w: i32) {
    ferrovgl_core::draw::utils::rgb565_swap(&mut buf[off..], w as u32);
}

/// The fields of an `lv_color16_t` bit-field view of a pixel, promoted to
/// `int` like C does on every use.
#[derive(Copy, Clone)]
struct Color16Fields {
    red: i32,
    green: i32,
    blue: i32,
}

/// Bit-field view of an RGB565 value (`(lv_color16_t *)` cast).
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

/// `RGB565_SWAPPED_MASK_FILL_PX(dest_px, mask_val)`: leave the pixel alone,
/// store the swapped colour, or swap in, mix and swap out.
#[inline(always)]
fn rgb565_swapped_mask_fill_px(buf: &mut [u8], off: usize, mask_val: u8, color16_swapped: u16, fg_prep: u32) {
    let a = mask_val as u32;
    if a != 0 {
        if a >= OPA_MAX as u32 {
            wr16(buf, off, color16_swapped);
        } else {
            let px = color_swap_16(rd16(buf, off)); /* Swap destination */
            let px = color_mix_16_to_16_prepared(fg_prep, px, a as u8); /* Mix */
            wr16(buf, off, color_swap_16(px)); /* Write back swapped */
        }
    }
}

/// The cache of `RGB565_SWAPPED_OPA_FILL_PX(dest_px)`: the last stored
/// background and its (stored) result, kept as a pair.
struct OpaFillCache {
    last_dest_color: u16,
    last_res32_color: u32,
}

impl OpaFillCache {
    /// `RGB565_SWAPPED_OPA_FILL_PX(dest_px)` with `RGB565_SWAPPED_MIX_PX`.
    #[inline(always)]
    fn px(&mut self, buf: &mut [u8], off: usize, color16: u16, opa: Opa) {
        let d = rd16(buf, off);
        if self.last_dest_color != d {
            let res = color_swap_16(color_16_16_mix(color16, color_swap_16(d), opa));
            self.last_dest_color = d;
            self.last_res32_color = (res as u32) | ((res as u32) << 16);
        }
        wr16(buf, off, self.last_res32_color as u16);
    }
}

/// `lv_draw_sw_blend_color_to_rgb565_swapped()`: fill with a colour (+ opa / mask)
/// into a byte-swapped RGB565 buffer.
///
/// Supports normal fill, fill with opacity, fill with mask, and fill with
/// mask and opacity. The background (`dest_buf`) cannot have an alpha
/// channel.
pub fn fill(dsc: &mut BlendFillDsc<'_>) {
    let mut w = dsc.dest_w;
    let mut h = dsc.dest_h;
    let color16 = dsc.color.to_u16(); /* Normal color */
    let color16_swapped = color_swap_16(color16); /* Swapped color, use directly if no mixing is needed */
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
            /*Back to back rows are one long run, so the setup and the odd pixel are paid
             *once instead of once per row*/
            if dest_stride == w.wrapping_mul(2) {
                w = w.wrapping_mul(h);
                h = 1;
            }

            for _y in 0..h {
                /*Upstream aligns to a word, stores 8 words unrolled, then
                 *word pairs, then the last pixel; every store is `color16_swapped`.*/
                for x in 0..w {
                    wr16(buf, dest + 2 * x as usize, color16_swapped);
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
                        /*Both pixels are equal, so one compare covers the pair and the cached
                         *result goes out as a single word*/
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
            /*Every glyph, rounded corner, border, arc and shadow ends up here. About half
             *of such a mask is fully transparent and a third fully opaque, and both are
             *handled above the mix, which also saves them the two byte swaps.*/

            /*Glyphs are a few pixels wide, where the alignment and tail handling of the
             *word based loop would cost more than it saves. Rounded corners and borders are
             *as wide as the widget and hold long runs of 0 and 255, where it pays off.*/
            if w < 32 {
                for _y in 0..h {
                    /*Upstream counts up to zero from -w; the pixel order is the same*/
                    let mut i: i32 = 0;
                    if w & 0x1 != 0 {
                        let iu = i as usize;
                        rgb565_swapped_mask_fill_px(
                            buf,
                            dest + 2 * iu,
                            mask[moff + iu],
                            color16_swapped,
                            fg_prep,
                        );
                        i += 1;
                    }
                    while i < w {
                        let iu = i as usize;
                        rgb565_swapped_mask_fill_px(
                            buf,
                            dest + 2 * iu,
                            mask[moff + iu],
                            color16_swapped,
                            fg_prep,
                        );
                        rgb565_swapped_mask_fill_px(
                            buf,
                            dest + 2 * (iu + 1),
                            mask[moff + iu + 1],
                            color16_swapped,
                            fg_prep,
                        );
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
                        rgb565_swapped_mask_fill_px(
                            buf,
                            dest + 2 * xu,
                            mask[moff + xu],
                            color16_swapped,
                            fg_prep,
                        );
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
                            wr16(buf, dest + 2 * xu, color16_swapped);
                            wr16(buf, dest + 2 * (xu + 1), color16_swapped);
                            wr16(buf, dest + 2 * (xu + 2), color16_swapped);
                            wr16(buf, dest + 2 * (xu + 3), color16_swapped);
                            continue;
                        }
                        /*Read the bytes again rather than split `mask32`, so the byte
                         *order doesn't matter*/
                        rgb565_swapped_mask_fill_px(buf, dest + 2 * xu, mask[m], color16_swapped, fg_prep);
                        rgb565_swapped_mask_fill_px(
                            buf,
                            dest + 2 * (xu + 1),
                            mask[m + 1],
                            color16_swapped,
                            fg_prep,
                        );
                        rgb565_swapped_mask_fill_px(
                            buf,
                            dest + 2 * (xu + 2),
                            mask[m + 2],
                            color16_swapped,
                            fg_prep,
                        );
                        rgb565_swapped_mask_fill_px(
                            buf,
                            dest + 2 * (xu + 3),
                            mask[m + 3],
                            color16_swapped,
                            fg_prep,
                        );
                    }
                    while x < w {
                        let xu = x as usize;
                        rgb565_swapped_mask_fill_px(
                            buf,
                            dest + 2 * xu,
                            mask[moff + xu],
                            color16_swapped,
                            fg_prep,
                        );
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
                    let px = color_swap_16(rd16(buf, p)); /* Swap destination */
                    let px = color_mix_16_to_16_prepared(fg_prep, px, a as u8); /* Color mix */
                    wr16(buf, p, color_swap_16(px)); /* Write back swapped */
                }
                dest = drawbuf_next_row(dest, dest_stride);
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_rgb565_swapped()`: blend an image of any
/// supported source format into a byte-swapped RGB565 buffer. Unsupported
/// formats are ignored (upstream only logs a warning).
pub fn image(dsc: &mut BlendImageDsc<'_>) {
    match dsc.src_color_format {
        ColorFormat::Rgb565Swapped => rgb565_swapped_image_blend(dsc),
        #[cfg(feature = "cf-rgb565")]
        ColorFormat::Rgb565 => rgb565_image_blend(dsc),
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
        _ => { /*LV_LOG_WARN("Not supported source color format 0x%02X")*/ }
    }
}

/**********************
 *   STATIC FUNCTIONS
 **********************/

/// The `switch(dsc->blend_mode)` of the L8 / AL88 images (`rb = lumi >> 3`,
/// `g = lumi >> 2` against the bit-fields of the already swapped back
/// destination `d`). `None` is the `default:` case that returns from the
/// blend.
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
        /*`default:`, which is also where `BlendMode::Normal` lands*/
        _ => return None,
    })
}

/// The `switch(dsc->blend_mode)` of the RGB888 / XRGB8888 / ARGB8888
/// images: `s` is `[b, g, r]`, `d` the swapped back destination. `None` is
/// the `default:` case.
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
        /*`default:`, which is also where `BlendMode::Normal` lands*/
        _ => return None,
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
/// `raw` is the source value as stored (used for the black / white skips),
/// `px` its decoded colour and `d` the swapped back destination. Upstream
/// swaps the destination in place only after the skip tests, which the
/// caller mirrors by writing only on [`ModeRes::Res`].
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
        /*`default:`, which is also where `BlendMode::Normal` lands*/
        _ => ModeRes::Return,
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
                        wr16(
                            dest_buf,
                            dest + 2 * dest_x as usize,
                            color_swap_16(l8_to_rgb565(chan_val)),
                        );
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(dest_buf, p, color_swap_16(lv_color_8_16_mix(chan_val, d, opa)));
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
                            wr16(dest_buf, p, color_swap_16(l8_to_rgb565(chan_val)));
                        } else {
                            let d = color_swap_16(rd16(dest_buf, p));
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(lv_color_8_16_mix(chan_val, d, a as u8)),
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
                        let src_x = dest_x;
                        let a = opa_mix2(mask_buf[moff + dest_x as usize], opa) as u32;
                        if a == 0 {
                            continue;
                        }
                        let chan_val = get_bit(src_buf_i1, src, src_x).wrapping_mul(255);
                        let p = dest + 2 * dest_x as usize;
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_8_16_mix(chan_val, d, a as u8)),
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
            let mut src_x: i32 = 0;
            for dest_x in 0..w {
                let p = dest + 2 * dest_x as usize;
                let d = color_swap_16(rd16(dest_buf, p)) as i32;
                let chan_val = get_bit_or0(src_buf_i1, src, src_x).wrapping_mul(255);
                let c = l8_to_rgb565(chan_val) as i32;
                let res: u16 = match dsc.blend_mode {
                    BlendMode::Additive => (d + c).min(0xFFFF) as u16,
                    BlendMode::Subtractive => (d - c).max(0) as u16,
                    BlendMode::Multiply => {
                        (((((d >> 11) * (c >> 3)) & 0x1F) << 11)
                            | ((((d >> 5) & 0x3F) * ((c >> 2) & 0x3F)) >> 6) << 5
                            | (((d & 0x1F) * (c & 0x1F)) >> 5)) as u16
                    }
                    BlendMode::Difference => lv_abs(d - c) as u16,
                    /*`default:`, which is also where `BlendMode::Normal` lands*/
                    _ => {
                        /*LV_LOG_WARN("Not supported blend mode")*/
                        return;
                    }
                };
                let d = d as u16;

                match mask_buf {
                    None if opa >= OPA_MAX => wr16(dest_buf, p, color_swap_16(res)),
                    None => wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, opa))),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, m)));
                        } else {
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(color_16_16_mix(res, d, opa_mix2(m, opa))),
                            );
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
                            wr16(dest_buf, p, color_swap_16(l8_to_rgb565(lumi)));
                        } else {
                            let d = color_swap_16(rd16(dest_buf, p));
                            wr16(dest_buf, p, color_swap_16(lv_color_8_16_mix(lumi, d, a as u8)));
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_8_16_mix(src_buf_al88[s], d, opa_mix2(alpha, opa))),
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_8_16_mix(src_buf_al88[s], d, opa_mix2(alpha, m))),
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_8_16_mix(src_buf_al88[s], d, opa_mix3(alpha, m, opa))),
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
            /*The row is swapped back in place first, the results are swapped out*/
            swap_row(dest_buf, dest, w);
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
                    None if opa >= OPA_MAX => {
                        wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, alpha)))
                    }
                    None => wr16(
                        dest_buf,
                        p,
                        color_swap_16(color_16_16_mix(res, d, opa_mix2(opa, alpha))),
                    ),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, m)));
                        } else {
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(color_16_16_mix(res, d, opa_mix3(m, opa, alpha))),
                            );
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
                        wr16(
                            dest_buf,
                            dest + 2 * dest_x as usize,
                            color_swap_16(l8_to_rgb565(s)),
                        );
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(dest_buf, p, color_swap_16(lv_color_8_16_mix(s, d, opa)));
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
                            wr16(dest_buf, p, color_swap_16(l8_to_rgb565(s)));
                        } else {
                            let d = color_swap_16(rd16(dest_buf, p));
                            wr16(dest_buf, p, color_swap_16(lv_color_8_16_mix(s, d, a as u8)));
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(dest_buf, p, color_swap_16(lv_color_8_16_mix(s, d, a as u8)));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                    moff = drawbuf_next_row(moff, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            /*The row is swapped back in place first, the results are swapped out*/
            swap_row(dest_buf, dest, w);
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
                    /*Upstream stores `res` without swapping it back here*/
                    None if opa >= OPA_MAX => wr16(dest_buf, p, res),
                    None => wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, opa))),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, m)));
                        } else {
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(color_16_16_mix(res, d, opa_mix2(m, opa))),
                            );
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
#[cfg(feature = "cf-rgb565")]
#[inline(always)]
fn rgb565_mask_px(dest_buf: &mut [u8], p: usize, s: u16, a: u32) {
    if a >= OPA_MAX as u32 {
        wr16(dest_buf, p, color_swap_16(s));
    } else if a != 0 {
        let d = color_swap_16(rd16(dest_buf, p));
        wr16(dest_buf, p, color_swap_16(color_16_16_mix(s, d, a as u8)));
    }
}

/// `rgb565_image_blend()`: a native RGB565 source.
#[cfg(feature = "cf-rgb565")]
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
                    swap_row(dest_buf, dest, w);
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(dest_buf, p, color_swap_16(color_16_16_mix(s, d, opa)));
                    }
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    /*The mask is the image's alpha plane, mostly 0 or 255. Both cases also skip the swaps*/
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(color_16_16_mix(s, d, opa_mix2(mask_buf[moff + xu], opa))),
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
                /*Swapped in place (`dest_buf_u16[x] = lv_color_swap_16(dest_buf_u16[x])`)
                 *only once the pixel is not skipped; the final store overwrites it*/
                let d = color_swap_16(rd16(dest_buf, p));
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
                    None => wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, opa))),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + xu];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, m)));
                        } else {
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(color_16_16_mix(res, d, opa_mix2(m, opa))),
                            );
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

/// `rgb565_swapped_image_blend()`: a byte-swapped RGB565 source.
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
                    dest = drawbuf_next_row(dest, dest_stride);
                    src = drawbuf_next_row(src, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let xu = x as usize;
                        let p = dest + 2 * xu;
                        let px = color_swap_16(rd16(dest_buf, p));
                        let px = color_16_16_mix(color_swap_16(rd16(src_buf_u16, src + 2 * xu)), px, opa);
                        wr16(dest_buf, p, color_swap_16(px));
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
                        let px = color_swap_16(rd16(dest_buf, p));
                        let px = color_16_16_mix(
                            color_swap_16(rd16(src_buf_u16, src + 2 * xu)),
                            px,
                            mask_buf[moff + xu],
                        );
                        wr16(dest_buf, p, color_swap_16(px));
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
                        let px = color_swap_16(rd16(dest_buf, p));
                        let px = color_16_16_mix(
                            color_swap_16(rd16(src_buf_u16, src + 2 * xu)),
                            px,
                            opa_mix2(mask_buf[moff + xu], opa),
                        );
                        wr16(dest_buf, p, color_swap_16(px));
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
                let d = color_swap_16(rd16(dest_buf, p));
                let stored = rd16(src_buf_u16, src + 2 * xu);
                let raw = color_swap_16(stored); /* swap byte order */
                let src_px = color16_fields(raw);

                let res = match blend_mode_565(dsc.blend_mode, d, stored, src_px) {
                    ModeRes::Res(res) => res,
                    ModeRes::Skip => continue,
                    ModeRes::Return => {
                        /*LV_LOG_WARN("Not supported blend mode")*/
                        return;
                    }
                };

                match mask_buf {
                    None => wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, opa))),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + xu];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, m)));
                        } else {
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(color_16_16_mix(res, d, opa_mix2(m, opa))),
                            );
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
                        color_swap_16(color_24_to_16(px24(src_buf_u8, s))),
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
                    let d = color_swap_16(rd16(dest_buf, p));
                    wr16(
                        dest_buf,
                        p,
                        color_swap_16(lv_color_24_16_mix(px24(src_buf_u8, s), d, opa)),
                    );
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
                        wr16(dest_buf, p, color_swap_16(color_24_to_16(px24(src_buf_u8, s))));
                    } else {
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_24_16_mix(px24(src_buf_u8, s), d, a as u8)),
                        );
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
                    let d = color_swap_16(rd16(dest_buf, p));
                    wr16(
                        dest_buf,
                        p,
                        color_swap_16(lv_color_24_16_mix(px24(src_buf_u8, s), d, a as u8)),
                    );
                }
                dest = drawbuf_next_row(dest, dest_stride);
                src = drawbuf_next_row(src, src_stride);
                moff = drawbuf_next_row(moff, mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            /*The row is swapped back in place first, the results are swapped out*/
            swap_row(dest_buf, dest, w);
            for dest_x in 0..w {
                let s = src + dest_x as usize * px_size;
                let p = dest + 2 * dest_x as usize;
                let d = rd16(dest_buf, p);
                let Some(res) = blend_mode_24(dsc.blend_mode, d, px24(src_buf_u8, s)) else {
                    /*LV_LOG_WARN("Not supported blend mode")*/
                    return;
                };

                match mask_buf {
                    None => wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, opa))),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, m)));
                        } else {
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(color_16_16_mix(res, d, opa_mix2(m, opa))),
                            );
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
                        /*Images are mostly fully transparent or fully opaque, and both
                         *cases also skip the two byte swaps*/
                        let a = src_buf_u8[s + 3] as u32;
                        if a == 0 {
                            continue;
                        }
                        let p = dest + 2 * dest_x as usize;
                        if a >= OPA_MAX as u32 {
                            wr16(dest_buf, p, color_swap_16(color_24_to_16(px24(src_buf_u8, s))));
                        } else {
                            let d = color_swap_16(rd16(dest_buf, p));
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(lv_color_24_16_mix(px24(src_buf_u8, s), d, a as u8)),
                            );
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_24_16_mix(
                                px24(src_buf_u8, s),
                                d,
                                opa_mix2(src_buf_u8[s + 3], opa),
                            )),
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_24_16_mix(
                                px24(src_buf_u8, s),
                                d,
                                opa_mix2(src_buf_u8[s + 3], m),
                            )),
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_24_16_mix(
                                px24(src_buf_u8, s),
                                d,
                                opa_mix3(src_buf_u8[s + 3], m, opa),
                            )),
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
            /*The row is swapped back in place first, the results are swapped out*/
            swap_row(dest_buf, dest, w);
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
                    None if opa >= OPA_MAX => {
                        wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, alpha)))
                    }
                    None => wr16(
                        dest_buf,
                        p,
                        color_swap_16(color_16_16_mix(res, d, opa_mix2(opa, alpha))),
                    ),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, m)));
                        } else {
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(color_16_16_mix(res, d, opa_mix3(m, opa, alpha))),
                            );
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
                        let d = color_swap_16(rd16(dest_buf, p));
                        /*For the trivial case use the premultiplied image as it is.
                         *For the other cases unpremultiply as another alpha also needs to be applied.*/
                        wr16(
                            dest_buf,
                            p,
                            color_swap_16(lv_color_24_16_mix_premult(
                                px24(src_buf_u8, s),
                                d,
                                src_buf_u8[s + 3],
                            )),
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
                            let d = color_swap_16(rd16(dest_buf, p));
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(lv_color_24_16_mix_premult(
                                    src_scaled,
                                    d,
                                    opa_mix2(src_buf_u8[s + 3], opa),
                                )),
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
                            let d = color_swap_16(rd16(dest_buf, p));
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(lv_color_24_16_mix_premult(
                                    src_scaled,
                                    d,
                                    opa_mix2(src_buf_u8[s + 3], m),
                                )),
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
                            let d = color_swap_16(rd16(dest_buf, p));
                            /*Combine the three factors in one step to keep the precision*/
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(lv_color_24_16_mix_premult(
                                    src_scaled,
                                    d,
                                    opa_mix3(src_buf_u8[s + 3], m, opa),
                                )),
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
            /*The row is swapped back in place first, the results are swapped out*/
            swap_row(dest_buf, dest, w);
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
                    None if opa >= OPA_MAX => {
                        wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, alpha)))
                    }
                    /* Blending premultiplied ARGB8888 to RGB565 with no mask and partial opacity */
                    None => wr16(
                        dest_buf,
                        p,
                        color_swap_16(color_16_16_mix(res, d, opa_mix2(opa, alpha))),
                    ),
                    Some(mask_buf) => {
                        let m = mask_buf[moff + dest_x as usize];
                        if opa >= OPA_MAX {
                            wr16(dest_buf, p, color_swap_16(color_16_16_mix(res, d, m)));
                        } else {
                            wr16(
                                dest_buf,
                                p,
                                color_swap_16(color_16_16_mix(res, d, opa_mix3(m, opa, alpha))),
                            );
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
#[cfg(any(feature = "cf-i1", feature = "cf-al88", feature = "cf-l8"))]
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
#[cfg(any(feature = "cf-i1", feature = "cf-al88", feature = "cf-l8"))]
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
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888", feature = "cf-argb8888"))]
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
    // The random buffer harness is only consumed by `against_rgb565`.
    #![cfg_attr(
        not(all(
            feature = "cf-rgb565",
            feature = "cf-rgb888",
            feature = "cf-xrgb8888",
            feature = "cf-argb8888",
            feature = "cf-argb8888-premultiplied",
            feature = "cf-l8",
            feature = "cf-al88",
            feature = "cf-i1"
        )),
        allow(dead_code)
    )]

    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use ferrovgl_core::misc::area::Area;
    use ferrovgl_core::misc::color::Color;

    /// Deterministic pseudo random bytes.
    struct Lcg(u32);
    impl Lcg {
        fn next(&mut self) -> u8 {
            self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (self.0 >> 16) as u8
        }
        /// Mask / alpha values biased towards the special cases.
        fn special(&mut self) -> u8 {
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

    /// Random destination / source / mask with padded strides.
    struct Bufs {
        dest: Vec<u8>,
        dest_stride: i32,
        src: Vec<u8>,
        src_stride: i32,
        mask: Vec<u8>,
        mask_stride: i32,
    }

    /// `src_row_bytes` is the meaningful length of one source row; `alpha`
    /// is `(pixel size, alpha byte offset)` for sources with an alpha
    /// channel (biased towards 0 / 255).
    fn bufs(w: i32, h: i32, src_row_bytes: usize, alpha: Option<(usize, usize)>, seed: u32) -> Bufs {
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
                let is_alpha = alpha.is_some_and(|(px, at)| x % px == at);
                src[y * src_stride as usize + x] = if is_alpha { rng.special() } else { rng.next() };
            }
            for x in 0..w as usize {
                mask[y * mask_stride as usize + x] = rng.special();
            }
        }
        // Pure black / white RGB565 source pixels for the blend mode skips.
        if src_row_bytes >= 4 {
            src[..4].copy_from_slice(&[0, 0, 0xFF, 0xFF]);
        }
        Bufs {
            dest,
            dest_stride,
            src,
            src_stride,
            mask,
            mask_stride,
        }
    }

    /// Byte swap every pixel of the `w x h` area (the padding is kept).
    fn swap_area(buf: &[u8], w: i32, h: i32, stride: i32) -> Vec<u8> {
        let mut out = buf.to_vec();
        for y in 0..h as usize {
            let row = y * stride as usize;
            ferrovgl_core::draw::utils::rgb565_swap(&mut out[row..], w as u32);
        }
        out
    }

    #[test]
    fn simple_fill_stores_the_swapped_colour() {
        let mut dest = vec![0u8; 2 * 3 + 2];
        let mut dsc = BlendFillDsc {
            dest_buf: &mut dest,
            dest_w: 3,
            dest_h: 1,
            dest_stride: 8,
            mask_buf: None,
            mask_stride: 0,
            color: Color::make(0xFF, 0, 0),
            opa: 255,
            relative_area: Area::default(),
        };
        fill(&mut dsc);
        // RGB565 red is 0xF800: big-endian bytes in memory.
        assert_eq!(dest, [0xF8, 0, 0xF8, 0, 0xF8, 0, 0, 0]);
    }

    #[test]
    fn swapped_source_is_copied_as_is() {
        let src = [0x12u8, 0x34, 0x56, 0x78];
        let mut dest = [0u8; 4];
        let mut dsc = BlendImageDsc {
            dest_buf: &mut dest,
            dest_w: 2,
            dest_h: 1,
            dest_stride: 4,
            mask_buf: None,
            mask_stride: 0,
            src_buf: &src,
            src_stride: 4,
            src_color_format: ColorFormat::Rgb565Swapped,
            opa: 255,
            blend_mode: BlendMode::Normal,
            relative_area: Area::default(),
            src_area: Area::default(),
        };
        image(&mut dsc);
        assert_eq!(dest, src);
    }

    #[test]
    fn mask_fill_is_swap_of_mixed_pixel() {
        // One pixel, mask 128: swap in, LV_COLOR_MIX_16_TO_16_PREPARED, swap out.
        let bg: u16 = 0x1234;
        let mut dest = color_swap_16(bg).to_le_bytes().to_vec();
        let mask = [128u8];
        let color = Color::make(0x20, 0x80, 0xF0);
        let mut dsc = BlendFillDsc {
            dest_buf: &mut dest,
            dest_w: 1,
            dest_h: 1,
            dest_stride: 2,
            mask_buf: Some(&mask),
            mask_stride: 1,
            color,
            opa: 255,
            relative_area: Area::default(),
        };
        fill(&mut dsc);
        let fg_prep = color_mix_16_prepare(color.to_u16());
        let expect = color_swap_16(color_mix_16_to_16_prepared(fg_prep, bg, 128));
        assert_eq!(rd16(&dest, 0), expect);
    }

    #[test]
    fn unsupported_source_is_ignored() {
        let src = [0u8; 8];
        let mut dest = [PAD; 4];
        let mut dsc = BlendImageDsc {
            dest_buf: &mut dest,
            dest_w: 2,
            dest_h: 1,
            dest_stride: 4,
            mask_buf: None,
            mask_stride: 0,
            src_buf: &src,
            src_stride: 8,
            src_color_format: ColorFormat::A8,
            opa: 255,
            blend_mode: BlendMode::Normal,
            relative_area: Area::default(),
            src_area: Area::default(),
        };
        image(&mut dsc);
        assert_eq!(dest, [PAD; 4]);
    }

    /// The swapped kernels must produce the byte swap of the native RGB565
    /// kernels run on the swapped back destination: upstream keeps the two
    /// files in lockstep, swapping on the way in and out.
    #[cfg(all(
        feature = "cf-rgb565",
        feature = "cf-rgb888",
        feature = "cf-xrgb8888",
        feature = "cf-argb8888",
        feature = "cf-argb8888-premultiplied",
        feature = "cf-l8",
        feature = "cf-al88",
        feature = "cf-i1"
    ))]
    mod against_rgb565 {
        use super::*;
        use crate::blend::to_rgb565;

        const SIZES: [(i32, i32); 5] = [(1, 1), (2, 3), (7, 2), (33, 3), (40, 2)];
        const OPAS: [u8; 5] = [255, 254, 253, 128, 1];
        const MODES: [BlendMode; 5] = [
            BlendMode::Normal,
            BlendMode::Additive,
            BlendMode::Subtractive,
            BlendMode::Multiply,
            BlendMode::Difference,
        ];

        type FillFn = fn(&mut BlendFillDsc<'_>);
        type ImageFn = fn(&mut BlendImageDsc<'_>);

        fn run_fill(color: Color, opa: u8, masked: bool) {
            for (i, &(w, h)) in SIZES.iter().enumerate() {
                let b = bufs(w, h, 0, None, 7 + i as u32);
                let mut native = swap_area(&b.dest, w, h, b.dest_stride);
                let mut swapped = b.dest.clone();
                for (buf, f) in [
                    (&mut native, to_rgb565::fill as FillFn),
                    (&mut swapped, fill as FillFn),
                ] {
                    let mut dsc = BlendFillDsc {
                        dest_buf: buf,
                        dest_w: w,
                        dest_h: h,
                        dest_stride: b.dest_stride,
                        mask_buf: masked.then_some(&b.mask[..]),
                        mask_stride: b.mask_stride,
                        color,
                        opa,
                        relative_area: Area::default(),
                    };
                    f(&mut dsc);
                }
                assert_eq!(
                    swap_area(&native, w, h, b.dest_stride),
                    swapped,
                    "fill {w}x{h} opa {opa} masked {masked}"
                );
            }
        }

        #[test]
        fn fill_matches() {
            for color in [
                Color::make(0xFF, 0, 0),
                Color::make(0x12, 0x9A, 0xEF),
                Color::WHITE,
            ] {
                for opa in OPAS {
                    run_fill(color, opa, false);
                    run_fill(color, opa, true);
                }
            }
        }

        /// Source row bytes and `(pixel size, alpha offset)` of a format.
        fn src_layout(cf: ColorFormat, w: i32) -> (usize, Option<(usize, usize)>) {
            let w = w as usize;
            match cf {
                ColorFormat::Rgb565 | ColorFormat::Rgb565Swapped => (2 * w, None),
                ColorFormat::Rgb888 => (3 * w, None),
                ColorFormat::Xrgb8888 => (4 * w, None),
                ColorFormat::Argb8888 | ColorFormat::Argb8888Premultiplied => (4 * w, Some((4, 3))),
                ColorFormat::L8 => (w, None),
                ColorFormat::Al88 => (2 * w, Some((2, 1))),
                ColorFormat::I1 => (w.div_ceil(8), None),
                _ => unreachable!(),
            }
        }

        fn run_image(cf: ColorFormat, mode: BlendMode, opa: u8, masked: bool) {
            for (i, &(w, h)) in SIZES.iter().enumerate() {
                let (row_bytes, alpha) = src_layout(cf, w);
                let mut b = bufs(w, h, row_bytes, alpha, 31 + i as u32 + cf as u32);
                if cf == ColorFormat::Argb8888Premultiplied {
                    // Keep the colour channels premultiplied (<= alpha).
                    for y in 0..h as usize {
                        for x in 0..w as usize {
                            let o = y * b.src_stride as usize + 4 * x;
                            let a = b.src[o + 3];
                            for c in 0..3 {
                                b.src[o + c] = opa_mix2(b.src[o + c], a);
                            }
                        }
                    }
                }
                let mut native = swap_area(&b.dest, w, h, b.dest_stride);
                let mut swapped = b.dest.clone();
                for (buf, f) in [
                    (&mut native, to_rgb565::image as ImageFn),
                    (&mut swapped, image as ImageFn),
                ] {
                    let mut dsc = BlendImageDsc {
                        dest_buf: buf,
                        dest_w: w,
                        dest_h: h,
                        dest_stride: b.dest_stride,
                        mask_buf: masked.then_some(&b.mask[..]),
                        mask_stride: b.mask_stride,
                        src_buf: &b.src,
                        src_stride: b.src_stride,
                        src_color_format: cf,
                        opa,
                        blend_mode: mode,
                        relative_area: Area::default(),
                        src_area: Area::default(),
                    };
                    f(&mut dsc);
                }

                let quirk = cf == ColorFormat::L8 && mode != BlendMode::Normal && !masked && opa >= OPA_MAX;
                let expect = if quirk {
                    // `dest_buf_u16[dest_x] = res;`: stored without the swap.
                    native
                } else {
                    swap_area(&native, w, h, b.dest_stride)
                };
                assert_eq!(
                    swapped, expect,
                    "{cf:?} {mode:?} {w}x{h} opa {opa} masked {masked}"
                );
            }
        }

        #[test]
        fn image_matches_for_every_source_and_mode() {
            let formats = [
                ColorFormat::Rgb565Swapped,
                ColorFormat::Rgb565,
                ColorFormat::Rgb888,
                ColorFormat::Xrgb8888,
                ColorFormat::Argb8888,
                ColorFormat::Argb8888Premultiplied,
                ColorFormat::L8,
                ColorFormat::Al88,
                ColorFormat::I1,
            ];
            for cf in formats {
                for mode in MODES {
                    for opa in OPAS {
                        run_image(cf, mode, opa, false);
                        run_image(cf, mode, opa, true);
                    }
                }
            }
        }
    }
}
