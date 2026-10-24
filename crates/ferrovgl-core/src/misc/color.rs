//! Colours, opacity and colour formats.
//!
//! Ported from lvgl v9.6.0: `src/misc/lv_color.c`, `src/misc/lv_color_op.c`,
//! `include/lvgl/draw/lv_color.h`, `include/lvgl/draw/lv_color_op.h`.
//!
//! [`Color`] is always 24-bit RGB (`lv_color_t`); the pixel format of a buffer
//! is a runtime [`ColorFormat`], exactly as in LVGL v9.

use super::math::udiv255;

/// Opacity (`lv_opa_t`): 0 = transparent, 255 = cover.
pub type Opa = u8;

/// `LV_OPA_TRANSP`
pub const OPA_TRANSP: Opa = 0;
/// `LV_OPA_0`
pub const OPA_0: Opa = 0;
/// `LV_OPA_10`
pub const OPA_10: Opa = 25;
/// `LV_OPA_20`
pub const OPA_20: Opa = 51;
/// `LV_OPA_30`
pub const OPA_30: Opa = 76;
/// `LV_OPA_40`
pub const OPA_40: Opa = 102;
/// `LV_OPA_50`
pub const OPA_50: Opa = 127;
/// `LV_OPA_60`
pub const OPA_60: Opa = 153;
/// `LV_OPA_70`
pub const OPA_70: Opa = 178;
/// `LV_OPA_80`
pub const OPA_80: Opa = 204;
/// `LV_OPA_90`
pub const OPA_90: Opa = 229;
/// `LV_OPA_100`
pub const OPA_100: Opa = 255;
/// `LV_OPA_COVER`
pub const OPA_COVER: Opa = 255;
/// `LV_OPA_MIN`: fully transparent if `opa <= OPA_MIN`.
pub const OPA_MIN: Opa = 2;
/// `LV_OPA_MAX`: fully covering if `opa >= OPA_MAX`.
pub const OPA_MAX: Opa = 253;

/// `LV_COLOR_MIX_ROUND_OFS` (0 in the default and test configurations).
pub const COLOR_MIX_ROUND_OFS: u32 = 0;

/// `LV_OPA_MIX2(a1, a2)`
#[inline]
pub const fn opa_mix2(a1: u8, a2: u8) -> Opa {
    ((a1 as i32 * a2 as i32) >> 8) as Opa
}

/// `LV_OPA_MIX3(a1, a2, a3)`
#[inline]
pub const fn opa_mix3(a1: u8, a2: u8, a3: u8) -> Opa {
    ((a1 as i32 * a2 as i32 * a3 as i32) >> 16) as Opa
}

/// `lv_color_t`: 24-bit RGB with LVGL's in-memory field order (blue first).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Color {
    /// Blue channel.
    pub blue: u8,
    /// Green channel.
    pub green: u8,
    /// Red channel.
    pub red: u8,
}

/// `lv_color32_t`: 32-bit BGRA in memory (ARGB8888 little-endian).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Color32 {
    /// Blue channel.
    pub blue: u8,
    /// Green channel.
    pub green: u8,
    /// Red channel.
    pub red: u8,
    /// Alpha channel.
    pub alpha: u8,
}

/// `lv_color16_t`: RGB565 packed in a `u16` (`b:5 | g:6 << 5 | r:5 << 11`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Color16(pub u16);

/// `lv_color16a_t`: luminance plus alpha (AL88).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Color16a {
    /// Luminance.
    pub lumi: u8,
    /// Alpha channel.
    pub alpha: u8,
}

/// `lv_color_hsv_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ColorHsv {
    /// Hue, 0..=359.
    pub h: u16,
    /// Saturation, 0..=100.
    pub s: u8,
    /// Value, 0..=100.
    pub v: u8,
}

