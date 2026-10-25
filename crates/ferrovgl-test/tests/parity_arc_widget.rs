//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_arc.c`.
//!
//! Every upstream test function, in upstream order, with its values and
//! screenshot names. `setUp` takes the active screen of a fresh
//! default-theme display with the upstream mouse/keypad/encoder test devices;
//! `tearDown`'s `lv_obj_clean` + `LV_LAYOUT_NONE` is the display drop.
//!
//! Upstream runs the tests sequentially on one display, so the pointer keeps
//! its last coordinates between tests. Tests that start by moving a released
//! pointer reproduce the coordinates left by the previous test explicitly.
//!
//! The parity config compiles out two test bodies: `test_arc_rgb565a8_image`
//! (`LV_BIN_DECODER_RAM_LOAD=0`) and `test_arc_properties`
//! (`LV_USE_OBJ_PROPERTY=0`); their ports are empty in the same way.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::group::{KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_UP};
use ferrovgl_core::misc::math::{TRIGO_SIN_MAX, trigo_cos, trigo_sin};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::{EventCode, EventParam, Obj, Part, StyleSelector, ValuePrecise};
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::{Arc, ArcMode};

const MAIN: StyleSelector = StyleSelector::DEFAULT;

fn indicator() -> StyleSelector {
    Part::INDICATOR.into()
}

fn knob() -> StyleSelector {
    Part::KNOB.into()
}

/// The globals of `test_arc.c` set by `setUp()` and the upstream test devices.
struct Fixture {
    display: TestDisplay,
    inputs: TestInputs,
    active_screen: Obj,
}

/// `setUp()`: `active_screen = lv_screen_active();`
fn set_up() -> Fixture {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    let active_screen = display.screen().unwrap();
    Fixture {
        display,
        inputs,
        active_screen,
    }
}

/// `dummy_event_cb()` / `dummy_event_cb2()`: count the events in `cnt`.
fn add_counter(f: &mut Fixture, obj: Obj, code: EventCode, cnt: &Rc<Cell<u32>>) {
    let cnt = cnt.clone();
    f.display
        .ui
        .obj_add_event_cb(obj, Rc::new(move |_, _| cnt.set(cnt.get() + 1)), code, None);
}

/// `lv_test_mouse_release(); lv_test_wait(50); lv_test_mouse_move_to(x, y);
/// lv_test_mouse_press(); lv_test_wait(500); lv_test_mouse_release();
/// lv_test_wait(50);`
fn long_click_at(f: &mut Fixture, x: i32, y: i32) {
    f.inputs.mouse.release();
    f.display.wait(50);
    f.inputs.mouse.move_to(x, y);
    f.inputs.mouse.press();
    f.display.wait(500);
    f.inputs.mouse.release();
    f.display.wait(50);
}

#[test]
fn arc_creation_successful() {
    let mut f = set_up();
    let arc = Arc::create(&mut f.display.ui, f.active_screen);

    assert!(f.display.ui.obj_is_valid(arc.obj()));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn arc_basic_render() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 100, 100);
    ui.obj_center(arc.obj());
    arc.set_value(ui, 70);

    f.display.screenshot_compare("widgets/arc_1.png").unwrap();
}

#[test]
fn arc_rgb565a8_image() {
    /*`LV_BIN_DECODER_RAM_LOAD` is 0 in the parity config: the upstream body
     *(and its `widgets/arc_2.png` comparison) is compiled out.*/
    let _f = set_up();
}

#[test]
fn arc_should_truncate_to_max_range_when_new_value_exceeds_it() {
    let mut f = set_up();
    /* Default max range is 100 */
    let value_after_truncation = 100;

    let arc = Arc::create(&mut f.display.ui, f.active_screen);

    arc.set_value(&mut f.display.ui, 200);

    assert_eq!(value_after_truncation, arc.value(&f.display.ui) as i16);
}

#[test]
fn arc_should_truncate_to_min_range_when_new_value_is_inferior() {
    let mut f = set_up();
    /* Default min range is 100 */
    let value_after_truncation = 0;

    let arc = Arc::create(&mut f.display.ui, f.active_screen);

    arc.set_value(&mut f.display.ui, 0);

    assert_eq!(value_after_truncation, arc.value(&f.display.ui) as i16);
}

