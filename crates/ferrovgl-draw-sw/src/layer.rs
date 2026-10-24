//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw.c` (`lv_draw_sw_layer`).

use ferrovgl_core::draw::image::DrawImageDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_layer()`: draw a finished sub-layer as an image.
pub fn layer(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &DrawImageDsc, coords: &Area) {
    todo!()
}
