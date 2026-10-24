//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend.c`,
//! `src/draw/sw/blend/lv_draw_sw_blend.h`, `src/draw/sw/blend/lv_draw_sw_blend_private.h`.
//!
//! Blend a colour or an image (with optional mask) into a layer buffer.
//! Per-destination-format kernels live in the `to_*` modules, gated like
//! the `LV_DRAW_SW_SUPPORT_*` options. Note the upstream routing:
//! `XRGB8888` is rendered by the **RGB888** kernel with `dest_px_size = 4`
//! (`lv_draw_sw_blend_color_to_rgb888(fill_dsc, 4)`), *not* by the
//! alpha-aware ARGB8888 kernel, which is guarded by `ARGB8888` only. Only
//! the rgb888 kernel takes a `dest_px_size`.

pub mod words;

#[cfg(feature = "cf-a8")]
pub mod to_a8;
#[cfg(feature = "cf-al88")]
pub mod to_al88;
#[cfg(feature = "cf-argb8888")]
pub mod to_argb8888;
#[cfg(feature = "cf-argb8888-premultiplied")]
pub mod to_argb8888_premultiplied;
#[cfg(feature = "cf-i1")]
pub mod to_i1;
#[cfg(feature = "cf-l8")]
pub mod to_l8;
#[cfg(feature = "cf-rgb565")]
pub mod to_rgb565;
#[cfg(feature = "cf-rgb565-swapped")]
pub mod to_rgb565_swapped;
#[cfg(any(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
pub mod to_rgb888;

use ferrovgl_core::draw::{DrawTask, Layer};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::{Color, Color32, ColorFormat, OPA_COVER, Opa};
use ferrovgl_core::misc::style::BlendMode;

pub use crate::mask::MaskRes;

/// `LV_DRAW_SW_I1_LUM_THRESHOLD` (lv_conf default): luminance above which an
/// I1 destination pixel is set.
pub const I1_LUM_THRESHOLD: u8 = 127;

/// `lv_draw_sw_blend_handler_t`: a custom blend function for one destination
/// colour format.
pub type BlendHandler = fn(t: &DrawTask, layer: &mut Layer, dsc: &BlendDsc<'_>);

/// `lv_draw_sw_custom_blend_handler_t`
///
/// No `PartialEq`: comparing `fn` pointers is not meaningful; handlers are
/// identified by `dest_cf`.
#[derive(Copy, Clone, Debug)]
pub struct CustomBlendHandler {
    /// `dest_cf`: the layer colour format this handler renders to.
    pub dest_cf: ColorFormat,
    /// `handler`
    pub handler: BlendHandler,
}

/// `lv_draw_sw_blend_dsc_t`
pub struct BlendDsc<'a> {
    /// `blend_area`: the area to blend, in layer coordinates.
    pub blend_area: Area,
    /// `src_buf`: `None` to fill with `color`.
    pub src_buf: Option<&'a [u8]>,
    /// `src_stride`
    pub src_stride: u32,
    /// `src_color_format`
    pub src_color_format: ColorFormat,
    /// `src_area`
    pub src_area: Option<Area>,
    /// `opa`
    pub opa: Opa,
    /// `color`
    pub color: Color,
    /// `mask_buf`
    pub mask_buf: Option<&'a [u8]>,
    /// `mask_res`
    pub mask_res: MaskRes,
    /// `mask_area`
    pub mask_area: Option<Area>,
    /// `mask_stride`
    pub mask_stride: i32,
    /// `blend_mode`
    pub blend_mode: BlendMode,
}

/// `lv_draw_sw_blend_fill_dsc_t`: a colour fill into a destination buffer,
/// with coordinates relative to the destination.
pub struct BlendFillDsc<'a> {
    /// `dest_buf`
    pub dest_buf: &'a mut [u8],
    /// `dest_w`
    pub dest_w: i32,
    /// `dest_h`
    pub dest_h: i32,
    /// `dest_stride` in bytes.
    pub dest_stride: i32,
    /// `mask_buf`
    pub mask_buf: Option<&'a [u8]>,
    /// `mask_stride`
    pub mask_stride: i32,
    /// `color`
    pub color: Color,
    /// `opa`
    pub opa: Opa,
    /// `relative_area`
    pub relative_area: Area,
}