#[test]
fn arc_should_update_value_after_updating_range() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let value_after_updating_max_range = 50;
    let value_after_updating_min_range = 30;

    let arc = Arc::create(ui, f.active_screen);

    arc.set_value(ui, 80);
    arc.set_range(ui, 1, 50);

    assert_eq!(value_after_updating_max_range, arc.value(ui) as i16);

    arc.set_value(ui, 10);
    arc.set_range(ui, 30, 50);

    assert_eq!(value_after_updating_min_range, arc.value(ui) as i16);
}

#[test]
fn arc_should_update_angles_when_changing_to_symmetrical_mode() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let expected_angle_start: ValuePrecise = 135 as ValuePrecise;
    let expected_angle_end: ValuePrecise = 270 as ValuePrecise;

    /* start angle is 135, end angle is 45 at creation */
    let arc = Arc::create(ui, f.active_screen);
    arc.set_mode(ui, ArcMode::Symmetrical);

    assert_eq!(expected_angle_start, arc.angle_start(ui));
    assert_eq!(expected_angle_end, arc.angle_end(ui));
}

#[test]
fn arc_should_update_angles_when_changing_to_symmetrical_mode_value_more_than_middle_range() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let expected_angle_start: ValuePrecise = 270 as ValuePrecise;
    let expected_angle_end: ValuePrecise = 45 as ValuePrecise;

    /* start angle is 135, end angle is 45 at creation */
    let arc = Arc::create(ui, f.active_screen);
    arc.set_value(ui, 100);
    arc.set_mode(ui, ArcMode::Symmetrical);

    assert_eq!(expected_angle_start, arc.angle_start(ui));
    assert_eq!(expected_angle_end, arc.angle_end(ui));
}

/* See #2522 for more information */
#[test]
fn arc_angles_when_reversed() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let expected_start_angle: ValuePrecise = 54 as ValuePrecise;
    let expected_end_angle: ValuePrecise = 90 as ValuePrecise;
    let expected_value = 40;

    let screen = ui.display_get_screen_active(None).unwrap();
    let arc_black = Arc::create(ui, screen);

    arc_black.set_mode(ui, ArcMode::Reverse);

    arc_black.set_bg_angles(ui, 0 as ValuePrecise, 90 as ValuePrecise);

    arc_black.set_value(ui, expected_value);

    assert_eq!(expected_start_angle, arc_black.angle_start(ui));
    assert_eq!(expected_end_angle, arc_black.angle_end(ui));
    assert_eq!(expected_value as i16, arc_black.value(ui) as i16);
}

#[test]
fn arc_click_area_with_adv_hittest() {
    let mut f = set_up();
    let event_cnt = Rc::new(Cell::new(0u32));
    let ui = &mut f.display.ui;
    let screen = ui.display_get_screen_active(None).unwrap();
    let arc = Arc::create(ui, screen);
    ui.obj_set_size(arc.obj(), 100, 100);
    ui.obj_set_style_arc_width(arc.obj(), 10, MAIN);
    ui.obj_set_adv_hittest(arc.obj(), true);
    add_counter(&mut f, arc.obj(), EventCode::PRESSED, &event_cnt);
    f.display.ui.obj_set_ext_click_area(arc.obj(), 5);

    /*No click detected at the middle*/
    event_cnt.set(0);
    f.inputs.mouse_click_at(&mut f.display, 50, 50);
    assert_eq!(0, event_cnt.get());

    /*No click close to the radius - bg_arc - ext_click_area*/
    event_cnt.set(0);
    f.inputs.mouse_click_at(&mut f.display, 83, 50);
    assert_eq!(0, event_cnt.get());

    /*Click on the radius - bg_arc - ext_click_area*/
    event_cnt.set(0);
    f.inputs.mouse_click_at(&mut f.display, 86, 50);
    assert!(event_cnt.get() > 0);

    /*Click on the radius + ext_click_area*/
    event_cnt.set(0);
    f.inputs.mouse_click_at(&mut f.display, 104, 50);
    assert!(event_cnt.get() > 0);

    /*No click beyond to the radius + ext_click_area*/
    event_cnt.set(0);
    f.inputs.mouse_click_at(&mut f.display, 106, 50);
    assert_eq!(0, event_cnt.get());
}

