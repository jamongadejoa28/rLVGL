//! Port of `examples/widgets/slider/lv_example_slider_modes.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::{Label, Slider, SliderMode};

use crate::util::screen_active;

/// `TEXT_COLOR`
const TEXT_COLOR: Color = Color::hex(0x013992);

/// `lv_example_slider_modes()`: normal, symmetrical and range sliders.
pub fn lv_example_slider_modes(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Normal mode */
    let slider_1 = Slider::create(ui, screen);
    ui.obj_set_size(slider_1.obj(), coord::pct(90), 20);
    slider_1.set_mode(ui, SliderMode::Normal);
    slider_1.set_min_value(ui, 0);
    slider_1.set_max_value(ui, 100);
    slider_1.set_value(ui, 70, ANIM_OFF);
    let label_1 = Label::create(ui, slider_1.obj());
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Normal");
    ui.obj_set_style_text_color(label_1.obj(), TEXT_COLOR, sel);

    /* Symmetrical mode around zero */
    let slider_2 = Slider::create(ui, screen);
    ui.obj_set_size(slider_2.obj(), coord::pct(90), 20);
    slider_2.set_mode(ui, SliderMode::Symmetrical);
    slider_2.set_min_value(ui, -100);
    slider_2.set_max_value(ui, 100);
    slider_2.set_value(ui, 45, ANIM_OFF);
    let label_2 = Label::create(ui, slider_2.obj());
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "Symmetrical");
    ui.obj_set_style_text_color(label_2.obj(), TEXT_COLOR, sel);

    /* Range mode with left and right values */
    let slider_3 = Slider::create(ui, screen);
    ui.obj_set_size(slider_3.obj(), coord::pct(90), 20);
    slider_3.set_mode(ui, SliderMode::Range);
    slider_3.set_min_value(ui, 0);
    slider_3.set_max_value(ui, 100);
    slider_3.set_start_value(ui, 25, ANIM_OFF);
    slider_3.set_value(ui, 80, ANIM_OFF);
    let label_3 = Label::create(ui, slider_3.obj());
    ui.obj_set_align(label_3.obj(), Align::Center);
    label_3.set_text(ui, "Range");
    ui.obj_set_style_text_color(label_3.obj(), TEXT_COLOR, sel);
}
