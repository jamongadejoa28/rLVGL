//! Port of `examples/widgets/led/lv_example_led_styling.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, StyleSelector, Ui};
use ferrovgl_widgets::Led;

use crate::util::screen_active;

/// `lv_example_led_styling()`: LEDs restyled with local border, outline and shadow.
pub fn lv_example_led_styling(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Row);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::SpaceEvenly, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);

    /* Round LED with a tight ring outline and a soft outer glow */
    let led_1 = Led::create(ui, screen);
    let obj = led_1.obj();
    ui.obj_set_size(obj, 50, 50);
    led_1.set_color(ui, Color::hex(0x22c55e));
    ui.obj_set_style_border_color(obj, Color::hex(0x14532d), sel);
    ui.obj_set_style_border_width(obj, 2, sel);
    ui.obj_set_style_outline_color(obj, Color::hex(0x22c55e), sel);
    ui.obj_set_style_outline_width(obj, 6, sel);
    ui.obj_set_style_outline_pad(obj, 4, sel);
    ui.obj_set_style_outline_opa(obj, 80, sel);
    ui.obj_set_style_shadow_color(obj, Color::hex(0x22c55e), sel);
    ui.obj_set_style_shadow_width(obj, 20, sel);
    ui.obj_set_style_shadow_opa(obj, 200, sel);

    /* Square LED via small radius + hard border, no shadow */
    let led_2 = Led::create(ui, screen);
    let obj = led_2.obj();
    ui.obj_set_size(obj, 50, 50);
    led_2.set_color(ui, Color::hex(0xef4444));
    ui.obj_set_style_radius(obj, 6, sel);
    ui.obj_set_style_border_color(obj, Color::hex(0x7f1d1d), sel);
    ui.obj_set_style_border_width(obj, 3, sel);
    ui.obj_set_style_shadow_width(obj, 0, sel);

    /* Dimmed LED with a heavy outline ring as a "pending" indicator */
    let led_3 = Led::create(ui, screen);
    let obj = led_3.obj();
    ui.obj_set_size(obj, 50, 50);
    led_3.set_color(ui, Color::hex(0xf59e0b));
    led_3.set_brightness(ui, 120);
    ui.obj_set_style_outline_color(obj, Color::hex(0xf59e0b), sel);
    ui.obj_set_style_outline_width(obj, 3, sel);
    ui.obj_set_style_outline_pad(obj, 6, sel);
    ui.obj_set_style_outline_opa(obj, 180, sel);
    ui.obj_set_style_shadow_width(obj, 0, sel);
}