/* Check value doesn't go to max when clicking on the other side of the arc */
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn arc_click_sustained_from_start_to_end_does_not_set_value_to_max() {
    let mut f = set_up();
    /*Inherited from `test_arc_click_area_with_adv_hittest`: its last click
     *left the released pointer at (106, 50).*/
    f.inputs.mouse.move_to(106, 50);

    let event_cnt = Rc::new(Cell::new(0u32));
    let ui = &mut f.display.ui;
    let screen = ui.display_get_screen_active(None).unwrap();
    let arc = Arc::create(ui, screen);
    arc.set_value(ui, 0);

    ui.obj_set_size(arc.obj(), 100, 100);
    ui.obj_center(arc.obj());
    add_counter(&mut f, arc.obj(), EventCode::PRESSED, &event_cnt);
    event_cnt.set(0);

    /* Click close to start angle */
    event_cnt.set(0);
    long_click_at(&mut f, 376, 285);

    assert_eq!(1, event_cnt.get());
    let ui = &f.display.ui;
    assert_eq!(arc.min_value(ui), arc.value(ui));

    /* Click close to end angle */
    event_cnt.set(0);

    f.inputs.mouse.release();
    f.display.wait(50);
    f.inputs.mouse.move_to(376, 285);
    f.inputs.mouse.press();
    f.display.wait(500);
    f.inputs.mouse.move_to(415, 281);
    f.display.wait(500);
    f.inputs.mouse.release();
    f.display.wait(50);

    assert_eq!(1, event_cnt.get());
    let ui = &f.display.ui;
    assert_eq!(arc.min_value(ui), arc.value(ui));

    f.display.screenshot_compare("widgets/arc_3.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn two_overlapping_arcs_can_be_interacted_independently() {
    let mut f = set_up();
    /*Inherited from the previous test: its last click left the released
     *pointer at (415, 281).*/
    f.inputs.mouse.move_to(415, 281);

    let event_cnt = Rc::new(Cell::new(0u32));
    let event_cnt2 = Rc::new(Cell::new(0u32));
    let ui = &mut f.display.ui;
    let screen = ui.display_get_screen_active(None).unwrap();
    let arc = Arc::create(ui, screen);
    let arc2 = Arc::create(ui, screen);

    arc.set_value(ui, 0);
    arc2.set_value(ui, 0);
    ui.obj_set_size(arc.obj(), 100, 100);
    ui.obj_set_size(arc2.obj(), 100, 100);
    arc.set_bg_angles(ui, 20 as ValuePrecise, 160 as ValuePrecise);
    arc2.set_bg_angles(ui, 200 as ValuePrecise, 340 as ValuePrecise);
    ui.obj_set_adv_hittest(arc.obj(), true);
    ui.obj_set_adv_hittest(arc2.obj(), true);
    arc.set_value(ui, 10);
    arc2.set_value(ui, 10);
    arc.set_rotation(ui, 355);
    arc2.set_rotation(ui, 355);
    ui.obj_center(arc.obj());
    ui.obj_center(arc2.obj());

    // Add event callback to both arcs
    add_counter(&mut f, arc.obj(), EventCode::PRESSED, &event_cnt);
    add_counter(&mut f, arc2.obj(), EventCode::PRESSED, &event_cnt2);

    // Reset event counters
    event_cnt.set(0);
    event_cnt2.set(0);

    // Click on the position of the first arc (center)
    long_click_at(&mut f, 400, 195);

    // Verify that the event callback was called for the first arc
    assert_eq!(0, event_cnt.get());
    assert_eq!(1, event_cnt2.get());

    // click on the position of the second arc (center)
    long_click_at(&mut f, 400, 285);

    // Verify that the event callback was called for the second arc
    assert_eq!(1, event_cnt.get());
    assert_eq!(1, event_cnt2.get());

    // Verify that the screen remains as expected after the interactions
    f.display
        .screenshot_compare("widgets/overlapping_arcs_test.png")
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn arc_padding() {
    let mut f = set_up();
    /*Inherited from the previous test: its last click left the released
     *pointer at (400, 285).*/
    f.inputs.mouse.move_to(400, 285);

    let ui = &mut f.display.ui;
    let screen = ui.display_get_screen_active(None).unwrap();
    let arc = Arc::create(ui, screen);
    ui.obj_set_size(arc.obj(), 412, 412);
    ui.obj_set_style_arc_width(arc.obj(), 100, MAIN);

    ui.obj_set_style_arc_width(arc.obj(), 60, indicator());
    ui.obj_set_style_pad_bottom(arc.obj(), 10, indicator());

    ui.obj_set_style_bg_color(arc.obj(), palette::darken(Palette::Blue, 3), knob());
    ui.obj_set_style_pad_all(arc.obj(), -5, knob());

    ui.obj_center(arc.obj());

    f.display.screenshot_compare("widgets/arc_4.png").unwrap();
}

#[test]
fn arc_properties() {
    /*`LV_USE_OBJ_PROPERTY` is 0 in the parity config: the upstream body is
     *compiled out.*/
    let _f = set_up();
}

#[test]
fn arc_set_change_rate() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);

    /* Test setting various change rate values */
    arc.set_change_rate(ui, 0);
    arc.set_change_rate(ui, 100);
    arc.set_change_rate(ui, 500);
    arc.set_change_rate(ui, 1000);
}

