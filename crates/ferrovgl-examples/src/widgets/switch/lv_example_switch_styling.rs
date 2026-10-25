//! Port of `examples/widgets/switch/lv_example_switch_styling.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, Part, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::Switch;

use crate::util::{screen_active, styles_once};

/// `style_switch_main`, `style_switch_indicator_checked`, `style_switch_knob`.
struct Styles;

/// `lv_example_switch_styling()`: styled track, checked indicator and knob.
pub fn lv_example_switch_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [
        style_switch_main,
        style_switch_indicator_checked,
        style_switch_knob,
    ] = styles_once::<Styles, 3>(ui, || {
        let mut style_switch_main = Style::new();
        style_switch_main.set_bg_color(Color::hex(0xc4d8cb));
        style_switch_main.set_bg_opa((255 * 100 / 100) as u8);
        style_switch_main.set_radius(999);
        style_switch_main.set_pad_all(6);
        style_switch_main.set_border_width(0);

        let mut style_switch_indicator_checked = Style::new();
        style_switch_indicator_checked.set_bg_color(Color::hex(0x22c55e));
        style_switch_indicator_checked.set_bg_opa((255 * 100 / 100) as u8);

        let mut style_switch_knob = Style::new();
        style_switch_knob.set_bg_color(Color::hex(0xffffff));
        style_switch_knob.set_bg_opa((255 * 100 / 100) as u8);
        style_switch_knob.set_pad_all(2);
        style_switch_knob.set_border_color(Color::hex(0xd1d5db));
        style_switch_knob.set_border_width(1);
        style_switch_knob.set_shadow_color(Color::hex(0x000000));
        style_switch_knob.set_shadow_opa((255 * 40 / 100) as u8);
        style_switch_knob.set_shadow_width(16);
        style_switch_knob.set_shadow_offset_y(2);
        [
            style_switch_main,
            style_switch_indicator_checked,
            style_switch_knob,
        ]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    for checked in [false, true] {
        let sw = Switch::create(ui, screen).obj();
        ui.obj_set_size(sw, 60, 30);
        if checked {
            ui.obj_set_state(sw, State::CHECKED, true);
        }
        ui.obj_add_style(sw, style_switch_main, Part::MAIN.into());
        ui.obj_add_style(
            sw,
            style_switch_indicator_checked,
            Part::INDICATOR | State::CHECKED,
        );
        ui.obj_add_style(sw, style_switch_knob, Part::KNOB.into());
    }
}
