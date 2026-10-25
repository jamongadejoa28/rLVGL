//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_spinner.c`.
//!
//! Every upstream test function, in upstream order, with its values and
//! screenshot names. `setUp` creates a centered 100x100 spinner on the active
//! screen of a fresh default-theme display with the upstream
//! mouse/keypad/encoder test devices; `tearDown`'s `lv_obj_clean` is the
//! display drop. `setUp` and `tearDown` set no screen styles or flags, so no
//! state is inherited between the tests.
//!
//! The parity config compiles out the body of `test_spinner_properties`
//! (`LV_USE_OBJ_PROPERTY=0`); its port is empty in the same way.

use ferrovgl_core::Obj;
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::{Arc, Spinner};

/// The globals of `test_spinner.c` set by `setUp()` and the upstream test devices.
struct Fixture {
    display: TestDisplay,
    #[allow(dead_code)] // Installed like the upstream test harness; no test uses them.
    inputs: TestInputs,
    #[allow(dead_code)] // Only used by `tearDown()`, which is the display drop.
    active_screen: Obj,
    spinner: Spinner,
}

/// `setUp()`
fn set_up() -> Fixture {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    let active_screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let spinner = Spinner::create(ui, active_screen);
    ui.obj_set_size(spinner.obj(), 100, 100);
    ui.obj_center(spinner.obj());
    Fixture {
        display,
        inputs,
        active_screen,
        spinner,
    }
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn spinner_spinning() {
    let mut f = set_up();
    for i in 0..10 {
        // `lv_tick_inc(50); lv_timer_handler();`: one handler run per 50 ms
        // step, not `lv_test_wait`'s per-millisecond loop.
        f.display.ui.tick_inc(50);
        f.display.ui.timer_handler();

        let filename = format!("widgets/spinner_{i:02}.png");
        f.display.screenshot_compare(filename).unwrap();
    }
}

#[test]
fn spinner_properties() {
    // `#if LV_USE_OBJ_PROPERTY`: disabled in the parity configuration.
}

// The checks below are not in `test_spinner.c`; they cover the public API that
// the upstream file only reaches through the compiled-out property test.

#[test]
fn spinner_defaults() {
    let f = set_up();
    let ui = &f.display.ui;
    let spinner = f.spinner;

    assert_eq!(spinner.anim_duration(ui), 1000);
    assert_eq!(spinner.arc_sweep(ui), 200);
    assert!(!ui.obj_has_flag(spinner.obj(), ferrovgl_core::ObjFlags::CLICKABLE));

    let arc = Arc::from_obj(ui, spinner.obj()).unwrap();
    assert_eq!(arc.rotation(ui), 270);
    assert_eq!(arc.bg_angle_start(ui), 0 as ferrovgl_core::ValuePrecise);
    assert_eq!(arc.bg_angle_end(ui), 360 as ferrovgl_core::ValuePrecise);
    // Early apply of the first animation value: `arc_anim_angles(obj, 0)`.
    assert_eq!(arc.angle_start(ui), 0 as ferrovgl_core::ValuePrecise);
    assert_eq!(arc.angle_end(ui), 200 as ferrovgl_core::ValuePrecise);
}

#[test]
fn spinner_set_anim_params() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let spinner = f.spinner;

    spinner.set_anim_params(ui, 2000, 270);
    assert_eq!(spinner.anim_duration(ui), 2000);
    assert_eq!(spinner.arc_sweep(ui), 270);

    spinner.set_anim_duration(ui, 1500);
    assert_eq!(spinner.anim_duration(ui), 1500);
    assert_eq!(spinner.arc_sweep(ui), 270);

    spinner.set_arc_sweep(ui, 180);
    assert_eq!(spinner.anim_duration(ui), 1500);
    assert_eq!(spinner.arc_sweep(ui), 180);

    // `lv_anim_delete(obj, NULL)` replaced the previous animations: one remains.
    assert_eq!(ui.anim_count_running(), 1);

    let obj = spinner.obj();
    ui.obj_delete(obj);
    assert_eq!(ui.anim_count_running(), 0);
    assert_eq!(spinner.anim_duration(ui), 0);
    assert_eq!(spinner.arc_sweep(ui), 0);
}
