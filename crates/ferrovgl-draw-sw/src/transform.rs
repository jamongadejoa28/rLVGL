//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_transform.c`.
//!
//! Rotation / scaling of images with nearest or bilinear sampling (the
//! software transform has no skew path upstream).
//!
//! Port notes:
//! - The upstream file has no `LV_DRAW_TRANSFORM_USE_MATRIX` branch; the
//!   inverse transformation is always computed analytically from
//!   `rotation`, `scale_x`, `scale_y` and `pivot`.
//! - Buffers are byte slices. Multi-byte pixels (`lv_color32_t`,
//!   `lv_draw_sw_word_t`, `lv_draw_sw_halfword_t`) are read and written
//!   little-endian, like the C structs on the reference targets. The C
//!   "row pointer += stride" becomes a per-row sub-slice.
//! - The separate alpha plane of the RGB565 / RGB565A8 / L8 / AL88
//!   destinations (`alpha_buf` upstream) is the tail of `dest_buf`, split
//!   off with `split_at_mut` at the same offset the C code computes.
//! - `LV_DRAW_SW_SUPPORT_*` map to the `cf-*` cargo features.
//!   `LV_DRAW_SW_SUPPORT_RGB565A8` has no feature and is always enabled
//!   (its `lv_conf` default).
//! - The non-swapped and swapped RGB565A8 row kernels share one body
//!   parametrised with a `SWAP` const generic; the swapped variant only
//!   applies `lv_color_swap_16()` to every colour read, exactly like the two
//!   upstream copies.
//! - `lv_color_mix32_inlined()` is [`Color32::mix32`] (same `LV_UDIV255`
//!   rounding, same `LV_OPA_MIN` / `LV_OPA_MAX` early returns).
//! - With `cf-argb8888-premultiplied` (the parity configuration) an
//!   anti-aliased ARGB8888 transform leaves the result premultiplied, as
//!   upstream does; without it the row is unpremultiplied at the end.
//! - A destination area with a non-positive width or height returns early:
//!   upstream writes nothing in that case either (all loops are empty).

use ferrovgl_core::draw::image::{DrawImageDsc, DrawImageSup};
use ferrovgl_core::misc::area::{Area, Point};
#[cfg(any(
    feature = "cf-rgb888",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
use ferrovgl_core::misc::color::Color32;
use ferrovgl_core::misc::color::ColorFormat;
use ferrovgl_core::misc::math::{TRIGO_SHIFT, trigo_sin};

/// Per-pixel sampling data (`point_transform_dsc_t`).
#[derive(Clone, Debug, Default)]
pub struct PointTransfDsc {
    /// `sinma`
    pub sinma: i32,
    /// `cosma`
    pub cosma: i32,
    /// `scale_x`
    pub scale_x: i32,
    /// `scale_y`
    pub scale_y: i32,
    /// `angle`
    pub angle: i32,
    /// `pivot_x_256`
    pub pivot_x_256: i32,
    /// `pivot_y_256`
    pub pivot_y_256: i32,
    /// `pivot`
    pub pivot: Point,
}

/* ---------------------------------------------------------------------- */
/* Byte access helpers                                                    */
/* ---------------------------------------------------------------------- */

#[cfg(any(
    feature = "cf-rgb888",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
/// Read a little-endian `u32` (`lv_draw_sw_word_t::u32`) at byte `i`.
#[inline(always)]
fn rd_u32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

#[cfg(any(
    feature = "cf-rgb888",
    feature = "cf-xrgb8888",
    feature = "cf-argb8888",
    feature = "cf-argb8888-premultiplied"
))]
/// Write a little-endian `u32` at byte `i`.
#[inline(always)]
fn wr_u32(b: &mut [u8], i: usize, v: u32) {
    b[i..i + 4].copy_from_slice(&v.to_le_bytes());
}

/// Read a little-endian `u16` (`lv_draw_sw_halfword_t::u16`) at byte `i`.
#[inline(always)]
fn rd_u16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

/// Write a little-endian `u16` at byte `i`.
#[inline(always)]
fn wr_u16(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}

/// Byte index of a C pointer offset (in-bounds by construction).
#[inline(always)]
fn ix(i: i32) -> usize {
    i as usize
}

/// `base + ((step * x_abs) >> 8)` with C's 32-bit wrapping arithmetic.
#[inline(always)]
fn ups(base: i32, step: i32, x_abs: i32) -> i32 {
    base.wrapping_add(step.wrapping_mul(x_abs) >> 8)
}

/// `lv_color_swap_16()`
#[inline(always)]
fn color_swap_16(c: u16) -> u16 {
    c.swap_bytes()
}

/* ---------------------------------------------------------------------- */
/* Global function                                                        */
/* ---------------------------------------------------------------------- */

/// `lv_draw_sw_transform()`: transform `src` (`src_w` x `src_h`, `src_cf`)
/// into `dest_buf` covering `dest_area`. The destination is always
/// ARGB8888 (or RGB565A8 / L8 / AL88 for matching sources, as upstream).
pub fn transform(
    dest_area: &Area,
    src_buf: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    draw_dsc: &DrawImageDsc,
    sup: Option<&DrawImageSup>,
    src_cf: ColorFormat,
    dest_buf: &mut [u8],
) {
    let _ = sup; /*LV_UNUSED(sup)*/
    transform_core(
        dest_area,
        src_buf,
        src_w,
        src_h,
        src_stride,
        draw_dsc.rotation,
        draw_dsc.scale_x,
        draw_dsc.scale_y,
        draw_dsc.pivot,
        draw_dsc.antialias,
        src_cf,
        dest_buf,
    );
}

/// Body of `lv_draw_sw_transform()` taking the used `lv_draw_image_dsc_t`
/// fields (`rotation`, `scale_x`, `scale_y`, `pivot`, `antialias`) directly.
fn transform_core(
    dest_area: &Area,
    src_buf: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    rotation: i32,
    scale_x: i32,
    scale_y: i32,
    pivot: Point,
    antialias: bool,
    src_cf: ColorFormat,
    dest_buf: &mut [u8],
) {
    let mut tr_dsc = PointTransfDsc {
        angle: rotation.wrapping_neg(),
        scale_x,
        scale_y,
        pivot,
        ..PointTransfDsc::default()
    };

    let angle_low = tr_dsc.angle / 10;
    let angle_high = angle_low + 1;
    let angle_rem = tr_dsc.angle - (angle_low * 10);

    let s1 = trigo_sin(angle_low as i16);
    let s2 = trigo_sin(angle_high as i16);

    let c1 = trigo_sin((angle_low + 90) as i16);
    let c2 = trigo_sin((angle_high + 90) as i16);

    tr_dsc.sinma = (s1 * (10 - angle_rem) + s2 * angle_rem) / 10;
    tr_dsc.cosma = (c1 * (10 - angle_rem) + c2 * angle_rem) / 10;
    tr_dsc.sinma >>= TRIGO_SHIFT - 10;
    tr_dsc.cosma >>= TRIGO_SHIFT - 10;
    tr_dsc.pivot_x_256 = tr_dsc.pivot.x.wrapping_mul(256);
    tr_dsc.pivot_y_256 = tr_dsc.pivot.y.wrapping_mul(256);

    let dest_w = dest_area.width();
    let dest_h = dest_area.height();
    /*Nothing would be written by upstream either*/
    if dest_w <= 0 || dest_h <= 0 {
        return;
    }

    let dest_stride_a8 = dest_w;
    let dest_stride: i32 = if src_cf == ColorFormat::Rgb888 {
        dest_w * ColorFormat::Argb8888.size() as i32
    } else if src_cf == ColorFormat::Rgb565A8 {
        dest_w * 2
    } else if src_cf == ColorFormat::L8 || src_cf == ColorFormat::Al88 {
        dest_w
    } else {
        dest_w * src_cf.size() as i32
    };

    let alpha_ofs: Option<usize> = if src_cf == ColorFormat::Rgb565
        || src_cf == ColorFormat::Rgb565Swapped
        || src_cf == ColorFormat::Rgb565A8
    {
        Some(ix(dest_stride * dest_h))
    } else if src_cf == ColorFormat::L8 || src_cf == ColorFormat::Al88 {
        Some(ix(dest_w * dest_h))
    } else {
        None
    };
    let (cbuf_all, abuf_all): (&mut [u8], &mut [u8]) = match alpha_ofs {
        Some(o) => dest_buf.split_at_mut(o),
        None => (dest_buf, &mut []),
    };

    let aa = antialias;
    let is_rotated = rotation != 0;

    /*Compute the steps and the anchor point of the inverse transformation analytically.
     *The source coordinate of a destination pixel is
     *    xs_ups(x, y) = x0_ups + ((sxy * y) >> 8) + ((sxx * x) >> 8)
     *where x and y are the absolute (image local) destination coordinates.
     *As it doesn't depend on the rendered area, rendering only a part of the image
     *results in exactly the same pixels as rendering the whole image
     *(needed for deterministic partial rendering).*/
    let mut sinma = tr_dsc.sinma;
    let mut cosma = tr_dsc.cosma;
    if tr_dsc.angle == 0 {
        /*Use the exact values to make the no-rotation case bit-exact (the table gives 1023 for cos(0))*/
        sinma = 0;
        cosma = 1024;
    }
    let sxx = ((cosma as i64 * 16384) / tr_dsc.scale_x as i64) as i32;
    let sxy = ((-(sinma as i64) * 16384) / tr_dsc.scale_x as i64) as i32;
    let syx = ((sinma as i64 * 16384) / tr_dsc.scale_y as i64) as i32;
    let syy = ((cosma as i64 * 16384) / tr_dsc.scale_y as i64) as i32;

    let xin0: i64 = -(tr_dsc.pivot.x as i64);
    let yin0: i64 = -(tr_dsc.pivot.y as i64);
    let x0_ups = ((((cosma as i64)
        .wrapping_mul(xin0)
        .wrapping_sub((sinma as i64).wrapping_mul(yin0)))
    .wrapping_mul(64))
        / tr_dsc.scale_x as i64) as i32;
    let x0_ups = x0_ups.wrapping_add(tr_dsc.pivot_x_256).wrapping_add(0x80);
    let y0_ups = ((((sinma as i64)
        .wrapping_mul(xin0)
        .wrapping_add((cosma as i64).wrapping_mul(yin0)))
    .wrapping_mul(64))
        / tr_dsc.scale_y as i64) as i32;
    let y0_ups = y0_ups.wrapping_add(tr_dsc.pivot_y_256).wrapping_add(0x80);

    /*If scaled only, keep the source coordinates inside the image to avoid
     *fading out the last column/row due to the rounding of the steps.*/
    let mut xs_clamp_ups = i32::MAX;
    let mut ys_clamp_ups = i32::MAX;
    if !is_rotated {
        xs_clamp_ups = (src_w - 1).wrapping_mul(256).wrapping_add(0x80);
        ys_clamp_ups = (src_h - 1).wrapping_mul(256).wrapping_add(0x80);
    }

    let x_start = dest_area.x1;
    let x_end_abs = dest_area.x1 + dest_w;
    for y in 0..dest_h {
        let y_abs = dest_area.y1 + y;
        let xs_ups = x0_ups.wrapping_add(((sxy as i64 * y_abs as i64) >> 8) as i32);
        let mut ys_ups = y0_ups.wrapping_add(((syy as i64 * y_abs as i64) >> 8) as i32);
        /*If scaled only, `ys` is constant in the row (syx == 0) so it can be clamped here*/
        if ys_ups > ys_clamp_ups {
            ys_ups = ys_clamp_ups;
        }
        let xs_step_256 = sxx;
        let ys_step_256 = syx;

        let c_ofs = ix(y * dest_stride);
        let crow = &mut cbuf_all[c_ofs..c_ofs + ix(dest_stride)];
        let arow: &mut [u8] = if alpha_ofs.is_some() {
            let a_ofs = ix(y * dest_stride_a8);
            &mut abuf_all[a_ofs..a_ofs + ix(dest_w)]
        } else {
            &mut []
        };

        match src_cf {
            #[cfg(feature = "cf-xrgb8888")]
            ColorFormat::Xrgb8888 => transform_rgb888(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                aa,
                4,
            ),
            #[cfg(feature = "cf-rgb888")]
            ColorFormat::Rgb888 => transform_rgb888(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                aa,
                3,
            ),
            #[cfg(feature = "cf-a8")]
            ColorFormat::A8 => transform_a8(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                aa,
            ),
            #[cfg(feature = "cf-argb8888")]
            ColorFormat::Argb8888 => transform_argb8888(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                aa,
            ),
            #[cfg(feature = "cf-argb8888-premultiplied")]
            ColorFormat::Argb8888Premultiplied => transform_argb8888_premultiplied(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                aa,
            ),
            #[cfg(feature = "cf-rgb565")]
            ColorFormat::Rgb565 => transform_rgb565a8(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                arow,
                false,
                aa,
            ),
            #[cfg(feature = "cf-rgb565-swapped")]
            ColorFormat::Rgb565Swapped => transform_rgb565a8_swapped(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                arow,
                false,
                aa,
            ),
            ColorFormat::Rgb565A8 => transform_rgb565a8(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                arow,
                true,
                aa,
            ),
            #[cfg(feature = "cf-l8")]
            ColorFormat::L8 => transform_al88(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                arow,
                false,
                aa,
            ),
            #[cfg(feature = "cf-al88")]
            ColorFormat::Al88 => transform_al88(
                src_buf,
                src_w,
                src_h,
                src_stride,
                xs_ups,
                ys_ups,
                xs_step_256,
                ys_step_256,
                x_start,
                x_end_abs,
                xs_clamp_ups,
                crow,
                arow,
                true,
                aa,
            ),
            /*LV_LOG_WARN("Color format 0x%02X is not enabled. ...")*/
            _ => return,
        }
    }
}

