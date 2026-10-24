//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_label.c`.

use ferrovgl_core::draw::label::LabelDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_label()`
pub fn label(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &LabelDsc, coords: &Area) {
    todo!()
}
