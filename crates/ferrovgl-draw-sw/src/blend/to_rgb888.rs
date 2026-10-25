//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_rgb888.c`.
//!
//! One kernel for both `RGB888` (`dest_px_size == 3`) and `XRGB8888`
//! (`dest_px_size == 4`). The word-at-a-time fills of upstream are ported
//! with [`crate::blend::words`] (aligned views, byte lead-in / tail), and
//! the words are kept little-endian in memory, which is exactly what the
//! upstream `LV_BIG_ENDIAN_SYSTEM == 0` code writes. The
//! `LV_DRAW_SW_*_TO_RGB888*` acceleration hooks are always
//! `LV_RESULT_INVALID` in the reference configuration and are omitted.
//!
//! Upstream quirks kept for parity: the non-normal blend modes of the
//! `L8`, `AL88`, `RGB888`/`XRGB8888`, `ARGB8888` and premultiplied sources
//! index the mask with the destination *byte* offset (`mask_buf[dest_x]`);
//! reads past the end of the mask slice yield 0 here instead of reading
//! unrelated memory.

#[cfg(any(
    feature = "cf-al88",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
use ferrovgl_core::misc::color::opa_mix3;
use ferrovgl_core::misc::color::{Color32, ColorFormat, OPA_MAX, Opa, opa_mix2};
use ferrovgl_core::misc::style::BlendMode;

use crate::blend::words;
use crate::blend::{BlendFillDsc, BlendImageDsc};