#[test]
fn arc_set_knob_offset() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let test_offset_positive = 15;
    let test_offset_negative = -10;
    let test_offset_zero = 0;

    let arc = Arc::create(ui, f.active_screen);

    /* Test setting positive offset */
    arc.set_knob_offset(ui, test_offset_positive);
    assert_eq!(test_offset_positive, arc.knob_offset(ui));

    /* Test setting negative offset */
    arc.set_knob_offset(ui, test_offset_negative);
    assert_eq!(test_offset_negative, arc.knob_offset(ui));

    /* Test setting zero offset */
    arc.set_knob_offset(ui, test_offset_zero);
    assert_eq!(test_offset_zero, arc.knob_offset(ui));
}

#[test]
fn arc_getter_functions() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);

    /* Test background angle getters */
    let bg_start_angle: ValuePrecise = 45 as ValuePrecise;
    let bg_end_angle: ValuePrecise = 270 as ValuePrecise;

    arc.set_bg_angles(ui, bg_start_angle, bg_end_angle);
    assert_eq!(bg_start_angle, arc.bg_angle_start(ui));
    assert_eq!(bg_end_angle, arc.bg_angle_end(ui));

    /* Test max value getter */
    let max_value = 200;
    arc.set_range(ui, 0, max_value);
    assert_eq!(max_value, arc.max_value(ui));

    /* Test mode getter */
    let mode = ArcMode::Symmetrical;
    arc.set_mode(ui, mode);
    assert_eq!(mode, arc.mode(ui));

    /* Test rotation getter */
    let rotation = 90;
    arc.set_rotation(ui, rotation);
    assert_eq!(rotation, arc.rotation(ui));

    /* Test knob offset getter */
    let knob_offset = 15;
    arc.set_knob_offset(ui, knob_offset);
    assert_eq!(knob_offset, arc.knob_offset(ui));
}

#[test]
fn arc_drag_prevents_big_angle_jumps() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let screen = ui.display_get_screen_active(None).unwrap();
    let arc = Arc::create(ui, screen);
    ui.obj_set_size(arc.obj(), 100, 100);
    ui.obj_center(arc.obj());
    ui.obj_update_layout(arc.obj());

    /* Set initial value */
    arc.set_value(ui, 25);

    /* Test normal drag - drag along the arc smoothly */
    f.inputs.mouse.move_to(376, 285); /* Start position on arc */
    f.inputs.mouse.press();
    f.display.wait(500);

    /* Drag to a nearby position on the arc */
    f.inputs.mouse.move_to(390, 290);
    f.display.wait(500);

    let value_after_small_drag = arc.value(&f.display.ui);

    f.inputs.mouse.release();
    f.display.wait(50);

    /* Value should have changed during drag */
    assert_ne!(25, value_after_small_drag);

    /* Test with different arc range to cover more of the logic */
    arc.set_bg_angles(&mut f.display.ui, 0 as ValuePrecise, 270 as ValuePrecise);
    arc.set_value(&mut f.display.ui, 50);

    f.inputs.mouse.move_to(380, 280);
    f.inputs.mouse.press();
    f.display.wait(500);
    f.inputs.mouse.move_to(400, 295);
    f.display.wait(500);
    f.inputs.mouse.release();
    f.display.wait(50);

    assert!(arc.value(&f.display.ui) >= 0);
    assert!(arc.value(&f.display.ui) <= 100);
}

