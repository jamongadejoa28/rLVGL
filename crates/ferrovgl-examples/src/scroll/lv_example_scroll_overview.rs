//! Port of `examples/scroll/lv_example_scroll_overview.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_scroll_overview()`: a fixed-height panel overflowed by its content.
pub fn lv_example_scroll_overview(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    /* Fixed-height panel; the column of buttons overflows it */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_size(container, 220, 140);
    ui.obj_set_flex_flow(container, FlexFlow::Column);
    ui.obj_set_style_pad_row(container, 8, sel);

    for i in 1..=6 {
        let button = Button::create(ui, container).obj();
        ui.obj_set_width(button, coord::pct(100));
        let label = Label::create(ui, button);
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text_fmt(ui, format_args!("Item {i}"));
    }
}
