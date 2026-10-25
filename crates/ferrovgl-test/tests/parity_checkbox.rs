//! Ports of LVGL v9.6.0 `tests/src/test_cases/widgets/test_checkbox.c`.
//! Run with `--include-ignored` after `cargo xtask refimgs sync`. References
//! are immutable upstream PNGs; actual pixels come from the Rust object pipeline.
//!
//! `test_checkbox_rtl` (`widgets/checkbox_rtl_1.png`) is not ported as a
//! runnable test: it needs `lv_font_dejavu_16_persian_hebrew` and the BiDi
//! paragraph algorithm (`LV_USE_BIDI`), neither of which exists in the core
//! yet. The scene is kept below as documentation for when they land.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::misc::color::OPA_0;
use ferrovgl_core::{EventCode, Part, State, StyleSelector};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::Checkbox;

/// `setUp()` is empty; `tearDown()` cleans the active screen, which a fresh
/// display per test provides.
fn scene() -> TestDisplay {
    TestDisplay::with_default_theme().unwrap()
}

/// `test_checkbox_creation_successful`
#[test]
fn checkbox_creation_successful() {
    let mut display = scene();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);
    assert!(display.ui.obj_is_valid(checkbox.obj()));
    assert_eq!(Checkbox::from_obj(&display.ui, checkbox.obj()), Some(checkbox));
}

/// `test_checkbox_should_call_event_handler_on_click_when_enabled`
#[test]
fn checkbox_should_call_event_handler_on_click_when_enabled() {
    let mut display = scene();
    let inputs = display.create_inputs();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);

    display.ui.obj_add_state(checkbox.obj(), State::CHECKED);
    let event_called = Rc::new(Cell::new(false));
    let flag = event_called.clone();
    display.ui.obj_add_event_cb(
        checkbox.obj(),
        Rc::new(move |_, event| {
            if event.code == EventCode::VALUE_CHANGED {
                flag.set(true);
            }
        }),
        EventCode::ALL,
        None,
    );

    let coords = display.ui.obj_get_coords(checkbox.obj());
    inputs.mouse_click_at(&mut display, coords.x1, coords.y1);

    assert!(event_called.get());
    // The click toggled the checkable object off.
    assert!(!display.ui.obj_has_state(checkbox.obj(), State::CHECKED));
}

/// `test_checkbox_should_have_default_text_when_created`
#[test]
fn checkbox_should_have_default_text_when_created() {
    let default_text = "Check box";
    let mut display = scene();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);

    assert_eq!(checkbox.text(&display.ui), default_text);
}

/// `test_checkbox_should_return_dynamically_allocated_text`
#[test]
fn checkbox_should_return_dynamically_allocated_text() {
    let message = "Hello World!";
    let mut display = scene();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);

    checkbox.set_text(&mut display.ui, message);

    assert_eq!(checkbox.text(&display.ui), message);
    // The text was copied, not borrowed.
    assert_ne!(checkbox.text(&display.ui).as_ptr(), message.as_ptr());
}

/// `test_checkbox_should_allocate_memory_for_static_text`. Upstream compares
/// `lv_mem_monitor()` free sizes, which have no counterpart in Rust's
/// allocator; the observable equivalent is that the static text is used in
/// place instead of copied.
#[test]
fn checkbox_should_allocate_memory_for_static_text() {
    static STATIC_TEXT: &str = "Keep me while you exist";
    let mut display = scene();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);

    checkbox.set_text_static(&mut display.ui, STATIC_TEXT);

    assert_eq!(checkbox.text(&display.ui).as_ptr(), STATIC_TEXT.as_ptr());
    assert_eq!(checkbox.text(&display.ui), STATIC_TEXT);
}

/// `test_checkbox_style_opa`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn checkbox_style_opa() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let obj = Checkbox::create(&mut display.ui, screen).obj();
    display
        .ui
        .obj_set_style_opa(obj, OPA_0, StyleSelector::from(Part::INDICATOR));
    display.ui.obj_center(obj);

    display.screenshot_compare("widgets/checkbox_1.png").unwrap();
}

/// `test_checkbox_properties`: its body is compiled only with
/// `LV_USE_OBJ_PROPERTY`, which the parity configuration (`full.defconfig`)
/// disables, so upstream runs it as an empty test. The setter/getter pair it
/// would check through the property table is exercised directly here.
#[test]
fn checkbox_properties() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let obj = Checkbox::create(&mut display.ui, screen);
    obj.set_text(&mut display.ui, "Test Checkbox");
    assert_eq!(obj.text(&display.ui), "Test Checkbox");
    display.ui.obj_delete(obj.obj());
}

// `test_checkbox_rtl` (not runnable yet, see the module documentation):
//
//     let message = "מעבד, או בשמו המלא יחידת עיבוד מרכזית (באנגלית: CPU - Central Processing Unit).";
//     let screen = ui.obj_create(active_screen);
//     ui.obj_remove_style_all(screen);
//     ui.obj_set_size(screen, 800, 480);
//     ui.obj_center(screen);
//     ui.obj_set_style_bg_color(screen, Color::WHITE, 0);
//     ui.obj_set_style_bg_opa(screen, OPA_100, 0);
//     ui.obj_set_style_pad_all(screen, 0, 0);
//     let test_checkbox = Checkbox::create(ui, active_screen);
//     test_checkbox.set_text(ui, message);
//     ui.obj_set_style_text_font(test_checkbox, &lv_font_dejavu_16_persian_hebrew, 0);
//     ui.obj_center(test_checkbox);
//     ui.obj_set_style_base_dir(test_checkbox, BaseDir::Rtl, 0);
//     screenshot_compare("widgets/checkbox_rtl_1.png");
