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

use ferrovgl_core::draw::{DrawTask, ImageHeader, Layer};
use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::color::{Color, Color32, ColorFormat, OPA_COVER, OPA_MIN, Opa};
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
    // Do not draw transparent things
    if dsc.opa <= OPA_MIN {
        return;
    }
    if dsc.mask_buf.is_some() && dsc.mask_res == MaskRes::Transp {
        return;
    }

    let Some(blend_area) = Area::intersect(&dsc.blend_area, &t.clip_area) else {
        return;
    };

    if let Some(handler) = get_blend_handler(handlers, layer.color_format) {
        handler(t, layer, dsc);
        return;
    }

    let layer_cf = layer.color_format;
    let buf_area = layer.buf_area;
    let Some(draw_buf) = layer.draw_buf.as_mut() else {
        return;
    };
    let header = draw_buf.header;
    let Some(data) = draw_buf.data_mut() else {
        return;
    };
    blend_to_buf(layer_cf, &buf_area, &header, data, blend_area, dsc);
}

/// `lv_draw_buf_goto_xy()` on a copy of the layer's header, so that the
/// offset can be computed while the pixel data is borrowed mutably.
fn buf_goto_xy(header: &ImageHeader, x: u32, y: u32) -> usize {
    // Skip palette
    let mut ofs = header.cf.indexed_palette_size() as usize * size_of::<Color32>();
    ofs += header.stride as usize * y as usize;
    if x == 0 {
        return ofs;
    }
    ofs + (x.wrapping_mul(header.cf.bpp() as u32) / 8) as usize
}

/// `ptr + ofs` on a slice: the tail starting at `ofs`, `None` when the
/// offset is negative or past the end (undefined behaviour upstream).
fn slice_from(buf: &[u8], ofs: i64) -> Option<&[u8]> {
    usize::try_from(ofs).ok().and_then(|o| buf.get(o..))
}

/// `dsc->mask_buf`, unless there is none or it is `FULL_COVER`.
fn effective_mask<'a>(dsc: &BlendDsc<'a>) -> Option<&'a [u8]> {
    match dsc.mask_buf {
        None => None,
        Some(_) if dsc.mask_res == MaskRes::FullCover => None,
        Some(m) => Some(m),
    }
}

