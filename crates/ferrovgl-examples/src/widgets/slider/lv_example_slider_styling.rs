//! Port of `examples/widgets/slider/lv_example_slider_styling.c`.

use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Color, Part, State, Style, StyleSelector, Ui};
use ferrovgl_widgets::Slider;

use crate::util::{screen_active, styles_once};

/// `style_slider_track`, `style_slider_indicator`, `style_slider_knob`, `style_slider_knob_pressed`.
struct Styles;

/// `lv_example_slider_styling()`: styled track, indicator and knob with a pressed knob style.
pub fn lv_example_slider_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let [
        style_slider_track,
        style_slider_indicator,
        style_slider_knob,
        style_slider_knob_pressed,
    ] = styles_once::<Styles, 4>(ui, || {
        let mut style_slider_track = Style::new();
        style_slider_track.set_bg_opa((255 * 100 / 100) as u8);
        style_slider_track.set_bg_color(Color::hex(0x1f2937));
        style_slider_track.set_radius(12);
        style_slider_track.set_border_color(Color::hex(0x0f172a));
        style_slider_track.set_border_width(1);

        let mut style_slider_indicator = Style::new();
        style_slider_indicator.set_bg_opa((255 * 100 / 100) as u8);
        style_slider_indicator.set_bg_color(Color::hex(0x8b5cf6));
        style_slider_indicator.set_bg_grad_color(Color::hex(0xec4899));
        style_slider_indicator.set_bg_grad_dir(GradDir::Hor);
        style_slider_indicator.set_radius(12);
        style_slider_indicator.set_shadow_color(Color::hex(0x6d28d9));
        style_slider_indicator.set_shadow_width(12);
        style_slider_indicator.set_shadow_spread(-1);
        style_slider_indicator.set_shadow_opa(150);

        let mut style_slider_knob = Style::new();
        style_slider_knob.set_bg_opa((255 * 100 / 100) as u8);
        style_slider_knob.set_bg_color(Color::hex(0xffffff));
        style_slider_knob.set_radius(100);
        style_slider_knob.set_border_color(Color::hex(0x8b5cf6));
        style_slider_knob.set_border_width(3);
        style_slider_knob.set_pad_all(6);
        style_slider_knob.set_shadow_color(Color::hex(0x000000));
        style_slider_knob.set_shadow_width(8);
        style_slider_knob.set_shadow_offset_y(2);
        style_slider_knob.set_shadow_opa(80);

        let mut style_slider_knob_pressed = Style::new();
        style_slider_knob_pressed.set_bg_color(Color::hex(0xfff7ed));
        style_slider_knob_pressed.set_outline_color(Color::hex(0xec4899));
        style_slider_knob_pressed.set_outline_width(6);
        style_slider_knob_pressed.set_outline_pad(4);
        style_slider_knob_pressed.set_outline_opa(120);
        [
            style_slider_track,
            style_slider_indicator,
            style_slider_knob,
            style_slider_knob_pressed,
        ]
    });

    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 20, sel);

    let slider = Slider::create(ui, screen);
    let obj = slider.obj();
    ui.obj_set_size(obj, coord::pct(90), 22);
    slider.set_min_value(ui, 0);
    slider.set_max_value(ui, 100);
    slider.set_value(ui, 55, ANIM_OFF);
    ui.obj_add_style(obj, style_slider_track, Part::MAIN.into());
    ui.obj_add_style(obj, style_slider_indicator, Part::INDICATOR.into());
    ui.obj_add_style(obj, style_slider_knob, Part::KNOB.into());
    ui.obj_add_style(obj, style_slider_knob_pressed, Part::KNOB | State::PRESSED);
}
