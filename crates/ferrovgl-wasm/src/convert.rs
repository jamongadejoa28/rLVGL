//! Conversion of a flushed display area into the non-premultiplied RGBA8
//! byte order of a Canvas2D `ImageData`.
//!
//! LVGL stores 32-bit pixels as little-endian words, so an XRGB8888 /
//! ARGB8888 pixel is `B, G, R, X/A` in memory and RGB888 is `B, G, R`.
//! The browser wants `R, G, B, A`. RGB565 is expanded with the rounding
//! multipliers of upstream `lv_draw_sw_blend_to_argb8888.c`
//! (`red * 2106 >> 8`, `green * 1037 >> 8`).

use std::fmt;

use ferrovgl_core::draw::draw_buf::DrawBuf;
use ferrovgl_core::{Area, ColorFormat, RenderMode};

/// Display color formats the canvas flush can convert.
pub const SUPPORTED_FORMATS: &[ColorFormat] = &[
    ColorFormat::Xrgb8888,
    ColorFormat::Argb8888,
    ColorFormat::Rgb888,
    ColorFormat::Rgb565,
];

/// Why a flushed area could not be converted.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConvertError {
    /// The color format is not in [`SUPPORTED_FORMATS`].
    UnsupportedFormat(ColorFormat),
    /// The area is empty or lies (partly) outside the display.
    BadArea(Area),
    /// The pixel buffer is shorter than the area and stride require.
    ShortBuffer {
        /// Bytes the area needs.
        needed: usize,
        /// Bytes the flush supplied.
        got: usize,
    },
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::UnsupportedFormat(cf) => write!(f, "unsupported canvas color format {cf:?}"),
            ConvertError::BadArea(a) => write!(
                f,
                "flush area ({}, {})..({}, {}) is outside the display",
                a.x1, a.y1, a.x2, a.y2
            ),
            ConvertError::ShortBuffer { needed, got } => {
                write!(f, "flush buffer has {got} bytes, the area needs {needed}")
            }
        }
    }
}

impl core::error::Error for ConvertError {}

/// Parses a color format name as used by the JavaScript API
/// (`"xrgb8888"`, `"argb8888"`, `"rgb888"`, `"rgb565"`; case-insensitive).
#[must_use]
pub fn parse_color_format(name: &str) -> Option<ColorFormat> {
    let lower = name.trim().to_ascii_lowercase();
    Some(match lower.as_str() {
        "" | "xrgb8888" | "native" => ColorFormat::Xrgb8888,
        "argb8888" => ColorFormat::Argb8888,
        "rgb888" => ColorFormat::Rgb888,
        "rgb565" => ColorFormat::Rgb565,
        _ => return None,
    })
}

/// Parses a render mode name (`"partial"`, `"direct"`, `"full"`;
/// case-insensitive, empty means partial like `LV_DISPLAY_RENDER_MODE_PARTIAL`).
#[must_use]
pub fn parse_render_mode(name: &str) -> Option<RenderMode> {
    let lower = name.trim().to_ascii_lowercase();
    Some(match lower.as_str() {
        "" | "partial" => RenderMode::Partial,
        "direct" => RenderMode::Direct,
        "full" => RenderMode::Full,
        _ => return None,
    })
}

/// Where the pixels of a flushed `area` live inside `px_map`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceLayout {
    /// Bytes per row of `px_map`.
    pub stride: usize,
    /// Column of the area's first pixel inside `px_map`.
    pub x: usize,
    /// Row of the area's first pixel inside `px_map`.
    pub y: usize,
}

/// Locates `area` in the flush buffer, following `refr_area()`:
/// in [`RenderMode::Partial`] the buffer is reshaped to the area with an
/// automatic stride (`lv_draw_buf_width_to_stride(area_w, cf)`), so the
/// area starts at `(0, 0)`. In direct and full mode the buffer is the
/// whole screen with `screen_stride` and the area sits at its own
/// coordinates.
#[must_use]
pub fn source_layout(mode: RenderMode, cf: ColorFormat, area: &Area, screen_stride: usize) -> SourceLayout {
    match mode {
        RenderMode::Partial => SourceLayout {
            stride: DrawBuf::width_to_stride(area.width().max(0) as u32, cf) as usize,
            x: 0,
            y: 0,
        },
        RenderMode::Direct | RenderMode::Full => SourceLayout {
            stride: screen_stride,
            x: area.x1.max(0) as usize,
            y: area.y1.max(0) as usize,
        },
    }
}