/* ---------------------------------------------------------------------- */
/* Static functions                                                       */
/* ---------------------------------------------------------------------- */

/// `unpremultiply()`: undo the alpha scaling of a premultiplied pixel,
/// giving back a straight color.
#[cfg(any(feature = "cf-argb8888", feature = "cf-argb8888-premultiplied"))]
#[inline]
fn unpremultiply(mut c: Color32) -> Color32 {
    if c.alpha == 0 {
        c.red = 0;
        c.green = 0;
        c.blue = 0;
    } else {
        let reciprocal_alpha: u32 = ((255 * 256) / c.alpha as u32) as u16 as u32;
        c.red = ((c.red as u32 * reciprocal_alpha) >> 8) as u8;
        c.green = ((c.green as u32 * reciprocal_alpha) >> 8) as u8;
        c.blue = ((c.blue as u32 * reciprocal_alpha) >> 8) as u8;
    }
    c
}

/// `transform_norm_inv`: `(32 << 8) / alpha_sum` for every possible alpha
/// sum, so the alpha weighted color interpolation can normalize without a
/// division.
#[rustfmt::skip]
static TRANSFORM_NORM_INV: [u16; 257] = [
    0, 8192, 4096, 2730, 2048, 1638, 1365, 1170, 1024, 910, 819, 744, 682, 630, 585, 546, 512, 481, 455,
    431, 409, 390, 372, 356, 341, 327, 315, 303, 292, 282, 273, 264, 256, 248, 240, 234, 227, 221, 215,
    210, 204, 199, 195, 190, 186, 182, 178, 174, 170, 167, 163, 160, 157, 154, 151, 148, 146, 143, 141,
    138, 136, 134, 132, 130, 128, 126, 124, 122, 120, 118, 117, 115, 113, 112, 110, 109, 107, 106, 105,
    103, 102, 101, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 81, 80,
    79, 78, 78, 77, 76, 75, 75, 74, 73, 73, 72, 71, 71, 70, 70, 69, 68, 68, 67, 67, 66, 66, 65, 65, 64,
    64, 63, 63, 62, 62, 61, 61, 60, 60, 59, 59, 58, 58, 58, 57, 57, 56, 56, 56, 55, 55, 54, 54, 54, 53,
    53, 53, 52, 52, 52, 51, 51, 51, 50, 50, 50, 49, 49, 49, 49, 48, 48, 48, 47, 47, 47, 47, 46, 46, 46,
    46, 45, 45, 45, 45, 44, 44, 44, 44, 43, 43, 43, 43, 42, 42, 42, 42, 42, 41, 41, 41, 41, 40, 40, 40,
    40, 40, 39, 39, 39, 39, 39, 39, 38, 38, 38, 38, 38, 37, 37, 37, 37, 37, 37, 36, 36, 36, 36, 36, 36,
    35, 35, 35, 35, 35, 35, 35, 34, 34, 34, 34, 34, 34, 33, 33, 33, 33, 33, 33, 33, 33, 32, 32, 32, 32,
    32, 32, 32, 32
];

/// `transform_alpha_weights()`: fold the bilinear weights of a 2x2
/// neighborhood together with its alphas. Writes the four normalized 5 bit
/// color weights (adding up to at most 32) to `t` and returns the
/// interpolated alpha (0..255).
#[inline]
fn transform_alpha_weights(
    a00: u32,
    a01: u32,
    a10: u32,
    a11: u32,
    fx: u32,
    fy: u32,
    t: &mut [u32; 4],
) -> u32 {
    let ifx = 256 - fx;
    let ify = 256 - fy;

    /*Scale each alpha to 0..256 so an opaque neighbor keeps its full weight*/
    let u00 = (((ifx * ify) >> 8) * (a00 + (a00 >> 7))) >> 8;
    let u01 = (((fx * ify) >> 8) * (a01 + (a01 >> 7))) >> 8;
    let u10 = (((ifx * fy) >> 8) * (a10 + (a10 >> 7))) >> 8;
    let u11 = (((fx * fy) >> 8) * (a11 + (a11 >> 7))) >> 8;

    let sum = u00 + u01 + u10 + u11;
    if sum == 0 {
        *t = [0; 4];
        return 0;
    }

    /*Normalize on the running total instead of each weight on its own*/
    let inv = TRANSFORM_NORM_INV[sum as usize] as u32;
    let c1 = (u00 * inv) >> 8;
    let c2 = ((u00 + u01) * inv) >> 8;
    let c3 = ((u00 + u01 + u10) * inv) >> 8;
    let c4 = (sum * inv) >> 8;
    t[0] = c1;
    t[1] = c2 - c1;
    t[2] = c3 - c2;
    t[3] = c4 - c3;

    /*Whatever the rounding left over goes to the neighbor that already weighs the most.*/
    let rem = 32 - c4;
    if rem != 0 {
        let mut m = 0;
        let mut best = u00;
        if u01 > best {
            best = u01;
            m = 1;
        }
        if u10 > best {
            best = u10;
            m = 2;
        }
        if u11 > best {
            m = 3;
        }
        t[m] += rem;
    }

    if sum > 255 { 255 } else { sum }
}

/// `argb8888_premul()`: premultiply an ARGB8888 pixel.
#[cfg(feature = "cf-argb8888")]
#[inline]
fn argb8888_premul(p: u32) -> u32 {
    let a = p >> 24;
    let a256 = a + (a >> 7);
    let rb = (((p & 0x00FF_00FF) * a256) >> 8) & 0x00FF_00FF;
    let g = (((p & 0x0000_FF00) * a256) >> 8) & 0x0000_FF00;
    rb | g | (a << 24)
}

/// `argb8888_bilinear_premul()`: bilinear filtering of a 2x2 ARGB8888
/// neighborhood, computed in premultiplied alpha space. Returns the
/// filtered pixel, premultiplied.
#[cfg(feature = "cf-argb8888")]
#[inline]
fn argb8888_bilinear_premul(p00: u32, p01: u32, p10: u32, p11: u32, fx: u32, fy: u32) -> u32 {
    let ifx = 256 - fx;
    let ify = 256 - fy;

    let (a00, a01, a10, a11) = (p00 >> 24, p01 >> 24, p10 >> 24, p11 >> 24);
    let u00 = (((ifx * ify) >> 8) * (a00 + (a00 >> 7))) >> 8;
    let u01 = (((fx * ify) >> 8) * (a01 + (a01 >> 7))) >> 8;
    let u10 = (((ifx * fy) >> 8) * (a10 + (a10 >> 7))) >> 8;
    let u11 = (((fx * fy) >> 8) * (a11 + (a11 >> 7))) >> 8;

    let rb = (p00 & 0x00FF_00FF) * u00
        + (p01 & 0x00FF_00FF) * u01
        + (p10 & 0x00FF_00FF) * u10
        + (p11 & 0x00FF_00FF) * u11;
    let g = (p00 & 0x0000_FF00) * u00
        + (p01 & 0x0000_FF00) * u01
        + (p10 & 0x0000_FF00) * u10
        + (p11 & 0x0000_FF00) * u11;

    /*In premultiplied space the resulting alpha is just the sum of the folded weights*/
    let mut a = u00 + u01 + u10 + u11;
    if a > 255 {
        a = 255;
    }

    ((rb >> 8) & 0x00FF_00FF) | ((g >> 8) & 0x0000_FF00) | (a << 24)
}

