//! Port of `examples/widgets/arc/lv_example_arc_rotation.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Align, StyleSelector, Ui};
use ferrovgl_widgets::{Arc, Label};

use crate::util::screen_active;

/// `lv_example_arc_rotation()`: arcs rotated by 45 and 135 degrees.
pub fn lv_example_arc_rotation(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    for (rotation, text) in [(45, "rot=45"), (135, "rot=135")] {
        let arc = Arc::create(ui, screen);
        ui.obj_set_size(arc.obj(), 120, 120);
        arc.set_bg_start_angle(ui, 0 as ValuePrecise);
        arc.set_bg_end_angle(ui, 270 as ValuePrecise);
        arc.set_rotation(ui, rotation);
        arc.set_value(ui, 60);
        let label = Label::create(ui, arc.obj());
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
    }
}
