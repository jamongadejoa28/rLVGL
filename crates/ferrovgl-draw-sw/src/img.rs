//! Ported from lvgl v9.6.0: `src/draw/sw/lv_draw_sw_img.c`.

use ferrovgl_core::draw::image::{DecodedImage, DrawImageDsc, DrawImageSup};
use ferrovgl_core::draw::{DrawCtx, DrawTask};
use ferrovgl_core::misc::area::Area;

use crate::SwDrawUnit;

/// `lv_draw_sw_image()`
pub fn image(unit: &mut SwDrawUnit, ctx: &mut DrawCtx, t: &DrawTask, dsc: &DrawImageDsc, coords: &Area) {
    todo!()
}

/// `img_draw_core()`: blit / transform `decoded` (the `decoder_dsc`) into
/// the layer; the `lv_draw_image_core_cb` of this renderer.
pub fn img_draw_core(
    unit: &mut SwDrawUnit,
    ctx: &mut DrawCtx,
    t: &DrawTask,
    draw_dsc: &DrawImageDsc,
    decoded: &DecodedImage<'_>,
    sup: Option<&DrawImageSup>,
    img_coords: &Area,
    clipped_img_area: &Area,
) {
    todo!()
}
