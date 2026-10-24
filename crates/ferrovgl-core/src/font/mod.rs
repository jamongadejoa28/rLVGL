//! Fonts and glyph lookup.
//!
//! Ported from lvgl v9.6.0: `src/font/lv_font.c`, `include/lvgl/font/lv_font.h`.
//!
//! A [`Font`] answers two questions: the metrics of a glyph
//! ([`Font::glyph_dsc`]) and its alpha bitmap ([`Font::glyph_bitmap_a8`]).
//! [`get_glyph_dsc`] adds LVGL's fallback-font chain and placeholder handling
//! on top of a single font.

pub mod fmt_txt;

pub use fmt_txt::FmtTxtFont;

/// `lv_font_glyph_format_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum GlyphFormat {
    /// `LV_FONT_GLYPH_FORMAT_NONE`: maybe not visible.
    #[default]
    None = 0,
    /// 1 bit per pixel.
    A1 = 0x01,
    /// 2 bits per pixel.
    A2 = 0x02,
    /// 3 bits per pixel.
    A3 = 0x03,
    /// 4 bits per pixel.
    A4 = 0x04,
    /// 8 bits per pixel.
    A8 = 0x08,
    /// `LV_FONT_GLYPH_FORMAT_IMAGE`
    Image = 0x19,
    /// `LV_FONT_GLYPH_FORMAT_VECTOR`
    Vector = 0x1A,
    /// `LV_FONT_GLYPH_FORMAT_SVG`
    Svg = 0x1B,
    /// `LV_FONT_GLYPH_FORMAT_CUSTOM`
    Custom = 0xFF,
}

impl GlyphFormat {
    /// Format for a bit depth of 1, 2, 3, 4 or 8.
    pub const fn from_bpp(bpp: u8) -> GlyphFormat {
        match bpp {
            1 => GlyphFormat::A1,
            2 => GlyphFormat::A2,
            3 => GlyphFormat::A3,
            4 => GlyphFormat::A4,
            8 => GlyphFormat::A8,
            _ => GlyphFormat::None,
        }
    }
}

/// `lv_font_subpx_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Subpx {
    /// `LV_FONT_SUBPX_NONE`
    #[default]
    None = 0,
    /// `LV_FONT_SUBPX_HOR`
    Hor = 1,
    /// `LV_FONT_SUBPX_VER`
    Ver = 2,
    /// `LV_FONT_SUBPX_BOTH`
    Both = 3,
}

/// `lv_font_kerning_t`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kerning {
    /// `LV_FONT_KERNING_NORMAL`
    #[default]
    Normal = 0,
    /// `LV_FONT_KERNING_NONE`
    None = 1,
}

/// `lv_font_glyph_dsc_t`: metrics of one glyph.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GlyphDsc {
    /// Glyph index inside the font (`gid.index`).
    pub gid: u32,
    /// Format of the bitmap returned for this glyph.
    pub format: GlyphFormat,
    /// Advance width in pixels.
    pub adv_w: u16,
    /// Width of the bounding box.
    pub box_w: u16,
    /// Height of the bounding box.
    pub box_h: u16,
    /// X offset of the bounding box.
    pub ofs_x: i16,
    /// Y offset of the bounding box, measured from the baseline.
    pub ofs_y: i16,
    /// Bytes per bitmap row, 0 when rows are packed without padding.
    pub stride: u16,
    /// The glyph is missing but a placeholder box will be drawn.
    pub is_placeholder: bool,
}