/// Read an `lv_color32_t` from byte offset `i` of an RGB888 / XRGB8888
/// source (`blue, green, red` byte order) with an opaque alpha.
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
#[inline(always)]
fn rgb888_px(src: &[u8], i: usize) -> Color32 {
    Color32 {
        red: src[i + 2],
        green: src[i + 1],
        blue: src[i],
        alpha: 0xff,
    }
}

/// `rgb888_row_checked()`: the edges of an RGB888 / XRGB8888 row, with
/// bounds checking and edge handling.
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
fn rgb888_row_checked(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_base: i32,
    ys_base: i32,
    xs_step: i32,
    ys_step: i32,
    x_from: i32,
    x_to: i32,
    x_offs: i32,
    xs_clamp_ups: i32,
    dest_c32: &mut [u8],
    aa: bool,
    px_size: i32,
) {
    for x in x_from..x_to {
        let x_abs = x + x_offs;
        let mut xs_ups = ups(xs_base, xs_step, x_abs);
        let ys_ups = ups(ys_base, ys_step, x_abs);
        if xs_ups > xs_clamp_ups {
            xs_ups = xs_clamp_ups;
        }

        let xs_int = xs_ups >> 8;
        let ys_int = ys_ups >> 8;
        let di = ix(x) * 4;

        /*Fully out of the image*/
        if xs_int < 0 || xs_int >= src_w || ys_int < 0 || ys_int >= src_h {
            dest_c32[di + 3] = 0x00;
            continue;
        }

        /*Get the direction the hor and ver neighbor
         *`fract` will be in range of 0x00..0xFF and `next` (+/-1) indicates the direction*/
        let mut xs_fract = xs_ups & 0xFF;
        let mut ys_fract = ys_ups & 0xFF;

        let x_next: i32;
        let y_next: i32;
        if xs_fract < 0x80 {
            x_next = -1;
            xs_fract = 0x7F - xs_fract;
        } else {
            x_next = 1;
            xs_fract -= 0x80;
        }
        if ys_fract < 0x80 {
            y_next = -1;
            ys_fract = 0x7F - ys_fract;
        } else {
            y_next = 1;
            ys_fract -= 0x80;
        }

        let src_u8 = ys_int * src_stride + xs_int * px_size;

        let mut d = rgb888_px(src, ix(src_u8));

        if aa
            && xs_int + x_next >= 0
            && xs_int + x_next < src_w
            && ys_int + y_next >= 0
            && ys_int + y_next < src_h
        {
            let mut px_hor = rgb888_px(src, ix(src_u8 + x_next * px_size));
            let mut px_ver = rgb888_px(src, ix(src_u8 + y_next * src_stride));

            if !d.eq(px_ver) {
                px_ver.alpha = ys_fract as u8;
                d = Color32::mix32(px_ver, d);
            }

            if !d.eq(px_hor) {
                px_hor.alpha = xs_fract as u8;
                d = Color32::mix32(px_hor, d);
            }
        }
        /*Partially out of the image*/
        else {
            let a: i32 = 0xff;

            if (xs_int == 0 && x_next < 0) || (xs_int == src_w - 1 && x_next > 0) {
                d.alpha = ((a * (0xFF - xs_fract)) >> 8) as u8;
            } else if (ys_int == 0 && y_next < 0) || (ys_int == src_h - 1 && y_next > 0) {
                d.alpha = ((a * (0xFF - ys_fract)) >> 8) as u8;
            }
        }
        wr_u32(dest_c32, di, d.to_u32());
    }
}

/// `rgb888_row_bilinear()`: the interior of an anti-aliased RGB888 /
/// XRGB8888 row where all four neighbors are inside the image.
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
fn rgb888_row_bilinear(
    src: &[u8],
    src_stride: i32,
    xs_base: i32,
    ys_base: i32,
    xs_step: i32,
    ys_step: i32,
    x_from: i32,
    x_to: i32,
    x_offs: i32,
    dest_c32: &mut [u8],
    px_size: i32,
) {
    for x in x_from..x_to {
        let x_abs = x + x_offs;
        let xs_ups = ups(xs_base, xs_step, x_abs).wrapping_sub(0x80);
        let ys_ups = ups(ys_base, ys_step, x_abs).wrapping_sub(0x80);
        let xs_int = xs_ups >> 8;
        let ys_int = ys_ups >> 8;

        let p = ix(ys_int * src_stride + xs_int * px_size);
        let p2 = p + ix(src_stride);
        let (p00, p01, p10, p11);
        if px_size == 4 {
            p00 = rd_u32(src, p);
            p01 = rd_u32(src, p + 4);
            p10 = rd_u32(src, p2);
            p11 = rd_u32(src, p2 + 4);
        } else {
            p00 = src[p] as u32 | ((src[p + 1] as u32) << 8) | ((src[p + 2] as u32) << 16);
            p01 = src[p + 3] as u32 | ((src[p + 4] as u32) << 8) | ((src[p + 5] as u32) << 16);
            p10 = src[p2] as u32 | ((src[p2 + 1] as u32) << 8) | ((src[p2 + 2] as u32) << 16);
            p11 = src[p2 + 3] as u32 | ((src[p2 + 4] as u32) << 8) | ((src[p2 + 5] as u32) << 16);
        }

        /*True bilinear interpolation. The red and blue channels are interpolated together
         *in one 32 bit value with a single multiplication each.*/
        let fx = (xs_ups & 0xFF) as u32;
        let fx_inv = 256 - fx;
        let t_rb = (((p00 & 0xFF00FF) * fx_inv + (p01 & 0xFF00FF) * fx) >> 8) & 0xFF00FF;
        let t_g = (((p00 & 0xFF00) * fx_inv + (p01 & 0xFF00) * fx) >> 8) & 0xFF00;
        let b_rb = (((p10 & 0xFF00FF) * fx_inv + (p11 & 0xFF00FF) * fx) >> 8) & 0xFF00FF;
        let b_g = (((p10 & 0xFF00) * fx_inv + (p11 & 0xFF00) * fx) >> 8) & 0xFF00;
        let fy = (ys_ups & 0xFF) as u32;
        let fy_inv = 256 - fy;
        let rb = ((t_rb * fy_inv + b_rb * fy) >> 8) & 0xFF00FF;
        let g = ((t_g * fy_inv + b_g * fy) >> 8) & 0xFF00;
        wr_u32(dest_c32, ix(x) * 4, rb | g | 0xFF00_0000);
    }
}

/// `transform_rgb888()`: transform one row of an RGB888 (`px_size == 3`) or
/// XRGB8888 (`px_size == 4`) image into ARGB8888.
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
fn transform_rgb888(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    x_start: i32,
    x_end: i32,
    xs_clamp_ups: i32,
    dest_c32: &mut [u8],
    aa: bool,
    px_size: i32,
) {
    /*The destination is indexed from zero while the source coordinates are calculated from the
     *absolute (image local) x coordinate to keep the partial rendering deterministic*/
    let w = x_end - x_start;

    /*In the middle of the row the pixel and all its neighbors are inside the source image,
     *so neither bounds checking nor edge handling is needed there*/
    let (mut fast_from, mut fast_to) =
        transform_safe_range(xs_ups, ys_ups, xs_step, ys_step, src_w, src_h, x_start, x_end, aa);
    fast_from -= x_start;
    fast_to -= x_start;

    rgb888_row_checked(
        src,
        src_w,
        src_h,
        src_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        0,
        fast_from,
        x_start,
        xs_clamp_ups,
        dest_c32,
        aa,
        px_size,
    );

    if aa {
        rgb888_row_bilinear(
            src, src_stride, xs_ups, ys_ups, xs_step, ys_step, fast_from, fast_to, x_start, dest_c32, px_size,
        );
    } else {
        for x in fast_from..fast_to {
            let x_abs = x + x_start;
            let xs_int = ups(xs_ups, xs_step, x_abs) >> 8;
            let ys_int = ups(ys_ups, ys_step, x_abs) >> 8;
            let s = ix(ys_int * src_stride + xs_int * px_size);
            wr_u32(dest_c32, ix(x) * 4, rgb888_px(src, s).to_u32());
        }
    }

    rgb888_row_checked(
        src,
        src_w,
        src_h,
        src_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        fast_to,
        w,
        x_start,
        xs_clamp_ups,
        dest_c32,
        aa,
        px_size,
    );
}

/// `argb8888_row_checked()`: the edges of an ARGB8888 row; taps outside the
/// image count as fully transparent.
#[cfg(feature = "cf-argb8888")]
fn argb8888_row_checked(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_base: i32,
    ys_base: i32,
    xs_step: i32,
    ys_step: i32,
    x_from: i32,
    x_to: i32,
    x_offs: i32,
    xs_clamp_ups: i32,
    dest_w: &mut [u8],
    aa: bool,
) {
    for x in x_from..x_to {
        let x_abs = x + x_offs;
        let mut xs_ups = ups(xs_base, xs_step, x_abs);
        let ys_ups = ups(ys_base, ys_step, x_abs);
        if xs_ups > xs_clamp_ups {
            xs_ups = xs_clamp_ups;
        }

        let xs_int = xs_ups >> 8;
        let ys_int = ys_ups >> 8;
        let di = ix(x) * 4;

        /*Fully out of the image*/
        if xs_int < 0 || xs_int >= src_w || ys_int < 0 || ys_int >= src_h {
            wr_u32(dest_w, di, 0x0000_0000);
            continue;
        }

        if !aa {
            wr_u32(dest_w, di, rd_u32(src, ix(ys_int * src_stride + xs_int * 4)));
            continue;
        }

        /*Sample the 2x2 neighborhood. Taps outside the image count as fully transparent,
         *so the edge fades out on its own and needs no dedicated handling.*/
        let xs_ofs = xs_ups.wrapping_sub(0x80);
        let ys_ofs = ys_ups.wrapping_sub(0x80);
        let x0 = xs_ofs >> 8;
        let y0 = ys_ofs >> 8;
        let mut p = [0u32; 4];
        for (i, pi) in p.iter_mut().enumerate() {
            let sx = x0 + (i as i32 & 1);
            let sy = y0 + (i as i32 >> 1);
            if sx >= 0 && sx < src_w && sy >= 0 && sy < src_h {
                *pi = rd_u32(src, ix(sy * src_stride + sx * 4));
            }
        }

        wr_u32(
            dest_w,
            di,
            argb8888_bilinear_premul(
                p[0],
                p[1],
                p[2],
                p[3],
                (xs_ofs & 0xFF) as u32,
                (ys_ofs & 0xFF) as u32,
            ),
        );
    }
}