/// `lv_color_format_t`, with upstream's numeric values.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum ColorFormat {
    /// `LV_COLOR_FORMAT_UNKNOWN`
    #[default]
    Unknown = 0,
    /// `LV_COLOR_FORMAT_RAW`
    Raw = 0x01,
    /// `LV_COLOR_FORMAT_RAW_ALPHA`
    RawAlpha = 0x02,
    /// `LV_COLOR_FORMAT_L8`
    L8 = 0x06,
    /// `LV_COLOR_FORMAT_I1`
    I1 = 0x07,
    /// `LV_COLOR_FORMAT_I2`
    I2 = 0x08,
    /// `LV_COLOR_FORMAT_I4`
    I4 = 0x09,
    /// `LV_COLOR_FORMAT_I8`
    I8 = 0x0A,
    /// `LV_COLOR_FORMAT_A1`
    A1 = 0x0B,
    /// `LV_COLOR_FORMAT_A2`
    A2 = 0x0C,
    /// `LV_COLOR_FORMAT_A4`
    A4 = 0x0D,
    /// `LV_COLOR_FORMAT_A8`
    A8 = 0x0E,
    /// `LV_COLOR_FORMAT_RGB888`
    Rgb888 = 0x0F,
    /// `LV_COLOR_FORMAT_ARGB8888`
    Argb8888 = 0x10,
    /// `LV_COLOR_FORMAT_XRGB8888`
    Xrgb8888 = 0x11,
    /// `LV_COLOR_FORMAT_RGB565`
    Rgb565 = 0x12,
    /// `LV_COLOR_FORMAT_ARGB8565`
    Argb8565 = 0x13,
    /// `LV_COLOR_FORMAT_RGB565A8`: colour array followed by an alpha array.
    Rgb565A8 = 0x14,
    /// `LV_COLOR_FORMAT_AL88`: L8 with alpha.
    Al88 = 0x15,
    /// `LV_COLOR_FORMAT_ARGB1555`
    Argb1555 = 0x16,
    /// `LV_COLOR_FORMAT_ARGB4444`
    Argb4444 = 0x17,
    /// `LV_COLOR_FORMAT_ARGB2222`
    Argb2222 = 0x18,
    /// `LV_COLOR_FORMAT_ARGB8888_PREMULTIPLIED`
    Argb8888Premultiplied = 0x1A,
    /// `LV_COLOR_FORMAT_RGB565_SWAPPED`
    Rgb565Swapped = 0x1B,
    /// `LV_COLOR_FORMAT_I420`
    I420 = 0x20,
    /// `LV_COLOR_FORMAT_I422`
    I422 = 0x21,
    /// `LV_COLOR_FORMAT_I444`
    I444 = 0x22,
    /// `LV_COLOR_FORMAT_I400`
    I400 = 0x23,
    /// `LV_COLOR_FORMAT_NV21`
    Nv21 = 0x24,
    /// `LV_COLOR_FORMAT_NV12`
    Nv12 = 0x25,
    /// `LV_COLOR_FORMAT_YUY2`
    Yuy2 = 0x26,
    /// `LV_COLOR_FORMAT_UYVY`
    Uyvy = 0x27,
    /// `LV_COLOR_FORMAT_NEMA_TSC4`
    NemaTsc4 = 0x30,
    /// `LV_COLOR_FORMAT_NEMA_TSC6`
    NemaTsc6 = 0x31,
    /// `LV_COLOR_FORMAT_NEMA_TSC6A`
    NemaTsc6a = 0x32,
    /// `LV_COLOR_FORMAT_NEMA_TSC6AP`
    NemaTsc6ap = 0x33,
    /// `LV_COLOR_FORMAT_NEMA_TSC12`
    NemaTsc12 = 0x34,
    /// `LV_COLOR_FORMAT_NEMA_TSC12A`
    NemaTsc12a = 0x35,
}

impl ColorFormat {
    /// `LV_COLOR_FORMAT_NATIVE`: the 24-bit [`Color`] layout.
    pub const NATIVE: ColorFormat = ColorFormat::Rgb888;
    /// `LV_COLOR_FORMAT_NATIVE_WITH_ALPHA`
    pub const NATIVE_WITH_ALPHA: ColorFormat = ColorFormat::Argb8888;

    /// `lv_color_format_get_bpp()`: bits per pixel.
    pub const fn bpp(self) -> u8 {
        use ColorFormat::*;
        match self {
            I1 | A1 => 1,
            I2 | A2 => 2,
            I4 | A4 | NemaTsc4 => 4,
            NemaTsc6 | NemaTsc6a | NemaTsc6ap => 6,
            L8 | A8 | I8 | Argb2222 => 8,
            NemaTsc12 | NemaTsc12a => 12,
            Rgb565A8 | Rgb565 | Rgb565Swapped | Yuy2 | Al88 | Argb1555 | Argb4444 => 16,
            Argb8565 | Rgb888 => 24,
            Argb8888 | Argb8888Premultiplied | Xrgb8888 => 32,
            _ => 0,
        }
    }