#[test]
fn arc_align_obj_to_angle() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 100, 100);
    ui.obj_center(arc.obj());

    /* Create an object to align */
    let aligned_obj = ui.obj_create(Some(arc.obj()));
    ui.obj_set_size(aligned_obj, 10, 10);

    /* Test align with different values and offsets */
    arc.set_value(ui, 0);
    arc.align_obj_to_angle(ui, aligned_obj, 0);

    /* Get coordinates after alignment */
    let x1 = ui.obj_get_x(aligned_obj);
    let y1 = ui.obj_get_y(aligned_obj);

    /* Test with different arc value */
    arc.set_value(ui, 50);
    arc.align_obj_to_angle(ui, aligned_obj, 0);

    /* Position should change when value changes */
    let x2 = ui.obj_get_x(aligned_obj);
    let y2 = ui.obj_get_y(aligned_obj);

    /* At least one coordinate should be different */
    assert!((x1 != x2) || (y1 != y2));

    /* Test with positive radius offset */
    arc.align_obj_to_angle(ui, aligned_obj, 10);
    let x3 = ui.obj_get_x(aligned_obj);
    let y3 = ui.obj_get_y(aligned_obj);

    /* Position should be different with offset */
    assert!((x2 != x3) || (y2 != y3));

    /* Test with negative radius offset */
    arc.align_obj_to_angle(ui, aligned_obj, -10);
    let x4 = ui.obj_get_x(aligned_obj);
    let y4 = ui.obj_get_y(aligned_obj);

    /* Position should be different with negative offset */
    assert!((x3 != x4) || (y3 != y4));

    /* Test with maximum value */
    arc.set_value(ui, 100);
    arc.align_obj_to_angle(ui, aligned_obj, 0);
}

#[test]
fn arc_rotate_obj_to_angle() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 100, 100);
    ui.obj_center(arc.obj());

    /* Create an object to rotate */
    let rotated_obj = ui.obj_create(Some(arc.obj()));
    ui.obj_set_size(rotated_obj, 10, 10);

    /* Test rotate with value 0 */
    arc.set_value(ui, 0);
    arc.rotate_obj_to_angle(ui, rotated_obj, 0);

    /* Get initial rotation */
    let rotation1 = ui.obj_get_style_transform_rotation(rotated_obj, Part::MAIN);

    /* Test with different arc value */
    arc.set_value(ui, 50);
    arc.rotate_obj_to_angle(ui, rotated_obj, 0);

    /* Rotation should change when value changes */
    let rotation2 = ui.obj_get_style_transform_rotation(rotated_obj, Part::MAIN);
    assert_ne!(rotation1, rotation2);

    /* Test with positive radius offset */
    arc.rotate_obj_to_angle(ui, rotated_obj, 15);

    /* Get coordinates with offset */
    let x1 = ui.obj_get_x(rotated_obj);
    let y1 = ui.obj_get_y(rotated_obj);

    /* Test with different offset */
    arc.rotate_obj_to_angle(ui, rotated_obj, -15);

    /* Position should change with different offset */
    let x2 = ui.obj_get_x(rotated_obj);
    let y2 = ui.obj_get_y(rotated_obj);
    assert!((x1 != x2) || (y1 != y2));

    /* Test with maximum value */
    arc.set_value(ui, 100);
    arc.rotate_obj_to_angle(ui, rotated_obj, 0);

    /* Rotation should be different at max value */
    let rotation3 = ui.obj_get_style_transform_rotation(rotated_obj, Part::MAIN);
    assert_ne!(rotation2, rotation3);

    /* Verify rotation is within expected range (angle * 10 + 900) */
    /* For arc angle 0-270 degrees, rotation should be 900-3600 */
    assert!(rotation3 >= 0);
}

/// `test_arc_key_step()`
fn arc_key_step(f: &mut Fixture, obj: Arc, start_val: i32, key: u32, expected_val: i32) {
    obj.set_value(&mut f.display.ui, start_val);
    f.display
        .ui
        .obj_send_event(obj.obj(), EventCode::KEY, EventParam::Key(key));
    assert_eq!(expected_val, obj.value(&f.display.ui));
}

