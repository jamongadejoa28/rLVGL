//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_arc.c`.

use ferrovgl_core::draw::arc::ArcDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_arc()`
pub fn arc(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &ArcDsc, coords: &Area) {
    todo!()
}
