//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_led.c`.
//!
//! Every upstream test function, in upstream order, with its values and
//! screenshot names. `setUp` takes the active screen of a fresh default-theme
//! display; `tearDown`'s `lv_obj_clean` is the display drop. No upstream test
//! changes the screen itself, so nothing is inherited between tests.

use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::{Align, Obj};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{LED_BRIGHT_MAX, LED_BRIGHT_MIN, Led};

/// `setUp()`: `active_screen = lv_screen_active();`
fn set_up() -> (TestDisplay, Obj) {
    let display = TestDisplay::with_default_theme().unwrap();
    let active_screen = display.screen().unwrap();
    (display, active_screen)
}

/// `test_led_render_1`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn led_render_1() {
    let (mut display, active_screen) = set_up();
    let ui = &mut display.ui;

    let led = Led::create(ui, active_screen);
    ui.obj_align(led.obj(), Align::Center, -80, 0);
    led.off(ui);

    let led = Led::create(ui, active_screen);
    ui.obj_align(led.obj(), Align::Center, 0, 0);
    led.set_brightness(ui, 150);
    led.set_color(ui, palette::main(Palette::Red));

    let led = Led::create(ui, active_screen);
    ui.obj_align(led.obj(), Align::Center, 80, 0);
    led.on(ui);

    display.screenshot_compare("widgets/led_1.png").unwrap();
}

/// `test_led_set_color_works`. Upstream reads `((lv_led_t *)led)->color`; the
/// port reads the same field through `lv_led_get_color`.
#[test]
fn led_set_color_works() {
    let (mut display, active_screen) = set_up();
    let ui = &mut display.ui;
    let led = Led::create(ui, active_screen);

    /* Test by changing colors randomly. */
    led.set_color(ui, palette::main(Palette::Amber));
    assert_eq!(palette::main(Palette::Amber), led.color(ui));

    led.set_color(ui, palette::main(Palette::Grey));
    assert_eq!(palette::main(Palette::Grey), led.color(ui));

    led.set_color(ui, palette::main(Palette::Red));
    assert_eq!(palette::main(Palette::Red), led.color(ui));
}

/// `test_led_set_brightness_works`
#[test]
fn led_set_brightness_works() {
    let (mut display, active_screen) = set_up();
    let ui = &mut display.ui;
    let led = Led::create(ui, active_screen);

    /* Test by changing brightness randomly. */
    led.set_brightness(ui, 150);
    assert_eq!(150, led.brightness(ui));

    led.set_brightness(ui, 10);
    assert_eq!(LED_BRIGHT_MIN, led.brightness(ui));

    led.set_brightness(ui, 255);
    assert_eq!(255, led.brightness(ui));
}

/// `test_led_on_off_works`
#[test]
fn led_on_off_works() {
    let (mut display, active_screen) = set_up();
    let ui = &mut display.ui;
    let led = Led::create(ui, active_screen);

    led.on(ui);
    assert_eq!(LED_BRIGHT_MAX, led.brightness(ui));

    led.off(ui);
    assert_eq!(LED_BRIGHT_MIN, led.brightness(ui));

    led.on(ui);
    assert_eq!(LED_BRIGHT_MAX, led.brightness(ui));
}

/// `test_led_toggle_works`
#[test]
fn led_toggle_works() {
    let (mut display, active_screen) = set_up();
    let ui = &mut display.ui;
    let led = Led::create(ui, active_screen);

    led.on(ui);
    led.toggle(ui);
    assert_eq!(LED_BRIGHT_MIN, led.brightness(ui));
    led.toggle(ui);
    assert_eq!(LED_BRIGHT_MAX, led.brightness(ui));
    led.toggle(ui);
    assert_eq!(LED_BRIGHT_MIN, led.brightness(ui));
}

/// `test_led_get_brightness_works`
#[test]
fn led_get_brightness_works() {
    let (mut display, active_screen) = set_up();
    let ui = &mut display.ui;
    let led = Led::create(ui, active_screen);

    led.on(ui);
    assert_eq!(LED_BRIGHT_MAX, led.brightness(ui));
    led.off(ui);
    assert_eq!(LED_BRIGHT_MIN, led.brightness(ui));
    led.set_brightness(ui, 150);
    assert_eq!(150, led.brightness(ui));
}

// `test_led_properties` is compiled out: `LV_USE_OBJ_PROPERTY` is disabled in
// the parity configuration (`tests/configs/full.defconfig`), so its body is
// empty upstream.
