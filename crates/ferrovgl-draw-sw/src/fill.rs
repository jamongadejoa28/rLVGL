//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_fill.c`.

use ferrovgl_core::draw::rect::FillDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_fill()`: rounded rectangle fill with optional gradient.
pub fn fill(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &FillDsc, coords: &Area) {
    todo!()
}
