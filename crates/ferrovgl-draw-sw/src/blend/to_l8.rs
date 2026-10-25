//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_l8.c`.
//!
//! Blend a colour or an image into an 8-bit luminance (`L8`) buffer.
//!
//! Port notes:
//! - The `LV_DRAW_SW_*_TO_L8*` acceleration hooks are always
//!   `LV_RESULT_INVALID` in the software renderer, so only the C fallback
//!   loops are ported.
//! - Buffers are byte slices; the C "row pointer += stride" becomes a byte
//!   offset advanced with [`drawbuf_next_row`]. Multi-byte source pixels are
//!   read little-endian, like the C structs on the reference targets.
//! - `LV_LOG_WARN` paths (unknown source format or blend mode) do nothing.
//! - The RGB888 kernel uses four independent `if`s upstream; their
//!   conditions are mutually exclusive and no body changes `opa` or the
//!   nullness of the mask, so a `match` is equivalent.

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
use ferrovgl_core::misc::color::{Color32, ColorFormat, OPA_MAX, Opa, opa_mix2};
use ferrovgl_core::misc::style::BlendMode;

use crate::blend::{BlendFillDsc, BlendImageDsc};
#[cfg(any(feature = "cf-argb8888", feature = "cf-argb8888-premultiplied"))]
use crate::blend::{color32_lumi_of, color32_unpremultiply};

