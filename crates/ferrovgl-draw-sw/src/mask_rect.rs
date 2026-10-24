//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_mask_rect.c`.

use ferrovgl_core::draw::mask::MaskRectDsc;
use ferrovgl_core::draw::{DrawCtx, DrawTask};

use crate::SwDrawUnit;

/// `lv_draw_sw_mask_rect()`: clear the layer outside a rounded rectangle.
pub fn mask_rect(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &MaskRectDsc) {
    todo!()
}
