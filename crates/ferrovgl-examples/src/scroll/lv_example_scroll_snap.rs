//! Port of `examples/scroll/lv_example_scroll_snap.c`.

use ferrovgl_core::core::obj::ScrollSnap;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_scroll_snap()`: snap the children to the center; one is not snappable.
pub fn lv_example_scroll_snap(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 290, 150);
    ui.obj_set_flex_flow(container, FlexFlow::Row);
    ui.obj_set_style_pad_column(container, 10, sel);
    ui.obj_set_scroll_snap_x(container, ScrollSnap::Center);

    for (i, text) in ["Panel 1", "Panel 2", "No snap", "Panel 4", "Panel 5", "Panel 6"]
        .into_iter()
        .enumerate()
    {
        let button = Button::create(ui, container).obj();
        ui.obj_set_size(button, 120, coord::pct(100));
        if i == 2 {
            ui.obj_set_snappable(button, false);
            ui.obj_set_style_bg_color(button, Color::hex(0x9429ff), sel);
        }
        ui.obj_set_scroll_on_focus(button, false);
        let label = Label::create(ui, button);
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
    }
}
