//! Port of `examples/widgets/arc/lv_example_arc_mode.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, StyleSelector, Ui};
use ferrovgl_widgets::{Arc, ArcMode, Label};

use crate::util::screen_active;

/// `lv_example_arc_mode()`: normal, reverse and symmetrical arc modes.
pub fn lv_example_arc_mode(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    for (mode, text) in [
        /* Normal mode: clockwise from minimum to current value */
        (ArcMode::Normal, "normal"),
        /* Reverse mode: counter-clockwise from maximum to current value */
        (ArcMode::Reverse, "rev."),
        /* Symmetrical mode: indicator grows from the middle */
        (ArcMode::Symmetrical, "sym."),
    ] {
        let arc = Arc::create(ui, screen);
        ui.obj_set_size(arc.obj(), 95, 95);
        arc.set_mode(ui, mode);
        arc.set_value(ui, 75);
        let label = Label::create(ui, arc.obj());
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
    }
}
