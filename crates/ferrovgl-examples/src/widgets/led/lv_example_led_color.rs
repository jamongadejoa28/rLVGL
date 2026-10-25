//! Port of `examples/widgets/led/lv_example_led_color.c`.

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{Color, StyleSelector, Ui};
use ferrovgl_widgets::Led;

use crate::util::screen_active;

/// `lv_example_led_color()`: red, green and blue LEDs.
pub fn lv_example_led_color(ui: &mut Ui) {
    let sel = StyleSelector::DEFAULT;
    let screen = screen_active(ui);
    ui.obj_set_flex_flow(screen, FlexFlow::Column);
    ui.obj_set_style_flex_main_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_cross_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_flex_track_place(screen, FlexAlign::Center, sel);
    ui.obj_set_style_pad_row(screen, 16, sel);

    /* Red LED, green LED, blue LED */
    for color in [0xff3030, 0x30c050, 0x3080ff] {
        let led = Led::create(ui, screen);
        ui.obj_set_size(led.obj(), 40, 40);
        led.set_color(ui, Color::hex(color));
    }
}
