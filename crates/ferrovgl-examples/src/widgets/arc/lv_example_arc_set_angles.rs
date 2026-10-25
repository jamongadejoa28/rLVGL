//! Port of `examples/widgets/arc/lv_example_arc_set_angles.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Align, Part, StyleSelector, Ui};
use ferrovgl_widgets::{Arc, Label};

use crate::util::screen_active;

/// `lv_example_arc_set_angles()`: a value-driven and an angle-driven indicator.
pub fn lv_example_arc_set_angles(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    /* Value-driven indicator arc */
    let arc_1 = Arc::create(ui, screen);
    ui.obj_set_size(arc_1.obj(), 120, 120);
    arc_1.set_bg_start_angle(ui, 135 as ValuePrecise);
    arc_1.set_bg_end_angle(ui, 45 as ValuePrecise);
    arc_1.set_min_value(ui, 100);
    arc_1.set_max_value(ui, 200);
    arc_1.set_value(ui, 145);
    let label_1 = Label::create(ui, arc_1.obj());
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "value");

    /* Indicator angle set explicitly; value/mode are ignored for indicator drawing */
    let arc_2 = Arc::create(ui, screen);
    ui.obj_set_size(arc_2.obj(), 120, 120);
    arc_2.set_bg_start_angle(ui, 135 as ValuePrecise);
    arc_2.set_bg_end_angle(ui, 45 as ValuePrecise);
    arc_2.set_start_angle(ui, 210 as ValuePrecise);
    arc_2.set_end_angle(ui, 330 as ValuePrecise);
    ui.obj_set_style_bg_opa(arc_2.obj(), (255 * 0 / 100) as u8, Part::KNOB.into());
    let label_2 = Label::create(ui, arc_2.obj());
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "angles");
}
