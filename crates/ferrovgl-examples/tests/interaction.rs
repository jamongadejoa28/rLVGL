//! Event logic of the interactive examples, driven with the synthetic mouse
//! of `ferrovgl-test` like the upstream `lv_test_mouse_click_at()` tests.
//! The expected texts and states follow the C event callbacks.

mod common;

use ferrovgl_core::misc::area::Area;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::{Obj, ObjFlags, Part, State};
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::{Bar, Label, Slider, Textarea};

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

/// The center of `obj` after a layout update.
fn center(display: &mut TestDisplay, obj: Obj) -> (i32, i32) {
    display.ui.obj_update_layout(obj);
    let a = display.ui.obj_get_coords(obj);
    (a.x1 + a.width() / 2, a.y1 + a.height() / 2)
}

/// Press at `from`, move to `to` in 10 steps of 10 ms, then release.
fn drag(display: &mut TestDisplay, inputs: &TestInputs, from: (i32, i32), to: (i32, i32)) {
    inputs.mouse.release();
    display.wait(50);
    inputs.mouse.move_to(from.0, from.1);
    inputs.mouse.press();
    display.wait(50);
    for i in 1..=10 {
        let x = from.0 + (to.0 - from.0) * i / 10;
        let y = from.1 + (to.1 - from.1) * i / 10;
        inputs.mouse.move_to(x, y);
        display.wait(10);
    }
    display.wait(50);
    inputs.mouse.release();
    display.wait(50);
}

/// The center of cell `(row, col)` of a `rows` x `cols` button matrix,
/// estimated from its content area.
fn btnm_cell(display: &mut TestDisplay, btnm: Obj, rows: i32, cols: i32, row: i32, col: i32) -> (i32, i32) {
    display.ui.obj_update_layout(btnm);
    let c: Area = display.ui.obj_get_content_coords(btnm);
    (
        c.x1 + c.width() * (2 * col + 1) / (2 * cols),
        c.y1 + c.height() * (2 * row + 1) / (2 * rows),
    )
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
    let before = d.ui.obj_get_style_bg_color(panel, Part::MAIN);
    click(&mut d, &inputs, button);
    let after = d.ui.obj_get_style_bg_color(panel, Part::MAIN);
    assert_ne!(before, after);
    assert_eq!(after, ferrovgl_core::Color::hex(0x1f2937));
}

#[test]
fn event_streak_reports_single_double_and_triple_clicks() {
    let (mut d, inputs) = setup("lv_example_event_streak");
    let info = by_text(&d, "No events yet");
    let label = by_text(&d, "Click me!");
    let button = d.ui.obj_get_parent(label).expect("button");
    click(&mut d, &inputs, button);
    assert_eq!(label_text(&d, label), "Single clicked");
    assert_eq!(label_text(&d, info), "Short click streak: 1");
    click(&mut d, &inputs, button);
    assert_eq!(label_text(&d, label), "Double clicked");
    assert_eq!(label_text(&d, info), "Short click streak: 2");
    click(&mut d, &inputs, button);
    assert_eq!(label_text(&d, label), "Triple clicked");
    assert_eq!(label_text(&d, info), "Short click streak: 3");
}

#[test]
fn event_button_shows_the_last_button_event() {
    let (mut d, inputs) = setup("lv_example_event_button");
    let info = by_text(&d, "The last button event:\nNone");
    let button = by_class(&d, "lv_button");
    let (x, y) = center(&mut d, button);
    inputs.mouse.move_to(x, y);
    inputs.mouse.press();
    d.wait(50);
    assert_eq!(label_text(&d, info), "The last button event:\nLV_EVENT_PRESSED");
    // LV_INDEV_DEF_LONG_PRESS_TIME (400 ms) after the first read of the
    // press, rounded up to the 33 ms read period.
    d.wait(500);
    assert_eq!(
        label_text(&d, info),
        "The last button event:\nLV_EVENT_LONG_PRESSED"
    );
    d.wait(150); /* LV_INDEV_DEF_LONG_PRESS_REP_TIME (100 ms) */
    assert_eq!(
        label_text(&d, info),
        "The last button event:\nLV_EVENT_LONG_PRESSED_REPEAT"
    );
    inputs.mouse.release();
    d.wait(50);
    assert_eq!(label_text(&d, info), "The last button event:\nLV_EVENT_CLICKED");
}

