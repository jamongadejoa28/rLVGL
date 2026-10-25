//! Port of `examples/widgets/checkbox/lv_example_checkbox_states.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{State, StyleSelector, Ui};
use ferrovgl_widgets::Checkbox;

use crate::util::screen_active;

/// `lv_example_checkbox_states()`: unchecked, checked and disabled checkboxes.
pub fn lv_example_checkbox_states(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Default unchecked */
    let checkbox_1 = Checkbox::create(ui, screen);
    checkbox_1.set_text(ui, "Unchecked");

    /* Checked */
    let checkbox_2 = Checkbox::create(ui, screen);
    checkbox_2.set_text(ui, "Checked");
    ui.obj_set_state(checkbox_2.obj(), State::CHECKED, true);

    /* Disabled */
    let checkbox_3 = Checkbox::create(ui, screen);
    checkbox_3.set_text(ui, "Disabled");
    ui.obj_set_state(checkbox_3.obj(), State::DISABLED, true);

    /* Disabled and Checked */
    let checkbox_4 = Checkbox::create(ui, screen);
    checkbox_4.set_text(ui, "Disabled + Checked");
    ui.obj_set_state(checkbox_4.obj(), State::DISABLED, true);
    ui.obj_set_state(checkbox_4.obj(), State::CHECKED, true);
}
