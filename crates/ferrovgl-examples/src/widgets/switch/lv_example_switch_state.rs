//! Port of `examples/widgets/switch/lv_example_switch_state.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{State, StyleSelector, Ui};
use ferrovgl_widgets::Switch;

use crate::util::screen_active;

/// `lv_example_switch_state()`: off, on, disabled and disabled + on switches.
pub fn lv_example_switch_state(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    Switch::create(ui, screen);

    let sw_2 = Switch::create(ui, screen);
    ui.obj_set_state(sw_2.obj(), State::CHECKED, true);

    let sw_3 = Switch::create(ui, screen);
    ui.obj_set_state(sw_3.obj(), State::DISABLED, true);

    let sw_4 = Switch::create(ui, screen);
    ui.obj_set_state(sw_4.obj(), State::DISABLED, true);
    ui.obj_set_state(sw_4.obj(), State::CHECKED, true);
}
