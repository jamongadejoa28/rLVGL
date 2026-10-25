//! Port of `examples/styles/lv_example_style_modal.c`.
//!
//! Upstream compiles it only with `LV_USE_LOG`; the port keeps the body and
//! drops the final `LV_LOG_USER()` print (the crate has no logger).

use ferrovgl_core::misc::color::OPA_50;
use ferrovgl_core::{Color, StyleSelector, Ui};
use ferrovgl_widgets::Slider;

use super::lv_example_style_multiple;
use crate::util::screen_active;

/// `lv_example_style_modal()`: a semi-transparent modal background on
/// `lv_layer_top()` over the cascading-styles scene, timing the render.
pub fn lv_example_style_modal(ui: &mut Ui) {
    /*Add the cascading-styles scene as background*/
    lv_example_style_multiple(ui);

    /* Simulate a modal background by setting a semi-transparent black background
     * on lv_layer_top(), the highest built-in layer.
     * (The `#if 0` recolor-overlay branch of upstream is not compiled.) */
    let Some(layer_top) = ui.layer_top() else {
        return;
    };
    ui.obj_set_style_bg_color(layer_top, Color::black(), StyleSelector::DEFAULT);
    ui.obj_set_style_bg_opa(layer_top, OPA_50, StyleSelector::DEFAULT);

    let obj = Slider::create(ui, layer_top).obj();
    ui.obj_center(obj);

    ui.refr_now(None); /*Update layouts and render*/

    let screen = screen_active(ui);
    ui.obj_invalidate(screen);

    let t = ui.tick_get();
    ui.refr_now(None); /*Render only*/
    // LV_LOG_USER("%" LV_PRIu32 " ms\n", lv_tick_elaps(t)): not printed, the crate has no logger.
    let _ = ui.tick_elaps(t);
}