/// Checks that `area` is a non-empty rectangle inside a `hor_res` x
/// `ver_res` display.
pub fn check_area(area: &Area, hor_res: i32, ver_res: i32) -> Result<(), ConvertError> {
    if area.x1 < 0
        || area.y1 < 0
        || area.x2 < area.x1
        || area.y2 < area.y1
        || area.x2 >= hor_res
        || area.y2 >= ver_res
    {
        return Err(ConvertError::BadArea(*area));
    }
    Ok(())
}

/// Converts a `w` x `h` block of `src` (`cf` pixels at `layout`) into
/// tightly packed RGBA8 in `dst` (cleared and resized to `w * h * 4`).
///
/// XRGB8888, RGB888 and RGB565 produce opaque pixels; ARGB8888 keeps its
/// straight alpha (the canvas composites it over the page).
pub fn area_to_rgba(
    cf: ColorFormat,
    src: &[u8],
    layout: SourceLayout,
    w: usize,
    h: usize,
    dst: &mut Vec<u8>,
) -> Result<(), ConvertError> {
    let bpp = match cf {
        ColorFormat::Xrgb8888 | ColorFormat::Argb8888 => 4,
        ColorFormat::Rgb888 => 3,
        ColorFormat::Rgb565 => 2,
        other => return Err(ConvertError::UnsupportedFormat(other)),
    };
    dst.clear();
    if w == 0 || h == 0 {
        return Ok(());
    }
    let row_bytes = w * bpp;
    let first = layout.y * layout.stride + layout.x * bpp;
    let needed = first + (h - 1) * layout.stride + row_bytes;
    if layout.stride < (layout.x * bpp + row_bytes) || src.len() < needed {
        return Err(ConvertError::ShortBuffer {
            needed: needed.max(first + row_bytes),
            got: src.len(),
        });
    }
    dst.resize(w * h * 4, 0);
    for (row, out) in dst.chunks_exact_mut(w * 4).enumerate() {
        let start = first + row * layout.stride;
        let line = &src[start..start + row_bytes];
        let out = out.as_chunks_mut::<4>().0;
        match cf {
            ColorFormat::Xrgb8888 => {
                for (o, p) in out.iter_mut().zip(line.as_chunks::<4>().0) {
                    *o = [p[2], p[1], p[0], 0xFF];
                }
            }
            ColorFormat::Argb8888 => {
                for (o, p) in out.iter_mut().zip(line.as_chunks::<4>().0) {
                    *o = [p[2], p[1], p[0], p[3]];
                }
            }
            ColorFormat::Rgb888 => {
                for (o, p) in out.iter_mut().zip(line.as_chunks::<3>().0) {
                    *o = [p[2], p[1], p[0], 0xFF];
                }
            }
            _ => {
                for (o, p) in out.iter_mut().zip(line.as_chunks::<2>().0) {
                    *o = rgb565_to_rgba(u16::from_le_bytes(*p));
                }
            }
        }
    }
    Ok(())
}