/// `argb8888_row_fast()`: the interior of an anti-aliased ARGB8888 row
/// where all four neighbors are inside the image.
#[cfg(feature = "cf-argb8888")]
fn argb8888_row_fast(
    src: &[u8],
    src_stride: i32,
    xs_base: i32,
    ys_base: i32,
    xs_step: i32,
    ys_step: i32,
    x_from: i32,
    x_to: i32,
    x_offs: i32,
    dest_w: &mut [u8],
) {
    for x in x_from..x_to {
        let x_abs = x + x_offs;
        let xs_ups = ups(xs_base, xs_step, x_abs);
        let ys_ups = ups(ys_base, ys_step, x_abs);
        let xs_ups_ofs = xs_ups.wrapping_sub(0x80);
        let ys_ups_ofs = ys_ups.wrapping_sub(0x80);
        let xs_int = xs_ups_ofs >> 8;
        let ys_int = ys_ups_ofs >> 8;

        let pb = ix(ys_int * src_stride + xs_int * 4);
        let pb2 = pb + ix(src_stride);
        let p00 = rd_u32(src, pb);
        let p01 = rd_u32(src, pb + 4);
        let p10 = rd_u32(src, pb2);
        let p11 = rd_u32(src, pb2 + 4);
        let di = ix(x) * 4;

        if p00 == p01 && p00 == p10 && p00 == p11 {
            /*Flat neighborhood: the filter is the identity.*/
            wr_u32(dest_w, di, argb8888_premul(p00));
        } else if ((p00 & p01 & p10 & p11) >> 24) == 0xFF {
            /*All the 4 neighbors are opaque: use true bilinear interpolation.*/
            let fx = (xs_ups_ofs & 0xFF) as u32;
            let fx_inv = 256 - fx;
            let t_rb = (((p00 & 0xFF00FF) * fx_inv + (p01 & 0xFF00FF) * fx) >> 8) & 0xFF00FF;
            let t_g = (((p00 & 0xFF00) * fx_inv + (p01 & 0xFF00) * fx) >> 8) & 0xFF00;
            let b_rb = (((p10 & 0xFF00FF) * fx_inv + (p11 & 0xFF00FF) * fx) >> 8) & 0xFF00FF;
            let b_g = (((p10 & 0xFF00) * fx_inv + (p11 & 0xFF00) * fx) >> 8) & 0xFF00;
            let fy = (ys_ups_ofs & 0xFF) as u32;
            let fy_inv = 256 - fy;
            let rb = ((t_rb * fy_inv + b_rb * fy) >> 8) & 0xFF00FF;
            let g = ((t_g * fy_inv + b_g * fy) >> 8) & 0xFF00;
            wr_u32(dest_w, di, rb | g | 0xFF00_0000);
        } else {
            /*Non opaque pixels are involved: filter in premultiplied space*/
            wr_u32(
                dest_w,
                di,
                argb8888_bilinear_premul(
                    p00,
                    p01,
                    p10,
                    p11,
                    (xs_ups_ofs & 0xFF) as u32,
                    (ys_ups_ofs & 0xFF) as u32,
                ),
            );
        }
    }
}

/// `transform_argb8888()`: transform one row of an ARGB8888 image.
#[cfg(feature = "cf-argb8888")]
fn transform_argb8888(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    x_start: i32,
    x_end: i32,
    xs_clamp_ups: i32,
    dest_buf: &mut [u8],
    aa: bool,
) {
    /*The destination is indexed from zero while the source coordinates are calculated from the
     *absolute (image local) x coordinate to keep the partial rendering deterministic*/
    let w = x_end - x_start;

    let (mut fast_from, mut fast_to) =
        transform_safe_range(xs_ups, ys_ups, xs_step, ys_step, src_w, src_h, x_start, x_end, aa);
    fast_from -= x_start;
    fast_to -= x_start;

    argb8888_row_checked(
        src,
        src_w,
        src_h,
        src_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        0,
        fast_from,
        x_start,
        xs_clamp_ups,
        dest_buf,
        aa,
    );

    if aa {
        argb8888_row_fast(
            src, src_stride, xs_ups, ys_ups, xs_step, ys_step, fast_from, fast_to, x_start, dest_buf,
        );
    } else {
        for x in fast_from..fast_to {
            let x_abs = x + x_start;
            let xs_int = ups(xs_ups, xs_step, x_abs) >> 8;
            let ys_int = ups(ys_ups, ys_step, x_abs) >> 8;
            wr_u32(
                dest_buf,
                ix(x) * 4,
                rd_u32(src, ix(ys_int * src_stride + xs_int * 4)),
            );
        }
    }

    argb8888_row_checked(
        src,
        src_w,
        src_h,
        src_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        fast_to,
        w,
        x_start,
        xs_clamp_ups,
        dest_buf,
        aa,
    );

    /*The antialiasing filter works in premultiplied space. Without the premultiplied blenders
     *the result is consumed as straight ARGB8888, so convert it back.*/
    #[cfg(not(feature = "cf-argb8888-premultiplied"))]
    if aa {
        for i in 0..w {
            let o = ix(i) * 4;
            let c = unpremultiply(Color32::from_u32(rd_u32(dest_buf, o)));
            wr_u32(dest_buf, o, c.to_u32());
        }
    }
}

/// `transform_argb8888_premultiplied()`: transform one row of a
/// premultiplied ARGB8888 image.
#[cfg(feature = "cf-argb8888-premultiplied")]
fn transform_argb8888_premultiplied(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    x_start: i32,
    x_end: i32,
    xs_clamp_ups: i32,
    dest_buf: &mut [u8],
    aa: bool,
) {
    let xs_ups_start = xs_ups;
    let ys_ups_start = ys_ups;
    let w = x_end - x_start;

    for x in 0..w {
        let x_abs = x + x_start;
        let mut xs_ups = ups(xs_ups_start, xs_step, x_abs);
        let ys_ups = ups(ys_ups_start, ys_step, x_abs);
        if xs_ups > xs_clamp_ups {
            xs_ups = xs_clamp_ups;
        }

        let xs_int = xs_ups >> 8;
        let ys_int = ys_ups >> 8;
        let di = ix(x) * 4;

        /*Fully out of the image*/
        if xs_int < 0 || xs_int >= src_w || ys_int < 0 || ys_int >= src_h {
            wr_u32(dest_buf, di, 0x0000_0000);
            continue;
        }

        /*Get the direction the hor and ver neighbor
         *`fract` will be in range of 0x00..0xFF and `next` (+/-1) indicates the direction*/
        let mut xs_fract = xs_ups & 0xFF;
        let mut ys_fract = ys_ups & 0xFF;

        let x_next: i32;
        let y_next: i32;
        if xs_fract < 0x80 {
            x_next = -1;
            xs_fract = 0x7F - xs_fract;
        } else {
            x_next = 1;
            xs_fract -= 0x80;
        }
        if ys_fract < 0x80 {
            y_next = -1;
            ys_fract = 0x7F - ys_fract;
        } else {
            y_next = 1;
            ys_fract -= 0x80;
        }

        let src_c32 = ys_int * src_stride + xs_int * 4;

        let mut d = Color32::from_u32(rd_u32(src, ix(src_c32)));

        if aa
            && xs_int + x_next >= 0
            && xs_int + x_next < src_w
            && ys_int + y_next >= 0
            && ys_int + y_next < src_h
        {
            let px_hor = Color32::from_u32(rd_u32(src, ix(src_c32 + x_next * 4)));
            let px_ver = Color32::from_u32(rd_u32(src, ix(src_c32 + y_next * src_stride)));

            /*Have the non-premultiplied colors first, mix them as needed,
             *and premultiply again*/
            d = unpremultiply(d);
            let mut px_hor = unpremultiply(px_hor);
            let mut px_ver = unpremultiply(px_ver);

            if px_ver.alpha == 0 {
                d.alpha = ((d.alpha as i32 * (0xFF - ys_fract)) >> 8) as u8;
            } else if !d.eq(px_ver) {
                if d.alpha != 0 {
                    d.alpha = (((px_ver.alpha as i32 * ys_fract) + (d.alpha as i32 * (0xFF - ys_fract))) >> 8)
                        as u8;
                }
                px_ver.alpha = ys_fract as u8;
                d = Color32::mix32(px_ver, d);
            }

            if px_hor.alpha == 0 {
                d.alpha = ((d.alpha as i32 * (0xFF - xs_fract)) >> 8) as u8;
            } else if !d.eq(px_hor) {
                if d.alpha != 0 {
                    d.alpha = (((px_hor.alpha as i32 * xs_fract) + (d.alpha as i32 * (0xFF - xs_fract))) >> 8)
                        as u8;
                }
                px_hor.alpha = xs_fract as u8;
                d = Color32::mix32(px_hor, d);
            }

            d.red = ((d.red as u32 * d.alpha as u32) >> 8) as u8;
            d.green = ((d.green as u32 * d.alpha as u32) >> 8) as u8;
            d.blue = ((d.blue as u32 * d.alpha as u32) >> 8) as u8;
        }
        /*Partially out of the image*/
        else if (xs_int == 0 && x_next < 0) || (xs_int == src_w - 1 && x_next > 0) {
            d = unpremultiply(d);
            let alpha = ((d.alpha as i32 * (0x7F - xs_fract)) >> 7) as u8;
            d.alpha = alpha;
            d.red = ((d.red as u32 * d.alpha as u32) >> 8) as u8;
            d.green = ((d.green as u32 * d.alpha as u32) >> 8) as u8;
            d.blue = ((d.blue as u32 * d.alpha as u32) >> 8) as u8;
        } else if (ys_int == 0 && y_next < 0) || (ys_int == src_h - 1 && y_next > 0) {
            d = unpremultiply(d);
            let alpha = ((d.alpha as i32 * (0x7F - ys_fract)) >> 7) as u8;
            d.alpha = alpha;
            d.red = ((d.red as u32 * d.alpha as u32) >> 8) as u8;
            d.green = ((d.green as u32 * d.alpha as u32) >> 8) as u8;
            d.blue = ((d.blue as u32 * d.alpha as u32) >> 8) as u8;
        }
        wr_u32(dest_buf, di, d.to_u32());
    }
}

