//! Port of `examples/widgets/slider/lv_example_slider_reversed_direction.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::{Label, Slider};

use crate::util::screen_active;

/// `TEXT_COLOR`
const TEXT_COLOR: Color = Color::hex(0x013992);

/// `lv_example_slider_reversed_direction()`: a forward and a reversed (`min > max`) slider.
pub fn lv_example_slider_reversed_direction(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Forward direction: range 0..100, then reverse direction: range 100..0 */
    for (min, max, text) in [(0, 100, "0..100"), (100, 0, "100..0")] {
        let slider = Slider::create(ui, screen);
        ui.obj_set_size(slider.obj(), coord::pct(90), 20);
        slider.set_min_value(ui, min);
        slider.set_max_value(ui, max);
        slider.set_value(ui, 30, ANIM_OFF);
        let label = Label::create(ui, slider.obj());
        ui.obj_set_align(label.obj(), Align::Center);
        label.set_text(ui, text);
        ui.obj_set_style_text_color(label.obj(), TEXT_COLOR, sel);
    }
}
