//! Port of `examples/layouts/flex/lv_example_flex_ignore_layout.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_flex_ignore_layout()`: an item that flex skips.
pub fn lv_example_flex_ignore_layout(ui: &mut Ui) {
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, StyleSelector::DEFAULT);

    /* Demo row mixing normal and ignore_layout items.
    The red item uses ignore_layout="true", so Flex skips it;
    it keeps its manual x/y position and the other items are laid out normally. */
    let container = ui.obj_create(Some(screen));
    ui.obj_set_flex_flow(container, FlexFlow::Row);
    ui.obj_set_size(container, coord::pct(90), 150);

    let button_1 = Button::create(ui, container).obj();
    ui.obj_set_height(button_1, coord::SIZE_CONTENT);
    let label_1 = Label::create(ui, button_1);
    label_1.set_text(ui, "Item A");
    ui.obj_set_align(label_1.obj(), Align::Center);

    /* This item is invisible to flex. It centers itself by its own
    align and does not occupy a flex slot. */
    let button_2 = Button::create(ui, container).obj();
    ui.obj_set_align(button_2, Align::Center);
    ui.obj_set_ignore_layout(button_2, true);
    ui.obj_set_style_bg_color(button_2, Color::hex(0xe74c3c), StyleSelector::DEFAULT);
    let label_2 = Label::create(ui, button_2);
    label_2.set_text(ui, "I'm Free!");
    ui.obj_set_align(label_2.obj(), Align::Center);

    let button_3 = Button::create(ui, container).obj();
    let label_3 = Label::create(ui, button_3);
    label_3.set_text(ui, "Item B");
    ui.obj_set_align(label_3.obj(), Align::Center);

    let button_4 = Button::create(ui, container).obj();
    ui.obj_set_height(button_4, coord::SIZE_CONTENT);
    let label_4 = Label::create(ui, button_4);
    label_4.set_text(ui, "Item C");
    ui.obj_set_align(label_4.obj(), Align::Center);
}
