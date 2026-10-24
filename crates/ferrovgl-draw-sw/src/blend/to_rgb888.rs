//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_to_rgb888.c`.

use crate::blend::{BlendFillDsc, BlendImageDsc};

/// `lv_draw_sw_blend_color_to_rgb888()`: fill with a colour (+ opa / mask).
/// `dest_px_size` is the bytes per destination pixel: 3 for `RGB888`, 4 for
/// `XRGB8888` (this kernel renders both, `LV_DRAW_SW_SUPPORT_RGB888 ||
/// LV_DRAW_SW_SUPPORT_XRGB8888`).
pub fn fill(dsc: &mut BlendFillDsc<'_>, dest_px_size: u32) {
    todo!()
}

/// `lv_draw_sw_blend_image_to_rgb888()`: blend an image of any supported
/// source format. `dest_px_size` is 3 (`RGB888`) or 4 (`XRGB8888`).
pub fn image(dsc: &mut BlendImageDsc<'_>, dest_px_size: u32) {
    todo!()
}