/// Read an RGB565 colour at byte offset `i`, byte swapped when `SWAP`.
#[inline(always)]
fn rgb565_px<const SWAP: bool>(src: &[u8], i: usize) -> u16 {
    let c = rd_u16(src, i);
    if SWAP { color_swap_16(c) } else { c }
}

/// Expand an RGB565 colour to the `0x07E0F81F` lane layout.
#[inline(always)]
fn rgb565_expand(c: u16) -> u32 {
    (c as u32 | ((c as u32) << 16)) & 0x7E0F81F
}

/// `rgb565a8_row_fast()` (`SWAP == false`) and
/// `rgb565a8_swapped_row_fast()` (`SWAP == true`): the interior of an
/// anti-aliased RGB565(A8) row where all four neighbors are inside the
/// image.
fn rgb565a8_row_fast_impl<const SWAP: bool>(
    src: &[u8],
    src_stride: i32,
    src_alpha: usize,
    alpha_stride: i32,
    xs_base: i32,
    ys_base: i32,
    xs_step: i32,
    ys_step: i32,
    x_from: i32,
    x_to: i32,
    x_offs: i32,
    cbuf: &mut [u8],
    abuf: &mut [u8],
    src_has_a8: bool,
) {
    for x in x_from..x_to {
        let x_abs = x + x_offs;
        let xs_ups = ups(xs_base, xs_step, x_abs);
        let ys_ups = ups(ys_base, ys_step, x_abs);
        let xs_ups_ofs = xs_ups.wrapping_sub(0x80);
        let ys_ups_ofs = ys_ups.wrapping_sub(0x80);
        let xs_int = xs_ups_ofs >> 8;
        let ys_int = ys_ups_ofs >> 8;
        let fx = (xs_ups_ofs & 0xFF) as u32;
        let fy = (ys_ups_ofs & 0xFF) as u32;
        let xu = ix(x);

        let mut alpha_out: u32 = 0xFF;
        if src_has_a8 {
            let pa = src_alpha + ix(ys_int * alpha_stride + xs_int);
            let a_s = ix(alpha_stride);
            let a00 = src[pa] as u32;
            let a01 = src[pa + 1] as u32;
            let a10 = src[pa + a_s] as u32;
            let a11 = src[pa + a_s + 1] as u32;
            if a00 == a01 && a00 == a10 && a00 == a11 {
                /*Uniform alpha: the weighting cancels out, so the plain interpolation below is
                 *already exact. Covers opaque interiors and fully transparent areas alike.*/
                alpha_out = a00;
                if a00 == 0 {
                    abuf[xu] = 0;
                    wr_u16(cbuf, xu * 2, 0);
                    continue;
                }
            } else {
                /*Mixed alpha: weight the colors with their alpha so a transparent neighbor
                 *cannot bleed its color in, then normalize back to a straight color*/
                let mut t = [0u32; 4];
                let a = transform_alpha_weights(a00, a01, a10, a11, fx, fy, &mut t);
                abuf[xu] = a as u8;
                if a == 0 {
                    wr_u16(cbuf, xu * 2, 0);
                    continue;
                }
                let pcb = ix(ys_int * src_stride + xs_int * 2);
                let pcb2 = pcb + ix(src_stride);
                let w00 = rgb565_expand(rgb565_px::<SWAP>(src, pcb));
                let w01 = rgb565_expand(rgb565_px::<SWAP>(src, pcb + 2));
                let w10 = rgb565_expand(rgb565_px::<SWAP>(src, pcb2));
                let w11 = rgb565_expand(rgb565_px::<SWAP>(src, pcb2 + 2));
                let acc = ((w00 * t[0] + w01 * t[1] + w10 * t[2] + w11 * t[3]) >> 5) & 0x7E0F81F;
                wr_u16(cbuf, xu * 2, (acc | (acc >> 16)) as u16);
                continue;
            }
        }
        abuf[xu] = alpha_out as u8;

        /*All the 4 neighbors are opaque: use true bilinear interpolation.
         *5 bit weights are used so the 11 bit lanes can't overflow (63 * 32 < 2048).*/
        let pb = ix(ys_int * src_stride + xs_int * 2);
        let pb2 = pb + ix(src_stride);
        let e00 = rgb565_expand(rgb565_px::<SWAP>(src, pb));
        let e01 = rgb565_expand(rgb565_px::<SWAP>(src, pb + 2));
        let e10 = rgb565_expand(rgb565_px::<SWAP>(src, pb2));
        let e11 = rgb565_expand(rgb565_px::<SWAP>(src, pb2 + 2));
        let fx5 = (fx + 4) >> 3;
        let fy5 = (fy + 4) >> 3;
        let top = ((e00 * (32 - fx5) + e01 * fx5) >> 5) & 0x7E0F81F;
        let bot = ((e10 * (32 - fx5) + e11 * fx5) >> 5) & 0x7E0F81F;
        let res = ((top * (32 - fy5) + bot * fy5) >> 5) & 0x7E0F81F;
        wr_u16(cbuf, xu * 2, (res | (res >> 16)) as u16);
    }
}

/// `rgb565a8_row_checked()` (`SWAP == false`) and
/// `rgb565a8_swapped_row_checked()` (`SWAP == true`): the edges of an
/// RGB565(A8) row; taps outside the image count as fully transparent.
fn rgb565a8_row_checked_impl<const SWAP: bool>(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    src_alpha: usize,
    alpha_stride: i32,
    xs_base: i32,
    ys_base: i32,
    xs_step: i32,
    ys_step: i32,
    x_from: i32,
    x_to: i32,
    x_offs: i32,
    xs_clamp_ups: i32,
    cbuf: &mut [u8],
    abuf: &mut [u8],
    src_has_a8: bool,
    aa: bool,
) {
    for x in x_from..x_to {
        let x_abs = x + x_offs;
        let mut xs_ups = ups(xs_base, xs_step, x_abs);
        let ys_ups = ups(ys_base, ys_step, x_abs);
        if xs_ups > xs_clamp_ups {
            xs_ups = xs_clamp_ups;
        }

        let xs_int = xs_ups >> 8;
        let ys_int = ys_ups >> 8;
        let xu = ix(x);

        /*Fully out of the image*/
        if xs_int < 0 || xs_int >= src_w || ys_int < 0 || ys_int >= src_h {
            wr_u16(cbuf, xu * 2, 0);
            abuf[xu] = 0x00;
            continue;
        }

        if !aa {
            let c = rgb565_px::<SWAP>(src, ix(ys_int * src_stride + xs_int * 2));
            wr_u16(cbuf, xu * 2, c);
            abuf[xu] = if src_has_a8 {
                src[src_alpha + ix(ys_int * alpha_stride + xs_int)]
            } else {
                0xFF
            };
            continue;
        }

        /*Sample the 2x2 neighborhood. Taps outside the image count as fully transparent,
         *so the edge fades out on its own and needs no dedicated handling.*/
        let xs_ofs = xs_ups.wrapping_sub(0x80);
        let ys_ofs = ys_ups.wrapping_sub(0x80);
        let x0 = xs_ofs >> 8;
        let y0 = ys_ofs >> 8;

        let mut e = [0u32; 4];
        let mut a = [0u32; 4];
        for i in 0..4 {
            let sx = x0 + (i as i32 & 1);
            let sy = y0 + (i as i32 >> 1);
            if sx >= 0 && sx < src_w && sy >= 0 && sy < src_h {
                let cc = rgb565_px::<SWAP>(src, ix(sy * src_stride + sx * 2));
                e[i] = rgb565_expand(cc);
                a[i] = if src_has_a8 {
                    src[src_alpha + ix(sy * alpha_stride + sx)] as u32
                } else {
                    0xFF
                };
            }
        }

        let mut t = [0u32; 4];
        let av = transform_alpha_weights(
            a[0],
            a[1],
            a[2],
            a[3],
            (xs_ofs & 0xFF) as u32,
            (ys_ofs & 0xFF) as u32,
            &mut t,
        );
        abuf[xu] = av as u8;
        if av == 0 {
            wr_u16(cbuf, xu * 2, 0);
            continue;
        }

        let acc = ((e[0] * t[0] + e[1] * t[1] + e[2] * t[2] + e[3] * t[3]) >> 5) & 0x7E0F81F;
        wr_u16(cbuf, xu * 2, (acc | (acc >> 16)) as u16);
    }
}

/// `transform_rgb565a8()` (`SWAP == false`) and
/// `transform_rgb565a8_swapped()` (`SWAP == true`): transform one row of an
/// RGB565 / RGB565A8 image into an RGB565 row plus an A8 row.
fn transform_rgb565a8_impl<const SWAP: bool>(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    x_start: i32,
    x_end: i32,
    xs_clamp_ups: i32,
    cbuf: &mut [u8],
    abuf: &mut [u8],
    src_has_a8: bool,
    aa: bool,
) {
    /*Byte offset of the alpha map (`src + src_stride * src_h`); only used with `src_has_a8`*/
    let src_alpha = ix(src_stride * src_h);

    /*Must be signed type, because we would use negative array index calculated from stride*/
    let alpha_stride = src_stride / 2; /*alpha map stride is always half of RGB map stride*/

    /*The destination is indexed from zero while the source coordinates are calculated from the
     *absolute (image local) x coordinate to keep the partial rendering deterministic*/
    let w = x_end - x_start;

    let (mut fast_from, mut fast_to) =
        transform_safe_range(xs_ups, ys_ups, xs_step, ys_step, src_w, src_h, x_start, x_end, aa);
    fast_from -= x_start;
    fast_to -= x_start;

    rgb565a8_row_checked_impl::<SWAP>(
        src,
        src_w,
        src_h,
        src_stride,
        src_alpha,
        alpha_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        0,
        fast_from,
        x_start,
        xs_clamp_ups,
        cbuf,
        abuf,
        src_has_a8,
        aa,
    );

    if aa {
        rgb565a8_row_fast_impl::<SWAP>(
            src,
            src_stride,
            src_alpha,
            alpha_stride,
            xs_ups,
            ys_ups,
            xs_step,
            ys_step,
            fast_from,
            fast_to,
            x_start,
            cbuf,
            abuf,
            src_has_a8,
        );
    } else {
        for x in fast_from..fast_to {
            let x_abs = x + x_start;
            let xs_int = ups(xs_ups, xs_step, x_abs) >> 8;
            let ys_int = ups(ys_ups, ys_step, x_abs) >> 8;
            let xu = ix(x);
            wr_u16(
                cbuf,
                xu * 2,
                rgb565_px::<SWAP>(src, ix(ys_int * src_stride + xs_int * 2)),
            );
            abuf[xu] = if src_has_a8 {
                src[src_alpha + ix(ys_int * alpha_stride + xs_int)]
            } else {
                0xFF
            };
        }
    }

    rgb565a8_row_checked_impl::<SWAP>(
        src,
        src_w,
        src_h,
        src_stride,
        src_alpha,
        alpha_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        fast_to,
        w,
        x_start,
        xs_clamp_ups,
        cbuf,
        abuf,
        src_has_a8,
        aa,
    );
}

