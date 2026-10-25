//! Port of `examples/scroll/lv_example_scroll_chain.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, Obj, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// A full-width button with a centered label.
fn row_button(ui: &mut Ui, parent: Obj, text: &str) {
    let button = Button::create(ui, parent).obj();
    ui.obj_set_width(button, coord::pct(100));
    let label = Label::create(ui, button);
    ui.obj_set_align(label.obj(), Align::Center);
    label.set_text(ui, text);
}

/// `lv_example_scroll_chain()`: a nested scrollable box with chaining disabled.
pub fn lv_example_scroll_chain(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 12, sel);

    /* Outer scrollable panel */
    let container_1 = ui.obj_create(Some(screen));
    ui.obj_set_size(container_1, 240, 180);
    ui.obj_set_flex_flow(container_1, FlexFlow::Column);
    ui.obj_set_style_pad_row(container_1, 8, sel);

    row_button(ui, container_1, "Outer 1");

    /* Inner scrollable box; chaining to the parent disabled */
    let container_2 = ui.obj_create(Some(container_1));
    ui.obj_set_size(container_2, coord::pct(100), 90);
    ui.obj_set_flex_flow(container_2, FlexFlow::Column);
    ui.obj_set_style_pad_row(container_2, 6, sel);
    ui.obj_set_scroll_chain(container_2, false);
    ui.obj_set_style_border_color(container_2, Color::hex(0x9429ff), sel);
    ui.obj_set_style_border_width(container_2, 2, sel);

    for text in ["Inner 1", "Inner 2", "Inner 3", "Inner 4"] {
        row_button(ui, container_2, text);
    }
    for text in ["Outer 2", "Outer 3", "Outer 4"] {
        row_button(ui, container_1, text);
    }
}
