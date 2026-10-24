//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_letter.c`.

use ferrovgl_core::draw::label::{DrawGlyphDsc, LetterDsc};
use ferrovgl_core::draw::rect::FillDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask, DrawTaskRef};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_letter()`
pub fn letter(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LetterDsc, coords: &Area) {
    todo!()
}

/// `draw_letter_cb()`: the glyph callback shared with `label`.
pub fn draw_letter_cb(
    ctx: &mut DrawCtx,
    t: DrawTaskRef,
    glyph_draw_dsc: &mut DrawGlyphDsc,
    fill: Option<(&FillDsc, &Area)>,
) {
    todo!()
}

/// `draw_letter_bitmap()`
pub fn draw_letter_bitmap(ctx: &mut DrawCtx, t: DrawTaskRef, dsc: &DrawGlyphDsc) {
    todo!()
}
