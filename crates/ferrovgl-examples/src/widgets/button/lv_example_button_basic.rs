//! Port of `examples/widgets/button/lv_example_button_basic.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_button_basic()`: a content-sized and a fixed-size button.
pub fn lv_example_button_basic(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Default size driven by label content */
    let button_1 = Button::create(ui, screen).obj();
    let label_1 = Label::create(ui, button_1);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Click me");

    /* Explicit width and height */
    let button_2 = Button::create(ui, screen).obj();
    ui.obj_set_size(button_2, 160, 48);
    let label_2 = Label::create(ui, button_2);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Wide button");
}
