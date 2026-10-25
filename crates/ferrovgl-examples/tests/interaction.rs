//! Event logic of the interactive examples, driven with the synthetic mouse
//! of `ferrovgl-test` like the upstream `lv_test_mouse_click_at()` tests.
//! The expected texts and states follow the C event callbacks.

mod common;

use ferrovgl_core::{Obj, ObjFlags, State};
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::{Bar, Label};

/// Build the example, install the input devices and render the frames.
fn setup(name: &str) -> (TestDisplay, TestInputs) {
    let example = ferrovgl_examples::find(name).expect("example in EXAMPLES");
    let mut display = common::build(example);
    let inputs = display.create_inputs();
    common::render_frames(&mut display);
    (display, inputs)
}

/// Click the center of `obj`.
fn click(display: &mut TestDisplay, inputs: &TestInputs, obj: Obj) {
    display.ui.obj_update_layout(obj);
    let a = display.ui.obj_get_coords(obj);
    let (x, y) = (a.x1 + a.width() / 2, a.y1 + a.height() / 2);
    inputs.mouse_click_at(display, x, y);
}

/// The object showing `text` under the active screen.
fn by_text(display: &TestDisplay, text: &str) -> Obj {
    let screen = display.screen().expect("active screen");
    common::find_text(&display.ui, screen, text).unwrap_or_else(|| panic!("no {text:?}"))
}

/// The first object of class `class` under the active screen.
fn by_class(display: &TestDisplay, class: &str) -> Obj {
    let screen = display.screen().expect("active screen");
    common::find_class(&display.ui, screen, class).unwrap_or_else(|| panic!("no {class}"))
}

fn label_text(display: &TestDisplay, obj: Obj) -> String {
    Label::from_obj(&display.ui, obj)
        .expect("label")
        .text(&display.ui)
        .to_owned()
}

#[test]
fn get_started_button_counts_clicks_in_its_label() {
    let (mut d, inputs) = setup("lv_example_get_started_button");
    let label = by_text(&d, "Button");
    let button = d.ui.obj_get_parent(label).expect("button");
    click(&mut d, &inputs, button);
    assert_eq!(label_text(&d, label), "Button: 1");
    click(&mut d, &inputs, button);
    assert_eq!(label_text(&d, label), "Button: 2");
}

#[test]
fn event_click_numbers_the_clicks() {
    let (mut d, inputs) = setup("lv_example_event_click");
    let label = by_text(&d, "Click me!");
    let button = d.ui.obj_get_parent(label).expect("button");
    click(&mut d, &inputs, button);
    assert_eq!(label_text(&d, label), "1");
    click(&mut d, &inputs, button);
    assert_eq!(label_text(&d, label), "2");
}

#[test]
fn button_event_updates_the_status_label() {
    let (mut d, inputs) = setup("lv_example_button_event");
    let status = by_text(&d, "Clicks: 0");
    let button = by_class(&d, "lv_button");
    click(&mut d, &inputs, button);
    assert_eq!(label_text(&d, status), "Clicks: 1");
}

#[test]
fn checkbox_event_reports_the_checked_state() {
    let (mut d, inputs) = setup("lv_example_checkbox_event");
    let status = by_text(&d, "Feature: OFF");
    let cb = by_class(&d, "lv_checkbox");
    click(&mut d, &inputs, cb);
    assert!(d.ui.obj_has_state(cb, State::CHECKED));
    assert_eq!(label_text(&d, status), "Feature: ON");
    click(&mut d, &inputs, cb);
    assert_eq!(label_text(&d, status), "Feature: OFF");
}

#[test]
fn switch_event_reports_the_state() {
    let (mut d, inputs) = setup("lv_example_switch_event");
    let status = by_text(&d, "State: OFF");
    let sw = by_class(&d, "lv_switch");
    click(&mut d, &inputs, sw);
    assert_eq!(label_text(&d, status), "State: ON");
}

#[test]
fn checkbox_bind_checked_shows_the_label_while_ticked() {
    let (mut d, inputs) = setup("lv_example_checkbox_bind_checked");
    let label = by_text(&d, "Extra details only visible while the box is ticked.");
    assert!(d.ui.obj_has_flag(label, ObjFlags::HIDDEN));
    let cb = by_class(&d, "lv_checkbox");
    click(&mut d, &inputs, cb);
    assert!(!d.ui.obj_has_flag(label, ObjFlags::HIDDEN));
}

#[test]
fn label_bind_text_follows_the_string_subject() {
    let (mut d, inputs) = setup("lv_example_label_bind_text");
    let label = by_text(&d, "Hello");
    let running = d.ui.obj_get_parent(by_text(&d, "Running")).expect("button");
    click(&mut d, &inputs, running);
    assert_eq!(label_text(&d, label), "Running");
    let idle = d.ui.obj_get_parent(by_text(&d, "Idle")).expect("button");
    click(&mut d, &inputs, idle);
    assert_eq!(label_text(&d, label), "Idle");
}

#[test]
fn bar_bind_value_steps_and_rolls_over() {
    let (mut d, inputs) = setup("lv_example_bar_bind_value");
    let value = by_text(&d, "50%");
    let bar = Bar::from_obj(&d.ui, by_class(&d, "lv_bar")).expect("bar");
    let plus = d.ui.obj_get_parent(by_text(&d, "+")).expect("button");
    let minus = d.ui.obj_get_parent(by_text(&d, "-")).expect("button");
    click(&mut d, &inputs, plus);
    assert_eq!(label_text(&d, value), "55%");
    assert_eq!(bar.value(&d.ui), 55);
    for _ in 0..12 {
        click(&mut d, &inputs, minus);
    }
    // 55 - 11 * 5 = 0, then the 12th step rolls over to the maximum.
    assert_eq!(label_text(&d, value), "100%");
    assert_eq!(bar.value(&d.ui), 100);
}

#[test]
fn obj_bind_style_toggles_the_dark_style() {
    let (mut d, inputs) = setup("lv_example_obj_bind_style");
    let panel = d.ui.obj_get_parent(by_text(&d, "Panel content")).expect("panel");
    let button = d.ui.obj_get_parent(by_text(&d, "Toggle theme")).expect("button");
    let before = d.ui.obj_get_style_bg_color(panel, ferrovgl_core::Part::MAIN);
    click(&mut d, &inputs, button);
    let after = d.ui.obj_get_style_bg_color(panel, ferrovgl_core::Part::MAIN);
    assert_ne!(before, after);
    assert_eq!(after, ferrovgl_core::Color::hex(0x1f2937));
}
