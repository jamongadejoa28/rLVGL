//! Port of `examples/widgets/obj/lv_example_obj_transform.c`: the
//! `LV_DRAW_TRANSFORM_USE_MATRIX == 0` branch, matching the upstream test
//! configuration (only the VG-Lite configurations enable the matrix path).

use ferrovgl_core::Ui;
use ferrovgl_widgets::Label;

use crate::util::screen_active;

/// `lv_example_obj_transform()` without `LV_DRAW_TRANSFORM_USE_MATRIX`.
pub fn lv_example_obj_transform(ui: &mut Ui) {
    let label = Label::create(ui, screen_active(ui));
    label.set_text_static(ui, "LV_DRAW_TRANSFORM_USE_MATRIX is not enabled");
    ui.obj_center(label.obj());
}
