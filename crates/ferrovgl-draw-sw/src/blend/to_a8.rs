//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_a8.c`.
//!
//! Blend a colour or an image into an alpha-only (`A8`) buffer. Only the
//! coverage is kept: every kernel raises the destination alpha to the
//! source alpha (`BLEND_A8`, a max operation); colours and blend modes are
//! ignored.
//!
//! Port notes:
//! - The `LV_DRAW_SW_*_TO_A8*` acceleration hooks are always
//!   `LV_RESULT_INVALID` in the software renderer, so only the C fallback
//!   loops are ported.
//! - Buffers are byte slices; the C "row pointer += stride" becomes a byte
//!   offset advanced with `drawbuf_next_row`.
//! - `LV_LOG_WARN` on an unknown source format does nothing.
//! - `mask_only_image_blend()` is compiled when any of its users is, which
//!   adds `RGB565_SWAPPED` to the upstream guard (upstream would not build
//!   with only that format enabled).

use ferrovgl_core::misc::color::{ColorFormat, OPA_MAX, Opa, opa_mix2, opa_mix3};

use crate::blend::{BlendFillDsc, BlendImageDsc};

/// `BLEND_A8(dest_a, src_a)`: `if(dest_a < src_a) dest_a = src_a`.
#[inline]
fn blend_a8(dest_a: &mut u8, src_a: Opa) {
    if *dest_a < src_a {
        *dest_a = src_a;
    }
}