/// The body of `lv_draw_sw_blend()` after the clip test and the custom
/// handler lookup: `blend_area` is already clipped to `t->clip_area`, and
/// `data` is the layer's pixel data described by `header` (the
/// `lv_draw_buf_t` of the layer), whose top-left pixel is at
/// `buf_area.x1/y1`.
pub(crate) fn blend_to_buf(
    layer_cf: ColorFormat,
    buf_area: &Area,
    header: &ImageHeader,
    data: &mut [u8],
    mut blend_area: Area,
    dsc: &BlendDsc<'_>,
) {
    let layer_stride_byte = header.stride as u32;

    match dsc.src_buf {
        None => {
            let mut mask_buf = effective_mask(dsc);
            let mut mask_stride = 0;

            let mut relative_area = blend_area;
            relative_area.move_by(-buf_area.x1, -buf_area.y1);
            let dest_ofs = buf_goto_xy(
                header,
                (blend_area.x1 - buf_area.x1) as u32,
                (blend_area.y1 - buf_area.y1) as u32,
            );
            if let Some(m) = mask_buf {
                // `dsc->mask_area` is dereferenced unconditionally upstream.
                let Some(mask_area) = dsc.mask_area else {
                    return;
                };
                mask_stride = if dsc.mask_stride == 0 {
                    mask_area.width()
                } else {
                    dsc.mask_stride
                };
                let ofs = mask_stride as i64 * (blend_area.y1 - mask_area.y1) as i64
                    + (blend_area.x1 - mask_area.x1) as i64;
                let Some(m) = slice_from(m, ofs) else {
                    return;
                };
                mask_buf = Some(m);
            }
            let Some(dest_buf) = data.get_mut(dest_ofs..) else {
                return;
            };

            let mut fill_dsc = BlendFillDsc {
                dest_buf,
                dest_w: blend_area.width(),
                dest_h: blend_area.height(),
                dest_stride: layer_stride_byte as i32,
                mask_buf,
                mask_stride,
                color: dsc.color,
                opa: dsc.opa,
                relative_area,
            };
            blend_color(layer_cf, &mut fill_dsc);
        }
        Some(src) => {
            // `dsc->src_area` is dereferenced unconditionally upstream.
            let Some(src_area) = dsc.src_area else {
                return;
            };
            let Some(a) = Area::intersect(&blend_area, &src_area) else {
                return;
            };
            blend_area = a;

            if let Some(mask_area) = dsc.mask_area {
                let Some(a) = Area::intersect(&blend_area, &mask_area) else {
                    return;
                };
                blend_area = a;
            }

            let src_stride = dsc.src_stride as i32;
            let src_px_size = dsc.src_color_format.bpp() as u32;
            let src_ofs = src_stride as i64 * (blend_area.y1 - src_area.y1) as i64
                + ((((blend_area.x1 - src_area.x1) as u32).wrapping_mul(src_px_size)) >> 3) as i64;
            let Some(src_buf) = slice_from(src, src_ofs) else {
                return;
            };

            let mut mask_stride = 0;
            let mut mask_buf = effective_mask(dsc);
            if let Some(m) = mask_buf {
                let Some(mask_area) = dsc.mask_area else {
                    return;
                };
                mask_stride = if dsc.mask_stride != 0 {
                    dsc.mask_stride
                } else {
                    mask_area.width()
                };
                let ofs = mask_stride as i64 * (blend_area.y1 - mask_area.y1) as i64
                    + (blend_area.x1 - mask_area.x1) as i64;
                let Some(m) = slice_from(m, ofs) else {
                    return;
                };
                mask_buf = Some(m);
            }

            let mut relative_area = blend_area;
            relative_area.move_by(-buf_area.x1, -buf_area.y1);

            let mut rel_src_area = src_area;
            rel_src_area.move_by(-buf_area.x1, -buf_area.y1);

            let dest_ofs = buf_goto_xy(
                header,
                (blend_area.x1 - buf_area.x1) as u32,
                (blend_area.y1 - buf_area.y1) as u32,
            );
            let Some(dest_buf) = data.get_mut(dest_ofs..) else {
                return;
            };

            let mut image_dsc = BlendImageDsc {
                dest_buf,
                dest_w: blend_area.width(),
                dest_h: blend_area.height(),
                dest_stride: layer_stride_byte as i32,
                mask_buf,
                mask_stride,
                src_buf,
                src_stride,
                src_color_format: dsc.src_color_format,
                opa: dsc.opa,
                blend_mode: dsc.blend_mode,
                relative_area,
                src_area: rel_src_area,
            };
            blend_image(layer_cf, &mut image_dsc);
        }
    }
}

/// `lv_draw_sw_blend_color()`: route a fill to the kernel of `layer_cf`.
#[inline]
fn blend_color(layer_cf: ColorFormat, fill_dsc: &mut BlendFillDsc<'_>) {
    match layer_cf {
        #[cfg(feature = "cf-rgb565")]
        ColorFormat::Rgb565 => to_rgb565::fill(fill_dsc),
        #[cfg(feature = "cf-rgb565-swapped")]
        ColorFormat::Rgb565Swapped => to_rgb565_swapped::fill(fill_dsc),
        #[cfg(feature = "cf-argb8888")]
        ColorFormat::Argb8888 => to_argb8888::fill(fill_dsc),
        #[cfg(feature = "cf-rgb888")]
        ColorFormat::Rgb888 => to_rgb888::fill(fill_dsc, 3),
        #[cfg(feature = "cf-xrgb8888")]
        ColorFormat::Xrgb8888 => to_rgb888::fill(fill_dsc, 4),
        #[cfg(feature = "cf-argb8888-premultiplied")]
        ColorFormat::Argb8888Premultiplied => to_argb8888_premultiplied::fill(fill_dsc),
        #[cfg(feature = "cf-a8")]
        ColorFormat::A8 => to_a8::fill(fill_dsc),
        #[cfg(feature = "cf-l8")]
        ColorFormat::L8 => to_l8::fill(fill_dsc),
        #[cfg(feature = "cf-al88")]
        ColorFormat::Al88 => to_al88::fill(fill_dsc),
        #[cfg(feature = "cf-i1")]
        ColorFormat::I1 => to_i1::fill(fill_dsc),
        _ => {}
    }
}