    /// `lv_color_format_get_size()`: bytes per pixel, rounded up.
    #[inline]
    pub const fn size(self) -> u8 {
        (self.bpp() + 7) >> 3
    }

    /// `lv_color_format_has_alpha()`
    pub const fn has_alpha(self) -> bool {
        use ColorFormat::*;
        matches!(
            self,
            A1 | A2
                | A4
                | A8
                | I1
                | I2
                | I4
                | I8
                | Rgb565A8
                | Argb8565
                | Argb8888
                | Argb8888Premultiplied
                | Al88
                | Argb2222
                | Argb1555
                | Argb4444
        )
    }

    /// `LV_COLOR_FORMAT_IS_YUV(cf)`
    #[inline]
    pub const fn is_yuv(self) -> bool {
        let v = self as u8;
        v >= ColorFormat::I420 as u8 && v <= ColorFormat::Uyvy as u8
    }

    /// `LV_COLOR_FORMAT_IS_INDEXED(cf)`
    #[inline]
    pub const fn is_indexed(self) -> bool {
        matches!(
            self,
            ColorFormat::I1 | ColorFormat::I2 | ColorFormat::I4 | ColorFormat::I8
        )
    }

    /// `LV_COLOR_FORMAT_IS_ALPHA_ONLY(cf)`
    #[inline]
    pub const fn is_alpha_only(self) -> bool {
        matches!(
            self,
            ColorFormat::A1 | ColorFormat::A2 | ColorFormat::A4 | ColorFormat::A8
        )
    }

    /// `LV_COLOR_INDEXED_PALETTE_SIZE(cf)`: palette entries for indexed formats.
    #[inline]
    pub const fn indexed_palette_size(self) -> u32 {
        match self {
            ColorFormat::I1 => 2,
            ColorFormat::I2 => 4,
            ColorFormat::I4 => 16,
            ColorFormat::I8 => 256,
            _ => 0,
        }
    }

    /// Convert the upstream numeric value back into a format, if known.
    pub const fn from_u8(v: u8) -> Option<ColorFormat> {
        use ColorFormat::*;
        Some(match v {
            0x00 => Unknown,
            0x01 => Raw,
            0x02 => RawAlpha,
            0x06 => L8,
            0x07 => I1,
            0x08 => I2,
            0x09 => I4,
            0x0A => I8,
            0x0B => A1,
            0x0C => A2,
            0x0D => A4,
            0x0E => A8,
            0x0F => Rgb888,
            0x10 => Argb8888,
            0x11 => Xrgb8888,
            0x12 => Rgb565,
            0x13 => Argb8565,
            0x14 => Rgb565A8,
            0x15 => Al88,
            0x16 => Argb1555,
            0x17 => Argb4444,
            0x18 => Argb2222,
            0x1A => Argb8888Premultiplied,
            0x1B => Rgb565Swapped,
            0x20 => I420,
            0x21 => I422,
            0x22 => I444,
            0x23 => I400,
            0x24 => Nv21,
            0x25 => Nv12,
            0x26 => Yuy2,
            0x27 => Uyvy,
            0x30 => NemaTsc4,
            0x31 => NemaTsc6,
            0x32 => NemaTsc6a,
            0x33 => NemaTsc6ap,
            0x34 => NemaTsc12,
            0x35 => NemaTsc12a,
            _ => return None,
        })
    }
}

impl Color {
    /// `lv_color_make()` / `LV_COLOR_MAKE()`
    #[inline]
    pub const fn make(r: u8, g: u8, b: u8) -> Self {
        Self {
            blue: b,
            green: g,
            red: r,
        }
    }

    /// `lv_color_hex()`: from `0xRRGGBB`.
    #[inline]
    pub const fn hex(c: u32) -> Self {
        Self {
            red: ((c >> 16) & 0xff) as u8,
            green: ((c >> 8) & 0xff) as u8,
            blue: (c & 0xff) as u8,
        }
    }

    /// `lv_color_hex3()`: from `0xRGB`, each nibble doubled.
    #[inline]
    pub const fn hex3(c: u32) -> Self {
        Self::make(
            (((c >> 4) & 0xF0) | ((c >> 8) & 0xF)) as u8,
            ((c & 0xF0) | ((c & 0xF0) >> 4)) as u8,
            ((c & 0xF) | ((c & 0xF) << 4)) as u8,
        )
    }