/// `lv_draw_sw_blend_image_dsc_t`
pub struct BlendImageDsc<'a> {
    /// `dest_buf`
    pub dest_buf: &'a mut [u8],
    /// `dest_w`
    pub dest_w: i32,
    /// `dest_h`
    pub dest_h: i32,
    /// `dest_stride` in bytes.
    pub dest_stride: i32,
    /// `mask_buf`
    pub mask_buf: Option<&'a [u8]>,
    /// `mask_stride`
    pub mask_stride: i32,
    /// `src_buf`
    pub src_buf: &'a [u8],
    /// `src_stride` in bytes.
    pub src_stride: i32,
    /// `src_color_format`
    pub src_color_format: ColorFormat,
    /// `opa`
    pub opa: Opa,
    /// `blend_mode`
    pub blend_mode: BlendMode,
    /// `relative_area`
    pub relative_area: Area,
    /// `src_area`
    pub src_area: Area,
}

/// `lv_draw_sw_blend()`: blend `dsc` into the buffer of `layer`
/// (`t.target_layer`). `handlers` are the registered custom blend handlers
/// (`lv_draw_sw_get_blend_handler()` is consulted first for
/// `layer.color_format`); pass `&unit.blend_handlers`.
///
/// Dispatch by `layer.color_format`: `RGB888 -> to_rgb888::*(dsc, 3)`,
/// `XRGB8888 -> to_rgb888::*(dsc, 4)`, `ARGB8888 -> to_argb8888`,
/// `ARGB8888_PREMULTIPLIED -> to_argb8888_premultiplied`, the rest 1:1.
pub fn blend(t: &DrawTask, layer: &mut Layer, dsc: &BlendDsc<'_>, handlers: &[CustomBlendHandler]) {
    todo!()
}

/// `lv_draw_sw_get_blend_handler()`: the custom handler registered for
/// `dest_cf`, if any.
pub fn get_blend_handler(handlers: &[CustomBlendHandler], dest_cf: ColorFormat) -> Option<BlendHandler> {
    handlers.iter().find(|h| h.dest_cf == dest_cf).map(|h| h.handler)
}

/// `LV_COLOR_MIX_16_PREPARE(c)`
#[inline]
pub const fn color_mix_16_prepare(c: u16) -> u32 {
    ((c as u32) | ((c as u32) << 16)) & 0x07E0_F81F
}

/// `lv_color_16_16_mix_inlined()`
#[inline]
pub const fn color_16_16_mix(c1: u16, c2: u16, mix: u8) -> u16 {
    if mix == 255 {
        return c1;
    }
    if mix == 0 {
        return c2;
    }
    if c1 == c2 {
        return c1;
    }
    let mix5 = ((mix as u32) + 4) >> 3;
    let bg = ((c2 as u32) | ((c2 as u32) << 16)) & 0x7E0_F81F;
    let fg = ((c1 as u32) | ((c1 as u32) << 16)) & 0x7E0_F81F;
    let result = ((fg.wrapping_sub(bg).wrapping_mul(mix5) >> 5).wrapping_add(bg)) & 0x7E0_F81F;
    ((result >> 16) | result) as u16
}

/// `LV_COLOR_MIX_16_TO_16_PREPARED(res, fg_prep, bg, mix)`: mix a prepared
/// foreground ([`color_mix_16_prepare`]) into an RGB565 background.
/// `mix` must be 1..254, the callers handle 0 and 255 themselves.
#[inline]
pub const fn color_mix_16_to_16_prepared(fg_prep: u32, bg: u16, mix: u8) -> u16 {
    let bg_prep = color_mix_16_prepare(bg);
    let mix5 = ((mix as u32) + 4) >> 3;
    let res = (((fg_prep.wrapping_sub(bg_prep).wrapping_mul(mix5)) >> 5).wrapping_add(bg_prep)) & 0x07E0_F81F;
    ((res >> 16) | res) as u16
}

/// `lv_color32_unpremultiply()`: undo the alpha scaling of a premultiplied
/// pixel, giving back straight channels.
#[inline]
pub const fn color32_unpremultiply(mut c: Color32) -> Color32 {
    if c.alpha == 0 {
        c.red = 0;
        c.green = 0;
        c.blue = 0;
    } else if c.alpha != OPA_COVER {
        let reciprocal = (255u32 * 256u32) / (c.alpha as u32);
        let r = ((c.red as u32) * reciprocal) >> 8;
        let g = ((c.green as u32) * reciprocal) >> 8;
        let b = ((c.blue as u32) * reciprocal) >> 8;
        c.red = if r > 255 { 255 } else { r as u8 };
        c.green = if g > 255 { 255 } else { g as u8 };
        c.blue = if b > 255 { 255 } else { b as u8 };
    }
    c
}

