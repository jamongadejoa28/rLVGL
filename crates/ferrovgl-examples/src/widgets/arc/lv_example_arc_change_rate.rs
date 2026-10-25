//! Port of `examples/widgets/arc/lv_example_arc_change_rate.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Align, StyleSelector, Ui};
use ferrovgl_widgets::{Arc, Label};

use crate::util::screen_active;

/// `lv_example_arc_change_rate()`: arcs with a slow and a fast knob response.
pub fn lv_example_arc_change_rate(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    /* Slower response arc, then faster response arc */
    for (rate, text) in [(40, "rate=40"), (360, "rate=360")] {
        let arc = Arc::create(ui, screen);
        ui.obj_set_size(arc.obj(), 120, 120);
        arc.set_bg_start_angle(ui, 135 as ValuePrecise);
        arc.set_bg_end_angle(ui, 45 as ValuePrecise);
        arc.set_change_rate(ui, rate);
        arc.set_value(ui, 35);
        let label = Label::create(ui, arc.obj());
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
    }
}