    /// `lv_color_white()`
    pub const WHITE: Color = Color::make(0xff, 0xff, 0xff);
    /// `lv_color_black()`
    pub const BLACK: Color = Color::make(0x00, 0x00, 0x00);

    /// `lv_color_white()`
    #[inline]
    pub const fn white() -> Self {
        Self::WHITE
    }

    /// `lv_color_black()`
    #[inline]
    pub const fn black() -> Self {
        Self::BLACK
    }

    /// `lv_color_to_32()`
    #[inline]
    pub const fn to_32(self, opa: Opa) -> Color32 {
        Color32 {
            blue: self.blue,
            green: self.green,
            red: self.red,
            alpha: opa,
        }
    }

    /// `lv_color_to_u16()`: pack as RGB565.
    #[inline]
    pub const fn to_u16(self) -> u16 {
        (((self.red & 0xF8) as u16) << 8)
            + (((self.green & 0xFC) as u16) << 3)
            + ((self.blue & 0xF8) as u16 >> 3)
    }

    /// `lv_color_to_u32()`: `0xFFRRGGBB`.
    #[inline]
    pub const fn to_u32(self) -> u32 {
        (0xffu32 << 24) + ((self.red as u32) << 16) + ((self.green as u32) << 8) + self.blue as u32
    }

    /// `lv_color_to_int()`: the three bytes as stored in memory (`0xRRGGBB`).
    #[inline]
    pub const fn to_int(self) -> u32 {
        self.blue as u32 + ((self.green as u32) << 8) + ((self.red as u32) << 16)
    }

    /// `lv_color_eq()`
    #[inline]
    pub const fn eq(self, other: Color) -> bool {
        self.to_int() == other.to_int()
    }

    /// `lv_color_mix()`: `c1 * mix + c2 * (255 - mix)`.
    #[inline]
    pub const fn mix(c1: Color, c2: Color, mix: u8) -> Color {
        let m = mix as u32;
        Color {
            red: udiv255(c1.red as u32 * m + c2.red as u32 * (255 - m) + COLOR_MIX_ROUND_OFS) as u8,
            green: udiv255(c1.green as u32 * m + c2.green as u32 * (255 - m) + COLOR_MIX_ROUND_OFS) as u8,
            blue: udiv255(c1.blue as u32 * m + c2.blue as u32 * (255 - m) + COLOR_MIX_ROUND_OFS) as u8,
        }
    }

    /// `lv_color_lighten()`
    #[inline]
    pub const fn lighten(self, lvl: Opa) -> Color {
        Color::mix(Color::WHITE, self, lvl)
    }

    /// `lv_color_darken()`
    #[inline]
    pub const fn darken(self, lvl: Opa) -> Color {
        Color::mix(Color::BLACK, self, lvl)
    }

