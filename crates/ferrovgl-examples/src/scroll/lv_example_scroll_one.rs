//! Port of `examples/scroll/lv_example_scroll_one.c`.

use ferrovgl_core::core::obj::ScrollSnap;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_scroll_one()`: snap scrolling that advances one panel at a time.
pub fn lv_example_scroll_one(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 300, 150);
    ui.obj_set_flex_flow(container, FlexFlow::Row);
    ui.obj_set_style_pad_column(container, 10, sel);
    ui.obj_set_scroll_snap_x(container, ScrollSnap::Center);
    ui.obj_set_scroll_one(container, true);

    for i in 1..=9 {
        let button = Button::create(ui, container).obj();
        ui.obj_set_size(button, 80, coord::pct(100));
        ui.obj_set_scroll_on_focus(button, false);
        let label = Label::create(ui, button);
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text_fmt(ui, format_args!("Panel {i}"));
    }
}