#[test]
fn arc_key_event_handling() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 100, 100);
    arc.set_range(ui, 0, 100);

    /* Test RIGHT key increases value */
    arc_key_step(&mut f, arc, 50, KEY_RIGHT, 51);

    /* Test UP key increases value */
    arc_key_step(&mut f, arc, 50, KEY_UP, 51);

    /* Test LEFT key decreases value */
    arc_key_step(&mut f, arc, 50, KEY_LEFT, 49);

    /* Test DOWN key decreases value */
    arc_key_step(&mut f, arc, 50, KEY_DOWN, 49);

    /* Test that value respects maximum limit */
    arc_key_step(&mut f, arc, 100, KEY_RIGHT, 100);

    /* Test that value respects minimum limit */
    arc_key_step(&mut f, arc, 0, KEY_LEFT, 0);
}

/// `test_arc_rotary_step()`
fn arc_rotary_step(f: &mut Fixture, obj: Arc, start_val: i32, rotary_diff: i32, expected_val: i32) {
    obj.set_value(&mut f.display.ui, start_val);
    f.display
        .ui
        .obj_send_event(obj.obj(), EventCode::ROTARY, EventParam::RotaryDiff(rotary_diff));
    assert_eq!(expected_val, obj.value(&f.display.ui));
}

#[test]
fn arc_rotary_event_handling() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 100, 100);
    arc.set_range(ui, 0, 100);

    /* Test positive rotary diff increases value */
    arc_rotary_step(&mut f, arc, 50, 5, 55);

    /* Test negative rotary diff decreases value */
    arc_rotary_step(&mut f, arc, 50, -5, 45);

    /* Test large positive rotary diff respects maximum */
    arc_rotary_step(&mut f, arc, 95, 10, 100);

    /* Test large negative rotary diff respects minimum */
    arc_rotary_step(&mut f, arc, 5, -10, 0);

    /* Test zero rotary diff keeps value unchanged */
    arc_rotary_step(&mut f, arc, 50, 0, 50);
}

#[test]
fn arc_encoder_focused_event() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 100, 100);
    arc.set_range(ui, 0, 100);
    arc.set_value(ui, 30);

    /* Create a group and add arc to it */
    let g = ui.group_create();
    ui.indev_set_group(f.inputs.encoder.id(), Some(g));
    ui.group_add_obj(g, arc.obj());

    /* Test encoder click to enter editing mode */
    ui.group_set_editing(g, false);
    assert!(!ui.group_get_editing(g));

    f.inputs.encoder_click(&mut f.display);
    f.display.wait(50);

    /* Clicking should enter editing mode */
    assert!(f.display.ui.group_get_editing(g));

    /* Test value change with encoder in editing mode */
    let mut value_before = arc.value(&f.display.ui);
    f.inputs.encoder_turn(&mut f.display, 10);
    f.display.wait(50);
    let mut value_after = arc.value(&f.display.ui);

    /* Value should change when encoder is used in editing mode */
    assert_eq!(value_before + 10, value_after);

    /* Test encoder click again to exit editing mode */
    f.inputs.encoder_click(&mut f.display);
    f.display.wait(50);
    assert!(!f.display.ui.group_get_editing(g));

    /* Test rotary in non-editing mode should not change value */
    value_before = arc.value(&f.display.ui);
    f.inputs.encoder_turn(&mut f.display, 5);
    f.display.wait(50);
    value_after = arc.value(&f.display.ui);

    /* Value should not change when not in editing mode */
    assert_eq!(value_before, value_after);
    f.display.ui.group_delete(g);
}

/// `run_arc_drag_test()`
fn run_arc_drag_test(
    f: &mut Fixture,
    arc: Arc,
    bg_start: i32,
    bg_end: i32,
    start_val: i32,
    drag_from: i32,
    drag_to: i32,
) {
    let ui = &mut f.display.ui;
    arc.set_bg_angles(ui, bg_start as ValuePrecise, bg_end as ValuePrecise);
    arc.set_value(ui, start_val);

    let center_x = ui.obj_get_x(arc.obj()) + ui.obj_get_width(arc.obj()) / 2;
    let center_y = ui.obj_get_y(arc.obj()) + ui.obj_get_height(arc.obj()) / 2;
    let radius = 85;

    let x_from = center_x + (radius * trigo_sin(drag_from as i16)) / TRIGO_SIN_MAX;
    let y_from = center_y - (radius * trigo_cos(drag_from as i16)) / TRIGO_SIN_MAX;
    let x_to = center_x + (radius * trigo_sin(drag_to as i16)) / TRIGO_SIN_MAX;
    let y_to = center_y - (radius * trigo_cos(drag_to as i16)) / TRIGO_SIN_MAX;

    f.inputs.mouse.move_to(x_from, y_from);
    f.inputs.mouse.press();
    f.display.wait(50);
    f.inputs.mouse.move_to(x_to, y_to);
    f.display.wait(50);
    f.inputs.mouse.release();
    f.display.wait(50);

    assert!(arc.value(&f.display.ui) >= 0);
    assert!(arc.value(&f.display.ui) <= 100);
}

