//! Ported from lvgl v9.6.0: `include/lvgl/draw/lv_draw_label.h`, `src/draw/lv_draw_label_private.h`,
//! `src/draw/lv_draw_label.c`.

#![allow(unused_variables)] // skeleton: bodies are todo!()

use alloc::rc::Rc;

use crate::draw::rect::FillDsc;
use crate::draw::{DrawBuf, DrawCtx, DrawDscBase, DrawTaskRef, LayerId};
use crate::font::{FontRef, GlyphDsc, GlyphFormat};
use crate::misc::area::{Area, Point};
use crate::misc::color::{Color, Opa};
use crate::misc::style::{BlendMode, ImageSrc};
use crate::misc::text::{BaseDir, TextAlign, TextDecor, TextFlags};

/// `LV_DRAW_LABEL_NO_TXT_SEL`
pub const DRAW_LABEL_NO_TXT_SEL: u32 = 0xFFFF;

/// `lv_draw_label_hint_t`: cache to speed up drawing of long texts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct LabelHint {
    /// `line_start`: index of the first letter of the cached line.
    pub line_start: i32,
    /// `y`: y coordinate of the cached line.
    pub y: i32,
    /// `coord_y`: the `y1` of the text area when the hint was saved.
    pub coord_y: i32,
}

/// `lv_draw_label_dsc_t`
#[derive(Clone, Debug)]
pub struct LabelDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `text`
    pub text: Rc<str>,
    /// `text_size`
    pub text_size: Point,
    /// `font`
    pub font: Option<FontRef>,
    /// `color`
    pub color: Color,
    /// `line_space`
    pub line_space: i32,
    /// `letter_space`
    pub letter_space: i32,
    /// `ofs_x`
    pub ofs_x: i32,
    /// `ofs_y`
    pub ofs_y: i32,
    /// `rotation`
    pub rotation: i32,
    /// `sel_start`
    pub sel_start: u32,
    /// `sel_end`
    pub sel_end: u32,
    /// `sel_color`
    pub sel_color: Color,
    /// `sel_bg_color`
    pub sel_bg_color: Color,
    /// `text_length`: number of bytes to draw (0 = whole text).
    pub text_length: u32,
    /// `align`
    pub align: TextAlign,
    /// `bidi_dir`
    pub bidi_dir: BaseDir,
    /// `opa`
    pub opa: Opa,
    /// `outline_stroke_opa`
    pub outline_stroke_opa: Opa,
    /// `decor`
    pub decor: TextDecor,
    /// `flag`
    pub flag: TextFlags,
    /// `text_local`: the text was copied into the task.
    pub text_local: bool,
    /// `text_static`: the text is in static memory.
    pub text_static: bool,
    /// `has_bided`: the text already went through BiDi processing.
    pub has_bided: bool,
    /// `hint`
    pub hint: Option<Rc<core::cell::Cell<LabelHint>>>,
    /// `outline_stroke_color`
    pub outline_stroke_color: Color,
    /// `outline_stroke_width`
    pub outline_stroke_width: i32,
}