/// A font (`lv_font_t`).
pub trait Font {
    /// `line_height`: the line height where any text fits.
    fn line_height(&self) -> i32;
    /// `base_line`: baseline measured from the bottom of the line height.
    fn base_line(&self) -> i32;
    /// `cap_height`
    fn cap_height(&self) -> i32 {
        0
    }
    /// `x_height`
    fn x_height(&self) -> i32 {
        0
    }
    /// `subpx`
    fn subpx(&self) -> Subpx {
        Subpx::None
    }
    /// `kerning`
    fn kerning(&self) -> Kerning {
        Kerning::Normal
    }
    /// `underline_position`: distance between the top of the underline and
    /// the baseline (negative means below).
    fn underline_position(&self) -> i8;
    /// `underline_thickness`
    fn underline_thickness(&self) -> i8;
    /// `static_bitmap`: glyph bitmaps live in constant memory and can be used
    /// in place through [`Font::glyph_static_bitmap`].
    fn has_static_bitmap(&self) -> bool {
        false
    }
    /// `fallback`: font to try when a glyph is missing.
    fn fallback(&self) -> Option<&dyn Font> {
        None
    }
    /// `get_glyph_dsc`: fill `out` for `letter`, adjusting `adv_w` for the
    /// kerning with `letter_next` (0 if none). Returns `false` if missing.
    fn glyph_dsc(&self, out: &mut GlyphDsc, letter: u32, letter_next: u32) -> bool;
    /// `get_glyph_bitmap` for bitmap formats: write the glyph as A8 rows of
    /// `stride` bytes into `out`. Returns `false` if the glyph has no bitmap.
    fn glyph_bitmap_a8(&self, g: &GlyphDsc, out: &mut [u8], stride: usize) -> bool;
    /// `lv_font_get_glyph_static_bitmap`: the packed bitmap in the font's own
    /// format (`g.format`), only for fonts with static bitmaps.
    fn glyph_static_bitmap(&self, _g: &GlyphDsc) -> Option<&[u8]> {
        None
    }
}

/// `LV_USE_FONT_PLACEHOLDER` (enabled in the default configuration).
pub const USE_FONT_PLACEHOLDER: bool = true;

/// `lv_font_get_glyph_dsc()`: look `letter` up in `font` and its fallback
/// chain. Returns the descriptor and the font that actually resolved it
/// (`resolved_font`); `None` for the font means a placeholder.
pub fn get_glyph_dsc<'f>(
    font: &'f dyn Font,
    letter: u32,
    letter_next: u32,
) -> (GlyphDsc, Option<&'f dyn Font>) {
    let has_kerning = font.kerning() != Kerning::None;
    let next = if has_kerning { letter_next } else { 0 };
    let mut dsc = GlyphDsc::default();
    let mut placeholder_font: Option<&'f dyn Font> = None;
    let mut f = Some(font);
    while let Some(cur) = f {
        if cur.glyph_dsc(&mut dsc, letter, next) {
            if !dsc.is_placeholder {
                return (dsc, Some(cur));
            } else if USE_FONT_PLACEHOLDER && placeholder_font.is_none() {
                placeholder_font = Some(cur);
            }
        }
        f = cur.fallback();
    }
    if USE_FONT_PLACEHOLDER && let Some(pf) = placeholder_font {
        pf.glyph_dsc(&mut dsc, letter, next);
        return (dsc, Some(pf));
    }
    let mut dsc = GlyphDsc::default();
    if USE_FONT_PLACEHOLDER {
        dsc.box_w = (font.line_height() / 2) as u16;
        dsc.adv_w = dsc.box_w + 2;
    }
    dsc.stride = 0;
    dsc.box_h = font.line_height() as u16;
    dsc.format = GlyphFormat::A1;
    dsc.is_placeholder = true;
    (dsc, None)
}

/// `lv_font_get_glyph_width()`: advance width of `letter` (0 for text markers).
pub fn get_glyph_width(font: &dyn Font, letter: u32, letter_next: u32) -> u16 {
    if crate::misc::text::is_marker(letter) {
        return 0;
    }
    get_glyph_dsc(font, letter, letter_next).0.adv_w
}

/// `lv_font_get_line_height()`
#[inline]
pub fn get_line_height(font: &dyn Font) -> i32 {
    font.line_height()
}
