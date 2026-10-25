//! Port of `examples/widgets/slider/lv_example_slider_value_range_orientation.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::text::TextAlign;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, Color, StyleSelector, Ui};
use ferrovgl_widgets::{Label, Slider, SliderOrientation};

use crate::util::screen_active;

/// `TEXT_COLOR`
const TEXT_COLOR: Color = Color::hex(0x013992);

/// `lv_example_slider_value_range_orientation()`: a custom range and a vertical slider.
pub fn lv_example_slider_value_range_orientation(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Horizontal slider with custom range */
    let slider_1 = Slider::create(ui, screen);
    ui.obj_set_size(slider_1.obj(), coord::pct(90), 20);
    slider_1.set_min_value(ui, -20);
    slider_1.set_max_value(ui, 80);
    slider_1.set_value(ui, 35, ANIM_OFF);
    let label_1 = Label::create(ui, slider_1.obj());
    ui.obj_set_align(label_1.obj(), Align::Center);
    label_1.set_text(ui, "Horizontal");
    ui.obj_set_style_text_color(label_1.obj(), TEXT_COLOR, sel);

    /* Vertical slider with explicit vertical orientation */
    let slider_2 = Slider::create(ui, screen);
    ui.obj_set_size(slider_2.obj(), 20, 130);
    slider_2.set_orientation(ui, SliderOrientation::Vertical);
    slider_2.set_min_value(ui, 0);
    slider_2.set_max_value(ui, 100);
    slider_2.set_value(ui, 60, ANIM_OFF);
    let label_2 = Label::create(ui, slider_2.obj());
    ui.obj_set_align(label_2.obj(), Align::Center);
    label_2.set_text(ui, "V\ne\nr");
    ui.obj_set_style_text_color(label_2.obj(), TEXT_COLOR, sel);
    ui.obj_set_style_text_align(label_2.obj(), TextAlign::Center, sel);
}