/// `lv_draw_sw_blend_color_to_l8()`: fill with a colour (+ opa / mask).
pub fn fill(dsc: &mut BlendFillDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let mask = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_stride = dsc.dest_stride;
    let color8 = dsc.color.luminance();
    let dest_buf = &mut *dsc.dest_buf;
    let mut d = 0usize;

    match mask {
        /*Simple fill*/
        None if opa >= OPA_MAX => {
            for _y in 0..h {
                let mut x = 0i32;
                while x < w - 16 {
                    for i in 0..16 {
                        dest_buf[d + (x + i) as usize] = color8;
                    }
                    x += 16;
                }
                while x < w {
                    dest_buf[d + x as usize] = color8;
                    x += 1;
                }
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        /*Opacity only*/
        None => {
            for _y in 0..h {
                for x in 0..w {
                    color_8_8_mix(color8, &mut dest_buf[d + x as usize], opa);
                }
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        /*Masked with full opacity*/
        Some(mask) if opa >= OPA_MAX => {
            let mut m = 0usize;
            for _y in 0..h {
                for x in 0..w {
                    color_8_8_mix(color8, &mut dest_buf[d + x as usize], mask[m + x as usize]);
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
        /*Masked with opacity*/
        Some(mask) => {
            let mut m = 0usize;
            for _y in 0..h {
                for x in 0..w {
                    color_8_8_mix(
                        color8,
                        &mut dest_buf[d + x as usize],
                        opa_mix2(mask[m + x as usize], opa),
                    );
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_l8()`: blend an image of any supported
/// source format.
pub fn image(dsc: &mut BlendImageDsc<'_>) {
    match dsc.src_color_format {
        #[cfg(feature = "cf-rgb565")]
        ColorFormat::Rgb565 => rgb565_image_blend(dsc),
        #[cfg(feature = "cf-rgb565-swapped")]
        ColorFormat::Rgb565Swapped => rgb565_image_blend_swapped(dsc),
        #[cfg(feature = "cf-rgb888")]
        ColorFormat::Rgb888 => rgb888_image_blend(dsc, 3),
        #[cfg(feature = "cf-xrgb8888")]
        ColorFormat::Xrgb8888 => rgb888_image_blend(dsc, 4),
        #[cfg(feature = "cf-argb8888")]
        ColorFormat::Argb8888 => argb8888_image_blend(dsc, false),
        #[cfg(feature = "cf-argb8888-premultiplied")]
        ColorFormat::Argb8888Premultiplied => argb8888_image_blend(dsc, true),
        ColorFormat::L8 => l8_image_blend(dsc),
        #[cfg(feature = "cf-al88")]
        ColorFormat::Al88 => al88_image_blend(dsc),
        #[cfg(feature = "cf-i1")]
        ColorFormat::I1 => i1_image_blend(dsc),
        _ => {
            // LV_LOG_WARN("Not supported source color format")
        }
    }
}

/// `i1_image_blend()`
#[cfg(feature = "cf-i1")]
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
    let dest_buf_l8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let chan_val = get_bit(&src_buf_i1[s..], x).wrapping_mul(255);
                        color_8_8_mix(chan_val, &mut dest_buf_l8[d + x as usize], opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let chan_val = get_bit(&src_buf_i1[s..], x).wrapping_mul(255);
                        color_8_8_mix(chan_val, &mut dest_buf_l8[d + x as usize], opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let chan_val = get_bit(&src_buf_i1[s..], x).wrapping_mul(255);
                        color_8_8_mix(
                            chan_val,
                            &mut dest_buf_l8[d + x as usize],
                            mask_buf[m + x as usize],
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        let chan_val = get_bit(&src_buf_i1[s..], x).wrapping_mul(255);
                        color_8_8_mix(
                            chan_val,
                            &mut dest_buf_l8[d + x as usize],
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
                blend_non_normal_pixel(&mut dest_buf_l8[d + x as usize], src_argb, blend_mode);
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
fn l8_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_l8 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_l8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    dest_buf_l8[d..d + w].copy_from_slice(&src_buf_l8[s..s + w]);
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(src_buf_l8[s + x], &mut dest_buf_l8[d + x], opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(src_buf_l8[s + x], &mut dest_buf_l8[d + x], mask_buf[m + x]);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(
                            src_buf_l8[s + x],
                            &mut dest_buf_l8[d + x],
                            opa_mix2(mask_buf[m + x], opa),
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
                let l = src_buf_l8[s + x];
                let alpha = match mask_buf {
                    None => opa,
                    Some(mask_buf) => opa_mix2(mask_buf[m + x], opa),
                };
                let src_argb = Color32 {
                    blue: l,
                    green: l,
                    red: l,
                    alpha,
                };
                blend_non_normal_pixel(&mut dest_buf_l8[d + x], src_argb, blend_mode);
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
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_al88 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_l8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `src_buf_al88[x].lumi` / `.alpha`
    let lumi = |s: usize, x: usize| src_buf_al88[s + x * 2];
    let alpha = |s: usize, x: usize| src_buf_al88[s + x * 2 + 1];

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(lumi(s, x), &mut dest_buf_l8[d + x], alpha(s, x));
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(lumi(s, x), &mut dest_buf_l8[d + x], opa_mix2(alpha(s, x), opa));
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(
                            lumi(s, x),
                            &mut dest_buf_l8[d + x],
                            opa_mix2(alpha(s, x), mask_buf[m + x]),
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(
                            lumi(s, x),
                            &mut dest_buf_l8[d + x],
                            opa_mix3(alpha(s, x), mask_buf[m + x], opa),
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
                    None => opa,
                    Some(mask_buf) => opa_mix2(mask_buf[m + x], opa),
                };
                let src_argb = Color32 {
                    blue: l,
                    green: l,
                    red: l,
                    alpha: a,
                };
                blend_non_normal_pixel(&mut dest_buf_l8[d + x], src_argb, blend_mode);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
        }
    }
}

/// `rgb565_image_blend()`
#[cfg(feature = "cf-rgb565")]
fn rgb565_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_c16 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_u8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `src_buf_c16[x]`
    let px = |s: usize, x: usize| {
        let o = s + x * 2;
        Color16(u16::from_le_bytes([src_buf_c16[o], src_buf_c16[o + 1]]))
    };

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        dest_buf_u8[d + x] = px(s, x).luminance();
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(px(s, x).luminance(), &mut dest_buf_u8[d + x], opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(px(s, x).luminance(), &mut dest_buf_u8[d + x], mask_buf[m + x]);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(
                            px(s, x).luminance(),
                            &mut dest_buf_u8[d + x],
                            opa_mix2(opa, mask_buf[m + x]),
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
                let c = px(s, x);
                let alpha = match mask_buf {
                    None => opa,
                    Some(mask_buf) => opa_mix2(mask_buf[m + x], opa),
                };
                let src_argb = Color32 {
                    red: ((c.red() as u32 * 2106) >> 8) as u8,
                    green: ((c.green() as u32 * 1037) >> 8) as u8,
                    blue: ((c.blue() as u32 * 2106) >> 8) as u8,
                    alpha,
                };
                blend_non_normal_pixel(&mut dest_buf_u8[d + x], src_argb, blend_mode);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
        }
    }
}

/// `rgb565_image_blend_swapped()`
#[cfg(feature = "cf-rgb565-swapped")]
fn rgb565_image_blend_swapped(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_u16 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_u8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `raw = lv_color_swap_16(src_buf_u16[x]); px = lv_color16_from_u16(raw);`
    let px = |s: usize, x: usize| {
        let o = s + x * 2;
        let raw = u16::from_le_bytes([src_buf_u16[o], src_buf_u16[o + 1]]).swap_bytes();
        color16_from_u16(raw)
    };

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        dest_buf_u8[d + x] = px(s, x).luminance();
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(px(s, x).luminance(), &mut dest_buf_u8[d + x], opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(px(s, x).luminance(), &mut dest_buf_u8[d + x], mask_buf[m + x]);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        color_8_8_mix(
                            px(s, x).luminance(),
                            &mut dest_buf_u8[d + x],
                            opa_mix2(opa, mask_buf[m + x]),
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
                let c = px(s, x);
                let alpha = match mask_buf {
                    None => opa,
                    Some(mask_buf) => opa_mix2(mask_buf[m + x], opa),
                };
                let src_argb = Color32 {
                    red: ((c.red() as u32 * 2106) >> 8) as u8,
                    green: ((c.green() as u32 * 1037) >> 8) as u8,
                    blue: ((c.blue() as u32 * 2106) >> 8) as u8,
                    alpha,
                };
                blend_non_normal_pixel(&mut dest_buf_u8[d + x], src_argb, blend_mode);
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
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_u8 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_l8 = &mut *dsc.dest_buf;
    let px_size = src_px_size as usize;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `lv_color24_luminance(&src_buf_u8[src_x])`
    let lumi = |o: usize| color24_luminance([src_buf_u8[o], src_buf_u8[o + 1], src_buf_u8[o + 2]]);

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            /*Special case*/
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        dest_buf_l8[d + dest_x] = lumi(s + dest_x * px_size);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        color_8_8_mix(lumi(s + dest_x * px_size), &mut dest_buf_l8[d + dest_x], opa);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let mask_x = dest_x;
                        color_8_8_mix(
                            lumi(s + dest_x * px_size),
                            &mut dest_buf_l8[d + dest_x],
                            mask_buf[m + mask_x],
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let mask_x = dest_x;
                        color_8_8_mix(
                            lumi(s + dest_x * px_size),
                            &mut dest_buf_l8[d + dest_x],
                            opa_mix2(opa, mask_buf[m + mask_x]),
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
            for dest_x in 0..w {
                let src_x = s + dest_x * px_size;
                let alpha = match mask_buf {
                    None => opa,
                    Some(mask_buf) => opa_mix2(mask_buf[m + dest_x], opa),
                };
                let src_argb = Color32 {
                    red: src_buf_u8[src_x + 2],
                    green: src_buf_u8[src_x + 1],
                    blue: src_buf_u8[src_x],
                    alpha,
                };
                blend_non_normal_pixel(&mut dest_buf_l8[d + dest_x], src_argb, blend_mode);
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
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_c32 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let blend_mode = dsc.blend_mode;
    let dest_buf_l8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `src_buf_c32[x]`
    let px = |s: usize, x: usize| {
        let o = s + x * 4;
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
                    for x in 0..w {
                        let c = px(s, x);
                        color_8_8_mix(
                            color32_lumi_of(c, premultiplied),
                            &mut dest_buf_l8[d + x],
                            c.alpha,
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let c = px(s, x);
                        color_8_8_mix(
                            color32_lumi_of(c, premultiplied),
                            &mut dest_buf_l8[d + x],
                            opa_mix2(c.alpha, opa),
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let c = px(s, x);
                        color_8_8_mix(
                            color32_lumi_of(c, premultiplied),
                            &mut dest_buf_l8[d + x],
                            opa_mix2(c.alpha, mask_buf[m + x]),
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        let c = px(s, x);
                        color_8_8_mix(
                            color32_lumi_of(c, premultiplied),
                            &mut dest_buf_l8[d + x],
                            opa_mix3(c.alpha, opa, mask_buf[m + x]),
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
                    Some(mask_buf) => opa_mix3(color_argb.alpha, mask_buf[m + x], opa),
                };
                blend_non_normal_pixel(&mut dest_buf_l8[d + x], color_argb, blend_mode);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
        }
    }
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

/// `get_bit()`: MSB-first bit `bit_idx` of `buf`.
#[cfg(feature = "cf-i1")]
#[inline]
fn get_bit(buf: &[u8], bit_idx: i32) -> u8 {
    (buf[(bit_idx / 8) as usize] >> (7 - (bit_idx % 8))) & 1
}

/// `blend_non_normal_pixel()`
#[inline]
fn blend_non_normal_pixel(dest: &mut u8, src: Color32, mode: BlendMode) {
    let src_lumi = src.luminance() as i32;
    let d = *dest as i32;
    let res: u8 = match mode {
        BlendMode::Additive => (d + src_lumi).min(255) as u8,
        BlendMode::Subtractive => (d - src_lumi).max(0) as u8,
        BlendMode::Multiply => ((d * src_lumi) >> 8) as u8,
        BlendMode::Difference => (d - src_lumi).unsigned_abs() as u8,
        _ => {
            // LV_LOG_WARN("Not supported blend mode")
            return;
        }
    };
    color_8_8_mix(res, dest, src.alpha);
}

/// `drawbuf_next_row()`: advance a byte offset by one row of `stride` bytes.
#[inline]
fn drawbuf_next_row(ofs: usize, stride: i32) -> usize {
    ofs.wrapping_add_signed(stride as isize)
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
        w: i32,
        h: i32,
        stride: i32,
        mask: Option<&'a [u8]>,
        color: Color,
        opa: Opa,
    ) -> BlendFillDsc<'a> {
        BlendFillDsc {
            dest_buf: dest,
            dest_w: w,
            dest_h: h,
            dest_stride: stride,
            mask_buf: mask,
            mask_stride: w,
            color,
            opa,
            relative_area: Area::new(0, 0, w - 1, h - 1),
        }
    }

    fn img_dsc<'a>(
        dest: &'a mut [u8],
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
            relative_area: Area::new(0, 0, w - 1, 0),
            src_area: Area::new(0, 0, w - 1, 0),
        }
    }

    #[cfg(any(
        feature = "cf-argb8888",
        feature = "cf-argb8888-premultiplied",
        feature = "cf-xrgb8888"
    ))]
    fn argb(pixels: &[(u8, u8, u8, u8)]) -> vec::Vec<u8> {
        pixels.iter().flat_map(|&(r, g, b, a)| [b, g, r, a]).collect()
    }

    #[test]
    fn mix_8_8() {
        let mut d = 100;
        color_8_8_mix(255, &mut d, 0);
        assert_eq!(d, 100);
        color_8_8_mix(7, &mut d, OPA_MAX);
        assert_eq!(d, 7);
        let mut d = 100;
        color_8_8_mix(255, &mut d, 128);
        // (255 * 128 + 100 * 127) >> 8
        assert_eq!(d, 177);
    }

    #[test]
    fn fill_simple_respects_stride() {
        // 20 px wide: exercises the 16-wide unrolled loop and the tail.
        let mut buf = vec![9u8; 24 * 2];
        let red = Color::make(255, 0, 0);
        fill(&mut fill_dsc(&mut buf, 20, 2, 24, None, red, 255));
        let lum = red.luminance();
        assert_eq!(lum, 76);
        for y in 0..2 {
            assert!(buf[y * 24..y * 24 + 20].iter().all(|&v| v == lum));
            assert!(buf[y * 24 + 20..y * 24 + 24].iter().all(|&v| v == 9));
        }
    }

    #[test]
    fn fill_opa_and_mask() {
        let white = Color::WHITE;
        let mut buf = vec![100u8; 3];
        fill(&mut fill_dsc(&mut buf, 3, 1, 3, None, white, 128));
        assert_eq!(buf, [177, 177, 177]);

        let mask = [0u8, 255, 128];
        let mut buf = vec![100u8; 3];
        fill(&mut fill_dsc(&mut buf, 3, 1, 3, Some(&mask), white, 255));
        assert_eq!(buf, [100, 255, 177]);

        let mask = [255u8, 0];
        let mut buf = vec![100u8; 2];
        fill(&mut fill_dsc(&mut buf, 2, 1, 2, Some(&mask), white, 128));
        // LV_OPA_MIX2(255, 128) = 127; (255 * 127 + 100 * 128) >> 8 = 176
        assert_eq!(buf, [176, 100]);
    }

    #[test]
    fn image_from_l8() {
        // Two rows with a stride of 4 bytes.
        let src = [10u8, 20, 30, 0, 50, 60, 70, 0];
        let mut dest = vec![0u8; 8];
        let mut dsc = img_dsc(&mut dest, 3, &src, ColorFormat::L8, None, 255, BlendMode::Normal);
        dsc.dest_h = 2;
        dsc.dest_stride = 4;
        dsc.src_stride = 4;
        image(&mut dsc);
        assert_eq!(dest, [10, 20, 30, 0, 50, 60, 70, 0]);

        let src = [255u8, 0];
        let mut dest = vec![100u8; 2];
        image(&mut img_dsc(
            &mut dest,
            2,
            &src,
            ColorFormat::L8,
            None,
            128,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [177, ((100u32 * 127) >> 8) as u8]);

        let mask = [128u8, 0];
        let mut dest = vec![100u8; 2];
        image(&mut img_dsc(
            &mut dest,
            2,
            &src,
            ColorFormat::L8,
            Some(&mask),
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [177, 100]);
    }

    #[test]
    fn image_from_l8_blend_modes() {
        let src = [100u8, 200];
        let run = |mode| {
            let mut dest = vec![150u8; 2];
            image(&mut img_dsc(&mut dest, 2, &src, ColorFormat::L8, None, 255, mode));
            dest
        };
        // A grey keeps its luminance: (77 + 151 + 28) * v >> 8 == v.
        assert_eq!(run(BlendMode::Additive), [250, 255]);
        assert_eq!(run(BlendMode::Subtractive), [50, 0]);
        assert_eq!(
            run(BlendMode::Multiply),
            [((150u32 * 100) >> 8) as u8, ((150u32 * 200) >> 8) as u8]
        );
        assert_eq!(run(BlendMode::Difference), [50, 50]);
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn image_from_argb8888() {
        let src = argb(&[(255, 0, 0, 255), (255, 255, 255, 128), (255, 255, 255, 0)]);
        let mut dest = vec![100u8; 3];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::Argb8888,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [76, 177, 100]);

        // opa only: alpha = LV_OPA_MIX2(255, 128) = 127
        let expect = ((76u32 * 127 + 100 * 128) >> 8) as u8;
        let mut dest = vec![100u8; 1];
        image(&mut img_dsc(
            &mut dest,
            1,
            &src[..4],
            ColorFormat::Argb8888,
            None,
            128,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [expect]);

        // mask + opa: LV_OPA_MIX3(255, 128, 255) = 127
        let mask = [255u8];
        let mut dest = vec![100u8; 1];
        image(&mut img_dsc(
            &mut dest,
            1,
            &src[..4],
            ColorFormat::Argb8888,
            Some(&mask),
            128,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [expect]);

        // Non-normal modes use the straight colour with the mixed alpha.
        let mut dest = vec![100u8; 1];
        image(&mut img_dsc(
            &mut dest,
            1,
            &src[..4],
            ColorFormat::Argb8888,
            None,
            255,
            BlendMode::Additive,
        ));
        assert_eq!(dest, [176]);
    }

    #[cfg(feature = "cf-argb8888-premultiplied")]
    #[test]
    fn image_from_argb8888_premultiplied() {
        // A premultiplied half-transparent white is unscaled back to ~255
        // before mixing.
        let src = argb(&[(128, 128, 128, 128)]);
        let mut dest = vec![0u8; 1];
        image(&mut img_dsc(
            &mut dest,
            1,
            &src,
            ColorFormat::Argb8888Premultiplied,
            None,
            255,
            BlendMode::Normal,
        ));
        let c = Color32 {
            blue: 128,
            green: 128,
            red: 128,
            alpha: 128,
        };
        let lumi = color32_lumi_of(c, true);
        assert_eq!(lumi, 255);
        assert_eq!(dest, [((lumi as u32 * 128) >> 8) as u8]);
    }

    #[cfg(feature = "cf-rgb565")]
    #[test]
    fn image_from_rgb565() {
        let px = Color16::new(31, 0, 0).0.to_le_bytes();
        let mut dest = vec![0u8; 1];
        image(&mut img_dsc(
            &mut dest,
            1,
            &px,
            ColorFormat::Rgb565,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [((635u32 * 31) >> 8) as u8]);
    }

    #[cfg(feature = "cf-rgb565-swapped")]
    #[test]
    fn image_from_rgb565_swapped() {
        let px = Color16::new(31, 0, 0).0.to_be_bytes();
        let mut dest = vec![0u8; 1];
        image(&mut img_dsc(
            &mut dest,
            1,
            &px,
            ColorFormat::Rgb565Swapped,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [((635u32 * 31) >> 8) as u8]);
    }

    #[cfg(feature = "cf-xrgb8888")]
    #[test]
    fn image_from_xrgb8888() {
        let src = argb(&[(0, 255, 0, 0), (0, 0, 255, 0)]);
        let mut dest = vec![0u8; 2];
        image(&mut img_dsc(
            &mut dest,
            2,
            &src,
            ColorFormat::Xrgb8888,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [((151u32 * 255) >> 8) as u8, ((28u32 * 255) >> 8) as u8]);
    }

    #[cfg(feature = "cf-al88")]
    #[test]
    fn image_from_al88() {
        let src = [200u8, 255, 200, 128, 200, 0];
        let mut dest = vec![100u8; 3];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::Al88,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [200, ((200u32 * 128 + 100 * 127) >> 8) as u8, 100]);
    }

    #[cfg(feature = "cf-i1")]
    #[test]
    fn image_from_i1() {
        assert_eq!(get_bit(&[0b1000_0001], 0), 1);
        assert_eq!(get_bit(&[0b1000_0001], 1), 0);
        assert_eq!(get_bit(&[0b1000_0001], 7), 1);
        let src = [0b1010_0000u8];
        let mut dest = vec![100u8; 3];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::I1,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [255, 0, 255]);
    }

    #[test]
    fn unsupported_format_is_ignored() {
        let src = [1u8, 2, 3, 4];
        let mut dest = vec![7u8; 2];
        image(&mut img_dsc(
            &mut dest,
            2,
            &src,
            ColorFormat::Yuy2,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [7, 7]);
    }
}