/// Expands one RGB565 pixel like upstream's RGB565 -> ARGB8888 blend
/// (`(c.red * 2106) >> 8`, `(c.green * 1037) >> 8`, `(c.blue * 2106) >> 8`).
#[must_use]
pub fn rgb565_to_rgba(px: u16) -> [u8; 4] {
    let red = u32::from(px >> 11) & 0x1F;
    let green = u32::from(px >> 5) & 0x3F;
    let blue = u32::from(px) & 0x1F;
    [
        ((red * 2106) >> 8) as u8,
        ((green * 1037) >> 8) as u8,
        ((blue * 2106) >> 8) as u8,
        0xFF,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;

    #[test]
    fn names_parse() {
        assert_eq!(parse_color_format("XRGB8888"), Some(ColorFormat::Xrgb8888));
        assert_eq!(parse_color_format(""), Some(ColorFormat::Xrgb8888));
        assert_eq!(parse_color_format("rgb565"), Some(ColorFormat::Rgb565));
        assert_eq!(parse_color_format("i1"), None);
        assert_eq!(parse_render_mode("Direct"), Some(RenderMode::Direct));
        assert_eq!(parse_render_mode(""), Some(RenderMode::Partial));
        assert_eq!(parse_render_mode("tiled"), None);
    }

    #[test]
    fn partial_layout_uses_area_stride() {
        let area = Area::new(10, 20, 14, 29);
        let l = source_layout(RenderMode::Partial, ColorFormat::Xrgb8888, &area, 3200);
        assert_eq!(
            l,
            SourceLayout {
                stride: 20,
                x: 0,
                y: 0
            }
        );
        let l = source_layout(RenderMode::Partial, ColorFormat::Rgb888, &area, 3200);
        assert_eq!(l.stride, 15);
        let l = source_layout(RenderMode::Direct, ColorFormat::Xrgb8888, &area, 3200);
        assert_eq!(
            l,
            SourceLayout {
                stride: 3200,
                x: 10,
                y: 20
            }
        );
        let l = source_layout(RenderMode::Full, ColorFormat::Xrgb8888, &area, 3200);
        assert_eq!((l.x, l.y), (10, 20));
    }

    #[test]
    fn area_checks() {
        assert!(check_area(&Area::new(0, 0, 9, 9), 10, 10).is_ok());
        assert!(check_area(&Area::new(0, 0, 10, 9), 10, 10).is_err());
        assert!(check_area(&Area::new(-1, 0, 3, 3), 10, 10).is_err());
        assert!(check_area(&Area::new(4, 4, 3, 5), 10, 10).is_err());
    }

    #[test]
    fn xrgb_and_argb_swizzle() {
        // 2x1 block at (1, 1) of a 3x2 XRGB buffer with a padded stride.
        let stride = 16;
        let mut src = vec![0u8; stride * 2];
        src[stride + 4..stride + 8].copy_from_slice(&[0x56, 0x34, 0x12, 0x00]);
        src[stride + 8..stride + 12].copy_from_slice(&[0x03, 0x02, 0x01, 0x80]);
        let layout = SourceLayout { stride, x: 1, y: 1 };
        let mut out = Vec::new();
        area_to_rgba(ColorFormat::Xrgb8888, &src, layout, 2, 1, &mut out).unwrap();
        assert_eq!(out, [0x12, 0x34, 0x56, 0xFF, 0x01, 0x02, 0x03, 0xFF]);
        area_to_rgba(ColorFormat::Argb8888, &src, layout, 2, 1, &mut out).unwrap();
        assert_eq!(out, [0x12, 0x34, 0x56, 0x00, 0x01, 0x02, 0x03, 0x80]);
    }

    #[test]
    fn rgb888_and_rgb565() {
        let src = [0x30, 0x20, 0x10, 0x60, 0x50, 0x40];
        let mut out = Vec::new();
        let layout = SourceLayout {
            stride: 6,
            x: 0,
            y: 0,
        };
        area_to_rgba(ColorFormat::Rgb888, &src, layout, 2, 1, &mut out).unwrap();
        assert_eq!(out, [0x10, 0x20, 0x30, 0xFF, 0x40, 0x50, 0x60, 0xFF]);
        assert_eq!(rgb565_to_rgba(0xFFFF), [255, 255, 255, 255]);
        assert_eq!(rgb565_to_rgba(0x0000), [0, 0, 0, 255]);
        // Pure red 0xF800, pure green 0x07E0, mid blue 0x0010.
        assert_eq!(rgb565_to_rgba(0xF800), [255, 0, 0, 255]);
        assert_eq!(rgb565_to_rgba(0x07E0), [0, 255, 0, 255]);
        assert_eq!(rgb565_to_rgba(0x0010), [0, 0, ((16 * 2106) >> 8) as u8, 255]);
        let src = 0xF800u16.to_le_bytes();
        area_to_rgba(
            ColorFormat::Rgb565,
            &src,
            SourceLayout {
                stride: 2,
                x: 0,
                y: 0,
            },
            1,
            1,
            &mut out,
        )
        .unwrap();
        assert_eq!(out, [255, 0, 0, 255]);
    }

    #[test]
    fn short_buffers_and_formats_are_rejected() {
        let mut out = Vec::new();
        let layout = SourceLayout {
            stride: 8,
            x: 0,
            y: 0,
        };
        assert!(matches!(
            area_to_rgba(ColorFormat::Xrgb8888, &[0; 12], layout, 2, 2, &mut out),
            Err(ConvertError::ShortBuffer { got: 12, .. })
        ));
        assert!(area_to_rgba(ColorFormat::Xrgb8888, &[0; 16], layout, 2, 2, &mut out).is_ok());
        assert_eq!(out.len(), 16);
        // A stride narrower than the block is a layout error, not UB.
        let narrow = SourceLayout {
            stride: 4,
            x: 0,
            y: 0,
        };
        assert!(area_to_rgba(ColorFormat::Xrgb8888, &[0; 64], narrow, 2, 2, &mut out).is_err());
        assert_eq!(
            area_to_rgba(ColorFormat::I1, &[0; 16], layout, 1, 1, &mut out),
            Err(ConvertError::UnsupportedFormat(ColorFormat::I1))
        );
        area_to_rgba(ColorFormat::Xrgb8888, &[], layout, 0, 3, &mut out).unwrap();
        assert!(out.is_empty());
    }
}