impl LabelDsc {
    /// `lv_draw_label_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for LabelDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_letter_dsc_t`
#[derive(Clone, Debug)]
pub struct LetterDsc {
    /// `base`
    pub base: DrawDscBase,
    /// `unicode`
    pub unicode: u32,
    /// `font`
    pub font: Option<FontRef>,
    /// `color`
    pub color: Color,
    /// `rotation`
    pub rotation: i32,
    /// `scale_x`
    pub scale_x: i32,
    /// `scale_y`
    pub scale_y: i32,
    /// `skew_x`
    pub skew_x: i32,
    /// `skew_y`
    pub skew_y: i32,
    /// `pivot`
    pub pivot: Point,
    /// `opa`
    pub opa: Opa,
    /// `decor`
    pub decor: TextDecor,
    /// `blend_mode`
    pub blend_mode: BlendMode,
    /// `outline_stroke_opa`
    pub outline_stroke_opa: Opa,
    /// `outline_stroke_width`
    pub outline_stroke_width: i32,
    /// `outline_stroke_color`
    pub outline_stroke_color: Color,
}

impl LetterDsc {
    /// `lv_draw_letter_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for LetterDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `glyph_data` of `lv_draw_glyph_dsc_t`: where the glyph pixels are.
///
/// Owned / indexed (no borrowed lifetime) so that the renderer callback can
/// set it after fetching the bitmap lazily, exactly like
/// `lv_draw_sw_letter.c` does.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum GlyphData {
    /// `NULL`: nothing to draw (placeholder or missing glyph).
    #[default]
    None,
    /// The glyph was rendered into [`DrawGlyphDsc::draw_buf`] (A8) with
    /// `Font::glyph_bitmap_a8` (`lv_font_get_glyph_bitmap(g, _draw_buf)`).
    ScratchBuf,
    /// `req_raw_bitmap` fast path: the font has a static bitmap, read it
    /// through `Font::glyph_static_bitmap` on [`DrawGlyphDsc::resolved_font`].
    StaticBitmap,
    /// `LV_FONT_GLYPH_FORMAT_IMAGE`: the glyph is an image source.
    Image(ImageSrc),
}

/// `lv_draw_glyph_dsc_t`: per-glyph data handed to the renderer callback.
pub struct DrawGlyphDsc {
    /// `glyph_data`: the rendered glyph, see [`GlyphData`].
    pub glyph_data: GlyphData,
    /// `_draw_buf`: the reusable A8 scratch buffer the glyph is rendered
    /// into (`lv_draw_buf_reshape` / `lv_draw_buf_create_ex(font_draw_buf_handlers, ...)`
    /// per glyph in `lv_draw_unit_draw_letter`). Do not read it directly,
    /// use `glyph_data`.
    pub draw_buf: Option<DrawBuf>,
    /// `g->resolved_font`: the font that actually has the glyph (after
    /// fallback resolution); `None` for a placeholder.
    pub resolved_font: Option<FontRef>,
    /// `format`
    pub format: GlyphFormat,
    /// `letter_coords`
    pub letter_coords: Area,
    /// `bg_coords`: background area (selection).
    pub bg_coords: Option<Area>,
    /// `g`: glyph metrics.
    pub g: GlyphDsc,
    /// `color`
    pub color: Color,
    /// `opa`
    pub opa: Opa,
    /// `outline_stroke_color`
    pub outline_stroke_color: Color,
    /// `outline_stroke_opa`
    pub outline_stroke_opa: Opa,
    /// `outline_stroke_width`
    pub outline_stroke_width: i32,
    /// `rotation`
    pub rotation: i32,
    /// `pivot`
    pub pivot: Point,
}

impl DrawGlyphDsc {
    /// `lv_draw_glyph_dsc_init()`
    pub fn new() -> Self {
        todo!()
    }
}

impl Default for DrawGlyphDsc {
    fn default() -> Self {
        Self::new()
    }
}

/// `lv_draw_glyph_cb_t`: called for every glyph (and for underline /
/// strikethrough fills, when `fill_dsc` is given).
pub type DrawGlyphCallback<'a> =
    &'a mut dyn FnMut(&mut DrawCtx, DrawTaskRef, &mut DrawGlyphDsc, Option<(&FillDsc, &Area)>);

impl DrawCtx {
    /// `lv_draw_label()`
    pub fn draw_label(&mut self, layer: LayerId, dsc: &LabelDsc, coords: &Area) {
        todo!()
    }

    /// `lv_draw_character()`
    pub fn draw_character(&mut self, layer: LayerId, dsc: &mut LabelDsc, point: Point, unicode_letter: u32) {
        todo!()
    }

    /// `lv_draw_letter()`
    pub fn draw_letter(&mut self, layer: LayerId, dsc: &mut LetterDsc, point: Point) {
        todo!()
    }

    /// `lv_draw_task_get_label_dsc()`
    pub fn task_get_label_dsc(&mut self, task: DrawTaskRef) -> Option<&mut LabelDsc> {
        todo!()
    }

    /// `lv_draw_label_iterate_characters()`: the shared text layout loop
    /// used by every renderer.
    pub fn label_iterate_characters(
        &mut self,
        t: DrawTaskRef,
        dsc: &LabelDsc,
        coords: &Area,
        cb: DrawGlyphCallback<'_>,
    ) {
        todo!()
    }

    /// `lv_draw_unit_draw_letter()`: resolve and draw a single glyph through `cb`.
    pub fn unit_draw_letter(
        &mut self,
        t: DrawTaskRef,
        dsc: &mut DrawGlyphDsc,
        pos: Point,
        font: &FontRef,
        letter: u32,
        cb: DrawGlyphCallback<'_>,
    ) {
        todo!()
    }
}
