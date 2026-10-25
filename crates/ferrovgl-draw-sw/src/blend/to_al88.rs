//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_al88.c`.
//!
//! Blend a colour or an image into a luminance + alpha (`AL88`) buffer.
//! Each destination pixel is two bytes: `lumi` then `alpha`
//! (`lv_color16a_t`).
//!
//! Port notes:
//! - The `LV_DRAW_SW_*_TO_AL88*` acceleration hooks are always
//!   `LV_RESULT_INVALID` in the software renderer, so only the C fallback
//!   loops are ported.
//! - Buffers are byte slices; the C "row pointer += stride" becomes a byte
//!   offset advanced with [`drawbuf_next_row`]. Multi-byte source pixels are
//!   read little-endian, like the C structs on the reference targets.
//! - `LV_LOG_WARN` paths (unknown source format or blend mode) do nothing.
//! - Upstream quirk kept: in `i1_image_blend()` the non-normal blend-mode
//!   loop is the last `else` of an exhaustive `else if` chain *inside* the
//!   `LV_BLEND_MODE_NORMAL` branch, so it is unreachable and I1 sources
//!   blended with a non-normal mode leave the destination untouched.
//! - The RGB888 kernel uses four independent `if`s upstream; their
//!   conditions are mutually exclusive and no body changes `opa` or the
//!   nullness of the mask, so a `match` is equivalent.

#[cfg(feature = "cf-rgb565")]
use ferrovgl_core::misc::color::Color16;
#[cfg(any(feature = "cf-argb8888", feature = "cf-argb8888-premultiplied"))]
use ferrovgl_core::misc::color::Color32;
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
use ferrovgl_core::misc::color::color24_luminance;
use ferrovgl_core::misc::color::{Color16a, ColorFormat, OPA_MAX, OPA_MIN, Opa, opa_mix2, opa_mix3};
use ferrovgl_core::misc::style::BlendMode;

#[cfg(any(feature = "cf-argb8888", feature = "cf-argb8888-premultiplied"))]
use crate::blend::color32_lumi_of;
use crate::blend::{BlendFillDsc, BlendImageDsc};

/// `lv_color_mix_alpha_cache_t`: remembers the last "both colours have
/// alpha" mix so that runs of identical pixels are not recomputed.
#[derive(Copy, Clone, Debug)]
struct ColorMixAlphaCache {
    fg_saved: Color16a,
    bg_saved: Color16a,
    res_saved: Color16a,
    res_alpha_saved: Opa,
    ratio_saved: Opa,
}