#[test]
fn event_bubble_turns_only_the_clicked_button_red() {
    let (mut d, inputs) = setup("lv_example_event_bubble");
    let three = d.ui.obj_get_parent(by_text(&d, "3")).expect("button 3");
    let four = d.ui.obj_get_parent(by_text(&d, "4")).expect("button 4");
    let red = palette::main(Palette::Red);
    assert_ne!(d.ui.obj_get_style_bg_color(three, Part::MAIN), red);
    click(&mut d, &inputs, three);
    assert_eq!(d.ui.obj_get_style_bg_color(three, Part::MAIN), red);
    assert_ne!(d.ui.obj_get_style_bg_color(four, Part::MAIN), red);
    // A click on the container itself (`target == current_target`) is ignored.
    let cont = d.ui.obj_get_parent(three).expect("container");
    let before = d.ui.obj_get_style_bg_color(cont, Part::MAIN);
    d.ui.obj_update_layout(cont);
    let a = d.ui.obj_get_coords(cont);
    inputs.mouse_click_at(&mut d, a.x1 + 3, a.y1 + 3);
    assert_eq!(d.ui.obj_get_style_bg_color(cont, Part::MAIN), before);
}

#[test]
fn event_trickle_focuses_the_children_with_the_container() {
    let (mut d, inputs) = setup("lv_example_event_trickle");
    let first = d.ui.obj_get_parent(by_text(&d, "0")).expect("sub-container");
    let cont = d.ui.obj_get_parent(first).expect("container");
    d.ui.obj_update_layout(cont);
    let a = d.ui.obj_get_coords(cont);
    // The container's top-left corner, outside every sub-container.
    let (x, y) = (a.x1 + 3, a.y1 + 3);
    for i in 0..d.ui.obj_get_child_count(cont) as i32 {
        let child = d.ui.obj_get_child(cont, i).expect("child");
        let c = d.ui.obj_get_coords(child);
        assert!(!(c.x1..=c.x2).contains(&x) || !(c.y1..=c.y2).contains(&y));
    }
    assert!(!d.ui.obj_has_state(first, State::FOCUSED));
    inputs.mouse_click_at(&mut d, x, y);
    assert!(d.ui.obj_has_state(cont, State::FOCUSED));
    for i in 0..d.ui.obj_get_child_count(cont) as i32 {
        let child = d.ui.obj_get_child(cont, i).expect("child");
        assert!(d.ui.obj_has_state(child, State::FOCUSED), "child {i}");
    }
}

#[test]
fn slider_event_reports_dragging_and_release() {
    let (mut d, inputs) = setup("lv_example_slider_event");
    let status = by_text(&d, "Drag the slider");
    let slider_obj = by_class(&d, "lv_slider");
    let slider = Slider::from_obj(&d.ui, slider_obj).expect("slider");
    d.ui.obj_update_layout(slider_obj);
    let a = d.ui.obj_get_coords(slider_obj);
    let y = a.y1 + a.height() / 2;
    drag(&mut d, &inputs, (a.x1 + 2, y), (a.x1 + a.width() / 2, y));
    let value = slider.value(&d.ui);
    assert!(value > 0, "dragged to {value}");
    assert_eq!(label_text(&d, status), format!("Released at: {value}"));

    inputs.mouse.move_to(a.x1 + a.width() / 2, y);
    inputs.mouse.press();
    d.wait(50);
    inputs.mouse.move_to(a.x2 + 50, y);
    d.wait(50);
    assert_eq!(label_text(&d, status), "Dragging: 100");
    inputs.mouse.release();
    d.wait(50);
    assert_eq!(label_text(&d, status), "Released at: 100");
}

#[test]
fn buttonmatrix_event_shows_the_pressed_button() {
    let (mut d, inputs) = setup("lv_example_buttonmatrix_event");
    let status = by_text(&d, "Pressed: \u{2014}");
    let btnm = by_class(&d, "lv_buttonmatrix");
    let (x, y) = btnm_cell(&mut d, btnm, 3, 3, 1, 1);
    inputs.mouse_click_at(&mut d, x, y);
    assert_eq!(label_text(&d, status), "Pressed: 5");
    let (x, y) = btnm_cell(&mut d, btnm, 3, 3, 2, 0);
    inputs.mouse_click_at(&mut d, x, y);
    assert_eq!(label_text(&d, status), "Pressed: 7");
}