/// `lv_draw_sw_blend_image()`: route an image blend to the kernel of
/// `layer_cf`.
#[inline]
fn blend_image(layer_cf: ColorFormat, image_dsc: &mut BlendImageDsc<'_>) {
    match layer_cf {
        #[cfg(feature = "cf-rgb565")]
        ColorFormat::Rgb565 | ColorFormat::Rgb565A8 => to_rgb565::image(image_dsc),
        #[cfg(feature = "cf-rgb565-swapped")]
        ColorFormat::Rgb565Swapped => to_rgb565_swapped::image(image_dsc),
        #[cfg(feature = "cf-argb8888")]
        ColorFormat::Argb8888 => to_argb8888::image(image_dsc),
        #[cfg(feature = "cf-rgb888")]
        ColorFormat::Rgb888 => to_rgb888::image(image_dsc, 3),
        #[cfg(feature = "cf-xrgb8888")]
        ColorFormat::Xrgb8888 => to_rgb888::image(image_dsc, 4),
        #[cfg(feature = "cf-argb8888-premultiplied")]
        ColorFormat::Argb8888Premultiplied => to_argb8888_premultiplied::image(image_dsc),
        #[cfg(feature = "cf-a8")]
        ColorFormat::A8 => to_a8::image(image_dsc),
        #[cfg(feature = "cf-l8")]
        ColorFormat::L8 => to_l8::image(image_dsc),
        #[cfg(feature = "cf-al88")]
        ColorFormat::Al88 => to_al88::image(image_dsc),
        #[cfg(feature = "cf-i1")]
        ColorFormat::I1 => to_i1::image(image_dsc),
        _ => {}
    }
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

    fn task(clip_area: Area) -> DrawTask {
        let mut keys = ferrovgl_core::misc::arena::Arena::<()>::new();
        DrawTask {
            ty: Default::default(),
            area: clip_area,
            real_area: clip_area,
            clip_area,
            target_layer: ferrovgl_core::draw::LayerId(keys.insert(())),
            draw_unit: None,
            state: Default::default(),
            dsc: ferrovgl_core::draw::DrawDsc::None,
            opa: OPA_COVER,
            preferred_draw_unit_id: 0,
            preference_score: 0,
        }
    }

    fn dsc(blend_area: Area) -> BlendDsc<'static> {
        BlendDsc {
            blend_area,
            src_buf: None,
            src_stride: 0,
            src_color_format: ColorFormat::Unknown,
            src_area: None,
            opa: OPA_COVER,
            color: Color::make(0x10, 0x20, 0x30),
            mask_buf: None,
            mask_res: MaskRes::Changed,
            mask_area: None,
            mask_stride: 0,
            blend_mode: BlendMode::Normal,
        }
    }

    /// Marks the layer so the test can see that the handler ran.
    fn marker(_: &DrawTask, layer: &mut Layer, _: &BlendDsc<'_>) {
        layer.opa = 7;
    }

    #[test]
    fn blend_early_returns_and_custom_handler() {
        static MASK: [u8; 4] = [255; 4];
        let hs = [CustomBlendHandler {
            dest_cf: ColorFormat::Rgb888,
            handler: marker,
        }];
        let t = task(Area::new(0, 0, 9, 9));
        let mut layer = Layer::new();
        layer.color_format = ColorFormat::Rgb888;
        let area = Area::new(2, 2, 3, 3);

        // Transparent: nothing happens.
        let mut d = dsc(area);
        d.opa = OPA_MIN;
        blend(&t, &mut layer, &d, &hs);
        assert_eq!(layer.opa, OPA_COVER);

        // Fully transparent mask.
        let mut d = dsc(area);
        d.mask_buf = Some(&MASK);
        d.mask_res = MaskRes::Transp;
        blend(&t, &mut layer, &d, &hs);
        assert_eq!(layer.opa, OPA_COVER);

        // Outside of the clip area.
        blend(&t, &mut layer, &dsc(Area::new(20, 20, 30, 30)), &hs);
        assert_eq!(layer.opa, OPA_COVER);

        // A registered handler takes over.
        blend(&t, &mut layer, &dsc(area), &hs);
        assert_eq!(layer.opa, 7);

        // No handler and no buffer: nothing to draw into.
        layer.opa = OPA_COVER;
        blend(&t, &mut layer, &dsc(area), &[]);
        assert_eq!(layer.opa, OPA_COVER);
    }

    #[test]
    fn goto_xy_matches_draw_buf_layout() {
        let h = ImageHeader::new(ColorFormat::Rgb888, 10, 4, 32);
        assert_eq!(buf_goto_xy(&h, 0, 0), 0);
        assert_eq!(buf_goto_xy(&h, 3, 2), 64 + 9);
        let h = ImageHeader::new(ColorFormat::I1, 16, 4, 2);
        // The 2-entry palette is skipped, then 1 bit per pixel.
        assert_eq!(buf_goto_xy(&h, 0, 1), 8 + 2);
        assert_eq!(buf_goto_xy(&h, 9, 1), 8 + 2 + 1);
        assert_eq!(slice_from(&[1, 2, 3], -1), None);
        assert_eq!(slice_from(&[1, 2, 3], 4), None);
        assert_eq!(slice_from(&[1, 2, 3], 1), Some(&[2u8, 3][..]));
    }

    /// An `RGB888` / `XRGB8888` layer of 8x5 pixels at (10, 20).
    #[cfg(all(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
    fn layer_buf(cf: ColorFormat) -> (Area, ImageHeader, alloc::vec::Vec<u8>) {
        let px = cf.size() as u16;
        let stride = 8 * px + 2;
        let header = ImageHeader::new(cf, 8, 5, stride);
        (
            Area::new(10, 20, 17, 24),
            header,
            alloc::vec![0u8; stride as usize * 5],
        )
    }

    #[cfg(all(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
    fn px_at(buf: &[u8], header: &ImageHeader, buf_area: &Area, x: i32, y: i32) -> [u8; 3] {
        let o = buf_goto_xy(header, (x - buf_area.x1) as u32, (y - buf_area.y1) as u32);
        [buf[o], buf[o + 1], buf[o + 2]]
    }

    #[cfg(all(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
    #[test]
    fn dispatch_fill_with_mask_offsets() {
        for cf in [ColorFormat::Rgb888, ColorFormat::Xrgb8888] {
            let (buf_area, header, mut data) = layer_buf(cf);
            // Mask covers (11, 21)..(16, 23), stride = width = 6.
            let mask_area = Area::new(11, 21, 16, 23);
            let mask: alloc::vec::Vec<u8> = (0..18).map(|i| if i % 2 == 0 { 255 } else { 0 }).collect();
            let blend_area = Area::new(12, 21, 14, 22);
            let mut d = dsc(blend_area);
            d.mask_buf = Some(&mask);
            d.mask_area = Some(mask_area);
            blend_to_buf(cf, &buf_area, &header, &mut data, blend_area, &d);
            let c = [0x30, 0x20, 0x10];
            for y in 20..25 {
                for x in 10..18 {
                    let inside = blend_area.is_point_on(ferrovgl_core::misc::area::Point::new(x, y), 0);
                    let m = if inside {
                        mask[((y - 21) * 6 + (x - 11)) as usize]
                    } else {
                        0
                    };
                    let expect = if m == 255 { c } else { [0, 0, 0] };
                    assert_eq!(
                        px_at(&data, &header, &buf_area, x, y),
                        expect,
                        "{cf:?} ({x}, {y})"
                    );
                }
            }

            // FULL_COVER ignores the mask buffer.
            let (_, _, mut data) = layer_buf(cf);
            d.mask_res = MaskRes::FullCover;
            blend_to_buf(cf, &buf_area, &header, &mut data, blend_area, &d);
            assert_eq!(px_at(&data, &header, &buf_area, 13, 21), c);
        }
    }

    #[cfg(all(feature = "cf-rgb888", feature = "cf-xrgb8888"))]
    #[test]
    fn dispatch_image_clips_to_src_and_mask_area() {
        for cf in [ColorFormat::Rgb888, ColorFormat::Xrgb8888] {
            let (buf_area, header, mut data) = layer_buf(cf);
            // A 4x3 RGB888 image at (9, 19): its left column and top row are
            // outside the layer clip given to `blend_to_buf`.
            let src_area = Area::new(9, 19, 12, 21);
            let src_stride = 4 * 3 + 1;
            let src: alloc::vec::Vec<u8> = (0..src_stride * 3).map(|i| i as u8 + 1).collect();
            let blend_area = Area::new(10, 20, 17, 24);
            let mut d = dsc(blend_area);
            d.src_buf = Some(&src);
            d.src_area = Some(src_area);
            d.src_stride = src_stride as u32;
            d.src_color_format = ColorFormat::Rgb888;
            // The mask area further limits the image to x <= 11.
            let mask = [255u8; 12 * 31];
            d.mask_area = Some(Area::new(0, 0, 11, 30));
            d.mask_buf = Some(&mask);
            d.mask_stride = 12;
            blend_to_buf(cf, &buf_area, &header, &mut data, blend_area, &d);
            for y in 20..25 {
                for x in 10..18 {
                    let expect = if x <= 11 && y <= 21 {
                        let s = ((y - 19) * src_stride + (x - 9) * 3) as usize;
                        [src[s], src[s + 1], src[s + 2]]
                    } else {
                        [0, 0, 0]
                    };
                    assert_eq!(
                        px_at(&data, &header, &buf_area, x, y),
                        expect,
                        "{cf:?} ({x}, {y})"
                    );
                }
            }
        }
    }
}