/// `lv_draw_sw_blend_color_to_al88()`: fill with a colour (+ opa / mask).
pub fn fill(dsc: &mut BlendFillDsc<'_>) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let mask = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_stride = dsc.dest_stride;
    let lumi = dsc.color.luminance();
    let dest_buf = &mut *dsc.dest_buf;

    let mut cache = color_mix_with_alpha_cache_init();
    let mut d = 0usize;

    match mask {
        /*Simple fill*/
        None if opa >= OPA_MAX => {
            let color16a = Color16a { lumi, alpha: 255 };
            for _y in 0..h {
                let mut x = 0i32;
                while x < w - 16 {
                    for i in 0..16 {
                        store(dest_buf, d, (x + i) as usize, color16a);
                    }
                    x += 16;
                }
                while x < w {
                    store(dest_buf, d, x as usize, color16a);
                    x += 1;
                }
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        /*Opacity only*/
        None => {
            let color16a = Color16a { lumi, alpha: opa };
            for _y in 0..h {
                for x in 0..w.max(0) as usize {
                    mix_at(dest_buf, d, x, color16a, &mut cache);
                }
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        /*Masked with full opacity*/
        Some(mask) if opa >= OPA_MAX => {
            let mut m = 0usize;
            for _y in 0..h {
                for x in 0..w.max(0) as usize {
                    let color16a = Color16a {
                        lumi,
                        alpha: mask[m + x],
                    };
                    mix_at(dest_buf, d, x, color16a, &mut cache);
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
        /*Masked with opacity*/
        Some(mask) => {
            let mut m = 0usize;
            for _y in 0..h {
                for x in 0..w.max(0) as usize {
                    let color16a = Color16a {
                        lumi,
                        alpha: opa_mix2(mask[m + x], opa),
                    };
                    mix_at(dest_buf, d, x, color16a, &mut cache);
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_al88()`: blend an image of any supported
/// source format.
pub fn image(dsc: &mut BlendImageDsc<'_>) {
    match dsc.src_color_format {
        #[cfg(feature = "cf-rgb565")]
        ColorFormat::Rgb565 => rgb565_image_blend(dsc),
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
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_i1 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_buf_al88 = &mut *dsc.dest_buf;

    let mut cache = color_mix_with_alpha_cache_init();
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    let lumi = |s: usize, x: usize| get_bit(&src_buf_i1[s..], x as i32).wrapping_mul(255);

    if dsc.blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        store(
                            dest_buf_al88,
                            d,
                            x,
                            Color16a {
                                lumi: lumi(s, x),
                                alpha: 255,
                            },
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: lumi(s, x),
                            alpha: opa,
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: lumi(s, x),
                            alpha: mask_buf[m + x],
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: lumi(s, x),
                            alpha: opa_mix2(mask_buf[m + x], opa),
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
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

/// `l8_image_blend()`
#[cfg(feature = "cf-l8")]
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
    let dest_buf_al88 = &mut *dsc.dest_buf;

    let mut cache = color_mix_with_alpha_cache_init();
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        store(
                            dest_buf_al88,
                            d,
                            x,
                            Color16a {
                                lumi: src_buf_l8[s + x],
                                alpha: 255,
                            },
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: src_buf_l8[s + x],
                            alpha: opa,
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: src_buf_l8[s + x],
                            alpha: mask_buf[m + x],
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: src_buf_l8[s + x],
                            alpha: opa_mix2(mask_buf[m + x], opa),
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
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
                let src_color = Color16a {
                    lumi: src_buf_l8[s + x],
                    alpha: match mask_buf {
                        None => opa,
                        Some(mask_buf) => opa_mix2(mask_buf[m + x], opa),
                    },
                };
                blend_non_normal_at(dest_buf_al88, d, x, src_color, blend_mode, &mut cache);
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
    let dest_buf_al88 = &mut *dsc.dest_buf;

    let mut cache = color_mix_with_alpha_cache_init();
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        mix_at(dest_buf_al88, d, x, load(src_buf_al88, s, x), &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let mut src_color = load(src_buf_al88, s, x);
                        src_color.alpha = opa_mix2(src_color.alpha, opa);
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let mut src_color = load(src_buf_al88, s, x);
                        src_color.alpha = opa_mix2(src_color.alpha, mask_buf[m + x]);
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        let mut src_color = load(src_buf_al88, s, x);
                        src_color.alpha = opa_mix3(src_color.alpha, mask_buf[m + x], opa);
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
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
                let mut src_color = load(src_buf_al88, s, x);
                src_color.alpha = match mask_buf {
                    None => opa_mix2(src_color.alpha, opa),
                    Some(mask_buf) => opa_mix3(src_color.alpha, mask_buf[m + x], opa),
                };
                blend_non_normal_at(dest_buf_al88, d, x, src_color, blend_mode, &mut cache);
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
    let dest_buf_al88 = &mut *dsc.dest_buf;

    let mut cache = color_mix_with_alpha_cache_init();
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `lv_color16_luminance(src_buf_c16[x])`
    let lumi = |s: usize, x: usize| {
        let o = s + x * 2;
        Color16(u16::from_le_bytes([src_buf_c16[o], src_buf_c16[o + 1]])).luminance()
    };

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        store(
                            dest_buf_al88,
                            d,
                            x,
                            Color16a {
                                lumi: lumi(s, x),
                                alpha: 255,
                            },
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: lumi(s, x),
                            alpha: opa,
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: lumi(s, x),
                            alpha: mask_buf[m + x],
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                    m = drawbuf_next_row(m, mask_stride);
                }
            }
            Some(mask_buf) => {
                for _y in 0..h {
                    for x in 0..w {
                        let src_color = Color16a {
                            lumi: lumi(s, x),
                            alpha: opa_mix2(mask_buf[m + x], opa),
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
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
                let src_color = Color16a {
                    lumi: lumi(s, x),
                    alpha: match mask_buf {
                        None => opa,
                        Some(mask_buf) => opa_mix2(mask_buf[m + x], opa),
                    },
                };
                blend_non_normal_at(dest_buf_al88, d, x, src_color, blend_mode, &mut cache);
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
    let dest_buf_al88 = &mut *dsc.dest_buf;
    let px_size = src_px_size as usize;

    let mut cache = color_mix_with_alpha_cache_init();
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `lv_color24_luminance(&src_buf_u8[src_x])`
    let lumi = |o: usize| color24_luminance([src_buf_u8[o], src_buf_u8[o + 1], src_buf_u8[o + 2]]);

    if blend_mode == BlendMode::Normal {
        match mask_buf {
            /*Special case*/
            None if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        store(
                            dest_buf_al88,
                            d,
                            dest_x,
                            Color16a {
                                lumi: lumi(s + dest_x * px_size),
                                alpha: 255,
                            },
                        );
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let src_color = Color16a {
                            lumi: lumi(s + dest_x * px_size),
                            alpha: opa,
                        };
                        mix_at(dest_buf_al88, d, dest_x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for dest_x in 0..w {
                        let mask_x = dest_x;
                        let src_color = Color16a {
                            lumi: lumi(s + dest_x * px_size),
                            alpha: mask_buf[m + mask_x],
                        };
                        mix_at(dest_buf_al88, d, dest_x, src_color, &mut cache);
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
                        let src_color = Color16a {
                            lumi: lumi(s + dest_x * px_size),
                            alpha: opa_mix2(mask_buf[m + mask_x], opa),
                        };
                        mix_at(dest_buf_al88, d, dest_x, src_color, &mut cache);
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
                let src_color = Color16a {
                    lumi: lumi(s + dest_x * px_size),
                    alpha: match mask_buf {
                        None => opa,
                        Some(mask_buf) => opa_mix2(mask_buf[m + dest_x], opa),
                    },
                };
                blend_non_normal_at(dest_buf_al88, d, dest_x, src_color, blend_mode, &mut cache);
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
    let dest_buf_al88 = &mut *dsc.dest_buf;

    let mut cache = color_mix_with_alpha_cache_init();
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
                        let src_color = Color16a {
                            lumi: color32_lumi_of(c, premultiplied),
                            alpha: c.alpha,
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            None => {
                for _y in 0..h {
                    for x in 0..w {
                        let c = px(s, x);
                        let src_color = Color16a {
                            lumi: color32_lumi_of(c, premultiplied),
                            alpha: opa_mix2(c.alpha, opa),
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
                    }
                    d = drawbuf_next_row(d, dest_stride);
                    s = drawbuf_next_row(s, src_stride);
                }
            }
            Some(mask_buf) if opa >= OPA_MAX => {
                for _y in 0..h {
                    for x in 0..w {
                        let c = px(s, x);
                        let src_color = Color16a {
                            lumi: color32_lumi_of(c, premultiplied),
                            alpha: opa_mix2(c.alpha, mask_buf[m + x]),
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
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
                        let src_color = Color16a {
                            lumi: color32_lumi_of(c, premultiplied),
                            alpha: opa_mix3(c.alpha, mask_buf[m + x], opa),
                        };
                        mix_at(dest_buf_al88, d, x, src_color, &mut cache);
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
                let mut src_color = Color16a {
                    lumi: color32_lumi_of(c, premultiplied),
                    alpha: c.alpha,
                };
                src_color.alpha = match mask_buf {
                    None => opa_mix2(src_color.alpha, opa),
                    Some(mask_buf) => opa_mix3(src_color.alpha, mask_buf[m + x], opa),
                };
                blend_non_normal_at(dest_buf_al88, d, x, src_color, blend_mode, &mut cache);
            }
            if mask_buf.is_some() {
                m = drawbuf_next_row(m, mask_stride);
            }
            d = drawbuf_next_row(d, dest_stride);
            s = drawbuf_next_row(s, src_stride);
        }
    }
}

/// `lv_color16a_eq()`: compares both bytes, like the C `uint16_t` compare.
#[inline]
fn color16a_eq(c1: Color16a, c2: Color16a) -> bool {
    c1.lumi == c2.lumi && c1.alpha == c2.alpha
}

/// `lv_color_mix16a()`: mix the luminance of `fg` over `bg` by `fg.alpha`,
/// keeping `bg.alpha`.
#[inline]
fn color_mix16a(fg: Color16a, mut bg: Color16a) -> Color16a {
    bg.lumi = ((fg.lumi as u32 * fg.alpha as u32 + bg.lumi as u32 * (255 - fg.alpha as u32)) >> 8) as u8;
    bg
}

/// `lv_color_16a_16a_mix()`: alpha-composite `fg` over `*bg`.
#[inline]
fn color_16a_16a_mix(mut fg: Color16a, bg: &mut Color16a, cache: &mut ColorMixAlphaCache) {
    /*Pick the foreground if it's fully opaque or the Background is fully transparent*/
    if fg.alpha >= OPA_MAX || bg.alpha <= OPA_MIN {
        *bg = fg;
    }
    /*Transparent foreground: use the Background*/
    else if fg.alpha <= OPA_MIN {
        /* no need to copy */
    }
    /*Opaque background: use simple mix*/
    else if bg.alpha == 255 {
        *bg = color_mix16a(fg, *bg);
    }
    /*Both colors have alpha. Expensive calculation needs to be applied*/
    else {
        /*Save the parameters and the result. If they will be asked again don't compute again*/

        /*Update the ratio and the result alpha value if the input alpha values change*/
        if bg.alpha != cache.bg_saved.alpha || fg.alpha != cache.fg_saved.alpha {
            /*Info:
             * https://en.wikipedia.org/wiki/Alpha_compositing#Analytical_derivation_of_the_over_operator*/
            cache.res_alpha_saved = 255 - opa_mix2(255 - fg.alpha, 255 - bg.alpha);
            debug_assert!(cache.res_alpha_saved != 0);
            cache.ratio_saved = ((fg.alpha as u32 * 255) / cache.res_alpha_saved as u32) as Opa;
        }

        if !color16a_eq(*bg, cache.bg_saved) || !color16a_eq(fg, cache.fg_saved) {
            cache.fg_saved = fg;
            cache.bg_saved = *bg;
            fg.alpha = cache.ratio_saved;
            cache.res_saved = color_mix16a(fg, *bg);
            cache.res_saved.alpha = cache.res_alpha_saved;
        }

        *bg = cache.res_saved;
    }
}

/// `lv_color_mix_with_alpha_cache_init()`
fn color_mix_with_alpha_cache_init() -> ColorMixAlphaCache {
    ColorMixAlphaCache {
        fg_saved: Color16a::default(),
        bg_saved: Color16a::default(),
        res_saved: Color16a::default(),
        res_alpha_saved: 255,
        ratio_saved: 255,
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
fn blend_non_normal_pixel(
    dest: &mut Color16a,
    src: Color16a,
    mode: BlendMode,
    cache: &mut ColorMixAlphaCache,
) {
    let dl = dest.lumi as i32;
    let sl = src.lumi as i32;
    let lumi: u8 = match mode {
        BlendMode::Additive => (dl + sl).min(255) as u8,
        BlendMode::Subtractive => (dl - sl).max(0) as u8,
        BlendMode::Multiply => ((dl * sl) >> 8) as u8,
        BlendMode::Difference => (dl - sl).unsigned_abs() as u8,
        _ => {
            // LV_LOG_WARN("Not supported blend mode")
            return;
        }
    };
    let res = Color16a {
        lumi,
        alpha: src.alpha,
    };
    color_16a_16a_mix(res, dest, cache);
}

/// `drawbuf_next_row()`: advance a byte offset by one row of `stride` bytes.
#[inline]
fn drawbuf_next_row(ofs: usize, stride: i32) -> usize {
    ofs.wrapping_add_signed(stride as isize)
}

/// Read `buf[row..]` as `lv_color16a_t` pixel `x`.
#[inline]
fn load(buf: &[u8], row: usize, x: usize) -> Color16a {
    let o = row + x * 2;
    Color16a {
        lumi: buf[o],
        alpha: buf[o + 1],
    }
}

/// Write `c` to `lv_color16a_t` pixel `x` of the row at `buf[row..]`.
#[inline]
fn store(buf: &mut [u8], row: usize, x: usize, c: Color16a) {
    let o = row + x * 2;
    buf[o] = c.lumi;
    buf[o + 1] = c.alpha;
}

/// `lv_color_16a_16a_mix(fg, &dest_buf[x], cache)` on a byte buffer.
#[inline]
fn mix_at(buf: &mut [u8], row: usize, x: usize, fg: Color16a, cache: &mut ColorMixAlphaCache) {
    let mut bg = load(buf, row, x);
    color_16a_16a_mix(fg, &mut bg, cache);
    store(buf, row, x, bg);
}

/// `blend_non_normal_pixel(&dest_buf[x], src, mode, cache)` on a byte buffer.
#[inline]
fn blend_non_normal_at(
    buf: &mut [u8],
    row: usize,
    x: usize,
    src: Color16a,
    mode: BlendMode,
    cache: &mut ColorMixAlphaCache,
) {
    let mut dest = load(buf, row, x);
    blend_non_normal_pixel(&mut dest, src, mode, cache);
    store(buf, row, x, dest);
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

    #[cfg(feature = "cf-argb8888")]
    fn argb(pixels: &[(u8, u8, u8, u8)]) -> vec::Vec<u8> {
        pixels.iter().flat_map(|&(r, g, b, a)| [b, g, r, a]).collect()
    }

    fn la(lumi: u8, alpha: u8) -> Color16a {
        Color16a { lumi, alpha }
    }

    #[test]
    fn mix_16a_cases() {
        let mut cache = color_mix_with_alpha_cache_init();
        // Opaque foreground or transparent background: copy.
        let mut bg = la(10, 128);
        color_16a_16a_mix(la(200, 255), &mut bg, &mut cache);
        assert_eq!(bg, la(200, 255));
        let mut bg = la(10, 1);
        color_16a_16a_mix(la(200, 50), &mut bg, &mut cache);
        assert_eq!(bg, la(200, 50));
        // Transparent foreground: keep.
        let mut bg = la(10, 128);
        color_16a_16a_mix(la(200, 2), &mut bg, &mut cache);
        assert_eq!(bg, la(10, 128));
        // Opaque background: simple mix, alpha stays 255.
        let mut bg = la(100, 255);
        color_16a_16a_mix(la(255, 128), &mut bg, &mut cache);
        assert_eq!(bg, la(177, 255));
        // Both have alpha:
        // res_alpha = 255 - LV_OPA_MIX2(127, 127) = 192,
        // ratio = 128 * 255 / 192 = 170,
        // lumi = (255 * 170 + 100 * 85) >> 8 = 202.
        let mut bg = la(100, 128);
        color_16a_16a_mix(la(255, 128), &mut bg, &mut cache);
        assert_eq!(bg, la(202, 192));
        // Same inputs again come from the cache.
        let mut bg = la(100, 128);
        color_16a_16a_mix(la(255, 128), &mut bg, &mut cache);
        assert_eq!(bg, la(202, 192));
        assert!(color16a_eq(cache.fg_saved, la(255, 128)));
        assert!(color16a_eq(cache.bg_saved, la(100, 128)));
    }

    #[test]
    fn fill_simple_and_opa() {
        let red = Color::make(255, 0, 0);
        let mut buf = vec![0u8; 2 * 20 + 4];
        fill(&mut fill_dsc(&mut buf, 20, 1, 44, None, red, 255));
        for x in 0..20 {
            assert_eq!((buf[x * 2], buf[x * 2 + 1]), (76, 255));
        }
        assert_eq!(&buf[40..], &[0, 0, 0, 0]);

        // Opa into a transparent and into an opaque destination.
        let mut buf = vec![0, 0, 100, 255];
        fill(&mut fill_dsc(&mut buf, 2, 1, 4, None, Color::WHITE, 128));
        assert_eq!(buf, [255, 128, 177, 255]);
    }

    #[test]
    fn fill_mask() {
        let mask = [0u8, 255, 128];
        let mut buf = vec![100, 255, 100, 255, 100, 255];
        fill(&mut fill_dsc(&mut buf, 3, 1, 6, Some(&mask), Color::WHITE, 255));
        assert_eq!(buf, [100, 255, 255, 255, 177, 255]);

        let mask = [255u8];
        let mut buf = vec![100, 255];
        fill(&mut fill_dsc(&mut buf, 1, 1, 2, Some(&mask), Color::WHITE, 128));
        // LV_OPA_MIX2(255, 128) = 127
        assert_eq!(buf, [((255u32 * 127 + 100 * 128) >> 8) as u8, 255]);
    }

    #[test]
    fn image_from_al88() {
        let src = [200u8, 255, 200, 128, 200, 0];
        let mut dest = vec![100, 255, 100, 255, 100, 255];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::Al88,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(
            dest,
            [200, 255, ((200u32 * 128 + 100 * 127) >> 8) as u8, 255, 100, 255]
        );

        // Opa scales the source alpha.
        let mut dest = vec![0, 0];
        image(&mut img_dsc(
            &mut dest,
            1,
            &src[..2],
            ColorFormat::Al88,
            None,
            128,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [200, 127]);

        // Additive.
        let src = [100u8, 255];
        let mut dest = vec![100, 255];
        image(&mut img_dsc(
            &mut dest,
            1,
            &src,
            ColorFormat::Al88,
            None,
            255,
            BlendMode::Additive,
        ));
        // alpha = LV_OPA_MIX2(255, 255) = 254 >= LV_OPA_MAX: copied.
        assert_eq!(dest, [200, 254]);
    }

    #[cfg(feature = "cf-l8")]
    #[test]
    fn image_from_l8() {
        let src = [10u8, 20];
        let mut dest = vec![0u8; 4];
        image(&mut img_dsc(
            &mut dest,
            2,
            &src,
            ColorFormat::L8,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [10, 255, 20, 255]);

        let mask = [128u8, 0];
        let mut dest = vec![100, 255, 100, 255];
        image(&mut img_dsc(
            &mut dest,
            2,
            &[255, 255],
            ColorFormat::L8,
            Some(&mask),
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [177, 255, 100, 255]);

        let mut dest = vec![150, 255];
        image(&mut img_dsc(
            &mut dest,
            1,
            &[100],
            ColorFormat::L8,
            None,
            255,
            BlendMode::Difference,
        ));
        assert_eq!(dest, [50, 255]);
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn image_from_argb8888() {
        let src = argb(&[(255, 0, 0, 255), (255, 255, 255, 128), (255, 255, 255, 0)]);
        let mut dest = vec![100, 255, 100, 255, 100, 255];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::Argb8888,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [76, 255, 177, 255, 100, 255]);

        // Into a transparent destination the source is copied with its alpha.
        let mut dest = vec![0u8; 6];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::Argb8888,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [76, 255, 255, 128, 255, 0]);
    }

    #[cfg(feature = "cf-rgb565")]
    #[test]
    fn image_from_rgb565() {
        let px = Color16::new(0, 63, 0).0.to_le_bytes();
        let mut dest = vec![0u8; 2];
        image(&mut img_dsc(
            &mut dest,
            1,
            &px,
            ColorFormat::Rgb565,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [((613u32 * 63) >> 8) as u8, 255]);
    }

    #[cfg(feature = "cf-rgb888")]
    #[test]
    fn image_from_rgb888() {
        let src = [0u8, 0, 255, 255, 255, 255];
        let mut dest = vec![0u8; 4];
        image(&mut img_dsc(
            &mut dest,
            2,
            &src,
            ColorFormat::Rgb888,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [76, 255, 255, 255]);
    }

    #[cfg(feature = "cf-i1")]
    #[test]
    fn image_from_i1() {
        let src = [0b0100_0000u8];
        let mut dest = vec![9u8; 4];
        image(&mut img_dsc(
            &mut dest,
            2,
            &src,
            ColorFormat::I1,
            None,
            255,
            BlendMode::Normal,
        ));
        assert_eq!(dest, [0, 255, 255, 255]);

        // Upstream quirk: the non-normal path is unreachable for I1 sources.
        let mut dest = vec![9u8; 4];
        image(&mut img_dsc(
            &mut dest,
            2,
            &src,
            ColorFormat::I1,
            None,
            255,
            BlendMode::Additive,
        ));
        assert_eq!(dest, [9, 9, 9, 9]);
    }
}