/// `lv_color32_lumi_of()`: luminance of an image pixel for the grayscale and
/// indexed outputs; a premultiplied source is unscaled first.
#[inline]
pub const fn color32_lumi_of(c: Color32, premultiplied: bool) -> u8 {
    if !premultiplied || c.alpha == 0 || c.alpha == OPA_COVER {
        return c.luminance();
    }
    let lumi = ((c.luminance() as u32) * ((255u32 * 256u32) / (c.alpha as u32))) >> 8;
    if lumi > 255 { 255 } else { lumi as u8 }
}

// `lv_color_mix32_inlined()` and `lv_color_mix32_premultiplied_inlined()` are
// identical to the public functions: use `Color32::mix32()` and
// `Color32::mix32_premultiplied()` from ferrovgl-core.

/// `LV_COLOR_24_TO_16(c)`: `c` is `[b, g, r]`.
#[inline]
pub const fn color_24_to_16(c: [u8; 3]) -> u16 {
    (((c[2] & 0xF8) as u16) << 8) + (((c[1] & 0xFC) as u16) << 3) + (((c[0] & 0xF8) as u16) >> 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_mix_endpoints() {
        assert_eq!(color_16_16_mix(0x1234, 0xABCD, 255), 0x1234);
        assert_eq!(color_16_16_mix(0x1234, 0xABCD, 0), 0xABCD);
        assert_eq!(color_16_16_mix(0x1234, 0x1234, 77), 0x1234);
        assert_eq!(color_24_to_16([0xFF, 0xFF, 0xFF]), 0xFFFF);
        assert_eq!(color_24_to_16([0, 0, 0xF8]), 0xF800);
        assert_eq!(color_mix_16_prepare(0xFFFF), 0x07E0_F81F);
    }

    #[test]
    fn prepared_mix_matches_inlined_mix() {
        let (c1, c2) = (0x1234u16, 0xABCDu16);
        let fg_prep = color_mix_16_prepare(c1);
        for mix in 1..=254u8 {
            assert_eq!(
                color_mix_16_to_16_prepared(fg_prep, c2, mix),
                color_16_16_mix(c1, c2, mix)
            );
        }
    }

    #[test]
    fn unpremultiply_and_lumi() {
        let c = Color32 {
            blue: 10,
            green: 20,
            red: 30,
            alpha: 0,
        };
        assert_eq!(
            color32_unpremultiply(c),
            Color32 {
                alpha: 0,
                ..Color32::default()
            }
        );
        let half = Color32 {
            blue: 50,
            green: 100,
            red: 120,
            alpha: 128,
        };
        let u = color32_unpremultiply(half);
        // reciprocal = 65280 / 128 = 510; (120 * 510) >> 8 = 239
        assert_eq!((u.red, u.green, u.blue, u.alpha), (239, 199, 99, 128));
        let sat = Color32 {
            blue: 200,
            green: 200,
            red: 200,
            alpha: 100,
        };
        assert_eq!(color32_unpremultiply(sat).red, 255);
        let opaque = Color32 {
            blue: 1,
            green: 2,
            red: 3,
            alpha: 255,
        };
        assert_eq!(color32_unpremultiply(opaque), opaque);
        assert_eq!(color32_lumi_of(half, false), half.luminance());
        assert_eq!(color32_lumi_of(opaque, true), opaque.luminance());
        let expect = ((half.luminance() as u32 * (65280 / 128)) >> 8).min(255) as u8;
        assert_eq!(color32_lumi_of(half, true), expect);
    }

    #[test]
    fn blend_handler_lookup() {
        fn h(_: &DrawTask, _: &mut Layer, _: &BlendDsc<'_>) {}
        let hs = [CustomBlendHandler {
            dest_cf: ColorFormat::Rgb565,
            handler: h,
        }];
        assert!(get_blend_handler(&hs, ColorFormat::Rgb565).is_some());
        assert!(get_blend_handler(&hs, ColorFormat::Argb8888).is_none());
        assert_eq!(I1_LUM_THRESHOLD, 127);
    }
}