/// `lv_draw_sw_blend_color_to_a8()`: fill with a colour (+ opa / mask).
pub fn fill(dsc: &mut BlendFillDsc<'_>) {
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let mask = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_stride = dsc.dest_stride;
    let dest_buf = &mut *dsc.dest_buf;
    let mut d = 0usize;

    match mask {
        /*Simple fill*/
        None if opa >= OPA_MAX => {
            for _y in 0..h {
                dest_buf[d..d + w].fill(0xff);
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        /*Opacity only*/
        None => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf[d + x], opa);
                }
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        /*Masked with full opacity*/
        Some(mask) if opa >= OPA_MAX => {
            let mut m = 0usize;
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf[d + x], mask[m + x]);
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
                    let opa_res = opa_mix2(mask[m + x], opa);
                    blend_a8(&mut dest_buf[d + x], opa_res);
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `lv_draw_sw_blend_image_to_a8()`: blend an image of any supported
/// source format.
pub fn image(dsc: &mut BlendImageDsc<'_>) {
    match dsc.src_color_format {
        #[cfg(feature = "cf-rgb565")]
        ColorFormat::Rgb565 => mask_only_image_blend(dsc),
        #[cfg(feature = "cf-rgb565-swapped")]
        ColorFormat::Rgb565Swapped => mask_only_image_blend(dsc),
        #[cfg(feature = "cf-rgb888")]
        ColorFormat::Rgb888 => mask_only_image_blend(dsc),
        #[cfg(feature = "cf-xrgb8888")]
        ColorFormat::Xrgb8888 => mask_only_image_blend(dsc),
        #[cfg(feature = "cf-argb8888")]
        ColorFormat::Argb8888 => argb8888_image_blend(dsc),
        /*Only the alpha is used here and premultiplying doesn't change it*/
        #[cfg(feature = "cf-argb8888-premultiplied")]
        ColorFormat::Argb8888Premultiplied => argb8888_image_blend(dsc),
        ColorFormat::A8 => a8_image_blend(dsc),
        #[cfg(feature = "cf-l8")]
        ColorFormat::L8 => mask_only_image_blend(dsc),
        #[cfg(feature = "cf-al88")]
        ColorFormat::Al88 => al88_image_blend(dsc),
        #[cfg(feature = "cf-i1")]
        ColorFormat::I1 => i1_image_blend(dsc),
        _ => {
            // LV_LOG_WARN("Not supported source color format")
        }
    }
}

/// `a8_image_blend()`
fn a8_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_a8 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_buf_a8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    match mask_buf {
        None if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_a8[d + x], src_buf_a8[s + x]);
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
            }
        }
        None => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_a8[d + x], opa_mix2(opa, src_buf_a8[s + x]));
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
            }
        }
        Some(mask_buf) if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(
                        &mut dest_buf_a8[d + x],
                        opa_mix2(mask_buf[m + x], src_buf_a8[s + x]),
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
                    blend_a8(
                        &mut dest_buf_a8[d + x],
                        opa_mix3(opa, mask_buf[m + x], src_buf_a8[s + x]),
                    );
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `mask_only_image_blend()`: opaque sources only contribute the
/// opacity and the mask.
#[cfg(any(
    feature = "cf-l8",
    feature = "cf-rgb565",
    feature = "cf-rgb565-swapped",
    feature = "cf-rgb888",
    feature = "cf-xrgb8888"
))]
fn mask_only_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_buf_u8 = &mut *dsc.dest_buf;
    let (mut d, mut m) = (0usize, 0usize);

    match mask_buf {
        None if opa >= OPA_MAX => {
            for _y in 0..h {
                dest_buf_u8[d..d + w].fill(0xff);
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        None => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_u8[d + x], opa);
                }
                d = drawbuf_next_row(d, dest_stride);
            }
        }
        Some(mask_buf) if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_u8[d + x], mask_buf[m + x]);
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
        Some(mask_buf) => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_u8[d + x], opa_mix2(opa, mask_buf[m + x]));
                }
                d = drawbuf_next_row(d, dest_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `i1_image_blend()`: only the set bits of the source contribute.
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
    let dest_buf_a8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    match mask_buf {
        None if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    if get_bit(&src_buf_i1[s..], x as i32) != 0 {
                        dest_buf_a8[d + x] = 0xff;
                    }
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
            }
        }
        None => {
            for _y in 0..h {
                for x in 0..w {
                    if get_bit(&src_buf_i1[s..], x as i32) != 0 {
                        blend_a8(&mut dest_buf_a8[d + x], opa);
                    }
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
            }
        }
        Some(mask_buf) if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    if get_bit(&src_buf_i1[s..], x as i32) != 0 {
                        blend_a8(&mut dest_buf_a8[d + x], mask_buf[m + x]);
                    }
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
        Some(mask_buf) => {
            for _y in 0..h {
                for x in 0..w {
                    if get_bit(&src_buf_i1[s..], x as i32) != 0 {
                        let opa_res = opa_mix2(mask_buf[m + x], opa);
                        blend_a8(&mut dest_buf_a8[d + x], opa_res);
                    }
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `al88_image_blend()`: uses the alpha byte of each `lv_color16a_t`.
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
    let dest_buf_a8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `src_buf_al88[x].alpha`
    let alpha = |s: usize, x: usize| src_buf_al88[s + x * 2 + 1];

    match mask_buf {
        None if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_a8[d + x], alpha(s, x));
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
            }
        }
        None => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_a8[d + x], opa_mix2(alpha(s, x), opa));
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
            }
        }
        Some(mask_buf) if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_a8[d + x], opa_mix2(alpha(s, x), mask_buf[m + x]));
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
        Some(mask_buf) => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(
                        &mut dest_buf_a8[d + x],
                        opa_mix3(alpha(s, x), mask_buf[m + x], opa),
                    );
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `argb8888_image_blend()`: uses the alpha byte of each `lv_color32_t`
/// (shared by `ARGB8888` and `ARGB8888_PREMULTIPLIED`).
#[cfg(any(feature = "cf-argb8888", feature = "cf-argb8888-premultiplied"))]
fn argb8888_image_blend(dsc: &mut BlendImageDsc<'_>) {
    let w = dsc.dest_w.max(0) as usize;
    let h = dsc.dest_h;
    let opa = dsc.opa;
    let dest_stride = dsc.dest_stride;
    let src_buf_c32 = dsc.src_buf;
    let src_stride = dsc.src_stride;
    let mask_buf = dsc.mask_buf;
    let mask_stride = dsc.mask_stride;
    let dest_buf_a8 = &mut *dsc.dest_buf;
    let (mut d, mut s, mut m) = (0usize, 0usize, 0usize);

    // `src_buf_c32[x].alpha`
    let alpha = |s: usize, x: usize| src_buf_c32[s + x * 4 + 3];

    match mask_buf {
        None if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_a8[d + x], alpha(s, x));
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
            }
        }
        None => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_a8[d + x], opa_mix2(alpha(s, x), opa));
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
            }
        }
        Some(mask_buf) if opa >= OPA_MAX => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(&mut dest_buf_a8[d + x], opa_mix2(alpha(s, x), mask_buf[m + x]));
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
        Some(mask_buf) => {
            for _y in 0..h {
                for x in 0..w {
                    blend_a8(
                        &mut dest_buf_a8[d + x],
                        opa_mix3(alpha(s, x), opa, mask_buf[m + x]),
                    );
                }
                d = drawbuf_next_row(d, dest_stride);
                s = drawbuf_next_row(s, src_stride);
                m = drawbuf_next_row(m, mask_stride);
            }
        }
    }
}

/// `get_bit()`: MSB-first bit `bit_idx` of `buf`.
#[cfg(feature = "cf-i1")]
#[inline]
fn get_bit(buf: &[u8], bit_idx: i32) -> u8 {
    (buf[(bit_idx / 8) as usize] >> (7 - (bit_idx % 8))) & 1
}

