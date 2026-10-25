//! Port of `examples/widgets/button/lv_example_button_states.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Align, State, StyleSelector, Ui};
use ferrovgl_widgets::{Button, Label};

use crate::util::screen_active;

/// `lv_example_button_states()`: normal, checked and disabled buttons.
pub fn lv_example_button_states(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Default button */
    let button_1 = Button::create(ui, screen).obj();
    ui.obj_set_size(button_1, 160, 40);
    let label_1 = Label::create(ui, button_1);
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Normal");

    /* Checkable, starts checked */
    let button_2 = Button::create(ui, screen).obj();
    ui.obj_set_size(button_2, 160, 40);
    ui.obj_set_checkable(button_2, true);
    ui.obj_set_state(button_2, State::CHECKED, true);
    let label_2 = Label::create(ui, button_2);
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Checked");

    /* Disabled */
    let button_3 = Button::create(ui, screen).obj();
    ui.obj_set_size(button_3, 160, 40);
    ui.obj_set_state(button_3, State::DISABLED, true);
    let label_3 = Label::create(ui, button_3);
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "Disabled");
}
