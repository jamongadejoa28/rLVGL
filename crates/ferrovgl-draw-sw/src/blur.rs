//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_blur.c`.

use ferrovgl_core::draw::blur::BlurDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_blur()`
pub fn blur(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &BlurDsc, coords: &Area) {
    todo!()
}