#[test]
fn textarea_buttonmatrix_types_deletes_and_keeps_the_text_on_enter() {
    let (mut d, inputs) = setup("lv_example_textarea_buttonmatrix");
    let ta = Textarea::from_obj(&d.ui, by_class(&d, "lv_textarea")).expect("text area");
    let btnm = by_class(&d, "lv_buttonmatrix");
    let hit = |d: &mut TestDisplay, row, col| {
        let (x, y) = btnm_cell(d, btnm, 4, 3, row, col);
        inputs.mouse_click_at(d, x, y);
    };
    hit(&mut d, 0, 0);
    assert_eq!(ta.text(&d.ui), "1");
    hit(&mut d, 1, 2);
    assert_eq!(ta.text(&d.ui), "16");
    hit(&mut d, 3, 1);
    assert_eq!(ta.text(&d.ui), "160");
    hit(&mut d, 3, 0); /* LV_SYMBOL_BACKSPACE */
    assert_eq!(ta.text(&d.ui), "16");
    hit(&mut d, 3, 2); /* LV_SYMBOL_NEW_LINE: LV_EVENT_READY, the text stays */
    assert_eq!(ta.text(&d.ui), "16");
    // The button matrix is not click-focusable, so the text area keeps focus.
    assert!(d.ui.obj_has_state(ta.obj(), State::FOCUSED));
}

#[test]
fn split_divider_resizes_the_left_pane_within_the_limits() {
    let (mut d, inputs) = setup("lv_example_split");
    let left =
        d.ui.obj_get_parent(by_text(&d, "Left\n(drag the divider)"))
            .expect("left pane");
    let cont = d.ui.obj_get_parent(left).expect("container");
    let divider = d.ui.obj_get_child(cont, 1).expect("divider");
    let right = d.ui.obj_get_child(cont, 2).expect("right pane");
    let content = d.ui.obj_get_content_coords(cont);
    let avail = content.width() - 12;

    let from = center(&mut d, divider);
    drag(&mut d, &inputs, from, (content.x1 + 200 + 6, from.1));
    d.ui.obj_update_layout(cont);
    assert_eq!(d.ui.obj_get_width(left), 200);
    // The right pane grows to the end (`avail` ignores the flex gaps, as upstream).
    assert_eq!(d.ui.obj_get_coords(right).x2, content.x2);

    let from = center(&mut d, divider);
    drag(&mut d, &inputs, from, (content.x1 + 5, from.1));
    d.ui.obj_update_layout(cont);
    assert_eq!(d.ui.obj_get_width(left), 60); /* SPLIT_MIN_PANE */

    let from = center(&mut d, divider);
    drag(&mut d, &inputs, from, (content.x2, from.1));
    d.ui.obj_update_layout(cont);
    assert_eq!(d.ui.obj_get_width(left), avail - 60);
    assert_eq!(d.ui.obj_get_coords(right).x2, content.x2);
}

#[test]
fn split_vertical_divider_resizes_the_top_pane_within_the_limits() {
    let (mut d, inputs) = setup("lv_example_split_vertical");
    let top =
        d.ui.obj_get_parent(by_text(&d, "Top\n(drag the divider)"))
            .expect("top pane");
    let cont = d.ui.obj_get_parent(top).expect("container");
    let divider = d.ui.obj_get_child(cont, 1).expect("divider");
    let bottom = d.ui.obj_get_child(cont, 2).expect("bottom pane");
    let content = d.ui.obj_get_content_coords(cont);
    let avail = content.height() - 12;

    let from = center(&mut d, divider);
    drag(&mut d, &inputs, from, (from.0, content.y1 + 150 + 6));
    d.ui.obj_update_layout(cont);
    assert_eq!(d.ui.obj_get_height(top), 150);
    // The bottom pane grows to the end (`avail` ignores the flex gaps, as upstream).
    assert_eq!(d.ui.obj_get_coords(bottom).y2, content.y2);

    let from = center(&mut d, divider);
    drag(&mut d, &inputs, from, (from.0, content.y1));
    d.ui.obj_update_layout(cont);
    assert_eq!(d.ui.obj_get_height(top), 60); /* SPLIT_MIN_PANE */

    let from = center(&mut d, divider);
    drag(&mut d, &inputs, from, (from.0, content.y2));
    d.ui.obj_update_layout(cont);
    assert_eq!(d.ui.obj_get_height(top), avail - 60);
    assert_eq!(d.ui.obj_get_coords(bottom).y2, content.y2);
}

