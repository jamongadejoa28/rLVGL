//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_i1.c`.
//!
//! Blend a colour or an image into a 1 bit per pixel (`I1`) buffer. Pixels
//! are packed MSB first (`bit 7` of byte 0 is the leftmost pixel); a pixel
//! is set when the blended luminance is above [`I1_LUM_THRESHOLD`]. The
//! destination row starts at bit `relative_area.x1 % 8` of `dest_buf[0]`.
//!
//! Port notes:
//! - The `LV_DRAW_SW_*_TO_I1*` acceleration hooks are always
//!   `LV_RESULT_INVALID` in the software renderer, so only the C fallback
//!   loops are ported.
//! - Buffers are byte slices; the C "row pointer += stride" becomes a byte
//!   offset advanced with [`drawbuf_next_row`]. Multi-byte source pixels are
//!   read little-endian, like the C structs on the reference targets.
//! - `LV_LOG_WARN` paths (unknown source format or blend mode) do nothing.
//! - Upstream quirk kept: in `rgb888_image_blend()` the non-normal
//!   blend-mode loop is the last `else` of an exhaustive `else if` chain
//!   *inside* the `LV_BLEND_MODE_NORMAL` branch, so it is unreachable and
//!   RGB888/XRGB8888 sources blended with a non-normal mode leave the
//!   destination untouched.

#[cfg(any(feature = "cf-rgb565", feature = "cf-rgb565-swapped"))]
use ferrovgl_core::misc::color::Color16;
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
use ferrovgl_core::misc::color::color24_luminance;
#[cfg(any(
    feature = "cf-al88",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
use ferrovgl_core::misc::color::opa_mix3;
use ferrovgl_core::misc::color::{Color32, ColorFormat, OPA_COVER, OPA_MAX, OPA_TRANSP, Opa, opa_mix2};
use ferrovgl_core::misc::style::BlendMode;

use crate::blend::I1_LUM_THRESHOLD;
use crate::blend::{BlendFillDsc, BlendImageDsc};
#[cfg(any(feature = "cf-argb8888", feature = "cf-argb8888-premultiplied"))]
use crate::blend::{color32_lumi_of, color32_unpremultiply};

