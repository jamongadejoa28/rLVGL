//! Port of `examples/widgets/led/lv_example_led_brightness.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, StyleSelector, Ui};
use ferrovgl_widgets::Led;

use crate::util::screen_active;

/// `lv_example_led_brightness()`: dim, mid and full brightness LEDs.
pub fn lv_example_led_brightness(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Dim, mid, full */
    for brightness in [60, 150, 255] {
        let led = Led::create(ui, screen);
        ui.obj_set_size(led.obj(), 40, 40);
        led.set_color(ui, Color::hex(0xff8000));
        led.set_brightness(ui, brightness);
    }
}