/// `transform_rgb565a8()`
#[inline]
fn transform_rgb565a8(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    x_start: i32,
    x_end: i32,
    xs_clamp_ups: i32,
    cbuf: &mut [u8],
    abuf: &mut [u8],
    src_has_a8: bool,
    aa: bool,
) {
    transform_rgb565a8_impl::<false>(
        src,
        src_w,
        src_h,
        src_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        x_start,
        x_end,
        xs_clamp_ups,
        cbuf,
        abuf,
        src_has_a8,
        aa,
    );
}

/// `transform_rgb565a8_swapped()`
#[cfg(feature = "cf-rgb565-swapped")]
#[inline]
fn transform_rgb565a8_swapped(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    x_start: i32,
    x_end: i32,
    xs_clamp_ups: i32,
    cbuf: &mut [u8],
    abuf: &mut [u8],
    src_has_a8: bool,
    aa: bool,
) {
    transform_rgb565a8_impl::<true>(
        src,
        src_w,
        src_h,
        src_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        x_start,
        x_end,
        xs_clamp_ups,
        cbuf,
        abuf,
        src_has_a8,
        aa,
    );
}

/// `transform_a8()`: transform one row of an A8 image.
#[cfg(feature = "cf-a8")]
fn transform_a8(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    x_start: i32,
    x_end: i32,
    xs_clamp_ups: i32,
    abuf: &mut [u8],
    aa: bool,
) {
    let xs_ups_start = xs_ups;
    let ys_ups_start = ys_ups;
    let w = x_end - x_start;

    for x in 0..w {
        let x_abs = x + x_start;
        let mut xu = ups(xs_ups_start, xs_step, x_abs);
        let yu = ups(ys_ups_start, ys_step, x_abs);
        if xu > xs_clamp_ups {
            xu = xs_clamp_ups;
        }

        let xi = xu >> 8;
        let yi = yu >> 8;
        let d = ix(x);
        if xi < 0 || xi >= src_w || yi < 0 || yi >= src_h {
            abuf[d] = 0x00;
            continue;
        }

        if !aa {
            abuf[d] = src[ix(yi * src_stride + xi)];
            continue;
        }

        let xo = xu.wrapping_sub(0x80);
        let yo = yu.wrapping_sub(0x80);
        let x0 = xo >> 8;
        let y0 = yo >> 8;
        let mut a = [0u32; 4];
        for (i, ai) in a.iter_mut().enumerate() {
            let sx = x0 + (i as i32 & 1);
            let sy = y0 + (i as i32 >> 1);
            if sx >= 0 && sx < src_w && sy >= 0 && sy < src_h {
                *ai = src[ix(sy * src_stride + sx)] as u32;
            }
        }

        /*An alpha only image carries no color, so nothing can bleed and plain bilinear
         *interpolation is already the correct filter*/
        let fx = (xo & 0xFF) as u32;
        let fy = (yo & 0xFF) as u32;
        let top = a[0] * (256 - fx) + a[1] * fx;
        let bot = a[2] * (256 - fx) + a[3] * fx;
        abuf[d] = ((top * (256 - fy) + bot * fy) >> 16) as u8;
    }
}

/// `al88_row_fast()`: the interior of an anti-aliased L8 / AL88 row where
/// every one of the four neighbors is inside the image.
#[cfg(any(feature = "cf-l8", feature = "cf-al88"))]
fn al88_row_fast(
    src: &[u8],
    src_stride: i32,
    xs_base: i32,
    ys_base: i32,
    xs_step: i32,
    ys_step: i32,
    x_from: i32,
    x_to: i32,
    x_offs: i32,
    cbuf: &mut [u8],
    abuf: &mut [u8],
    src_has_a8: bool,
) {
    let px_size: usize = if src_has_a8 { 2 } else { 1 };
    for x in x_from..x_to {
        let x_abs = x + x_offs;
        let xs_ups_ofs = ups(xs_base, xs_step, x_abs).wrapping_sub(0x80);
        let ys_ups_ofs = ups(ys_base, ys_step, x_abs).wrapping_sub(0x80);
        let xs_int = xs_ups_ofs >> 8;
        let ys_int = ys_ups_ofs >> 8;
        let fx = (xs_ups_ofs & 0xFF) as u32;
        let fy = (ys_ups_ofs & 0xFF) as u32;
        let xu = ix(x);

        let p = ix(ys_int * src_stride + xs_int * px_size as i32);
        let p2 = p + ix(src_stride);
        let l00 = src[p] as u32;
        let l01 = src[p + px_size] as u32;
        let l10 = src[p2] as u32;
        let l11 = src[p2 + px_size] as u32;
        let (a00, a01, a10, a11) = if src_has_a8 {
            (
                src[p + 1] as u32,
                src[p + px_size + 1] as u32,
                src[p2 + 1] as u32,
                src[p2 + px_size + 1] as u32,
            )
        } else {
            (0xFF, 0xFF, 0xFF, 0xFF)
        };

        if a00 == a01 && a00 == a10 && a00 == a11 {
            /*Uniform alpha: the weighting cancels, plain bilinear on the luminance is exact*/
            abuf[xu] = a00 as u8;
            if a00 == 0 {
                cbuf[xu] = 0;
                continue;
            }
            let top = l00 * (256 - fx) + l01 * fx;
            let bot = l10 * (256 - fx) + l11 * fx;
            cbuf[xu] = ((top * (256 - fy) + bot * fy) >> 16) as u8;
        } else {
            /*Mixed alpha: weight the luminance with alpha so a transparent neighbor cannot
             *bleed in, then normalize back*/
            let mut t = [0u32; 4];
            let av = transform_alpha_weights(a00, a01, a10, a11, fx, fy, &mut t);
            abuf[xu] = av as u8;
            if av == 0 {
                cbuf[xu] = 0;
                continue;
            }
            cbuf[xu] = ((l00 * t[0] + l01 * t[1] + l10 * t[2] + l11 * t[3]) >> 5) as u8;
        }
    }
}

/// `al88_row_checked()`: the edges of an L8 / AL88 row, where a neighbor
/// can fall outside the image (an outside tap counts as fully transparent).
#[cfg(any(feature = "cf-l8", feature = "cf-al88"))]
fn al88_row_checked(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_base: i32,
    ys_base: i32,
    xs_step: i32,
    ys_step: i32,
    x_from: i32,
    x_to: i32,
    x_offs: i32,
    xs_clamp_ups: i32,
    cbuf: &mut [u8],
    abuf: &mut [u8],
    src_has_a8: bool,
    aa: bool,
) {
    let px_size: i32 = if src_has_a8 { 2 } else { 1 };
    for x in x_from..x_to {
        let x_abs = x + x_offs;
        let mut xu = ups(xs_base, xs_step, x_abs);
        let yu = ups(ys_base, ys_step, x_abs);
        if xu > xs_clamp_ups {
            xu = xs_clamp_ups;
        }

        let xi = xu >> 8;
        let yi = yu >> 8;
        let d = ix(x);
        if xi < 0 || xi >= src_w || yi < 0 || yi >= src_h {
            cbuf[d] = 0x00;
            abuf[d] = 0x00;
            continue;
        }

        if !aa {
            let pn = ix(yi * src_stride + xi * px_size);
            cbuf[d] = src[pn];
            abuf[d] = if src_has_a8 { src[pn + 1] } else { 0xFF };
            continue;
        }

        let xo = xu.wrapping_sub(0x80);
        let yo = yu.wrapping_sub(0x80);
        let x0 = xo >> 8;
        let y0 = yo >> 8;
        let fx = (xo & 0xFF) as u32;
        let fy = (yo & 0xFF) as u32;

        let (mut l00, mut l01, mut l10, mut l11) = (0u32, 0u32, 0u32, 0u32);
        let (mut a00, mut a01, mut a10, mut a11) = (0u32, 0u32, 0u32, 0u32);
        let y0_in = y0 >= 0 && y0 < src_h;
        let y1_in = y0 + 1 >= 0 && y0 + 1 < src_h;
        let x0_in = x0 >= 0 && x0 < src_w;
        let x1_in = x0 + 1 >= 0 && x0 + 1 < src_w;
        if y0_in {
            let pr = y0 * src_stride;
            if x0_in {
                l00 = src[ix(pr + x0 * px_size)] as u32;
                a00 = if src_has_a8 {
                    src[ix(pr + x0 * px_size + 1)] as u32
                } else {
                    0xFF
                };
            }
            if x1_in {
                l01 = src[ix(pr + (x0 + 1) * px_size)] as u32;
                a01 = if src_has_a8 {
                    src[ix(pr + (x0 + 1) * px_size + 1)] as u32
                } else {
                    0xFF
                };
            }
        }
        if y1_in {
            let pr = (y0 + 1) * src_stride;
            if x0_in {
                l10 = src[ix(pr + x0 * px_size)] as u32;
                a10 = if src_has_a8 {
                    src[ix(pr + x0 * px_size + 1)] as u32
                } else {
                    0xFF
                };
            }
            if x1_in {
                l11 = src[ix(pr + (x0 + 1) * px_size)] as u32;
                a11 = if src_has_a8 {
                    src[ix(pr + (x0 + 1) * px_size + 1)] as u32
                } else {
                    0xFF
                };
            }
        }

        if a00 == a01 && a00 == a10 && a00 == a11 {
            abuf[d] = a00 as u8;
            if a00 == 0 {
                cbuf[d] = 0;
                continue;
            }
            let top = l00 * (256 - fx) + l01 * fx;
            let bot = l10 * (256 - fx) + l11 * fx;
            cbuf[d] = ((top * (256 - fy) + bot * fy) >> 16) as u8;
        } else {
            let mut t = [0u32; 4];
            let av = transform_alpha_weights(a00, a01, a10, a11, fx, fy, &mut t);
            abuf[d] = av as u8;
            if av == 0 {
                cbuf[d] = 0;
                continue;
            }
            cbuf[d] = ((l00 * t[0] + l01 * t[1] + l10 * t[2] + l11 * t[3]) >> 5) as u8;
        }
    }
}

