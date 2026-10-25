//! Port of `examples/scroll/lv_example_scroll_scrollable.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Obj, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// A title label followed by the buttons `A` to `D`.
fn fill_panel(ui: &mut Ui, panel: Obj, title: &str) {
    let label = Label::create(ui, panel);
    label.set_text(ui, title);
    for text in ["A", "B", "C", "D"] {
        let button = Button::create(ui, panel).obj();
        ui.obj_set_width(button, coord::pct(100));
        let label = Label::create(ui, button);
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
    }
}

/// `lv_example_scroll_scrollable()`: a scrollable panel next to a clipped one.
pub fn lv_example_scroll_scrollable(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_1, coord::pct(100), 160);
    ui.obj_set_flex_flow(container_1, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(container_1, FlexAlign::Center, sel);
    ui.obj_set_style_pad_column(container_1, 16, sel);

    /* Default: scrollable */
    let container_2 = ui.obj_create(Some(container_1));
    ui.obj_set_size(container_2, 130, coord::pct(100));
    ui.obj_set_flex_flow(container_2, FlexFlow::Column);
    ui.obj_set_style_pad_row(container_2, 8, sel);
    fill_panel(ui, container_2, "scrollable");

    /* Flag cleared: content is clipped, no scrolling */
    let container_3 = ui.obj_create(Some(container_1));
    ui.obj_set_size(container_3, 130, coord::pct(100));
    ui.obj_set_flex_flow(container_3, FlexFlow::Column);
    ui.obj_set_style_pad_row(container_3, 8, sel);
    ui.obj_set_scrollable(container_3, false);
    ui.obj_set_style_border_color(container_3, Color::hex(0x9429ff), sel);
    ui.obj_set_style_border_width(container_3, 2, sel);
    fill_panel(ui, container_3, "clipped");
}