/// `lv_draw_sw_blend_color_to_i1()`: fill with a colour (+ opa / mask).
pub fn fill(dsc: &mut BlendFillDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let mask = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_stride = dsc.dest_stride;

    let src_color: u8 = dsc.color.luminance() / (I1_LUM_THRESHOLD + 1);
    let dest_buf = &mut *dsc.dest_buf;
    let mut d = 0usize;

    let bit_ofs: i32 = dsc.relative_area.x1 % 8;

    match mask {
        /* Simple fill */
        None if opa >= OPA_MAX => {
            let fill_byte: u8 = if src_color != 0 { 0xFF } else { 0x00 };
            for _y in 0..h {
                let row = &mut dest_buf[d..];
                let mut x = 0i32;
                while x < w && ((x + bit_ofs) % 8) != 0 {
                    if src_color != 0 {
                        set_bit(row, x + bit_ofs);
                    } else {
                        clear_bit(row, x + bit_ofs);
                    }
                    x += 1;
                }
                let full_bytes = (w - x) / 8;
                if full_bytes > 0 {
                    let start = ((x + bit_ofs) / 8) as usize;
                    row[start..start + full_bytes as usize].fill(fill_byte);
                    x += full_bytes * 8;
                }
                while x < w {
                    if src_color != 0 {
                        set_bit(row, x + bit_ofs);
                    } else {
                        clear_bit(row, x + bit_ofs);
                    }
                    x += 1;
                }
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        /* Opacity only */
        None => {
            for _y in 0..h {
                let row = &mut dest_buf[d..];
                for x in 0..w {
                    let current_bit = get_bit(row, x + bit_ofs);
                    let new_bit = ((opa as i32 * src_color as i32 + (255 - opa as i32) * current_bit as i32)
                        / 255) as u8;
                    if new_bit != 0 {
                        set_bit(row, x + bit_ofs);
                    } else {
                        clear_bit(row, x + bit_ofs);
                    }
                }
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        /* Masked with full opacity */
        Some(mask) if opa >= OPA_MAX => {
            let mut m = 0usize;
            for _y in 0..h {
                let row = &mut dest_buf[d..];
                for x in 0..w {
                    let mask_val = mask[m + x as usize];
                    if mask_val == OPA_TRANSP {
                        continue;
                    }
                    if mask_val == OPA_COVER {
                        if src_color != 0 {
                            set_bit(row, x + bit_ofs);
                        } else {
                            clear_bit(row, x + bit_ofs);
                        }
                    } else {
                        let current_bit = get_bit(row, x + bit_ofs);
                        let new_bit = ((mask_val as i32 * src_color as i32
                            + (255 - mask_val as i32) * current_bit as i32)
                            / 255) as u8;
                        if new_bit != 0 {
                            set_bit(row, x + bit_ofs);
                        } else {
                            clear_bit(row, x + bit_ofs);
                        }
                    }
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
        /* Masked with opacity */
        Some(mask) => {
            let mut m = 0usize;
            for _y in 0..h {
                let row = &mut dest_buf[d..];
                for x in 0..w {
                    let mask_val = mask[m + x as usize];
                    if mask_val == OPA_TRANSP {
                        continue;
                    }
                    let current_bit = get_bit(row, x + bit_ofs);
                    let blended_opa = ((mask_val as i32 * opa as i32) / 255) as u8;
                    let new_bit = ((blended_opa as i32 * src_color as i32
                        + (255 - blended_opa as i32) * current_bit as i32)
                        / 255) as u8;
                    if new_bit != 0 {
                        set_bit(row, x + bit_ofs);
                    } else {
                        clear_bit(row, x + bit_ofs);
                    }
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_i1()`: blend an image of any supported
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
        #[cfg(feature = "cf-argb8888")]
        ColorFormat::Argb8888 => argb8888_image_blend(dsc, false),
        #[cfg(feature = "cf-argb8888-premultiplied")]
        ColorFormat::Argb8888Premultiplied => argb8888_image_blend(dsc, true),
        #[cfg(feature = "cf-l8")]
        ColorFormat::L8 => l8_image_blend(dsc),
        #[cfg(feature = "cf-al88")]
        ColorFormat::Al88 => al88_image_blend(dsc),
        ColorFormat::I1 => i1_image_blend(dsc),
        _ => {
            // LV_LOG_WARN("Not supported source color format")
        }
    }
}

/// Set or clear destination bit `idx` of `row` by thresholding `lumi`
/// (the recurring `if(v > I1_LUM_THRESHOLD) set_bit else clear_bit`).
#[inline]
fn store_thresholded(row: &mut [u8], idx: i32, lumi: u8) {
    if lumi > I1_LUM_THRESHOLD {
        set_bit(row, idx);
    } else {
        clear_bit(row, idx);
    }
}

/// `get_bit(dest, idx) * 255`, mixed with `src` by `mix`
/// (`lv_color_8_8_mix(src, &dest, mix)`), then thresholded back.
#[cfg(any(
    feature = "cf-l8",
    feature = "cf-al88",
    feature = "cf-rgb565",
    feature = "cf-rgb565-swapped",
    feature = "cf-rgb888",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
#[inline]
fn mix_into_bit(row: &mut [u8], idx: i32, src: u8, mix: u8) {
    let mut dest = get_bit(row, idx).wrapping_mul(255);
    color_8_8_mix(src, &mut dest, mix);
    store_thresholded(row, idx, dest);
}

/// `i1_image_blend()`
fn i1_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_i1 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_i1 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    let bit_ofs: i32 = dsc.relative_area.x1 % 8;

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        if get_bit(&src_buf_i1[s..], x) != 0 {
                            set_bit(row, x + bit_ofs);
                        } else {
                            clear_bit(row, x + bit_ofs);
                        }
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let src = get_bit(&src_buf_i1[s..], x) as i32;
                        let dest = get_bit(row, x + bit_ofs) as i32;
                        let blended = (src * opa as i32 + dest * (255 - opa as i32)) as u8;
                        store_thresholded(row, x + bit_ofs, blended);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let mask_val = mask_buf[m + x as usize] as i32;
                        let src = get_bit(&src_buf_i1[s..], x) as i32;
                        let dest = get_bit(row, x + bit_ofs) as i32;
                        let blended = (src * mask_val + dest * (255 - mask_val)) as u8;
                        store_thresholded(row, x + bit_ofs, blended);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let mask_val = mask_buf[m + x as usize];
                        if mask_val == OPA_TRANSP {
                            continue;
                        }
                        let src = get_bit(&src_buf_i1[s..], x) as i32;
                        let dest = get_bit(row, x + bit_ofs) as i32;
                        let blend_opa = opa_mix2(mask_val, opa) as i32;
                        let blended = (src * blend_opa + dest * (255 - blend_opa)) as u8;
                        store_thresholded(row, x + bit_ofs, blended);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                let red = get_bit(&src_buf_i1[s..], x).wrapping_mul(255);
                let alpha = match mask_buf {
                    None => opa,
                    Some(mask_buf) => opa_mix2(mask_buf[m + x as usize], opa),
                };
                let src_argb = Color32 {
                    blue: red,
                    green: red,
                    red,
                    alpha,
                };
                blend_non_normal_pixel(&mut dest_buf_i1[d..], x + bit_ofs, src_argb, blend_mode);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
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
    let src_buf_l8 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_i1 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    let bit_ofs: i32 = dsc.relative_area.x1 % 8;

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        store_thresholded(row, x + bit_ofs, src_buf_l8[s + x as usize]);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        mix_into_bit(row, x + bit_ofs, src_buf_l8[s + x as usize], opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let src_luminance = src_buf_l8[s + x as usize];
                        mix_into_bit(row, x + bit_ofs, src_luminance, mask_buf[m + x as usize]);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let src_luminance = src_buf_l8[s + x as usize];
                        mix_into_bit(
                            row,
                            x + bit_ofs,
                            src_luminance,
                            opa_mix2(mask_buf[m + x as usize], opa),
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                let l = src_buf_l8[s + x as usize];
                let alpha = match mask_buf {
                    None => opa,
                    Some(mask_buf) => opa_mix2(mask_buf[m + x as usize], opa),
                };
                let src_argb = Color32 {
                    blue: l,
                    green: l,
                    red: l,
                    alpha,
                };
                blend_non_normal_pixel(&mut dest_buf_i1[d..], x + bit_ofs, src_argb, blend_mode);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
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
    let src_buf_al88 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_i1 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    let bit_ofs: i32 = dsc.relative_area.x1 % 8;

    // `src_buf_al88[x].lumi` / `.alpha`
    let lumi = |s: usize, x: i32| src_buf_al88[s + x as usize * 2];
    let alpha = |s: usize, x: i32| src_buf_al88[s + x as usize * 2 + 1];

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        mix_into_bit(row, x + bit_ofs, lumi(s, x), alpha(s, x));
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        mix_into_bit(row, x + bit_ofs, lumi(s, x), opa_mix2(alpha(s, x), opa));
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        mix_into_bit(
                            row,
                            x + bit_ofs,
                            lumi(s, x),
                            opa_mix2(alpha(s, x), mask_buf[m + x as usize]),
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        mix_into_bit(
                            row,
                            x + bit_ofs,
                            lumi(s, x),
                            opa_mix3(alpha(s, x), mask_buf[m + x as usize], opa),
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                let l = lumi(s, x);
                let a = match mask_buf {
                    None => opa_mix2(alpha(s, x), opa),
                    Some(mask_buf) => opa_mix3(alpha(s, x), mask_buf[m + x as usize], opa),
                };
                let src_argb = Color32 {
                    blue: l,
                    green: l,
                    red: l,
                    alpha: a,
                };
                blend_non_normal_pixel(&mut dest_buf_i1[d..], x + bit_ofs, src_argb, blend_mode);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
        }
    }
}

/// `argb8888_image_blend()`: `premultiplied` selects
/// `ARGB8888_PREMULTIPLIED` sources.
#[cfg(any(feature = "cf-argb8888", feature = "cf-argb8888-premultiplied"))]
fn argb8888_image_blend(dsc: &mut BlendImageDsc<'_>, premultiplied: bool) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_c32 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_i1 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    let bit_ofs: i32 = dsc.relative_area.x1 % 8;

    // `src_buf_c32[x]`
    let px = |s: usize, x: i32| {
        let o = s + x as usize * 4;
        Color32 {
            blue: src_buf_c32[o],
            green: src_buf_c32[o + 1],
            red: src_buf_c32[o + 2],
            alpha: src_buf_c32[o + 3],
        }
    };

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let c = px(s, x);
                        let src = color32_lumi_of(c, premultiplied);
                        mix_into_bit(row, x + bit_ofs, src, c.alpha);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let c = px(s, x);
                        let src = color32_lumi_of(c, premultiplied);
                        mix_into_bit(row, x + bit_ofs, src, opa_mix2(opa, c.alpha));
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let c = px(s, x);
                        let src = color32_lumi_of(c, premultiplied);
                        mix_into_bit(row, x + bit_ofs, src, opa_mix2(mask_buf[m + x as usize], c.alpha));
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for x in 0..w {
                        let c = px(s, x);
                        let src = color32_lumi_of(c, premultiplied);
                        mix_into_bit(
                            row,
                            x + bit_ofs,
                            src,
                            opa_mix3(opa, mask_buf[m + x as usize], c.alpha),
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                /*The blend modes work on straight colors*/
                let mut color_argb = if premultiplied {
                    color32_unpremultiply(px(s, x))
                } else {
                    px(s, x)
                };
                color_argb.alpha = match mask_buf {
                    None => opa_mix2(color_argb.alpha, opa),
                    Some(mask_buf) => opa_mix3(color_argb.alpha, mask_buf[m + x as usize], opa),
                };
                blend_non_normal_pixel(&mut dest_buf_i1[d..], x + bit_ofs, color_argb, blend_mode);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
        }
    }
}

/// `rgb888_image_blend()`: `src_px_size` is 3 for RGB888 and 4 for XRGB8888.
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
fn rgb888_image_blend(dsc: &mut BlendImageDsc<'_>, src_px_size: u8) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_rgb888 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_buf_i1 = &mut *dsc.dest_buf;
    let px_size = src_px_size as usize;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    let bit_ofs: i32 = dsc.relative_area.x1 % 8;

    // `lv_color24_luminance(&src_buf_rgb888[src_x])`
    let lumi = |s: usize, dest_x: i32| {
        let o = s + dest_x as usize * px_size;
        color24_luminance([src_buf_rgb888[o], src_buf_rgb888[o + 1], src_buf_rgb888[o + 2]])
    };

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            /*Special case*/
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for dest_x in 0..w {
                        store_thresholded(row, dest_x + bit_ofs, lumi(s, dest_x));
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for dest_x in 0..w {
                        mix_into_bit(row, dest_x + bit_ofs, lumi(s, dest_x), opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for dest_x in 0..w {
                        let mask_x = dest_x as usize;
                        mix_into_bit(row, dest_x + bit_ofs, lumi(s, dest_x), mask_buf[m + mask_x]);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    let row = &mut dest_buf_i1[d..];
                    for dest_x in 0..w {
                        let mask_x = dest_x as usize;
                        mix_into_bit(
                            row,
                            dest_x + bit_ofs,
                            lumi(s, dest_x),
                            opa_mix2(mask_buf[m + mask_x], opa),
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
        }
        // The upstream non-normal loop sits here as the unreachable last
        // `else` of the exhaustive chain above (see the module notes).
    }
}

/// `rgb565_image_blend()`
#[cfg(feature = "cf-rgb565")]
fn rgb565_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_c16 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let bit_ofs: i32 = dsc.relative_area.x1 % 8;
    let dest_buf_u8 = &mut *dsc.dest_buf;

    // `src_buf_c16[x]`
    let px = |s: usize, x: i32| {
        let o = s + x as usize * 2;
        Color16(u16::from_le_bytes([src_buf_c16[o], src_buf_c16[o + 1]]))
    };

    rgb565_kernel(
        dest_buf_u8,
        Rgb565Params {
            w,
            h,
            opa,
            dest_stride,
            src_stride,
            mask_buf,
            mask_stride,
            blend_mode,
            bit_ofs,
        },
        px,
    );
}

/// `rgb565_swapped_image_blend()`
#[cfg(feature = "cf-rgb565-swapped")]
fn rgb565_swapped_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_u16 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let bit_ofs: i32 = dsc.relative_area.x1 % 8;
    let dest_buf_u8 = &mut *dsc.dest_buf;

    // `px = lv_color16_from_u16(lv_color_swap_16(src_buf_u16[x]))`
    let px = |s: usize, x: i32| {
        let o = s + x as usize * 2;
        color16_from_u16(u16::from_le_bytes([src_buf_u16[o], src_buf_u16[o + 1]]).swap_bytes())
    };

    rgb565_kernel(
        dest_buf_u8,
        Rgb565Params {
            w,
            h,
            opa,
            dest_stride,
            src_stride,
            mask_buf,
            mask_stride,
            blend_mode,
            bit_ofs,
        },
        px,
    );
}

/// The scalar parameters shared by the two RGB565 kernels.
#[cfg(any(feature = "cf-rgb565", feature = "cf-rgb565-swapped"))]
struct Rgb565Params<'m> {
    w: i32,
    h: i32,
    opa: Opa,
    dest_stride: i32,
    src_stride: i32,
    mask_buf: Option<&'m [u8]>,
    mask_stride: i32,
    blend_mode: BlendMode,
    bit_ofs: i32,
}

/// Body of `rgb565_image_blend()` / `rgb565_swapped_image_blend()`: the two
/// upstream functions are identical except for how a source pixel is read,
/// which `px(row_ofs, x)` abstracts.
#[cfg(any(feature = "cf-rgb565", feature = "cf-rgb565-swapped"))]
fn rgb565_kernel(dest_buf_u8: &mut [u8], p: Rgb565Params<'_>, px: impl Fn(usize, i32) -> Color16) {
    let Rgb565Params {
        w,
        h,
        opa,
        dest_stride,
        src_stride,
        mask_buf,
        mask_stride,
        blend_mode,
        bit_ofs,
    } = p;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_u8[d..];
                    for x in 0..w {
                        let src = px(s, x).luminance();
                        store_thresholded(row, x + bit_ofs, src);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    let row = &mut dest_buf_u8[d..];
                    for x in 0..w {
                        let src = px(s, x).luminance();
                        mix_into_bit(row, x + bit_ofs, src, opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    let row = &mut dest_buf_u8[d..];
                    for x in 0..w {
                        let mask_x = x as usize;
                        let src = px(s, x).luminance();
                        mix_into_bit(row, x + bit_ofs, src, mask_buf[m + mask_x]);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    let row = &mut dest_buf_u8[d..];
                    for x in 0..w {
                        let mask_x = x as usize;
                        let src = px(s, x).luminance();
                        mix_into_bit(row, x + bit_ofs, src, opa_mix2(mask_buf[m + mask_x], opa));
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
        }
    } else {
        for _y in 0..h {
            for x in 0..w {
                let c = px(s, x);
                let alpha = match mask_buf {
                    None => opa,
                    Some(mask_buf) => opa_mix2(mask_buf[m + x as usize], opa),
                };
                let src_argb = Color32 {
                    red: ((c.red() as u32 * 2106) >> 8) as u8,
                    green: ((c.green() as u32 * 1037) >> 8) as u8,
                    blue: ((c.blue() as u32 * 2106) >> 8) as u8,
                    alpha,
                };
                blend_non_normal_pixel(&mut dest_buf_u8[d..], x + bit_ofs, src_argb, blend_mode);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
        }
    }
}

/// `blend_non_normal_pixel()`: blend `src` into bit `dest_x` of `dest_buf`.
#[inline]
fn blend_non_normal_pixel(dest_buf: &mut [u8], dest_x: i32, src: Color32, mode: BlendMode) {
    let src_lumi = src.luminance() as i32;
    let mut dest_lumi: u8 = get_bit(dest_buf, dest_x).wrapping_mul(255);
    let dl = dest_lumi as i32;
    let res: u8 = match mode {
        BlendMode::Additive => (dl + src_lumi).min(255) as u8,
        BlendMode::Subtractive => (dl - src_lumi).max(0) as u8,
        BlendMode::Multiply => ((dl * src_lumi) >> 8) as u8,
        BlendMode::Difference => (dl - src_lumi).unsigned_abs() as u8,
        _ => {
            // LV_LOG_WARN("Not supported blend mode")
            return;
        }
    };

    color_8_8_mix(res, &mut dest_lumi, src.alpha);
    store_thresholded(dest_buf, dest_x, dest_lumi);
}

/// `lv_color_8_8_mix()`: mix `src` into `*dest` with weight `mix`.
#[inline]
fn color_8_8_mix(src: u8, dest: &mut u8, mix: u8) {
    if mix == 0 {
        return;
    }

    if mix >= OPA_MAX {
        *dest = src;
    } else {
        let mix_inv: Opa = 255 - mix;
        *dest = ((src as u32 * mix as u32 + *dest as u32 * mix_inv as u32) >> 8) as u8;
    }
}

/// `drawbuf_next_row()`: advance a byte offset by one row of `stride` bytes.
#[inline]
fn drawbuf_next_row(ofs: usize, stride: i32) -> usize {
    ofs.wrapping_add_signed(stride as isize)
}

/// `set_bit()`: set MSB-first bit `bit_idx` of `buf`.
#[inline]
fn set_bit(buf: &mut [u8], bit_idx: i32) {
    buf[(bit_idx / 8) as usize] |= 1 << (7 - (bit_idx % 8));
}

/// `clear_bit()`: clear MSB-first bit `bit_idx` of `buf`.
#[inline]
fn clear_bit(buf: &mut [u8], bit_idx: i32) {
    buf[(bit_idx / 8) as usize] &= !(1u8 << (7 - (bit_idx % 8)));
}

/// `get_bit()`: MSB-first bit `bit_idx` of `buf`.
#[inline]
fn get_bit(buf: &[u8], bit_idx: i32) -> u8 {
    (buf[(bit_idx / 8) as usize] >> (7 - (bit_idx % 8))) & 1
}

/// `lv_color16_from_u16()`
#[cfg(feature = "cf-rgb565-swapped")]
#[inline]
fn color16_from_u16(raw: u16) -> Color16 {
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
    use ferrovgl_core::misc::area::Area;
    use ferrovgl_core::misc::color::Color;

    fn fill_dsc<'a>(
        dest: &'a mut [u8],
        x1: i32,
        w: i32,
        mask: Option<&'a [u8]>,
        color: Color,
        opa: Opa,
    ) -> BlendFillDsc<'a> {
        let dest_stride = dest.len() as i32;
        BlendFillDsc {
            dest_buf: dest,
            dest_w: w,
            dest_h: 1,
            dest_stride,
            mask_buf: mask,
            mask_stride: w,
            color,
            opa,
            relative_area: Area::new(x1, 0, x1 + w - 1, 0),
        }
    }

    fn img_dsc<'a>(
        dest: &'a mut [u8],
        x1: i32,
        w: i32,
        src: &'a [u8],
        cf: ColorFormat,
        mask: Option<&'a [u8]>,
        opa: Opa,
        mode: BlendMode,
    ) -> BlendImageDsc<'a> {
        let dest_stride = dest.len() as i32;
        let src_stride = src.len() as i32;
        BlendImageDsc {
            dest_buf: dest,
            dest_w: w,
            dest_h: 1,
            dest_stride,
            mask_buf: mask,
            mask_stride: w,
            src_buf: src,
            src_stride,
            src_color_format: cf,
            opa,
            blend_mode: mode,
            relative_area: Area::new(x1, 0, x1 + w - 1, 0),
            src_area: Area::new(0, 0, w - 1, 0),
        }
    }

    #[test]
    fn bit_helpers_are_msb_first() {
        let mut b = [0u8; 2];
        set_bit(&mut b, 0);
        set_bit(&mut b, 9);
        assert_eq!(b, [0x80, 0x40]);
        assert_eq!(get_bit(&b, 0), 1);
        assert_eq!(get_bit(&b, 1), 0);
        assert_eq!(get_bit(&b, 9), 1);
        clear_bit(&mut b, 0);
        assert_eq!(b, [0x00, 0x40]);
        assert_eq!(I1_LUM_THRESHOLD, 127);
    }

    #[test]
    fn fill_simple_with_bit_offset() {
        // x1 = 3, 12 px: bits 3..=14.
        let mut buf = vec![0u8; 3];
        fill(&mut fill_dsc(&mut buf, 3, 12, None, Color::WHITE, 255));
        assert_eq!(buf, [0x1F, 0xFE, 0x00]);

        // x1 = 3, 20 px: head bits, one memset byte, tail bits.
        let mut buf = vec![0u8; 3];
        fill(&mut fill_dsc(&mut buf, 3, 20, None, Color::WHITE, 255));
        assert_eq!(buf, [0x1F, 0xFF, 0xFE]);

        // x1 = 11 behaves like a bit offset of 3.
        let mut buf = vec![0xFFu8; 3];
        fill(&mut fill_dsc(&mut buf, 11, 20, None, Color::BLACK, 255));
        assert_eq!(buf, [0xE0, 0x00, 0x01]);

        // Luminance 127 is below the threshold: `127 / 128 == 0`.
        let mut buf = vec![0xFFu8; 1];
        fill(&mut fill_dsc(
            &mut buf,
            0,
            8,
            None,
            Color::make(127, 127, 127),
            255,
        ));
        assert_eq!(buf, [0x00]);
    }

    #[test]
    fn fill_opa() {
        // new_bit = (opa * src + (255 - opa) * cur) / 255: a white fill with
        // opa < 255 cannot set a clear bit and keeps a set one.
        let mut buf = vec![0b1000_0000u8];
        fill(&mut fill_dsc(&mut buf, 0, 2, None, Color::WHITE, 200));
        assert_eq!(buf, [0b1000_0000]);
        // A black fill with opa > 0 clears.
        let mut buf = vec![0b1100_0000u8];
        fill(&mut fill_dsc(&mut buf, 0, 2, None, Color::BLACK, 1));
        assert_eq!(buf, [0b0000_0000]);
    }

    #[test]
    fn fill_mask() {
        let mask = [0u8, 255, 128, 255];
        let mut buf = vec![0b1010_0000u8];
        fill(&mut fill_dsc(&mut buf, 0, 4, Some(&mask), Color::WHITE, 255));
        // px0: transparent, kept 1; px1: cover, set; px2: (128 + 127) / 255 = 1;
        // px3: cover, set.
        assert_eq!(buf, [0b1111_0000]);

        let mut buf = vec![0b1111_0000u8];
        fill(&mut fill_dsc(&mut buf, 0, 4, Some(&mask), Color::BLACK, 255));
        // px2: (128 * 0 + 127 * 1) / 255 = 0
        assert_eq!(buf, [0b1000_0000]);

        // Mask + opa: blended_opa = mask * opa / 255.
        let mut buf = vec![0b0100_0000u8];
        fill(&mut fill_dsc(&mut buf, 0, 4, Some(&mask), Color::WHITE, 128));
        assert_eq!(buf, [0b0100_0000]);
    }

    #[test]
    fn image_from_i1() {
        let src = [0b1011_0000u8];
        let mut dest = vec![0u8; 2];
        image(&mut img_dsc(
            &mut dest,
            6,
            4,
            &src,
            ColorFormat::I1,
            None,
            255,
            BlendMode::Normal,
        ));
        // Bits 6..=9 receive 1, 0, 1, 1.
        assert_eq!(dest, [0b0000_0010, 0b1100_0000]);

        // opa: blended = src * opa + dest * (255 - opa), truncated to u8.
        let src = [0b1000_0000u8];
        let mut dest = vec![0u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            1,
            &src,
            ColorFormat::I1,
            None,
            128,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [0b1000_0000]);
        let mut dest = vec![0u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            1,
            &src,
            ColorFormat::I1,
            None,
            127,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [0]);

        // Mask + opa skips transparent mask pixels.
        let mask = [0u8, 255];
        let src = [0b0000_0000u8];
        let mut dest = vec![0b1100_0000u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            2,
            &src,
            ColorFormat::I1,
            Some(&mask),
            128,
            BlendMode::Normal,
        ));
        // px1: blend_opa = 127, blended = 0 * 127 + 1 * 128 = 128 > 127.
        assert_eq!(dest, [0b1100_0000]);

        // Difference of white over white is black.
        let src = [0b1000_0000u8];
        let mut dest = vec![0b1000_0000u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            1,
            &src,
            ColorFormat::I1,
            None,
            255,
            BlendMode::Difference,
        ));
        assert_eq!(dest, [0]);
    }

    #[cfg(feature = "cf-l8")]
    #[test]
    fn image_from_l8_thresholds() {
        let src = [0u8, 127, 128, 255];
        let mut dest = vec![0b0101_0000u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            4,
            &src,
            ColorFormat::L8,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [0b0011_0000]);

        // opa: dest_val = get_bit * 255 mixed with the source.
        let src = [255u8, 0];
        let mut dest = vec![0b0100_0000u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            2,
            &src,
            ColorFormat::L8,
            None,
            128,
            BlendMode::Normal,
        ));
        // px0: (255 * 128 + 0) >> 8 = 127 -> clear; px1: (0 + 255 * 127) >> 8 = 126 -> clear.
        assert_eq!(dest, [0]);

        // Additive on black: the source luminance decides.
        let mut dest = vec![0u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            2,
            &[200, 100],
            ColorFormat::L8,
            None,
            255,
            BlendMode::Additive,
        ));
        assert_eq!(dest, [0b1000_0000]);
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn image_from_argb8888() {
        // White opaque, white with alpha 0, black opaque.
        let src = [255u8, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 255];
        let mut dest = vec![0b0010_0000u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            3,
            &src,
            ColorFormat::Argb8888,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [0b1000_0000]);

        let mask = [255u8, 255, 0];
        let mut dest = vec![0b0010_0000u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            3,
            &src,
            ColorFormat::Argb8888,
            Some(&mask),
            255,
            BlendMode::Normal,
        ));
        // px0: alpha = LV_OPA_MIX2(255, 255) = 254 -> copy white.
        assert_eq!(dest, [0b1010_0000]);
    }

    #[cfg(feature = "cf-al88")]
    #[test]
    fn image_from_al88() {
        let src = [255u8, 255, 255, 0, 0, 255];
        let mut dest = vec![0b0010_0000u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            3,
            &src,
            ColorFormat::Al88,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [0b1000_0000]);
    }

    #[cfg(feature = "cf-rgb888")]
    #[test]
    fn image_from_rgb888() {
        let src = [255u8, 255, 255, 0, 0, 255];
        let mut dest = vec![0u8];
        image(&mut img_dsc(
            &mut dest,
            1,
            2,
            &src,
            ColorFormat::Rgb888,
            None,
            255,
            BlendMode::Normal,
        ));
        // White -> set, red (luminance 76) -> clear.
        assert_eq!(dest, [0b0100_0000]);

        // Upstream quirk: the non-normal path is unreachable for RGB888.
        let mut dest = vec![0u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            2,
            &src,
            ColorFormat::Rgb888,
            None,
            255,
            BlendMode::Additive,
        ));
        assert_eq!(dest, [0]);
    }

    #[cfg(feature = "cf-rgb565")]
    #[test]
    fn image_from_rgb565() {
        let mut src = [0u8; 4];
        src[..2].copy_from_slice(&0xFFFFu16.to_le_bytes());
        src[2..].copy_from_slice(&Color16::new(0, 0, 31).0.to_le_bytes());
        let mut dest = vec![0u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            2,
            &src,
            ColorFormat::Rgb565,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [0b1000_0000]);
    }

    #[cfg(feature = "cf-rgb565-swapped")]
    #[test]
    fn image_from_rgb565_swapped() {
        let mut src = [0u8; 4];
        src[..2].copy_from_slice(&Color16::new(0, 0, 31).0.to_be_bytes());
        src[2..].copy_from_slice(&Color16::new(0, 63, 0).0.to_be_bytes());
        let mut dest = vec![0u8];
        image(&mut img_dsc(
            &mut dest,
            0,
            2,
            &src,
            ColorFormat::Rgb565Swapped,
            None,
            255,
            BlendMode::Normal,
        ));
        // Blue: (231 * 31) >> 8 = 27 -> clear; green: (613 * 63) >> 8 = 150 -> set.
        assert_eq!(dest, [0b0100_0000]);
    }
}