#[test]
fn arc_angle_within_bg_bounds_edge_cases() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let screen = ui.display_get_screen_active(None).unwrap();
    let arc = Arc::create(ui, screen);
    ui.obj_set_size(arc.obj(), 200, 200);
    ui.obj_center(arc.obj());
    ui.obj_update_layout(arc.obj());

    /* Test 1: angle in [360-tolerance, 360] range */
    run_arc_drag_test(&mut f, arc, 60, 270, 10, 90, 50);

    /* Test 2: angle near 360° with small tolerance */
    run_arc_drag_test(&mut f, arc, 10, 270, 20, 60, 5);

    /* Test 3: angle after end within tolerance */
    run_arc_drag_test(&mut f, arc, 40, 250, 80, 240, 275);

    /* Test 4: tolerance wraps past 360° */
    run_arc_drag_test(&mut f, arc, 10, 355, 90, 40, 5);

    /* Edge test 1: 360° boundary tolerance */
    run_arc_drag_test(&mut f, arc, 60, 300, 15, 100, 350);

    /* Edge test 2: wrapped arc tolerance */
    run_arc_drag_test(&mut f, arc, 300, 60, 50, 30, 245);

    /* Edge test 3: after-end tolerance */
    run_arc_drag_test(&mut f, arc, 30, 240, 85, 240, 280);

    /* Edge test 4: wrap past 360° tolerance */
    run_arc_drag_test(&mut f, arc, 15, 350, 92, 30, 8);

    /* Edge test 5: wrapped arc after-end tolerance */
    run_arc_drag_test(&mut f, arc, 300, 60, 80, 60, 75);
}

/* Regression: negative inner radius in inv_arc_area() when padding
 * exceeds the arc radius. Previously passed a negative value to
 * lv_draw_arc_get_area(), causing undefined trig results. */
#[test]
fn arc_negative_radius_no_crash() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 10, 10);
    ui.obj_set_style_arc_width(arc.obj(), 50, indicator());
    ui.obj_set_style_arc_width(arc.obj(), 50, MAIN);
    ui.obj_set_style_pad_all(arc.obj(), 30, MAIN);

    arc.set_value(ui, 50);
    ui.obj_invalidate(arc.obj());
    ui.refr_now(None);

    assert!(f.display.ui.obj_is_valid(arc.obj()));
}

/* Regression: stroke width wider than radius in lv_draw_arc_get_area()
 * produced a negative inner radius, leading to bad trig calculations. */
#[test]
fn arc_stroke_wider_than_radius_no_crash() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 30, 30);
    ui.obj_set_style_arc_width(arc.obj(), 40, indicator());
    ui.obj_set_style_arc_width(arc.obj(), 40, MAIN);

    arc.set_value(ui, 70);
    ui.obj_invalidate(arc.obj());
    ui.refr_now(None);

    assert!(f.display.ui.obj_is_valid(arc.obj()));
}

/* Edge case: zero-size arc should not crash during invalidation
 * or redraw. */
#[test]
fn arc_zero_size_no_crash() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let arc = Arc::create(ui, f.active_screen);
    ui.obj_set_size(arc.obj(), 1, 1);

    arc.set_value(ui, 50);
    ui.obj_invalidate(arc.obj());
    ui.refr_now(None);

    assert!(ui.obj_is_valid(arc.obj()));

    /* Also try true zero size */
    ui.obj_set_size(arc.obj(), 0, 0);
    arc.set_value(ui, 80);
    ui.obj_invalidate(arc.obj());
    ui.refr_now(None);

    assert!(ui.obj_is_valid(arc.obj()));
}