    /// `lv_color_hsv_to_rgb()`: `h` in 0..=359, `s` and `v` in 0..=100.
    pub const fn from_hsv(h: u16, s: u8, v: u8) -> Color {
        let h = ((h as u32 * 255) / 360) as u8;
        let s = ((s as u16 * 255) / 100) as u8;
        let v = ((v as u16 * 255) / 100) as u8;
        if s == 0 {
            return Color::make(v, v, v);
        }
        let region = h / 43;
        let remainder = ((h - (region * 43)) as u32 * 6) as u8; // u8 arithmetic in C: (h - region*43) < 43, *6 < 258 -> wraps at 256 like C uint8
        let remainder = remainder as u32;
        let (v32, s32) = (v as u32, s as u32);
        let p = ((v32 * (255 - s32)) >> 8) as u8;
        let q = ((v32 * (255 - ((s32 * remainder) >> 8))) >> 8) as u8;
        let t = ((v32 * (255 - ((s32 * (255 - remainder)) >> 8))) >> 8) as u8;
        let (r, g, b) = match region {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        Color::make(r, g, b)
    }

    /// `lv_color_rgb_to_hsv()`
    pub fn rgb_to_hsv(r8: u8, g8: u8, b8: u8) -> ColorHsv {
        let r = ((r8 as u32) << 10) / 255;
        let g = ((g8 as u32) << 10) / 255;
        let b = ((b8 as u32) << 10) / 255;
        let rgb_min = r.min(g).min(b);
        let rgb_max = r.max(g).max(b);
        let mut hsv = ColorHsv {
            h: 0,
            s: 0,
            v: ((100 * rgb_max) >> 10) as u8,
        };
        let delta = rgb_max as i32 - rgb_min as i32;
        if delta < 3 {
            return hsv;
        }
        hsv.s = (100 * delta as u32 / rgb_max) as u8;
        if hsv.s < 3 {
            return hsv;
        }
        let (r, g, b) = (r as i32, g as i32, b as i32);
        let mut h: i32 = if rgb_max as i32 == r {
            (((g - b) << 10) / delta) + if g < b { 6 << 10 } else { 0 }
        } else if rgb_max as i32 == g {
            (((b - r) << 10) / delta) + (2 << 10)
        } else if rgb_max as i32 == b {
            (((r - g) << 10) / delta) + (4 << 10)
        } else {
            0
        };
        h *= 60;
        h >>= 10;
        if h < 0 {
            h += 360;
        }
        hsv.h = h as u16;
        hsv
    }

    /// `lv_color_to_hsv()`
    #[inline]
    pub fn to_hsv(self) -> ColorHsv {
        Color::rgb_to_hsv(self.red, self.green, self.blue)
    }

    /// `lv_color_luminance()`
    #[inline]
    pub const fn luminance(self) -> u8 {
        ((77u32 * self.red as u32 + 151u32 * self.green as u32 + 28u32 * self.blue as u32) >> 8) as u8
    }

    /// `lv_color_brightness()`
    #[inline]
    pub const fn brightness(self) -> u8 {
        let bright = 3u32 * self.red as u32 + self.green as u32 + 4u32 * self.blue as u32;
        (bright >> 3) as u8
    }

    /// `lv_color_filter_shade` callback: lighten below 50% opacity, darken above.
    pub const fn shade(self, opa: Opa) -> Color {
        if opa == OPA_50 {
            self
        } else if opa < OPA_50 {
            self.lighten(((OPA_50 - opa) as u32 * 2) as u8)
        } else {
            // Upstream computes `(opa - LV_OPA_50 * LV_OPA_50) * 2` in int and truncates to lv_opa_t.
            self.darken(((opa as i32 - OPA_50 as i32 * OPA_50 as i32) * 2) as u8)
        }
    }
}

impl Color32 {
    /// `lv_color32_make()`
    #[inline]
    pub const fn make(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            blue: b,
            green: g,
            red: r,
            alpha: a,
        }
    }

    /// The 24-bit colour part.
    #[inline]
    pub const fn color(self) -> Color {
        Color {
            blue: self.blue,
            green: self.green,
            red: self.red,
        }
    }

    /// Pack as little-endian ARGB8888 (`0xAARRGGBB`).
    #[inline]
    pub const fn to_u32(self) -> u32 {
        u32::from_le_bytes([self.blue, self.green, self.red, self.alpha])
    }

    /// Unpack from little-endian ARGB8888.
    #[inline]
    pub const fn from_u32(v: u32) -> Self {
        let [blue, green, red, alpha] = v.to_le_bytes();
        Self {
            blue,
            green,
            red,
            alpha,
        }
    }

    /// `lv_color32_eq()`
    #[inline]
    pub const fn eq(self, other: Color32) -> bool {
        self.to_u32() == other.to_u32()
    }

    /// `lv_color_mix32()`: blend `fg` over an opaque `bg`, keeping `bg`'s alpha.
    pub const fn mix32(fg: Color32, bg: Color32) -> Color32 {
        if fg.alpha >= OPA_MAX {
            return Color32 {
                alpha: bg.alpha,
                ..fg
            };
        }
        if fg.alpha <= OPA_MIN {
            return bg;
        }
        let a = fg.alpha as u32;
        Color32 {
            red: udiv255(fg.red as u32 * a + bg.red as u32 * (255 - a)) as u8,
            green: udiv255(fg.green as u32 * a + bg.green as u32 * (255 - a)) as u8,
            blue: udiv255(fg.blue as u32 * a + bg.blue as u32 * (255 - a)) as u8,
            alpha: bg.alpha,
        }
    }

