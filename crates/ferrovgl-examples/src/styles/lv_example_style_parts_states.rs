//! Port of `examples/styles/lv_example_style_parts_states.c`.

use ferrovgl_core::draw::grad::GradDir;
use ferrovgl_core::misc::anim::ANIM_OFF;
use ferrovgl_core::{Align, Color, Part, State, Style, Ui};
use ferrovgl_widgets::Slider;

use crate::util::{screen_active, styles_once};

/// `style_indicator`, `style_indicator_pressed`.
struct Styles;

/// `lv_example_style_parts_states()`: styles for a part and a part + state selector.
pub fn lv_example_style_parts_states(ui: &mut Ui) {
    let [style_indicator, style_indicator_pressed] = styles_once::<Styles, 2>(ui, || {
        let mut style_indicator = Style::new();
        style_indicator.set_bg_opa((255 * 100 / 100) as u8);
        style_indicator.set_bg_color(Color::hex(0x8b5cf6));
        style_indicator.set_bg_grad_color(Color::hex(0xec4899));
        style_indicator.set_bg_grad_dir(GradDir::Hor);
        style_indicator.set_radius(10);

        let mut style_indicator_pressed = Style::new();
        style_indicator_pressed.set_shadow_color(Color::hex(0xec4899));
        style_indicator_pressed.set_shadow_width(18);
        style_indicator_pressed.set_shadow_spread(2);
        [style_indicator, style_indicator_pressed]
    });

    let screen = screen_active(ui);

    let slider = Slider::create(ui, screen);
    ui.obj_set_align(slider.obj(), Align::Center);
    ui.obj_set_size(slider.obj(), 220, 16);
    slider.set_min_value(ui, 0);
    slider.set_max_value(ui, 100);
    slider.set_value(ui, 65, ANIM_OFF);
    ui.obj_add_style(slider.obj(), style_indicator, Part::INDICATOR.into());
    ui.obj_add_style(
        slider.obj(),
        style_indicator_pressed,
        Part::INDICATOR | State::PRESSED,
    );
}
