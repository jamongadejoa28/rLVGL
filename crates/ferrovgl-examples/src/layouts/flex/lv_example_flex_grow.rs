//! Port of `examples/layouts/flex/lv_example_flex_grow.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_flex_grow()`: fixed and growing items in a row.
pub fn lv_example_flex_grow(ui: &mut Ui) {
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);

    /* Demo row for grow behavior */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(container, FlexFlow::Row);
    ui.obj_set_size(container, coord::pct(90), coord::SIZE_CONTENT);

    /* Fixed-width item on the left */
    let button_1 = Button::create(ui, container).obj();
    ui.obj_set_size(button_1, 50, coord::SIZE_CONTENT);
    let label_1 = Label::create(ui, button_1);
    label_1.set_text(ui, "Fix");
    ui.obj_set_align(label_1.obj(), Align::Center);

    /* Grows with weight 1 */
    let button_2 = Button::create(ui, container).obj();
    ui.obj_set_height(button_2, coord::SIZE_CONTENT);
    ui.obj_set_flex_grow(button_2, 1);
    let label_2 = Label::create(ui, button_2);
    label_2.set_text(ui, "Grow x1");
    ui.obj_set_align(label_2.obj(), Align::Center);

    /* Grows with weight 2 — receives twice as much free space */
    let button_3 = Button::create(ui, container).obj();
    ui.obj_set_height(button_3, coord::SIZE_CONTENT);
    ui.obj_set_flex_grow(button_3, 2);
    let label_3 = Label::create(ui, button_3);
    label_3.set_text(ui, "Grow x2");
    ui.obj_set_align(label_3.obj(), Align::Center);
}
