//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_border.c`.

use ferrovgl_core::draw::rect::BorderDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_border()`
pub fn border(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &BorderDsc, coords: &Area) {
    todo!()
}