#[test]
fn obj_draggable_follows_the_pointer() {
    let (mut d, inputs) = setup("lv_example_obj_draggable");
    let obj = d.ui.obj_get_parent(by_text(&d, "Drag me")).expect("object");
    let (x0, y0) = (d.ui.obj_get_x_aligned(obj), d.ui.obj_get_y_aligned(obj));
    let from = center(&mut d, obj);
    drag(&mut d, &inputs, from, (from.0 + 100, from.1 + 50));
    assert_eq!(d.ui.obj_get_x_aligned(obj), x0 + 100);
    assert_eq!(d.ui.obj_get_y_aligned(obj), y0 + 50);
}

#[test]
fn slider_bind_state_disables_the_button_above_80() {
    let (mut d, inputs) = setup("lv_example_slider_bind_state");
    let label = by_text(&d, "Current: 50 (disabled above 80)");
    let button = d.ui.obj_get_parent(by_text(&d, "Submit")).expect("button");
    let slider_obj = by_class(&d, "lv_slider");
    assert!(!d.ui.obj_has_state(button, State::DISABLED));
    d.ui.obj_update_layout(slider_obj);
    let a = d.ui.obj_get_coords(slider_obj);
    let from = center(&mut d, slider_obj);
    drag(&mut d, &inputs, from, (a.x2 + 50, from.1));
    assert_eq!(label_text(&d, label), "Current: 100 (disabled above 80)");
    assert!(d.ui.obj_has_state(button, State::DISABLED));
    let from = center(&mut d, slider_obj);
    drag(&mut d, &inputs, from, (a.x1 - 50, from.1));
    assert_eq!(label_text(&d, label), "Current: 0 (disabled above 80)");
    assert!(!d.ui.obj_has_state(button, State::DISABLED));
}

#[test]
fn image_recolor_opa_follows_the_slider() {
    let (mut d, inputs) = setup("lv_example_image_recolor");
    let label = by_text(&d, "recolor_opa = 128 / 255");
    let image = by_class(&d, "lv_image");
    assert_eq!(d.ui.obj_get_style_image_recolor_opa(image, Part::MAIN), 128);
    let slider_obj = by_class(&d, "lv_slider");
    d.ui.obj_update_layout(slider_obj);
    let a = d.ui.obj_get_coords(slider_obj);
    let from = center(&mut d, slider_obj);
    drag(&mut d, &inputs, from, (a.x2 + 50, from.1));
    assert_eq!(label_text(&d, label), "recolor_opa = 255 / 255");
    assert_eq!(d.ui.obj_get_style_image_recolor_opa(image, Part::MAIN), 255);
    let from = center(&mut d, slider_obj);
    drag(&mut d, &inputs, from, (a.x1 - 50, from.1));
    assert_eq!(label_text(&d, label), "recolor_opa = 0 / 255");
    assert_eq!(d.ui.obj_get_style_image_recolor_opa(image, Part::MAIN), 0);
}

#[test]
fn get_started_slider_label_tracks_the_slider() {
    let (mut d, inputs) = setup("lv_example_get_started_slider");
    let label = by_text(&d, "50");
    let slider_obj = by_class(&d, "lv_slider");
    d.ui.obj_update_layout(slider_obj);
    let a = d.ui.obj_get_coords(slider_obj);
    let from = center(&mut d, slider_obj);
    drag(&mut d, &inputs, from, (a.x1 - 50, from.1));
    assert_eq!(label_text(&d, label), "0");
    assert_eq!(
        Slider::from_obj(&d.ui, slider_obj).expect("slider").value(&d.ui),
        0
    );
}