/// `transform_al88()`: transform one row of an L8 (`src_has_a8 == false`)
/// or AL88 image into an L8 row plus an A8 row.
#[cfg(any(feature = "cf-l8", feature = "cf-al88"))]
fn transform_al88(
    src: &[u8],
    src_w: i32,
    src_h: i32,
    src_stride: i32,
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    x_start: i32,
    x_end: i32,
    xs_clamp_ups: i32,
    cbuf: &mut [u8],
    abuf: &mut [u8],
    src_has_a8: bool,
    aa: bool,
) {
    let w = x_end - x_start;
    let px_size: i32 = if src_has_a8 { 2 } else { 1 };

    let (mut fast_from, mut fast_to) =
        transform_safe_range(xs_ups, ys_ups, xs_step, ys_step, src_w, src_h, x_start, x_end, aa);
    fast_from -= x_start;
    fast_to -= x_start;

    al88_row_checked(
        src,
        src_w,
        src_h,
        src_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        0,
        fast_from,
        x_start,
        xs_clamp_ups,
        cbuf,
        abuf,
        src_has_a8,
        aa,
    );

    if aa {
        al88_row_fast(
            src, src_stride, xs_ups, ys_ups, xs_step, ys_step, fast_from, fast_to, x_start, cbuf, abuf,
            src_has_a8,
        );
    } else {
        for x in fast_from..fast_to {
            let x_abs = x + x_start;
            let xs_int = ups(xs_ups, xs_step, x_abs) >> 8;
            let ys_int = ups(ys_ups, ys_step, x_abs) >> 8;
            let pn = ix(ys_int * src_stride + xs_int * px_size);
            let d = ix(x);
            cbuf[d] = src[pn];
            abuf[d] = if src_has_a8 { src[pn + 1] } else { 0xFF };
        }
    }

    al88_row_checked(
        src,
        src_w,
        src_h,
        src_stride,
        xs_ups,
        ys_ups,
        xs_step,
        ys_step,
        fast_to,
        w,
        x_start,
        xs_clamp_ups,
        cbuf,
        abuf,
        src_has_a8,
        aa,
    );
}

/// `transform_safe_interval()`: find the sub-range of `[x_from, x_to)`
/// where `lo <= base + ((step * x) >> 8) <= hi`. Returns `(from, to)`.
fn transform_safe_interval(base: i32, step: i32, lo: i32, hi: i32, x_from: i32, x_to: i32) -> (i32, i32) {
    let mut xa = x_from;
    let mut xb = x_to;

    /* Solve lo <= base + ((step * x) >> 8) <= hi.
     * As `>> 8` is a floor operation:
     *    floor(t / 256) >= k  <=>  t >= 256 * k
     *    floor(t / 256) <= k  <=>  t <= 256 * k + 255  */
    let lo_n: i64 = (lo.wrapping_sub(base) as i64) * 256;
    let hi_n: i64 = (hi.wrapping_sub(base) as i64) * 256 + 255;

    if step == 0 {
        if lo_n > 0 || hi_n < 0 {
            xb = xa;
        }
    } else if step > 0 {
        let step = step as i64;
        /* ceil(lo_n / step) */
        let mut from = if lo_n > 0 {
            (lo_n + step - 1) / step
        } else {
            -((-lo_n) / step)
        };
        /* floor(hi_n / step) */
        let mut to = if hi_n >= 0 {
            hi_n / step
        } else {
            -((-hi_n + step - 1) / step)
        };
        if from > x_to as i64 {
            from = x_to as i64;
        }
        if to < (x_from - 1) as i64 {
            to = (x_from - 1) as i64;
        }
        if from as i32 > xa {
            xa = from as i32;
        }
        if ((to + 1) as i32) < xb {
            xb = (to + 1) as i32;
        }
    } else {
        let step_p: i64 = -(step as i64);
        /* floor(lo_n / step) */
        let mut to = if lo_n <= 0 {
            (-lo_n) / step_p
        } else {
            -((lo_n + step_p - 1) / step_p)
        };
        /* ceil(hi_n / step) */
        let mut from = if hi_n <= 0 {
            ((-hi_n) + step_p - 1) / step_p
        } else {
            -(hi_n / step_p)
        };
        if from > x_to as i64 {
            from = x_to as i64;
        }
        if to < (x_from - 1) as i64 {
            to = (x_from - 1) as i64;
        }
        if from as i32 > xa {
            xa = from as i32;
        }
        if ((to + 1) as i32) < xb {
            xb = (to + 1) as i32;
        }
    }

    if xb < xa {
        xb = xa;
    }
    (xa, xb)
}