/// `lv_draw_sw_blend_color_to_rgb888()`: fill with a colour (+ opa / mask).
/// `dest_px_size` is the bytes per destination pixel: 3 for `RGB888`, 4 for
/// `XRGB8888` (this kernel renders both, `LV_DRAW_SW_SUPPORT_RGB888 ||
/// LV_DRAW_SW_SUPPORT_XRGB8888`).
pub fn fill(dsc: &mut BlendFillDsc<'_>, dest_px_size: u32) {
    let mut w = dsc.dest_w;
    let mut h = dsc.dest_h;
    let opa = dsc.opa;
    let mask = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_stride = dsc.dest_stride;
    let color = dsc.color;
    let dest_buf: &mut [u8] = dsc.dest_buf;

    match mask {
        // Simple fill
        None if opa >= OPA_MAX => {
            if dest_px_size == 3 {
                w *= dest_px_size as i32;
                // 4 pixels are exactly 3 words and the channel order inside a
                // word repeats every 3 words, so the whole area is written
                // with word stores and never read back.
                let b_ = color.blue as u32;
                let g_ = color.green as u32;
                let r_ = color.red as u32;
                let k: [u32; 3] = [
                    b_ | (g_ << 8) | (r_ << 16) | (b_ << 24),
                    g_ | (r_ << 8) | (b_ << 16) | (g_ << 24),
                    r_ | (b_ << 8) | (g_ << 16) | (r_ << 24),
                ];

                // Back to back rows are one long run
                if dest_stride == w {
                    w *= h;
                    h = 1;
                }

                let mut row_ofs = 0usize;
                for _y in 0..h {
                    let row = &mut dest_buf[row_ofs..row_ofs + w as usize];
                    let (head, dest_word, tail) = words::split_u32_mut(row);

                    // Lead in until word aligned
                    let mut ofs = 0usize;
                    for b in head.iter_mut() {
                        *b = (k[ofs] & 0xFF) as u8;
                        ofs += 1;
                        if ofs == 3 {
                            ofs = 0;
                        }
                    }

                    // 3 words are 4 whole pixels, so a block of 3 repeats
                    // forever and the channel order never has to be advanced
                    // inside the loop
                    let mut a0 = k[ofs];
                    let mut a1 = k[if ofs == 2 { 0 } else { ofs + 1 }];
                    let a2 = k[if ofs == 0 { 2 } else { ofs - 1 }];
                    let (w0, w1, w2) = (a0.to_le(), a1.to_le(), a2.to_le());
                    let end = dest_word.len();
                    let mut i = 0usize;
                    while i + 12 <= end {
                        dest_word[i] = w0;
                        dest_word[i + 1] = w1;
                        dest_word[i + 2] = w2;
                        dest_word[i + 3] = w0;
                        dest_word[i + 4] = w1;
                        dest_word[i + 5] = w2;
                        dest_word[i + 6] = w0;
                        dest_word[i + 7] = w1;
                        dest_word[i + 8] = w2;
                        dest_word[i + 9] = w0;
                        dest_word[i + 10] = w1;
                        dest_word[i + 11] = w2;
                        i += 12;
                    }
                    while i + 3 <= end {
                        dest_word[i] = w0;
                        dest_word[i + 1] = w1;
                        dest_word[i + 2] = w2;
                        i += 3;
                    }
                    if i < end {
                        dest_word[i] = a0.to_le();
                        i += 1;
                        a0 = a1;
                        a1 = a2;
                    }
                    if i < end {
                        dest_word[i] = a0.to_le();
                        a0 = a1;
                    }

                    // At most 3 bytes left, and `a0` already starts on the
                    // right channel
                    for b in tail.iter_mut() {
                        *b = (a0 & 0xFF) as u8;
                        a0 >>= 8;
                    }

                    row_ofs = row_ofs.wrapping_add(dest_stride as usize);
                }
            }
            if dest_px_size == 4 {
                let color32 = color.to_u32();
                // Back to back rows are one long run
                if dest_stride == w * 4 {
                    w *= h;
                    h = 1;
                }
                let mut row_ofs = 0usize;
                for _y in 0..h {
                    let row = &mut dest_buf[row_ofs..row_ofs + (w as usize) * 4];
                    // The 16x unrolled stores of upstream: plain word stores.
                    words::fill_u32_le(row, color32);
                    row_ofs = row_ofs.wrapping_add(dest_stride as usize);
                }
            }
        }
        // Opacity only
        None => {
            w *= dest_px_size as i32;

            // Every channel is `(fg * opa + bg * (255 - opa)) >> 8`, which
            // stays below 255 * 255, so two of them fit in the 16 bit halves
            // of a word and blend with one multiplication.
            let mix_inv: u32 = 255 - opa as u32;
            let fg_premult: [u32; 3] = [
                color.blue as u32 * opa as u32,
                color.green as u32 * opa as u32,
                color.red as u32 * opa as u32,
            ];

            if dest_px_size == 3 {
                // Constants for a word starting on channel `r`: k_lo for
                // bytes 0 and 2, k_hi for bytes 1 and 3
                let mut k_lo = [0u32; 3];
                let mut k_hi = [0u32; 3];
                for x in 0..3 {
                    k_lo[x] = fg_premult[x] | (fg_premult[(x + 2) % 3] << 16);
                    k_hi[x] = fg_premult[(x + 1) % 3] | (fg_premult[x] << 16);
                }
                // Flat areas are common, so remember the last destination
                // word and its result, one per channel role. Seeded with 0 so
                // the entry is always valid.
                let mut last_d = [0u32; 3];
                let mut last_res = [0u32; 3];
                for x in 0..3 {
                    last_res[x] = ((k_lo[x] >> 8) & 0x00FF_00FF) | (((k_hi[x] >> 8) & 0x00FF_00FF) << 8);
                }
                let mut row_ofs = 0usize;
                for _y in 0..h {
                    let mut role = 0usize;
                    let row = &mut dest_buf[row_ofs..row_ofs + w as usize];
                    let (head, dest_word, tail) = words::split_u32_mut(row);
                    // Lead in until word aligned
                    for b in head.iter_mut() {
                        *b = ((fg_premult[role] + *b as u32 * mix_inv) >> 8) as u8;
                        role += 1;
                        if role == 3 {
                            role = 0;
                        }
                    }
                    for word in dest_word.iter_mut() {
                        let d = u32::from_le(*word);
                        let res = if d == last_d[role] {
                            last_res[role]
                        } else {
                            let lo = ((((d & 0x00FF_00FF) * mix_inv) + k_lo[role]) >> 8) & 0x00FF_00FF;
                            let hi = (((((d >> 8) & 0x00FF_00FF) * mix_inv) + k_hi[role]) >> 8) & 0x00FF_00FF;
                            let res = lo | (hi << 8);
                            last_d[role] = d;
                            last_res[role] = res;
                            res
                        };
                        *word = res.to_le();
                        // 4 bytes move the channel order on by one
                        role += 1;
                        if role == 3 {
                            role = 0;
                        }
                    }
                    for b in tail.iter_mut() {
                        *b = ((fg_premult[role] + *b as u32 * mix_inv) >> 8) as u8;
                        role += 1;
                        if role == 3 {
                            role = 0;
                        }
                    }
                    row_ofs = row_ofs.wrapping_add(dest_stride as usize);
                }
            } else {
                // 32 bit pixels: one word each, and the 4th byte must be left
                // alone
                let k_lo = fg_premult[0] | (fg_premult[2] << 16);
                let k_g = fg_premult[1];
                // Remember the last destination pixel and its result. Seeded
                // with 0 so the entry is always valid.
                let mut last_d = 0u32;
                let mut last_res = ((k_lo >> 8) & 0x00FF_00FF) | (((k_g >> 8) & 0xFF) << 8);
                let mut row_ofs = 0usize;
                for _y in 0..h {
                    let row = &mut dest_buf[row_ofs..row_ofs + w as usize];
                    words::map_u32_le(row, |d| {
                        if d == last_d {
                            last_res
                        } else {
                            let lo = ((((d & 0x00FF_00FF) * mix_inv) + k_lo) >> 8) & 0x00FF_00FF;
                            let g = (((((d >> 8) & 0xFF) * mix_inv) + k_g) >> 8) & 0xFF;
                            let res = lo | (g << 8) | (d & 0xFF00_0000);
                            last_d = d;
                            last_res = res;
                            res
                        }
                    });
                    row_ofs = row_ofs.wrapping_add(dest_stride as usize);
                }
            }
        }
        // Masked with full opacity
        Some(mask) if opa >= OPA_MAX => {
            let color32 = color.to_u32().to_le_bytes();
            w *= dest_px_size as i32;
            let mut dest_ofs = 0usize;
            let mut mask_ofs = 0usize;
            for _y in 0..h {
                let mut x = 0i32;
                let mut mask_x = 0usize;
                while x < w {
                    let m = mask[mask_ofs + mask_x];
                    if m != 0 {
                        let d = dest_ofs + x as usize;
                        color_24_24_mix(&color32, &mut dest_buf[d..d + 3], m);
                    }
                    x += dest_px_size as i32;
                    mask_x += 1;
                }
                dest_ofs = dest_ofs.wrapping_add(dest_stride as usize);
                mask_ofs = mask_ofs.wrapping_add(mask_stride as usize);
            }
        }
        // Masked with opacity
        Some(mask) => {
            let color32 = color.to_u32().to_le_bytes();
            w *= dest_px_size as i32;
            let mut dest_ofs = 0usize;
            let mut mask_ofs = 0usize;
            for _y in 0..h {
                let mut x = 0i32;
                let mut mask_x = 0usize;
                while x < w {
                    let d = dest_ofs + x as usize;
                    color_24_24_mix(
                        &color32,
                        &mut dest_buf[d..d + 3],
                        opa_mix2(opa, mask[mask_ofs + mask_x]),
                    );
                    x += dest_px_size as i32;
                    mask_x += 1;
                }
                dest_ofs = dest_ofs.wrapping_add(dest_stride as usize);
                mask_ofs = mask_ofs.wrapping_add(mask_stride as usize);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_rgb888()`: blend an image of any supported
/// source format. `dest_px_size` is 3 (`RGB888`) or 4 (`XRGB8888`).
pub fn image(dsc: &mut BlendImageDsc<'_>, dest_px_size: u32) {
    match dsc.src_color_format {
        #[cfg(feature = "cf-rgb565")]
        ColorFormat::Rgb565 => rgb565_image_blend(dsc, dest_px_size),
        ColorFormat::Rgb888 => rgb888_image_blend(dsc, dest_px_size as u8, 3),
        #[cfg(feature = "cf-xrgb8888")]
        ColorFormat::Xrgb8888 => rgb888_image_blend(dsc, dest_px_size as u8, 4),
        #[cfg(feature = "cf-argb8888")]
        ColorFormat::Argb8888 => argb8888_image_blend(dsc, dest_px_size),
        #[cfg(feature = "cf-argb8888-premultiplied")]
        ColorFormat::Argb8888Premultiplied => argb8888_premultiplied_image_blend(dsc, dest_px_size),
        #[cfg(feature = "cf-l8")]
        ColorFormat::L8 => l8_image_blend(dsc, dest_px_size),
        #[cfg(feature = "cf-al88")]
        ColorFormat::Al88 => al88_image_blend(dsc, dest_px_size),
        #[cfg(feature = "cf-i1")]
        ColorFormat::I1 => i1_image_blend(dsc, dest_px_size),
        // LV_LOG_WARN("Not supported source color format")
        _ => {}
    }
}

/// `mask_buf[i]` for the upstream paths that may index past the row with a
/// byte offset; 0 past the end of the slice.
#[inline]
fn mask_at(mask: &[u8], i: usize) -> Opa {
    mask.get(i).copied().unwrap_or(0)
}

/// `i1_image_blend()`
#[cfg(feature = "cf-i1")]
fn i1_image_blend(dsc: &mut BlendImageDsc<'_>, dest_px_size: u32) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride as usize;
    let src_buf_i1 = dsc.src_buf;
    let src_stride = dsc.src_stride as usize;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride as usize;
    let blend_mode = dsc.blend_mode;
    let dest_buf_u8: &mut [u8] = dsc.dest_buf;
    let px = dest_px_size as usize;

    let mut dest_ofs = 0usize;
    let mut src_ofs = 0usize;
    let mut mask_ofs = 0usize;

    if blend_mode == BlendMode::Normal {
        for _y in 0..h {
            let src_row = &src_buf_i1[src_ofs..];
            let mut dest_x = dest_ofs;
            for src_x in 0..w {
                let chan_val = get_bit(src_row, src_x).wrapping_mul(255);
                let dest = &mut dest_buf_u8[dest_x..dest_x + 3];
                match mask_buf {
                    None if opa >= OPA_MAX => {
                        dest[2] = chan_val;
                        dest[1] = chan_val;
                        dest[0] = chan_val;
                    }
                    None => color_8_24_mix(chan_val, dest, opa),
                    Some(m) if opa >= OPA_MAX => color_8_24_mix(chan_val, dest, m[mask_ofs + src_x as usize]),
                    Some(m) => color_8_24_mix(chan_val, dest, opa_mix2(opa, m[mask_ofs + src_x as usize])),
                }
                dest_x += px;
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            let src_row = &src_buf_i1[src_ofs..];
            let mut dest_x = dest_ofs;
            for src_x in 0..w {
                let v = get_bit(src_row, src_x).wrapping_mul(255);
                let src_argb = Color32 {
                    red: v,
                    green: v,
                    blue: v,
                    alpha: match mask_buf {
                        None => opa,
                        Some(m) => opa_mix2(m[mask_ofs + src_x as usize], opa),
                    },
                };
                blend_non_normal_pixel(&mut dest_buf_u8[dest_x..dest_x + 3], src_argb, blend_mode);
                dest_x += px;
            }
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
        }
    }
}

/// `al88_image_blend()`
#[cfg(feature = "cf-al88")]
fn al88_image_blend(dsc: &mut BlendImageDsc<'_>, dest_px_size: u32) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride as usize;
    let src_buf_al88 = dsc.src_buf;
    let src_stride = dsc.src_stride as usize;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride as usize;
    let blend_mode = dsc.blend_mode;
    let dest_buf_u8: &mut [u8] = dsc.dest_buf;
    let px = dest_px_size as usize;

    let mut dest_ofs = 0usize;
    let mut src_ofs = 0usize;
    let mut mask_ofs = 0usize;

    if blend_mode == BlendMode::Normal {
        for _y in 0..h {
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let s = src_ofs + src_x * 2;
                let (lumi, alpha) = (src_buf_al88[s], src_buf_al88[s + 1]);
                let d = dest_ofs + dest_x;
                let dest = &mut dest_buf_u8[d..d + 3];
                let mix = match mask_buf {
                    None if opa >= OPA_MAX => alpha,
                    None => opa_mix2(alpha, opa),
                    Some(m) if opa >= OPA_MAX => opa_mix2(alpha, m[mask_ofs + src_x]),
                    Some(m) => opa_mix3(alpha, m[mask_ofs + src_x], opa),
                };
                color_8_24_mix(lumi, dest, mix);
                dest_x += px;
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let s = src_ofs + src_x * 2;
                let (lumi, alpha) = (src_buf_al88[s], src_buf_al88[s + 1]);
                let src_argb = Color32 {
                    red: lumi,
                    green: lumi,
                    blue: lumi,
                    alpha: match mask_buf {
                        None => opa_mix2(alpha, opa),
                        Some(m) => opa_mix3(alpha, mask_at(m, mask_ofs + dest_x), opa),
                    },
                };
                let d = dest_ofs + dest_x;
                blend_non_normal_pixel(&mut dest_buf_u8[d..d + 3], src_argb, blend_mode);
                dest_x += px;
            }
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
        }
    }
}

/// `l8_image_blend()`
#[cfg(feature = "cf-l8")]
fn l8_image_blend(dsc: &mut BlendImageDsc<'_>, dest_px_size: u32) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride as usize;
    let src_buf_l8 = dsc.src_buf;
    let src_stride = dsc.src_stride as usize;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride as usize;
    let blend_mode = dsc.blend_mode;
    let dest_buf_u8: &mut [u8] = dsc.dest_buf;
    let px = dest_px_size as usize;

    let mut dest_ofs = 0usize;
    let mut src_ofs = 0usize;
    let mut mask_ofs = 0usize;

    if blend_mode == BlendMode::Normal {
        for _y in 0..h {
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let l = src_buf_l8[src_ofs + src_x];
                let d = dest_ofs + dest_x;
                let dest = &mut dest_buf_u8[d..d + 3];
                match mask_buf {
                    None if opa >= OPA_MAX => {
                        dest[2] = l;
                        dest[1] = l;
                        dest[0] = l;
                    }
                    None => color_8_24_mix(l, dest, opa),
                    Some(m) if opa >= OPA_MAX => color_8_24_mix(l, dest, m[mask_ofs + src_x]),
                    Some(m) => color_8_24_mix(l, dest, opa_mix2(opa, m[mask_ofs + src_x])),
                }
                dest_x += px;
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let l = src_buf_l8[src_ofs + src_x];
                let src_argb = Color32 {
                    red: l,
                    green: l,
                    blue: l,
                    alpha: match mask_buf {
                        None => opa,
                        Some(m) => opa_mix2(mask_at(m, mask_ofs + dest_x), opa),
                    },
                };
                let d = dest_ofs + dest_x;
                blend_non_normal_pixel(&mut dest_buf_u8[d..d + 3], src_argb, blend_mode);
                dest_x += px;
            }
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
        }
    }
}

/// `src_buf_c16[i].red * 2106 >> 8` etc.: the rounded RGB565 -> RGB888
/// expansion of `rgb565_image_blend()`, as `[blue, green, red]`.
#[cfg(feature = "cf-rgb565")]
#[inline]
fn rgb565_to_888(src: &[u8], i: usize) -> [u8; 3] {
    let c = u16::from_le_bytes([src[i * 2], src[i * 2 + 1]]) as u32;
    let blue = c & 0x1F;
    let green = (c >> 5) & 0x3F;
    let red = (c >> 11) & 0x1F;
    // To make it rounded
    [
        ((blue * 2106) >> 8) as u8,
        ((green * 1037) >> 8) as u8,
        ((red * 2106) >> 8) as u8,
    ]
}

/// `rgb565_image_blend()`
#[cfg(feature = "cf-rgb565")]
fn rgb565_image_blend(dsc: &mut BlendImageDsc<'_>, dest_px_size: u32) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride as usize;
    let src_buf_c16 = dsc.src_buf;
    let src_stride = dsc.src_stride as usize;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride as usize;
    let blend_mode = dsc.blend_mode;
    let dest_buf_u8: &mut [u8] = dsc.dest_buf;
    let px = dest_px_size as usize;

    let mut dest_ofs = 0usize;
    let mut src_ofs = 0usize;
    let mut mask_ofs = 0usize;

    if blend_mode == BlendMode::Normal {
        for _y in 0..h {
            let src_row = &src_buf_c16[src_ofs..];
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let res = rgb565_to_888(src_row, src_x);
                let d = dest_ofs + dest_x;
                let dest = &mut dest_buf_u8[d..d + 3];
                match mask_buf {
                    None if opa >= OPA_MAX => {
                        dest[2] = res[2];
                        dest[1] = res[1];
                        dest[0] = res[0];
                    }
                    None => color_24_24_mix(&res, dest, opa),
                    Some(m) if opa >= OPA_MAX => color_24_24_mix(&res, dest, m[mask_ofs + src_x]),
                    Some(m) => color_24_24_mix(&res, dest, opa_mix2(opa, m[mask_ofs + src_x])),
                }
                dest_x += px;
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            let src_row = &src_buf_c16[src_ofs..];
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let res = rgb565_to_888(src_row, src_x);
                let src_argb = Color32 {
                    red: res[2],
                    green: res[1],
                    blue: res[0],
                    alpha: match mask_buf {
                        None => opa,
                        Some(m) => opa_mix2(m[mask_ofs + src_x], opa),
                    },
                };
                let d = dest_ofs + dest_x;
                blend_non_normal_pixel(&mut dest_buf_u8[d..d + 3], src_argb, blend_mode);
                dest_x += px;
            }
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
        }
    }
}

/// `rgb888_image_blend()`: `RGB888` (`src_px_size == 3`) and `XRGB8888`
/// (`src_px_size == 4`) sources.
fn rgb888_image_blend(dsc: &mut BlendImageDsc<'_>, dest_px_size: u8, src_px_size: u32) {
    let w = (dsc.dest_w * dest_px_size as i32) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride as usize;
    let src_buf = dsc.src_buf;
    let src_stride = dsc.src_stride as usize;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride as usize;
    let blend_mode = dsc.blend_mode;
    let dest_buf: &mut [u8] = dsc.dest_buf;
    let dpx = dest_px_size as usize;
    let spx = src_px_size as usize;

    let mut dest_ofs = 0usize;
    let mut src_ofs = 0usize;
    let mut mask_ofs = 0usize;

    if blend_mode == BlendMode::Normal {
        // Special case
        if mask_buf.is_none() && opa >= OPA_MAX {
            if src_px_size == dest_px_size as u32 {
                for _y in 0..h {
                    dest_buf[dest_ofs..dest_ofs + w].copy_from_slice(&src_buf[src_ofs..src_ofs + w]);
                    dest_ofs = dest_ofs.wrapping_add(dest_stride);
                    src_ofs = src_ofs.wrapping_add(src_stride);
                }
            } else {
                for _y in 0..h {
                    let mut dest_x = 0usize;
                    let mut src_x = 0usize;
                    while dest_x < w {
                        let d = dest_ofs + dest_x;
                        let s = src_ofs + src_x;
                        dest_buf[d] = src_buf[s];
                        dest_buf[d + 1] = src_buf[s + 1];
                        dest_buf[d + 2] = src_buf[s + 2];
                        dest_x += dpx;
                        src_x += spx;
                    }
                    dest_ofs = dest_ofs.wrapping_add(dest_stride);
                    src_ofs = src_ofs.wrapping_add(src_stride);
                }
            }
        }
        if mask_buf.is_none() && opa < OPA_MAX {
            for _y in 0..h {
                let mut dest_x = 0usize;
                let mut src_x = 0usize;
                while dest_x < w {
                    let d = dest_ofs + dest_x;
                    let s = src_ofs + src_x;
                    color_24_24_mix(&src_buf[s..s + 3], &mut dest_buf[d..d + 3], opa);
                    dest_x += dpx;
                    src_x += spx;
                }
                dest_ofs = dest_ofs.wrapping_add(dest_stride);
                src_ofs = src_ofs.wrapping_add(src_stride);
            }
        }
        if let Some(m) = mask_buf {
            for _y in 0..h {
                let mut mask_x = 0usize;
                let mut dest_x = 0usize;
                let mut src_x = 0usize;
                while dest_x < w {
                    let d = dest_ofs + dest_x;
                    let s = src_ofs + src_x;
                    let mix = if opa >= OPA_MAX {
                        m[mask_ofs + mask_x]
                    } else {
                        opa_mix2(opa, m[mask_ofs + mask_x])
                    };
                    color_24_24_mix(&src_buf[s..s + 3], &mut dest_buf[d..d + 3], mix);
                    mask_x += 1;
                    dest_x += dpx;
                    src_x += spx;
                }
                dest_ofs = dest_ofs.wrapping_add(dest_stride);
                src_ofs = src_ofs.wrapping_add(src_stride);
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            let mut dest_x = 0usize;
            let mut src_x = 0usize;
            while dest_x < w {
                let s = src_ofs + src_x;
                let src_argb = Color32 {
                    red: src_buf[s + 2],
                    green: src_buf[s + 1],
                    blue: src_buf[s],
                    alpha: match mask_buf {
                        None => opa,
                        Some(m) => opa_mix2(mask_at(m, mask_ofs + dest_x), opa),
                    },
                };
                let d = dest_ofs + dest_x;
                blend_non_normal_pixel(&mut dest_buf[d..d + 3], src_argb, blend_mode);
                dest_x += dpx;
                src_x += spx;
            }
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
        }
    }
}

/// `argb8888_image_blend()`
#[cfg(feature = "cf-argb8888")]
fn argb8888_image_blend(dsc: &mut BlendImageDsc<'_>, dest_px_size: u32) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride as usize;
    let src_buf_c32 = dsc.src_buf;
    let src_stride = dsc.src_stride as usize;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride as usize;
    let blend_mode = dsc.blend_mode;
    let dest_buf: &mut [u8] = dsc.dest_buf;
    let px = dest_px_size as usize;

    let mut dest_ofs = 0usize;
    let mut src_ofs = 0usize;
    let mut mask_ofs = 0usize;

    if blend_mode == BlendMode::Normal {
        for _y in 0..h {
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let s = src_ofs + src_x * 4;
                let src = &src_buf_c32[s..s + 4];
                let alpha = src[3];
                let mix = match mask_buf {
                    None if opa >= OPA_MAX => alpha,
                    None => opa_mix2(alpha, opa),
                    Some(m) if opa >= OPA_MAX => opa_mix2(alpha, m[mask_ofs + src_x]),
                    Some(m) => opa_mix3(alpha, m[mask_ofs + src_x], opa),
                };
                let d = dest_ofs + dest_x;
                color_24_24_mix(src, &mut dest_buf[d..d + 3], mix);
                dest_x += px;
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let s = src_ofs + src_x * 4;
                let mut src_argb = color32_at(src_buf_c32, s);
                src_argb.alpha = match mask_buf {
                    None => opa_mix2(src_argb.alpha, opa),
                    Some(m) => opa_mix3(src_argb.alpha, mask_at(m, mask_ofs + dest_x), opa),
                };
                let d = dest_ofs + dest_x;
                blend_non_normal_pixel(&mut dest_buf[d..d + 3], src_argb, blend_mode);
                dest_x += px;
            }
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
        }
    }
}

/// An `lv_color32_t` read from 4 bytes (`[b, g, r, a]`) at `s`.
#[cfg(any(feature = "cf-argb8888", feature = "cf-argb8888-premultiplied"))]
#[inline]
fn color32_at(buf: &[u8], s: usize) -> Color32 {
    Color32 {
        blue: buf[s],
        green: buf[s + 1],
        red: buf[s + 2],
        alpha: buf[s + 3],
    }
}

/// `lv_color_24_24_mix_premult()`: `src` is premultiplied, so only the
/// background is weighted (`dest = src + dest * (255 - mix) >> 8`).
#[cfg(feature = "cf-argb8888-premultiplied")]
#[inline]
fn color_24_24_mix_premult(src: &[u8], dest: &mut [u8], mix: u8) {
    if mix == 0 {
        return;
    }

    if mix >= OPA_MAX {
        dest[0] = src[0];
        dest[1] = src[1];
        dest[2] = src[2];
    } else {
        // Weight the first and third background channels in one 32 bit
        // value with a single multiplication. The 16 bit lanes can't
        // overflow as their max value is 255 * 255
        let mix_inv = (255 - mix) as u32;
        let rb = (((dest[2] as u32) << 16) | dest[0] as u32) * mix_inv;
        dest[0] = src[0].wrapping_add(((rb >> 8) & 0xFF) as u8);
        dest[1] = (src[1] as u32).wrapping_add((dest[1] as u32 * mix_inv) >> 8) as u8;
        dest[2] = (src[2] as u32).wrapping_add(rb >> 24) as u8;
    }
}

/// `argb8888_premultiplied_image_blend()`
#[cfg(feature = "cf-argb8888-premultiplied")]
fn argb8888_premultiplied_image_blend(dsc: &mut BlendImageDsc<'_>, dest_px_size: u32) {
    let w = dsc.dest_w;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride as usize;
    let src_buf_c32 = dsc.src_buf;
    let src_stride = dsc.src_stride as usize;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride as usize;
    let blend_mode = dsc.blend_mode;
    let dest_buf: &mut [u8] = dsc.dest_buf;
    let px = dest_px_size as usize;

    let mut dest_ofs = 0usize;
    let mut src_ofs = 0usize;
    let mut mask_ofs = 0usize;

    if blend_mode == BlendMode::Normal {
        for _y in 0..h {
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let s = src_ofs + src_x * 4;
                let src_pixel = color32_at(src_buf_c32, s);
                let d = dest_ofs + dest_x;
                let dest = &mut dest_buf[d..d + 3];
                match mask_buf {
                    None if opa >= OPA_MAX => {
                        // For the trivial case use the premultiplied image as
                        // it is.
                        color_24_24_mix_premult(&src_buf_c32[s..s + 3], dest, src_pixel.alpha);
                    }
                    None => {
                        if src_pixel.alpha > 0 {
                            // Scale the premultiplied channels by `opa` and
                            // blend the background with the remaining
                            // (255 - alpha * opa) weight
                            let src_scaled = [
                                opa_mix2(src_pixel.blue, opa),
                                opa_mix2(src_pixel.green, opa),
                                opa_mix2(src_pixel.red, opa),
                            ];
                            color_24_24_mix_premult(&src_scaled, dest, opa_mix2(src_pixel.alpha, opa));
                        }
                    }
                    Some(m) if opa >= OPA_MAX => {
                        if src_pixel.alpha > 0 {
                            let mv = m[mask_ofs + src_x];
                            let src_scaled = [
                                opa_mix2(src_pixel.blue, mv),
                                opa_mix2(src_pixel.green, mv),
                                opa_mix2(src_pixel.red, mv),
                            ];
                            color_24_24_mix_premult(&src_scaled, dest, opa_mix2(src_pixel.alpha, mv));
                        }
                    }
                    Some(m) => {
                        if src_pixel.alpha > 0 {
                            let mv = m[mask_ofs + src_x];
                            let scale = opa_mix2(mv, opa);
                            let src_scaled = [
                                opa_mix2(src_pixel.blue, scale),
                                opa_mix2(src_pixel.green, scale),
                                opa_mix2(src_pixel.red, scale),
                            ];
                            // Combine the three factors in one step to keep
                            // the precision
                            color_24_24_mix_premult(&src_scaled, dest, opa_mix3(src_pixel.alpha, mv, opa));
                        }
                    }
                }
                dest_x += px;
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
        }
    } else {
        for _y in 0..h {
            let mut dest_x = 0usize;
            for src_x in 0..w as usize {
                let s = src_ofs + src_x * 4;
                let mut src_argb = color32_at(src_buf_c32, s);
                if src_argb.alpha > 0 {
                    let reciprocal = ((255u32 * 256) / src_argb.alpha as u32) as u16 as u32;
                    src_argb.red = ((src_argb.red as u32 * reciprocal) >> 8) as u8;
                    src_argb.green = ((src_argb.green as u32 * reciprocal) >> 8) as u8;
                    src_argb.blue = ((src_argb.blue as u32 * reciprocal) >> 8) as u8;
                }
                src_argb.alpha = match mask_buf {
                    None => opa_mix2(src_argb.alpha, opa),
                    Some(m) => opa_mix3(src_argb.alpha, mask_at(m, mask_ofs + dest_x), opa),
                };
                let d = dest_ofs + dest_x;
                blend_non_normal_pixel(&mut dest_buf[d..d + 3], src_argb, blend_mode);
                dest_x += px;
            }
            if mask_buf.is_some() {
                mask_ofs = mask_ofs.wrapping_add(mask_stride);
            }
            dest_ofs = dest_ofs.wrapping_add(dest_stride);
            src_ofs = src_ofs.wrapping_add(src_stride);
        }
    }
}

/// `blend_non_normal_pixel()`: additive / subtractive / multiply /
/// difference of `src` onto the 3 bytes `dest`, then mixed with
/// `src.alpha`.
#[inline]
fn blend_non_normal_pixel(dest: &mut [u8], src: Color32, mode: BlendMode) {
    let mut res = [0u8; 3];
    let d = [dest[0] as i32, dest[1] as i32, dest[2] as i32];
    let s = [src.blue as i32, src.green as i32, src.red as i32];
    match mode {
        BlendMode::Additive => {
            for i in 0..3 {
                res[i] = (d[i] + s[i]).min(255) as u8;
            }
        }
        BlendMode::Subtractive => {
            for i in 0..3 {
                res[i] = (d[i] - s[i]).max(0) as u8;
            }
        }
        BlendMode::Multiply => {
            for i in 0..3 {
                res[i] = ((d[i] * s[i]) >> 8) as u8;
            }
        }
        BlendMode::Difference => {
            for i in 0..3 {
                res[i] = (d[i] - s[i]).unsigned_abs() as u8;
            }
        }
        // LV_LOG_WARN("Not supported blend mode")
        _ => return,
    }
    color_24_24_mix(&res, dest, src.alpha);
}

/// `lv_color_8_24_mix()`: mix a grey level into the 3 bytes `dest`.
#[cfg(any(feature = "cf-i1", feature = "cf-al88", feature = "cf-l8"))]
#[inline]
fn color_8_24_mix(src: u8, dest: &mut [u8], mix: u8) {
    if mix == 0 {
        return;
    }

    if mix >= OPA_MAX {
        dest[0] = src;
        dest[1] = src;
        dest[2] = src;
    } else {
        // Mix the first and third channels in one 32 bit value with a single
        // multiplication each. The 16 bit lanes can't overflow as their max
        // value is 255 * 255
        let mix_inv = (255 - mix) as u32;
        let mix = mix as u32;
        let s = src as u32;
        let rb = (((s << 16) | s) * mix) + ((((dest[2] as u32) << 16) | dest[0] as u32) * mix_inv);
        dest[0] = (rb >> 8) as u8;
        dest[1] = ((s * mix + dest[1] as u32 * mix_inv) >> 8) as u8;
        dest[2] = (rb >> 24) as u8;
    }
}

/// `lv_color_24_24_mix()`: mix the 3 bytes `src` into the 3 bytes `dest`.
#[inline]
fn color_24_24_mix(src: &[u8], dest: &mut [u8], mix: u8) {
    if mix == 0 {
        return;
    }

    if mix >= OPA_MAX {
        dest[0] = src[0];
        dest[1] = src[1];
        dest[2] = src[2];
    } else {
        // Mix the first and third channels in one 32 bit value with a single
        // multiplication each. The 16 bit lanes can't overflow as their max
        // value is 255 * 255
        let mix_inv = (255 - mix) as u32;
        let mix = mix as u32;
        let rb = ((((src[2] as u32) << 16) | src[0] as u32) * mix)
            + ((((dest[2] as u32) << 16) | dest[0] as u32) * mix_inv);
        dest[0] = (rb >> 8) as u8;
        dest[1] = ((src[1] as u32 * mix + dest[1] as u32 * mix_inv) >> 8) as u8;
        dest[2] = (rb >> 24) as u8;
    }
}

/// `get_bit()`: bit `bit_idx` of an MSB-first 1 bpp row.
#[cfg(feature = "cf-i1")]
#[inline]
fn get_bit(buf: &[u8], bit_idx: i32) -> u8 {
    (buf[(bit_idx / 8) as usize] >> (7 - (bit_idx % 8))) & 1
}

#[cfg(test)]
#[cfg(all(
    feature = "cf-rgb565",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied",
    feature = "cf-l8",
    feature = "cf-al88",
    feature = "cf-i1"
))]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;
    use ferrovgl_core::misc::area::Area;
    use ferrovgl_core::misc::color::{Color, OPA_50, OPA_COVER};

    /// Tiny LCG so that the destination content is irregular.
    fn noise(n: usize, seed: u32) -> Vec<u8> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (s >> 16) as u8
            })
            .collect()
    }

    /// Per channel `lv_color_24_24_mix()` reference.
    fn ref_mix(s: u8, d: u8, mix: u8) -> u8 {
        if mix == 0 {
            d
        } else if mix >= OPA_MAX {
            s
        } else {
            ((s as u32 * mix as u32 + d as u32 * (255 - mix as u32)) >> 8) as u8
        }
    }

    const COLOR: Color = Color {
        red: 0x12,
        green: 0xA4,
        blue: 0xE6,
    };

    fn fill_dsc<'a>(
        dest: &'a mut [u8],
        w: i32,
        h: i32,
        stride: i32,
        opa: Opa,
        mask: Option<&'a [u8]>,
        mask_stride: i32,
    ) -> BlendFillDsc<'a> {
        BlendFillDsc {
            dest_buf: dest,
            dest_w: w,
            dest_h: h,
            dest_stride: stride,
            mask_buf: mask,
            mask_stride,
            color: COLOR,
            opa,
            relative_area: Area::new(0, 0, w - 1, h - 1),
        }
    }

    /// Run `check(px_bytes, x, y, before)` for every pixel of a `w * h`
    /// area at byte `ofs` and verify that the bytes outside of it are
    /// untouched.
    fn check_area(
        after: &[u8],
        before: &[u8],
        ofs: usize,
        (w, h, stride, px): (usize, usize, usize, usize),
        mut check: impl FnMut(&[u8], usize, usize, &[u8]),
    ) {
        for i in 0..after.len() {
            let inside = i >= ofs && {
                let r = i - ofs;
                r / stride < h && r % stride < w * px
            };
            if !inside {
                assert_eq!(after[i], before[i], "byte {i} outside the area changed");
            }
        }
        for y in 0..h {
            for x in 0..w {
                let p = ofs + y * stride + x * px;
                check(&after[p..p + px], x, y, &before[p..p + px]);
            }
        }
    }

    #[test]
    fn fill_cover_odd_widths_all_alignments() {
        for px in [3usize, 4] {
            for ofs in 0..4 {
                for w in [1usize, 2, 3, 4, 5, 7, 13, 17, 33] {
                    for pad in [0usize, 2, 4] {
                        let h = 3;
                        let stride = w * px + pad;
                        let before = noise(ofs + stride * h + 5, (w * 31 + ofs + pad) as u32);
                        let mut buf = before.clone();
                        let mut dsc = fill_dsc(
                            &mut buf[ofs..],
                            w as i32,
                            h as i32,
                            stride as i32,
                            OPA_COVER,
                            None,
                            0,
                        );
                        fill(&mut dsc, px as u32);
                        check_area(&buf, &before, ofs, (w, h, stride, px), |p, _, _, _| {
                            assert_eq!(&p[..3], &[COLOR.blue, COLOR.green, COLOR.red]);
                            if px == 4 {
                                assert_eq!(p[3], 0xFF);
                            }
                        });
                    }
                }
            }
        }
    }

    #[test]
    fn fill_opa_matches_byte_formula() {
        for px in [3usize, 4] {
            for opa in [OPA_50, 3, 100, 252] {
                for ofs in 0..4 {
                    for w in [1usize, 3, 5, 8, 11] {
                        let h = 4;
                        let stride = w * px + 3;
                        let mut before = noise(ofs + stride * h + 3, (opa as usize * 7 + w + ofs) as u32);
                        // Flat rows exercise the "last word" cache.
                        for b in &mut before[ofs + stride..ofs + 3 * stride] {
                            *b = 0x40;
                        }
                        let mut buf = before.clone();
                        let mut dsc =
                            fill_dsc(&mut buf[ofs..], w as i32, h as i32, stride as i32, opa, None, 0);
                        fill(&mut dsc, px as u32);
                        let c = [COLOR.blue, COLOR.green, COLOR.red];
                        check_area(&buf, &before, ofs, (w, h, stride, px), |p, _, _, b| {
                            for i in 0..3 {
                                let e = ((c[i] as u32 * opa as u32 + b[i] as u32 * (255 - opa as u32)) >> 8)
                                    as u8;
                                assert_eq!(p[i], e);
                            }
                            if px == 4 {
                                // The X byte is left alone.
                                assert_eq!(p[3], b[3]);
                            }
                        });
                    }
                }
            }
        }
    }

    #[test]
    fn fill_with_mask_and_mask_opa() {
        for px in [3usize, 4] {
            for opa in [OPA_COVER, OPA_50] {
                let (w, h) = (5usize, 3usize);
                let stride = w * px + 1;
                let mask_stride = w + 2;
                let mask: Vec<u8> = (0..mask_stride * h)
                    .map(|i| [0u8, 255, 128, 254, 1, 77, 253][i % 7])
                    .collect();
                let before = noise(stride * h, 99);
                let mut buf = before.clone();
                let mut dsc = fill_dsc(
                    &mut buf,
                    w as i32,
                    h as i32,
                    stride as i32,
                    opa,
                    Some(&mask),
                    mask_stride as i32,
                );
                fill(&mut dsc, px as u32);
                let c = [COLOR.blue, COLOR.green, COLOR.red];
                check_area(&buf, &before, 0, (w, h, stride, px), |p, x, y, b| {
                    let m = mask[y * mask_stride + x];
                    let mix = if opa >= OPA_MAX { m } else { opa_mix2(opa, m) };
                    for i in 0..3 {
                        assert_eq!(p[i], ref_mix(c[i], b[i], mix));
                    }
                    if px == 4 {
                        assert_eq!(p[3], b[3]);
                    }
                });
            }
        }
    }

    /// Image geometry of the tests: `(w, h, dest_stride, src_stride,
    /// mask_stride)`.
    type Geom = (i32, i32, i32, i32, i32);

    fn image_dsc<'a>(
        dest: &'a mut [u8],
        (w, h, dest_stride, src_stride, mask_stride): Geom,
        src: &'a [u8],
        cf: ColorFormat,
        opa: Opa,
        mask: Option<&'a [u8]>,
        mode: BlendMode,
    ) -> BlendImageDsc<'a> {
        BlendImageDsc {
            dest_buf: dest,
            dest_w: w,
            dest_h: h,
            dest_stride,
            mask_buf: mask,
            mask_stride,
            src_buf: src,
            src_stride,
            src_color_format: cf,
            opa,
            blend_mode: mode,
            relative_area: Area::new(0, 0, w - 1, h - 1),
            src_area: Area::new(0, 0, w - 1, h - 1),
        }
    }

    /// Blend a 7x3 image of `cf` (`src_px` bytes per pixel) into both
    /// destination pixel sizes and compare every pixel with
    /// `expect(src_px_bytes, dest_before, mask) -> [b, g, r]`.
    fn run_image(
        cf: ColorFormat,
        src_px: usize,
        src: &[u8],
        opa: Opa,
        use_mask: bool,
        mode: BlendMode,
        mut expect: impl FnMut(&[u8], &[u8], u8) -> [u8; 3],
    ) {
        let (w, h) = (7usize, 3usize);
        let src_stride = w * src_px + 2;
        assert!(src.len() >= src_stride * h);
        let mask_stride = w + 1;
        let mask: Vec<u8> = (0..mask_stride * h + 32)
            .map(|i| [255u8, 0, 128, 200, 254, 3][i % 6])
            .collect();
        for px in [3usize, 4] {
            let stride = w * px + 1;
            let before = noise(stride * h, (src_px * 13 + px) as u32);
            let mut buf = before.clone();
            let mut dsc = image_dsc(
                &mut buf,
                (
                    w as i32,
                    h as i32,
                    stride as i32,
                    src_stride as i32,
                    mask_stride as i32,
                ),
                src,
                cf,
                opa,
                if use_mask { Some(&mask) } else { None },
                mode,
            );
            image(&mut dsc, px as u32);
            check_area(&buf, &before, 0, (w, h, stride, px), |p, x, y, b| {
                let s = &src[y * src_stride + x * src_px..y * src_stride + (x + 1) * src_px];
                let m = if use_mask { mask[y * mask_stride + x] } else { 255 };
                assert_eq!(&p[..3], &expect(s, b, m), "{cf:?} px {px} at ({x}, {y})");
                if px == 4 {
                    // XRGB8888 -> XRGB8888 cover is a row `memcpy`, X included.
                    let copied = src_px == 4
                        && cf == ColorFormat::Xrgb8888
                        && !use_mask
                        && opa >= OPA_MAX
                        && mode == BlendMode::Normal;
                    assert_eq!(p[3], if copied { s[3] } else { b[3] });
                }
            });
        }
    }

    /// The mix of the `RGB888` / `RGB565` / `L8` normal paths.
    fn plain_mix(use_mask: bool, opa: Opa, m: u8) -> u8 {
        match (use_mask, opa >= OPA_MAX) {
            (false, _) => opa,
            (true, true) => m,
            (true, false) => opa_mix2(opa, m),
        }
    }

    /// The mix of the `ARGB8888` / `AL88` normal paths.
    fn alpha_mix(use_mask: bool, opa: Opa, m: u8, a: u8) -> u8 {
        match (use_mask, opa >= OPA_MAX) {
            (false, true) => a,
            (false, false) => opa_mix2(a, opa),
            (true, true) => opa_mix2(a, m),
            (true, false) => opa_mix3(a, m, opa),
        }
    }

    fn rgb565_conv(s: &[u8]) -> [u8; 3] {
        let c = u16::from_le_bytes([s[0], s[1]]) as u32;
        [
            (((c & 0x1F) * 2106) >> 8) as u8,
            ((((c >> 5) & 0x3F) * 1037) >> 8) as u8,
            ((((c >> 11) & 0x1F) * 2106) >> 8) as u8,
        ]
    }

    #[test]
    fn image_from_argb8888() {
        let src = noise(40 * 3, 5);
        for opa in [OPA_COVER, OPA_50] {
            for use_mask in [false, true] {
                run_image(
                    ColorFormat::Argb8888,
                    4,
                    &src,
                    opa,
                    use_mask,
                    BlendMode::Normal,
                    |s, b, m| {
                        let mix = alpha_mix(use_mask, opa, m, s[3]);
                        [
                            ref_mix(s[0], b[0], mix),
                            ref_mix(s[1], b[1], mix),
                            ref_mix(s[2], b[2], mix),
                        ]
                    },
                );
            }
        }
    }

    #[test]
    fn image_from_rgb565() {
        let mut src = noise(20 * 3, 6);
        // Pure channels expand to 255.
        src[0..2].copy_from_slice(&0xF800u16.to_le_bytes());
        src[2..4].copy_from_slice(&0x07E0u16.to_le_bytes());
        src[4..6].copy_from_slice(&0x001Fu16.to_le_bytes());
        assert_eq!(rgb565_to_888(&src, 0), [0, 0, 255]);
        assert_eq!(rgb565_to_888(&src, 1), [0, 255, 0]);
        assert_eq!(rgb565_to_888(&src, 2), [255, 0, 0]);
        for opa in [OPA_COVER, OPA_50] {
            for use_mask in [false, true] {
                run_image(
                    ColorFormat::Rgb565,
                    2,
                    &src,
                    opa,
                    use_mask,
                    BlendMode::Normal,
                    |s, b, m| {
                        let c = rgb565_conv(s);
                        let mix = plain_mix(use_mask, opa, m);
                        [
                            ref_mix(c[0], b[0], mix),
                            ref_mix(c[1], b[1], mix),
                            ref_mix(c[2], b[2], mix),
                        ]
                    },
                );
            }
        }
    }

    #[test]
    fn image_from_l8_and_al88() {
        let src = noise(20 * 3, 7);
        for opa in [OPA_COVER, OPA_50] {
            for use_mask in [false, true] {
                run_image(
                    ColorFormat::L8,
                    1,
                    &src,
                    opa,
                    use_mask,
                    BlendMode::Normal,
                    |s, b, m| {
                        let mix = plain_mix(use_mask, opa, m);
                        [
                            ref_mix(s[0], b[0], mix),
                            ref_mix(s[0], b[1], mix),
                            ref_mix(s[0], b[2], mix),
                        ]
                    },
                );
                run_image(
                    ColorFormat::Al88,
                    2,
                    &src,
                    opa,
                    use_mask,
                    BlendMode::Normal,
                    |s, b, m| {
                        let mix = alpha_mix(use_mask, opa, m, s[1]);
                        [
                            ref_mix(s[0], b[0], mix),
                            ref_mix(s[0], b[1], mix),
                            ref_mix(s[0], b[2], mix),
                        ]
                    },
                );
            }
        }
    }

    #[test]
    fn image_from_a8_is_not_supported() {
        let src = noise(64, 8);
        let before = noise(64, 9);
        let mut buf = before.clone();
        let mut dsc = image_dsc(
            &mut buf,
            (4, 2, 12, 4, 0),
            &src,
            ColorFormat::A8,
            OPA_COVER,
            None,
            BlendMode::Normal,
        );
        image(&mut dsc, 3);
        assert_eq!(buf, before);
    }

    #[test]
    fn image_from_i1() {
        // 7 pixels per row, MSB first.
        let rows = [0b1010_1100u8, 0b0111_0010, 0b1111_1110];
        let w = 7usize;
        for opa in [OPA_COVER, OPA_50] {
            for use_mask in [false, true] {
                for px in [3usize, 4] {
                    let stride = w * px;
                    let mask: Vec<u8> = (0..w * 3).map(|i| [255u8, 0, 128][i % 3]).collect();
                    let before = noise(stride * 3, 10);
                    let mut buf = before.clone();
                    let mut dsc = image_dsc(
                        &mut buf,
                        (w as i32, 3, stride as i32, 1, w as i32),
                        &rows,
                        ColorFormat::I1,
                        opa,
                        if use_mask { Some(&mask) } else { None },
                        BlendMode::Normal,
                    );
                    image(&mut dsc, px as u32);
                    check_area(&buf, &before, 0, (w, 3, stride, px), |p, x, y, b| {
                        let v = ((rows[y] >> (7 - x)) & 1) * 255;
                        let mix = plain_mix(use_mask, opa, mask[y * w + x]);
                        for i in 0..3 {
                            assert_eq!(p[i], ref_mix(v, b[i], mix));
                        }
                    });
                }
            }
        }
    }

    #[test]
    fn image_from_rgb888_and_xrgb8888() {
        let src = noise(40 * 3, 11);
        for (cf, spx) in [(ColorFormat::Rgb888, 3usize), (ColorFormat::Xrgb8888, 4)] {
            for opa in [OPA_COVER, OPA_50] {
                for use_mask in [false, true] {
                    run_image(cf, spx, &src, opa, use_mask, BlendMode::Normal, |s, b, m| {
                        let mix = plain_mix(use_mask, opa, m);
                        [
                            ref_mix(s[0], b[0], mix),
                            ref_mix(s[1], b[1], mix),
                            ref_mix(s[2], b[2], mix),
                        ]
                    });
                }
            }
        }
        // Same pixel size: whole rows are copied, the X byte included.
        let src = noise(16, 12);
        let mut dest = vec![0u8; 16];
        let mut dsc = image_dsc(
            &mut dest,
            (4, 1, 16, 16, 0),
            &src,
            ColorFormat::Xrgb8888,
            OPA_COVER,
            None,
            BlendMode::Normal,
        );
        image(&mut dsc, 4);
        assert_eq!(dest, src);
    }

    #[test]
    fn image_from_premultiplied() {
        let src: Vec<u8> = noise(40 * 3, 13)
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|c| {
                // Make the channels valid premultiplied values.
                let a = c[3];
                [opa_mix2(c[0], a), opa_mix2(c[1], a), opa_mix2(c[2], a), a]
            })
            .collect();
        let premult = |s: u8, d: u8, mix: u8| -> u8 {
            if mix == 0 {
                d
            } else if mix >= OPA_MAX {
                s
            } else {
                s.wrapping_add(((d as u32 * (255 - mix as u32)) >> 8) as u8)
            }
        };
        for opa in [OPA_COVER, OPA_50] {
            for use_mask in [false, true] {
                run_image(
                    ColorFormat::Argb8888Premultiplied,
                    4,
                    &src,
                    opa,
                    use_mask,
                    BlendMode::Normal,
                    |s, b, m| {
                        if !use_mask && opa >= OPA_MAX {
                            return [
                                premult(s[0], b[0], s[3]),
                                premult(s[1], b[1], s[3]),
                                premult(s[2], b[2], s[3]),
                            ];
                        }
                        if s[3] == 0 {
                            return [b[0], b[1], b[2]];
                        }
                        let (scale, mix) = match (use_mask, opa >= OPA_MAX) {
                            (false, _) => (opa, opa_mix2(s[3], opa)),
                            (true, true) => (m, opa_mix2(s[3], m)),
                            (true, false) => (opa_mix2(m, opa), opa_mix3(s[3], m, opa)),
                        };
                        [
                            premult(opa_mix2(s[0], scale), b[0], mix),
                            premult(opa_mix2(s[1], scale), b[1], mix),
                            premult(opa_mix2(s[2], scale), b[2], mix),
                        ]
                    },
                );
            }
        }
    }

    #[test]
    fn blend_modes() {
        let d = [200u8, 10, 128];
        let s = Color32 {
            blue: 100,
            green: 50,
            red: 128,
            alpha: OPA_COVER,
        };
        let run = |mode| {
            let mut p = d;
            blend_non_normal_pixel(&mut p, s, mode);
            p
        };
        assert_eq!(run(BlendMode::Additive), [255, 60, 255]);
        assert_eq!(run(BlendMode::Subtractive), [100, 0, 0]);
        assert_eq!(run(BlendMode::Multiply), [78, 1, 64]);
        assert_eq!(run(BlendMode::Difference), [100, 40, 0]);
        assert_eq!(run(BlendMode::Normal), d);
        // Half alpha mixes the result back with the destination.
        let mut p = d;
        blend_non_normal_pixel(&mut p, Color32 { alpha: 128, ..s }, BlendMode::Additive);
        assert_eq!(
            p,
            [
                ref_mix(255, 200, 128),
                ref_mix(60, 10, 128),
                ref_mix(255, 128, 128)
            ]
        );
    }

    #[test]
    fn image_blend_modes_per_source() {
        let src = noise(40 * 3, 14);
        let modes = [
            BlendMode::Additive,
            BlendMode::Subtractive,
            BlendMode::Multiply,
            BlendMode::Difference,
        ];
        let apply = |mode: BlendMode, d: &[u8], c: [u8; 3], alpha: u8| -> [u8; 3] {
            let mut p = [d[0], d[1], d[2]];
            let src = Color32 {
                blue: c[0],
                green: c[1],
                red: c[2],
                alpha,
            };
            blend_non_normal_pixel(&mut p, src, mode);
            p
        };
        for mode in modes {
            for opa in [OPA_COVER, OPA_50] {
                run_image(ColorFormat::Rgb888, 3, &src, opa, false, mode, |s, b, _| {
                    apply(mode, b, [s[0], s[1], s[2]], opa)
                });
                run_image(ColorFormat::Argb8888, 4, &src, opa, false, mode, |s, b, _| {
                    apply(mode, b, [s[0], s[1], s[2]], opa_mix2(s[3], opa))
                });
                run_image(ColorFormat::L8, 1, &src, opa, false, mode, |s, b, _| {
                    apply(mode, b, [s[0]; 3], opa)
                });
                run_image(ColorFormat::Al88, 2, &src, opa, false, mode, |s, b, _| {
                    apply(mode, b, [s[0]; 3], opa_mix2(s[1], opa))
                });
                // The mask is indexed per pixel for RGB565 and I1 sources.
                run_image(ColorFormat::Rgb565, 2, &src, opa, true, mode, |s, b, m| {
                    apply(mode, b, rgb565_conv(s), opa_mix2(m, opa))
                });
            }
        }
    }

    #[test]
    fn premultiplied_non_normal_unpremultiplies_with_truncation() {
        // reciprocal = 65280 / 100 = 652; 200 * 652 >> 8 = 509 -> 253 (u8)
        let src = [200u8, 50, 100, 100];
        let mut dest = [0u8; 3];
        let mut dsc = image_dsc(
            &mut dest,
            (1, 1, 3, 4, 0),
            &src,
            ColorFormat::Argb8888Premultiplied,
            OPA_COVER,
            None,
            BlendMode::Additive,
        );
        image(&mut dsc, 3);
        let a = opa_mix2(100, OPA_COVER);
        assert_eq!(dest, [ref_mix(253, 0, a), ref_mix(127, 0, a), ref_mix(254, 0, a)]);
    }

    #[test]
    fn mix_helpers_match_reference() {
        for mix in [0u8, 1, 64, 127, 128, 200, 252, 253, 255] {
            for (s, d) in [(0u8, 255u8), (255, 0), (17, 230), (128, 128)] {
                let before = [d, d.wrapping_add(1), d.wrapping_sub(1)];
                let src = [s, s.wrapping_add(3), s.wrapping_sub(3)];
                let mut p = before;
                color_24_24_mix(&src, &mut p, mix);
                for i in 0..3 {
                    assert_eq!(p[i], ref_mix(src[i], before[i], mix));
                }
                let mut q = before;
                color_8_24_mix(s, &mut q, mix);
                for i in 0..3 {
                    assert_eq!(q[i], ref_mix(s, before[i], mix));
                }
            }
        }
        let bits = [0b1000_0001u8, 0b0100_0000];
        assert_eq!(get_bit(&bits, 0), 1);
        assert_eq!(get_bit(&bits, 7), 1);
        assert_eq!(get_bit(&bits, 8), 0);
        assert_eq!(get_bit(&bits, 9), 1);
    }
}