    /// `lv_color_mix32_premultiplied()`
    pub const fn mix32_premultiplied(fg: Color32, bg: Color32) -> Color32 {
        if fg.alpha >= OPA_MAX {
            return fg;
        }
        if fg.alpha <= OPA_MIN {
            return bg;
        }
        let inv = (OPA_MAX - fg.alpha) as u32;
        Color32 {
            red: (fg.red as u32 + ((bg.red as u32 * inv) >> 8)) as u8,
            green: (fg.green as u32 + ((bg.green as u32 * inv) >> 8)) as u8,
            blue: (fg.blue as u32 + ((bg.blue as u32 * inv) >> 8)) as u8,
            alpha: bg.alpha,
        }
    }

    /// `lv_color_over32()`: Porter-Duff "over" with both alphas.
    pub const fn over32(fg: Color32, bg: Color32) -> Color32 {
        if fg.alpha >= OPA_MAX || bg.alpha <= OPA_MIN {
            return fg;
        } else if fg.alpha <= OPA_MIN {
            return bg;
        } else if bg.alpha == 255 {
            return Color32::mix32(fg, bg);
        }
        let res_alpha: Opa = 255 - opa_mix2(255 - fg.alpha, 255 - bg.alpha);
        let ratio: Opa = ((fg.alpha as u32 * 255) / res_alpha as u32) as Opa;
        let fg = Color32 { alpha: ratio, ..fg };
        let mut res = Color32::mix32(fg, bg);
        res.alpha = res_alpha;
        res
    }

    /// `lv_color_premultiply()`
    pub const fn premultiply(self) -> Color32 {
        if self.alpha == OPA_COVER {
            return self;
        }
        if self.alpha == OPA_TRANSP {
            return Color32 {
                blue: 0,
                green: 0,
                red: 0,
                alpha: 0,
            };
        }
        Color32 {
            red: opa_mix2(self.red, self.alpha),
            green: opa_mix2(self.green, self.alpha),
            blue: opa_mix2(self.blue, self.alpha),
            alpha: self.alpha,
        }
    }

    /// `lv_color32_luminance()`
    #[inline]
    pub const fn luminance(self) -> u8 {
        self.color().luminance()
    }
}

impl Color16 {
    /// Pack from 5/6/5-bit channels.
    #[inline]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self(((red as u16 & 0x1F) << 11) | ((green as u16 & 0x3F) << 5) | (blue as u16 & 0x1F))
    }

    /// From a 24-bit colour (`lv_color_to_u16`).
    #[inline]
    pub const fn from_color(c: Color) -> Self {
        Self(c.to_u16())
    }

    /// 5-bit red channel.
    #[inline]
    pub const fn red(self) -> u8 {
        (self.0 >> 11) as u8
    }
    /// 6-bit green channel.
    #[inline]
    pub const fn green(self) -> u8 {
        ((self.0 >> 5) & 0x3F) as u8
    }
    /// 5-bit blue channel.
    #[inline]
    pub const fn blue(self) -> u8 {
        (self.0 & 0x1F) as u8
    }

    /// `lv_color16_luminance()`
    #[inline]
    pub const fn luminance(self) -> u8 {
        ((635u32 * self.red() as u32 + 613u32 * self.green() as u32 + 231u32 * self.blue() as u32) >> 8) as u8
    }

    /// `lv_color16_premultiply()`
    pub const fn premultiply(self, a: Opa) -> Color16 {
        if a == OPA_COVER {
            return self;
        }
        if a == OPA_TRANSP {
            return Color16(0);
        }
        Color16::new(
            opa_mix2(self.red(), a),
            opa_mix2(self.green(), a),
            opa_mix2(self.blue(), a),
        )
    }

    /// `lv_color_16_16_mix()`: blend two RGB565 values, `mix` = weight of `c1`.
    pub const fn mix(c1: u16, c2: u16, mix: u8) -> u16 {
        if mix == 255 {
            return c1;
        }
        if mix == 0 {
            return c2;
        }
        if c1 == c2 {
            return c1;
        }
        let mix = (mix as u32 + 4) >> 3;
        // 0x7E0F81F = 0b00000111111000001111100000011111
        let bg = (c2 as u32 | ((c2 as u32) << 16)) & 0x7E0F81F;
        let fg = (c1 as u32 | ((c1 as u32) << 16)) & 0x7E0F81F;
        let result = ((((fg.wrapping_sub(bg)).wrapping_mul(mix)) >> 5).wrapping_add(bg)) & 0x7E0F81F;
        ((result >> 16) as u16) | (result as u16)
    }
}

