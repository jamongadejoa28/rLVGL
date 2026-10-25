//! Port of `examples/widgets/switch/lv_example_switch_orientation.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{State, StyleSelector, Ui};
use ferrovgl_widgets::{Switch, SwitchOrientation};

use crate::util::screen_active;

/// `lv_example_switch_orientation()`: a horizontal and a vertical switch.
pub fn lv_example_switch_orientation(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    let sw_1 = Switch::create(ui, screen);
    ui.obj_set_size(sw_1.obj(), 60, 30);
    sw_1.set_orientation(ui, SwitchOrientation::Horizontal);
    ui.obj_set_state(sw_1.obj(), State::CHECKED, true);

    let sw_2 = Switch::create(ui, screen);
    ui.obj_set_size(sw_2.obj(), 30, 60);
    sw_2.set_orientation(ui, SwitchOrientation::Vertical);
    ui.obj_set_state(sw_2.obj(), State::CHECKED, true);
}