/// `transform_safe_range()`: find the destination x range where the source
/// coordinates of the pixels (and with anti-aliasing their neighbors too)
/// are surely inside the source image. Returns `(from, to)`.
fn transform_safe_range(
    xs_ups: i32,
    ys_ups: i32,
    xs_step: i32,
    ys_step: i32,
    src_w: i32,
    src_h: i32,
    x_from: i32,
    x_to: i32,
    aa: bool,
) -> (i32, i32) {
    /*With anti-aliasing both direct neighbors are needed, without it only the pixel itself*/
    let lo = if aa { 256 } else { 0 };
    let hi_x = if aa {
        (src_w - 1) * 256 - 1
    } else {
        src_w * 256 - 1
    };
    let hi_y = if aa {
        (src_h - 1) * 256 - 1
    } else {
        src_h * 256 - 1
    };

    let (mut xa, mut xb) = transform_safe_interval(xs_ups, xs_step, lo, hi_x, x_from, x_to);
    let (ya, yb) = transform_safe_interval(ys_ups, ys_step, lo, hi_y, x_from, x_to);
    if ya > xa {
        xa = ya;
    }
    if yb < xb {
        xb = yb;
    }
    if xb < xa {
        xb = xa;
    }
    (xa, xb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// Run the transform over `area` with the given parameters.
    fn run(
        area: Area,
        src: &[u8],
        w: i32,
        h: i32,
        stride: i32,
        rotation: i32,
        scale: i32,
        pivot: Point,
        aa: bool,
        cf: ColorFormat,
        out_len: usize,
    ) -> Vec<u8> {
        let mut out = vec![0xA5u8; out_len];
        transform_core(
            &area, src, w, h, stride, rotation, scale, scale, pivot, aa, cf, &mut out,
        );
        out
    }

    fn pattern(n: usize, seed: u32) -> Vec<u8> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (s >> 16) as u8
            })
            .collect()
    }

    #[test]
    fn safe_interval_matches_brute_force() {
        let mut s: u32 = 7;
        let mut rnd = |m: i32| -> i32 {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12345);
            ((s >> 8) % (2 * m as u32 + 1)) as i32 - m
        };
        for _ in 0..3000 {
            let base = rnd(4000);
            let step = rnd(700);
            let lo = rnd(300);
            let hi = lo + rnd(3000).abs();
            let x_from = rnd(40);
            let x_to = x_from + rnd(60).abs();
            let (a, b) = transform_safe_interval(base, step, lo, hi, x_from, x_to);
            assert!(x_from <= a && a <= b && b <= x_to.max(x_from));
            for x in x_from..x_to {
                let v = base + ((step * x) >> 8);
                let inside = lo <= v && v <= hi;
                assert_eq!(
                    inside,
                    a <= x && x < b,
                    "base={base} step={step} lo={lo} hi={hi} x={x}"
                );
            }
        }
    }

    #[test]
    fn alpha_weights_sum_to_32() {
        for &(a00, a01, a10, a11) in &[
            (255, 255, 255, 255),
            (0, 255, 128, 3),
            (10, 0, 0, 0),
            (200, 100, 50, 25),
        ] {
            for fx in (0..256).step_by(17) {
                for fy in (0..256).step_by(13) {
                    let mut t = [0u32; 4];
                    let a = transform_alpha_weights(a00, a01, a10, a11, fx, fy, &mut t);
                    assert!(a <= 255);
                    if a == 0 {
                        assert_eq!(t, [0; 4]);
                    } else {
                        assert_eq!(t.iter().sum::<u32>(), 32);
                    }
                    /*A transparent neighbor never gets a color weight*/
                    for (i, &ai) in [a00, a01, a10, a11].iter().enumerate() {
                        if ai == 0 && a != 0 {
                            assert_eq!(t[i], 0);
                        }
                    }
                }
            }
        }
        let mut t = [0u32; 4];
        assert_eq!(transform_alpha_weights(0, 0, 0, 0, 10, 20, &mut t), 0);
    }

    #[test]
    fn norm_inv_table() {
        for (s, &v) in TRANSFORM_NORM_INV.iter().enumerate().skip(1) {
            assert_eq!(v as usize, (32 << 8) / s);
        }
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn argb8888_identity_is_exact() {
        let (w, h) = (7, 5);
        let mut src = pattern((w * h * 4) as usize, 3);
        for i in 0..(w * h) as usize {
            src[i * 4 + 3] = 0xFF;
        }
        let area = Area::new(0, 0, w - 1, h - 1);
        for aa in [false, true] {
            let out = run(
                area,
                &src,
                w,
                h,
                w * 4,
                0,
                256,
                Point::new(3, 2),
                aa,
                ColorFormat::Argb8888,
                src.len(),
            );
            assert_eq!(out, src, "aa={aa}");
        }
        /*Non-opaque source without anti-aliasing is copied as is*/
        let src2 = pattern((w * h * 4) as usize, 9);
        let out = run(
            area,
            &src2,
            w,
            h,
            w * 4,
            0,
            256,
            Point::new(0, 0),
            false,
            ColorFormat::Argb8888,
            src2.len(),
        );
        assert_eq!(out, src2);
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn argb8888_out_of_image_is_transparent() {
        let (w, h) = (4, 4);
        let src = vec![0xFFu8; (w * h * 4) as usize];
        /*The destination area is larger than the image*/
        let area = Area::new(-2, -2, 5, 5);
        let out = run(
            area,
            &src,
            w,
            h,
            w * 4,
            0,
            256,
            Point::new(0, 0),
            false,
            ColorFormat::Argb8888,
            8 * 8 * 4,
        );
        /*Left / top of the image: fully out. Right / bottom: scale-only transforms clamp
         *the source coordinate to the last column / row (upstream `xs_clamp_ups`).*/
        for y in 0..8 {
            for x in 0..8 {
                let inside = x >= 2 && y >= 2;
                let px = rd_u32(&out, (y * 8 + x) * 4);
                assert_eq!(px, if inside { 0xFFFF_FFFF } else { 0 }, "x={x} y={y}");
            }
        }
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn rotate_90_nearest() {
        /*3x3 image, pivot at the center: dest(x, y) = src(y, 2 - x)*/
        let (w, h) = (3, 3);
        let src = pattern((w * h * 4) as usize, 11);
        let area = Area::new(0, 0, 2, 2);
        let out = run(
            area,
            &src,
            w,
            h,
            w * 4,
            900,
            256,
            Point::new(1, 1),
            false,
            ColorFormat::Argb8888,
            36,
        );
        for y in 0..3usize {
            for x in 0..3usize {
                assert_eq!(rd_u32(&out, (y * 3 + x) * 4), rd_u32(&src, ((2 - x) * 3 + y) * 4));
            }
        }
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn rotate_180_nearest() {
        /*Pivot (2, 2) on a 4x4 image: dest x maps to src 4 - x*/
        let (w, h) = (4, 4);
        let src = pattern((w * h * 4) as usize, 5);
        let area = Area::new(1, 1, 4, 4);
        let out = run(
            area,
            &src,
            w,
            h,
            w * 4,
            1800,
            256,
            Point::new(2, 2),
            false,
            ColorFormat::Argb8888,
            64,
        );
        for y in 0..4usize {
            for x in 0..4usize {
                let (dx, dy) = (x + 1, y + 1);
                assert_eq!(
                    rd_u32(&out, (y * 4 + x) * 4),
                    rd_u32(&src, ((4 - dy) * 4 + (4 - dx)) * 4)
                );
            }
        }
    }

    /// Rendering a part of the image gives the same pixels as rendering the
    /// whole one (deterministic partial rendering, upstream's design goal).
    fn partial_equals_full(cf: ColorFormat, bpp_c: usize, has_a: bool, src_px: i32) {
        let (w, h) = (13, 11);
        let stride = w * src_px;
        let mut src_len = (stride * h) as usize;
        if cf == ColorFormat::Rgb565A8 {
            src_len += (w * h) as usize;
        }
        let src = pattern(src_len, 42);
        for &(rot, scale) in &[
            (0, 256),
            (300, 256),
            (1234, 300),
            (0, 700),
            (2700, 128),
            (-450, 512),
        ] {
            for aa in [false, true] {
                let full = Area::new(-6, -6, 25, 25);
                let (fw, fh) = (full.width() as usize, full.height() as usize);
                let len = |a: &Area| (a.width() * a.height()) as usize * (bpp_c + has_a as usize);
                let out_full = run(
                    full,
                    &src,
                    w,
                    h,
                    stride,
                    rot,
                    scale,
                    Point::new(6, 5),
                    aa,
                    cf,
                    len(&full),
                );
                let part = Area::new(3, 2, 17, 19);
                let out_part = run(
                    part,
                    &src,
                    w,
                    h,
                    stride,
                    rot,
                    scale,
                    Point::new(6, 5),
                    aa,
                    cf,
                    len(&part),
                );
                let (pw, ph) = (part.width() as usize, part.height() as usize);
                for y in 0..ph {
                    for x in 0..pw {
                        let fx = x + (part.x1 - full.x1) as usize;
                        let fy = y + (part.y1 - full.y1) as usize;
                        let pc = &out_part[(y * pw + x) * bpp_c..][..bpp_c];
                        let fc = &out_full[(fy * fw + fx) * bpp_c..][..bpp_c];
                        if has_a {
                            let pa = out_part[pw * ph * bpp_c + y * pw + x];
                            let fa = out_full[fw * fh * bpp_c + fy * fw + fx];
                            assert_eq!(pa, fa, "{cf:?} rot={rot} scale={scale} aa={aa}");
                            if pa == 0 {
                                continue;
                            }
                        } else if cf == ColorFormat::Rgb888 || cf == ColorFormat::Xrgb8888 {
                            assert_eq!(pc[3], fc[3]);
                            if pc[3] == 0 {
                                /*Upstream only clears the alpha of fully out pixels*/
                                continue;
                            }
                        }
                        assert_eq!(pc, fc, "{cf:?} rot={rot} scale={scale} aa={aa} x={x} y={y}");
                    }
                }
            }
        }
    }

    #[cfg(feature = "cf-argb8888")]
    #[test]
    fn partial_argb8888() {
        partial_equals_full(ColorFormat::Argb8888, 4, false, 4);
    }

    #[cfg(feature = "cf-argb8888-premultiplied")]
    #[test]
    fn partial_argb8888_premultiplied() {
        partial_equals_full(ColorFormat::Argb8888Premultiplied, 4, false, 4);
    }

    #[cfg(feature = "cf-rgb888")]
    #[test]
    fn partial_rgb888() {
        partial_equals_full(ColorFormat::Rgb888, 4, false, 3);
    }

    #[cfg(feature = "cf-xrgb8888")]
    #[test]
    fn partial_xrgb8888() {
        partial_equals_full(ColorFormat::Xrgb8888, 4, false, 4);
    }

    #[cfg(feature = "cf-rgb565")]
    #[test]
    fn partial_rgb565() {
        partial_equals_full(ColorFormat::Rgb565, 2, true, 2);
    }

    #[cfg(feature = "cf-rgb565-swapped")]
    #[test]
    fn partial_rgb565_swapped() {
        partial_equals_full(ColorFormat::Rgb565Swapped, 2, true, 2);
    }

    #[test]
    fn partial_rgb565a8() {
        partial_equals_full(ColorFormat::Rgb565A8, 2, true, 2);
    }

    #[cfg(feature = "cf-a8")]
    #[test]
    fn partial_a8() {
        partial_equals_full(ColorFormat::A8, 1, false, 1);
    }

    #[cfg(feature = "cf-l8")]
    #[test]
    fn partial_l8() {
        partial_equals_full(ColorFormat::L8, 1, true, 1);
    }

    #[cfg(feature = "cf-al88")]
    #[test]
    fn partial_al88() {
        partial_equals_full(ColorFormat::Al88, 1, true, 2);
    }

    #[cfg(feature = "cf-rgb888")]
    #[test]
    fn rgb888_identity() {
        let (w, h) = (5, 4);
        let src = pattern((w * h * 3) as usize, 1);
        let area = Area::new(0, 0, w - 1, h - 1);
        for aa in [false, true] {
            let out = run(
                area,
                &src,
                w,
                h,
                w * 3,
                0,
                256,
                Point::new(2, 2),
                aa,
                ColorFormat::Rgb888,
                (w * h * 4) as usize,
            );
            for i in 0..(w * h) as usize {
                assert_eq!(&out[i * 4..i * 4 + 3], &src[i * 3..i * 3 + 3]);
                /*Upstream quirk: the "partially out" branch fades the last column / row
                 *to `(0xFF * 0xFF) >> 8` even without rotation or scaling. Without
                 *anti-aliasing every pixel is in the unchecked fast range.*/
                let (x, y) = (i as i32 % w, i as i32 / w);
                let edge = aa && (x == w - 1 || y == h - 1);
                assert_eq!(out[i * 4 + 3], if edge { 0xFE } else { 0xFF }, "aa={aa} i={i}");
            }
        }
    }

    #[test]
    fn rgb565a8_identity() {
        let (w, h) = (6, 3);
        let src = pattern((w * h * 3) as usize, 77);
        let area = Area::new(0, 0, w - 1, h - 1);
        let n = (w * h) as usize;
        let out = run(
            area,
            &src,
            w,
            h,
            w * 2,
            0,
            256,
            Point::new(0, 0),
            false,
            ColorFormat::Rgb565A8,
            n * 3,
        );
        assert_eq!(out, src);
    }

    #[cfg(feature = "cf-al88")]
    #[test]
    fn al88_identity_and_planes() {
        let (w, h) = (4, 3);
        let src = pattern((w * h * 2) as usize, 21);
        let area = Area::new(0, 0, w - 1, h - 1);
        let n = (w * h) as usize;
        let out = run(
            area,
            &src,
            w,
            h,
            w * 2,
            0,
            256,
            Point::new(0, 0),
            false,
            ColorFormat::Al88,
            n * 2,
        );
        for i in 0..n {
            assert_eq!(out[i], src[i * 2]);
            assert_eq!(out[n + i], src[i * 2 + 1]);
        }
    }

    #[cfg(feature = "cf-a8")]
    #[test]
    fn a8_upscale_interpolates() {
        /*2x upscale of a 2x1 A8 image [0, 255]: monotonic ramp inside*/
        let src = [0u8, 255];
        let area = Area::new(0, 0, 3, 0);
        let out = run(
            area,
            &src,
            2,
            1,
            2,
            0,
            512,
            Point::new(0, 0),
            true,
            ColorFormat::A8,
            4,
        );
        for i in 1..4 {
            assert!(out[i] >= out[i - 1], "{out:?}");
        }
    }

    #[test]
    fn unsupported_format_writes_nothing() {
        let src = [0u8; 16];
        let out = run(
            Area::new(0, 0, 1, 1),
            &src,
            2,
            2,
            2,
            0,
            256,
            Point::new(0, 0),
            false,
            ColorFormat::I4,
            16,
        );
        assert!(out.iter().all(|&b| b == 0xA5));
    }
}