/// `lv_color24_luminance()`: luminance of a `[b, g, r]` byte triple.
#[inline]
pub const fn color24_luminance(c: [u8; 3]) -> u8 {
    ((77u32 * c[2] as u32 + 151u32 * c[1] as u32 + 28u32 * c[0] as u32) >> 8) as u8
}

/// `lv_color_filter_dsc_t`: a function that maps a colour and an intensity to
/// a new colour.
#[derive(Clone, Copy)]
pub struct ColorFilter {
    /// `filter_cb`
    pub filter: fn(Color, Opa) -> Color,
}

impl ColorFilter {
    /// `lv_color_filter_shade`
    pub const SHADE: ColorFilter = ColorFilter { filter: Color::shade };

    /// `lv_color_filter_dsc_init()`
    pub const fn new(filter: fn(Color, Opa) -> Color) -> Self {
        Self { filter }
    }

    /// Apply the filter.
    #[inline]
    pub fn apply(&self, c: Color, opa: Opa) -> Color {
        (self.filter)(c, opa)
    }
}

impl core::fmt::Debug for ColorFilter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ColorFilter(..)")
    }
}

impl PartialEq for ColorFilter {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::fn_addr_eq(self.filter, other.filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Port of upstream `test_color.c: test_color_format_get_bpp / has_alpha`.
    #[test]
    fn format_bpp_and_alpha() {
        use ColorFormat::*;
        for (cf, bpp) in [
            (I1, 1),
            (A1, 1),
            (I2, 2),
            (A2, 2),
            (I4, 4),
            (A4, 4),
            (L8, 8),
            (A8, 8),
            (I8, 8),
            (Argb2222, 8),
            (Rgb565A8, 16),
            (Rgb565, 16),
            (Yuy2, 16),
            (Argb1555, 16),
            (Argb4444, 16),
            (Argb8565, 24),
            (Rgb888, 24),
            (Argb8888, 32),
            (Xrgb8888, 32),
            (Unknown, 0),
        ] {
            assert_eq!(cf.bpp(), bpp, "{cf:?}");
        }
        for (cf, alpha) in [
            (A1, true),
            (A2, true),
            (A4, true),
            (A8, true),
            (I1, true),
            (I2, true),
            (I4, true),
            (I8, true),
            (Rgb565A8, true),
            (Argb8565, true),
            (Argb8888, true),
            (Argb2222, true),
            (Argb1555, true),
            (Argb4444, true),
            (Xrgb8888, false),
            (Rgb888, false),
            (Yuy2, false),
            (Rgb565, false),
            (L8, false),
            (Unknown, false),
        ] {
            assert_eq!(cf.has_alpha(), alpha, "{cf:?}");
        }
        assert_eq!(Xrgb8888.size(), 4);
        assert_eq!(Rgb888.size(), 3);
        assert_eq!(I1.size(), 1);
        for v in 0u8..=0x40 {
            if let Some(cf) = ColorFormat::from_u8(v) {
                assert_eq!(cf as u8, v);
            }
        }
    }

    /// Ports of upstream `test_color.c: test_color_to_32/u16/u32/lighten`.
    #[test]
    fn conversions_match_upstream() {
        let c = Color::make(255, 128, 64);
        let c32 = c.to_32(192);
        assert_eq!((c32.red, c32.green, c32.blue, c32.alpha), (255, 128, 64, 192));
        assert_eq!(c.to_u16(), 0xFC08);
        assert_eq!(c.to_u32(), 0xFFFF8040);
        assert_eq!(Color::make(0, 0, 0).to_u32(), 0xFF000000);
        assert_eq!(Color::make(255, 255, 255).to_u32(), 0xFFFFFFFF);
        assert_eq!(Color::make(123, 45, 67).to_u32(), 0xFF7B2D43);

        assert_eq!(Color::make(100, 100, 100).lighten(128).to_u32(), 0xFFB1B1B1);
        assert_eq!(Color::make(50, 100, 150).lighten(0).to_u32(), 0xFF326496);
        assert_eq!(Color::make(100, 100, 100).lighten(255).to_u32(), 0xFFFFFFFF);
        assert_eq!(Color::make(100, 100, 100).darken(255).to_u32(), 0xFF000000);

        assert_eq!(Color::hex(0x123456), Color::make(0x12, 0x34, 0x56));
        assert_eq!(Color::hex3(0x123), Color::make(0x11, 0x22, 0x33));
        assert_eq!(Color::hex(0x123456).to_int(), 0x123456);
        assert!(Color::hex(0x123456).eq(Color::make(0x12, 0x34, 0x56)));
    }

    #[test]
    fn mix_endpoints_and_midpoint() {
        let a = Color::make(200, 100, 0);
        let b = Color::make(0, 100, 200);
        assert_eq!(Color::mix(a, b, 255), a);
        assert_eq!(Color::mix(a, b, 0), b);
        let m = Color::mix(a, b, 128);
        assert_eq!((m.red, m.green, m.blue), (100, 100, 99));
    }

    #[test]
    fn mix32_and_over32() {
        let fg = Color32::make(255, 0, 0, 128);
        let bg = Color32::make(0, 0, 255, 255);
        let m = Color32::mix32(fg, bg);
        assert_eq!((m.red, m.green, m.blue, m.alpha), (128, 0, 127, 255));
        assert_eq!(
            Color32::mix32(Color32::make(1, 2, 3, 255), bg),
            Color32::make(1, 2, 3, 255)
        );
        assert_eq!(Color32::mix32(Color32::make(1, 2, 3, 0), bg), bg);
        let o = Color32::over32(fg, Color32::make(0, 0, 255, 128));
        assert_eq!(o.alpha, 255 - opa_mix2(127, 127));
        assert_eq!(Color32::over32(fg, Color32::make(0, 0, 255, 0)), fg);
        assert_eq!(
            Color32::make(200, 100, 50, 128).premultiply(),
            Color32::make(100, 50, 25, 128)
        );
        assert_eq!(Color32::make(1, 2, 3, 0).premultiply(), Color32::make(0, 0, 0, 0));
        assert_eq!(Color32::from_u32(0xAABBCCDD).to_u32(), 0xAABBCCDD);
        assert_eq!(
            Color32::from_u32(0xAABBCCDD),
            Color32::make(0xBB, 0xCC, 0xDD, 0xAA)
        );
    }

    #[test]
    fn rgb565_helpers() {
        let c = Color16::from_color(Color::make(255, 128, 64));
        assert_eq!(c.0, 0xFC08);
        assert_eq!((c.red(), c.green(), c.blue()), (31, 32, 8));
        assert_eq!(Color16::mix(0xF800, 0x001F, 255), 0xF800);
        assert_eq!(Color16::mix(0xF800, 0x001F, 0), 0x001F);
        let m = Color16::mix(0xF800, 0x001F, 128);
        let m = Color16(m);
        assert!(
            (m.red() as i32 - 16).abs() <= 1 && (m.blue() as i32 - 15).abs() <= 1,
            "{m:?}"
        );
        assert_eq!(Color16(0xFFFF).premultiply(0).0, 0);
        assert_eq!(Color16(0xFFFF).premultiply(255).0, 0xFFFF);
    }

    #[test]
    fn hsv_roundtrip() {
        assert_eq!(Color::from_hsv(0, 100, 100), Color::make(255, 0, 0));
        assert_eq!(Color::from_hsv(0, 0, 50), Color::make(127, 127, 127));
        let hsv = Color::make(255, 0, 0).to_hsv();
        assert_eq!((hsv.h, hsv.s, hsv.v), (0, 100, 100));
        let hsv = Color::make(0, 255, 0).to_hsv();
        assert_eq!(hsv.h, 120);
        let hsv = Color::make(0, 0, 255).to_hsv();
        assert_eq!(hsv.h, 240);
        let hsv = Color::make(10, 10, 10).to_hsv();
        assert_eq!((hsv.h, hsv.s), (0, 0));
    }

    #[test]
    fn luminance_and_filters() {
        assert_eq!(Color::WHITE.luminance(), 255);
        assert_eq!(Color::BLACK.luminance(), 0);
        assert_eq!(Color::make(255, 0, 0).luminance(), 76);
        assert_eq!(Color::WHITE.brightness(), 255);
        assert_eq!(color24_luminance([0, 0, 255]), 76);
        let f = ColorFilter::SHADE;
        assert_eq!(
            f.apply(Color::make(100, 100, 100), OPA_50),
            Color::make(100, 100, 100)
        );
        assert!(f.apply(Color::make(100, 100, 100), 0).red > 100);
        assert_eq!(f, ColorFilter::SHADE);
    }
}