/// `drawbuf_next_row()`: advance a byte offset by one row of `stride` bytes.
#[inline]
fn drawbuf_next_row(ofs: usize, stride: i32) -> usize {
    ofs.wrapping_add_signed(stride as isize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use ferrovgl_core::misc::area::Area;
    use ferrovgl_core::misc::color::Color;
    use ferrovgl_core::misc::style::BlendMode;

    fn fill_dsc<'a>(
        dest: &'a mut [u8],
        w: i32,
        h: i32,
        stride: i32,
        mask: Option<&'a [u8]>,
        opa: Opa,
    ) -> BlendFillDsc<'a> {
        BlendFillDsc {
            dest_buf: dest,
            dest_w: w,
            dest_h: h,
            dest_stride: stride,
            mask_buf: mask,
            mask_stride: w,
            color: Color::make(12, 34, 56),
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
            // Ignored by every A8 kernel.
            blend_mode: BlendMode::Additive,
            relative_area: Area::new(0, 0, w - 1, 0),
            src_area: Area::new(0, 0, w - 1, 0),
        }
    }

    #[test]
    fn blend_a8_is_max() {
        let mut a = 10;
        blend_a8(&mut a, 5);
        assert_eq!(a, 10);
        blend_a8(&mut a, 50);
        assert_eq!(a, 50);
    }

    #[test]
    fn fill_cases() {
        let mut buf = vec![0u8, 0, 0, 7, 0, 0, 0, 7];
        fill(&mut fill_dsc(&mut buf, 3, 2, 4, None, 255));
        assert_eq!(buf, [255, 255, 255, 7, 255, 255, 255, 7]);

        let mut buf = vec![10u8, 200];
        fill(&mut fill_dsc(&mut buf, 2, 1, 2, None, 100));
        assert_eq!(buf, [100, 200]);

        let mask = [0u8, 60, 255];
        let mut buf = vec![50u8; 3];
        fill(&mut fill_dsc(&mut buf, 3, 1, 3, Some(&mask), 255));
        assert_eq!(buf, [50, 60, 255]);

        let mask = [255u8, 100];
        let mut buf = vec![50u8; 2];
        fill(&mut fill_dsc(&mut buf, 2, 1, 2, Some(&mask), 128));
        // LV_OPA_MIX2(255, 128) = 127, LV_OPA_MIX2(100, 128) = 50
        assert_eq!(buf, [127, 50]);
    }

    #[test]
    fn image_from_a8() {
        let src = [0u8, 100, 255];
        let mut dest = vec![50u8; 3];
        image(&mut img_dsc(&mut dest, 3, &src, ColorFormat::A8, None, 255));
        assert_eq!(dest, [50, 100, 255]);

        let mut dest = vec![0u8; 3];
        image(&mut img_dsc(&mut dest, 3, &src, ColorFormat::A8, None, 128));
        assert_eq!(dest, [0, 50, 127]);

        let mask = [255u8, 128, 128];
        let mut dest = vec![0u8; 3];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::A8,
            Some(&mask),
            255,
        ));
        assert_eq!(dest, [0, 50, 127]);

        let mut dest = vec![0u8; 3];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::A8,
            Some(&mask),
            128,
        ));
        // LV_OPA_MIX3(128, 128, 255) = (128 * 128 * 255) >> 16 = 63
        assert_eq!(dest, [0, ((128u32 * 128 * 100) >> 16) as u8, 63]);
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn image_from_argb8888() {
        let src = [1u8, 2, 3, 0, 1, 2, 3, 128, 1, 2, 3, 255];
        let mut dest = vec![10u8; 3];
        image(&mut img_dsc(&mut dest, 3, &src, ColorFormat::Argb8888, None, 255));
        assert_eq!(dest, [10, 128, 255]);

        let mask = [255u8, 255, 0];
        let mut dest = vec![10u8; 3];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::Argb8888,
            Some(&mask),
            255,
        ));
        assert_eq!(dest, [10, 127, 10]);
    }

    #[cfg(feature = "cf-l8")]
    #[test]
    fn image_from_l8_uses_only_opa_and_mask() {
        let src = [0u8, 0, 0];
        let mut dest = vec![10u8; 3];
        image(&mut img_dsc(&mut dest, 3, &src, ColorFormat::L8, None, 255));
        assert_eq!(dest, [255, 255, 255]);

        let mut dest = vec![10u8; 3];
        image(&mut img_dsc(&mut dest, 3, &src, ColorFormat::L8, None, 100));
        assert_eq!(dest, [100, 100, 100]);

        let mask = [0u8, 128, 255];
        let mut dest = vec![10u8; 3];
        image(&mut img_dsc(
            &mut dest,
            3,
            &src,
            ColorFormat::L8,
            Some(&mask),
            128,
        ));
        assert_eq!(dest, [10, 64, 127]);
    }

    #[cfg(feature = "cf-al88")]
    #[test]
    fn image_from_al88() {
        let src = [255u8, 0, 0, 200];
        let mut dest = vec![10u8; 2];
        image(&mut img_dsc(&mut dest, 2, &src, ColorFormat::Al88, None, 255));
        assert_eq!(dest, [10, 200]);
    }

    #[cfg(feature = "cf-i1")]
    #[test]
    fn image_from_i1() {
        let src = [0b1010_0000u8];
        let mut dest = vec![10u8; 3];
        image(&mut img_dsc(&mut dest, 3, &src, ColorFormat::I1, None, 255));
        assert_eq!(dest, [255, 10, 255]);

        let mut dest = vec![10u8; 3];
        image(&mut img_dsc(&mut dest, 3, &src, ColorFormat::I1, None, 100));
        assert_eq!(dest, [100, 10, 100]);
    }
}
