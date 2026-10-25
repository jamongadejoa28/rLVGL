//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_slider.c`.
//!
//! Every upstream test function, in upstream order, with its values and
//! screenshot names. `setUp` creates the four sliders on the active screen of
//! a fresh default-theme display and attaches a new group to the encoder;
//! `tearDown`'s `lv_obj_clean` is the display drop. Direct reads and writes of
//! `lv_slider_t` fields go through the matching [`Slider`] accessors.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::group::{GroupId, KEY_DOWN, KEY_ENTER, KEY_LEFT, KEY_RIGHT, KEY_UP};
use ferrovgl_core::misc::event::HitTestInfo;
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::{Dir, EventCode, EventParam, Obj, Point, StyleSelector};
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::slider::SliderValueTarget;
use ferrovgl_widgets::{Slider, SliderMode};

const MAIN: StyleSelector = StyleSelector::DEFAULT;
const ANIM_OFF: bool = false;

/// The globals of `test_slider.c` set by `setUp()`.
struct Fixture {
    display: TestDisplay,
    inputs: TestInputs,
    active_screen: Obj,
    slider: Slider,
    slider_range_mode: Slider,
    slider_normal_mode: Slider,
    #[allow(dead_code)] // Created by `setUp()` but not used by any test.
    slider_symmetrical_mode: Slider,
    g: GroupId,
}

/// `setUp()`
fn set_up() -> Fixture {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    let active_screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let slider = Slider::create(ui, active_screen);
    let slider_range_mode = Slider::create(ui, active_screen);
    let slider_normal_mode = Slider::create(ui, active_screen);
    let slider_symmetrical_mode = Slider::create(ui, active_screen);

    slider_range_mode.set_mode(ui, SliderMode::Range);
    slider_normal_mode.set_mode(ui, SliderMode::Normal);
    slider_symmetrical_mode.set_mode(ui, SliderMode::Symmetrical);

    let g = ui.group_create();
    ui.indev_set_group(inputs.encoder.id(), Some(g));

    Fixture {
        display,
        inputs,
        active_screen,
        slider,
        slider_range_mode,
        slider_normal_mode,
        slider_symmetrical_mode,
        g,
    }
}

/// `lv_obj_send_event(obj, LV_EVENT_KEY, &key)`
fn send_key(f: &mut Fixture, obj: Slider, key: u32) {
    f.display
        .ui
        .obj_send_event(obj.obj(), EventCode::KEY, EventParam::Key(key));
}

/// `lv_obj_send_event(obj, LV_EVENT_HIT_TEST, &info)`
fn send_hit_test(f: &mut Fixture, obj: Slider, info: &mut HitTestInfo) {
    f.display
        .ui
        .obj_send_event(obj.obj(), EventCode::HIT_TEST, EventParam::HitTest(info));
}

#[test]
fn textarea_should_have_valid_documented_default_values() {
    let f = set_up();
    let ui = &f.display.ui;
    let objw = ui.obj_get_width(f.slider.obj());
    let objh = ui.obj_get_height(f.slider.obj());

    /* Horizontal slider */
    assert!(objw >= objh);
    assert!(!(ui.obj_is_scroll_chain_hor(f.slider.obj()) && ui.obj_is_scroll_chain_ver(f.slider.obj())));
    assert!(!ui.obj_is_scrollable(f.slider.obj()));
}

#[test]
fn slider_event_keys_right_and_up_increment_value_by_one() {
    let mut f = set_up();
    let slider = f.slider;
    let mut key = KEY_RIGHT;
    slider.set_value(&mut f.display.ui, 10, ANIM_OFF);
    let value = slider.value(&f.display.ui);

    send_key(&mut f, slider, key);

    let new_value = slider.value(&f.display.ui);
    assert_eq!(value + 1, new_value);

    key = KEY_UP;
    send_key(&mut f, slider, key);
    assert_eq!(new_value + 1, slider.value(&f.display.ui));
}

#[test]
fn slider_event_keys_left_and_down_decrement_value_by_one() {
    let mut f = set_up();
    let slider = f.slider;
    let mut key = KEY_LEFT;
    slider.set_value(&mut f.display.ui, 10, ANIM_OFF);
    let value = slider.value(&f.display.ui);

    send_key(&mut f, slider, key);

    let new_value = slider.value(&f.display.ui);
    assert_eq!(value - 1, new_value);

    key = KEY_DOWN;
    send_key(&mut f, slider, key);
    assert_eq!(new_value - 1, slider.value(&f.display.ui));
}

#[test]
fn slider_event_invalid_key_should_not_change_values() {
    let mut f = set_up();
    let slider = f.slider;
    let key = KEY_ENTER;
    slider.set_value(&mut f.display.ui, 10, ANIM_OFF);
    let value = slider.value(&f.display.ui);

    send_key(&mut f, slider, key);

    assert_eq!(value, slider.value(&f.display.ui));
}

#[test]
fn slider_range_mode_should_leave_edit_mode_if_released() {
    let mut f = set_up();
    let ptr = f.slider_range_mode;

    /* Setup group and encoder indev */
    f.display.ui.group_add_obj(f.g, f.slider_normal_mode.obj());
    f.display.ui.group_set_editing(f.g, true);

    f.inputs.encoder_click(&mut f.display);

    /* Always executed when handling LV_EVENT_RELEASED or
     * LV_EVENT_PRESS_LOST */
    let ui = &f.display.ui;
    assert!(!ptr.is_dragged(ui));
    assert_eq!(None, ptr.value_to_set(ui));
    assert!(!ptr.left_knob_focus(ui));

    /* Group leaved edit mode */
    assert!(!ui.group_get_editing(f.g));
}

#[test]
fn slider_range_mode_should_not_leave_edit_mode_if_released_with_no_left_knob_focus() {
    let mut f = set_up();
    let ptr = f.slider_range_mode;

    /* Setup group and encoder indev */
    f.display.ui.group_add_obj(f.g, f.slider_range_mode.obj());
    f.display.ui.group_set_editing(f.g, true);

    f.inputs.encoder.release();
    f.display.wait(50);

    /* Always executed when handling LV_EVENT_RELEASED or
     * LV_EVENT_PRESS_LOST */
    let ui = &f.display.ui;
    assert!(!ptr.is_dragged(ui));
    assert_eq!(None, ptr.value_to_set(ui));

    assert!(ui.group_get_editing(f.g));
}

#[test]
fn slider_normal_mode_should_leave_edit_mode_if_released() {
    let mut f = set_up();
    let ptr = f.slider_normal_mode;
    ptr.set_left_knob_focus(&mut f.display.ui, true);

    /* Setup group and encoder indev */
    f.display.ui.group_add_obj(f.g, f.slider_normal_mode.obj());
    f.display.ui.group_set_editing(f.g, true);

    f.inputs.encoder_click(&mut f.display);

    /* Always executed when handling LV_EVENT_RELEASED or
     * LV_EVENT_PRESS_LOST */
    let ui = &f.display.ui;
    assert!(!ptr.is_dragged(ui));
    assert_eq!(None, ptr.value_to_set(ui));
    assert!(!ptr.left_knob_focus(ui));

    /* Group leaved edit mode */
    assert!(!ui.group_get_editing(f.g));
}

#[test]
fn ranged_mode_adjust_with_encoder() {
    let mut f = set_up();
    let s = f.slider_range_mode;
    s.set_value(&mut f.display.ui, 90, ANIM_OFF);
    s.set_start_value(&mut f.display.ui, 10, ANIM_OFF);

    /* Setup group and encoder indev */
    f.display.ui.group_add_obj(f.g, s.obj());
    f.display.ui.group_set_editing(f.g, false);

    /*Go the edit mode*/
    f.inputs.encoder_click(&mut f.display);

    /*Adjust the right knob*/
    f.inputs.encoder_turn(&mut f.display, -10);
    assert_eq!(80, s.value(&f.display.ui)); /*Updated?*/
    assert_eq!(10, s.left_value(&f.display.ui)); /*Maintained?*/

    /*Focus the left knob*/
    f.inputs.encoder_click(&mut f.display);

    /*Adjust the left knob*/
    f.inputs.encoder_turn(&mut f.display, 5);
    assert_eq!(80, s.value(&f.display.ui)); /*Maintained?*/
    assert_eq!(15, s.left_value(&f.display.ui)); /*Updated?*/
}

#[test]
fn normal_mode_slider_hit_test() {
    let mut f = set_up();
    /* Validate if point 0,0 can click in the slider */
    let point = Point::new(0, 0);

    let mut info = HitTestInfo { res: false, point };

    let s = f.slider_normal_mode;
    s.set_value(&mut f.display.ui, 100, ANIM_OFF);
    send_hit_test(&mut f, s, &mut info);

    /* point can click slider */
    assert!(info.res);
}

#[test]
fn slider_range_event_hit_test() {
    let mut f = set_up();
    /* Validate if point 0,0 can click in the slider */
    let point = Point::new(0, 0);

    let mut info = HitTestInfo { res: false, point };
    let s = f.slider_range_mode;
    send_hit_test(&mut f, s, &mut info);

    /* point can click slider in the left knob */
    assert!(info.res);
}

/// `test_slider_properties` is compiled only with `LV_USE_OBJ_PROPERTY`,
/// which the parity configuration (`full.defconfig`) leaves off, so upstream
/// runs an empty body. Property reflection is not implemented; the same
/// sequence is checked through the functions the `lv_slider_properties[]`
/// table maps to.
#[test]
fn slider_properties() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let obj = Slider::create(ui, f.active_screen);

    /*LV_PROPERTY_SLIDER_RANGE*/
    obj.set_range(ui, 10, 100);
    assert_eq!(obj.min_value(ui), 10);
    assert_eq!(obj.max_value(ui), 100);

    /*LV_PROPERTY_SLIDER_VALUE*/
    obj.set_value(ui, 50, ANIM_OFF);
    assert_eq!(obj.value(ui), 50);

    /*LV_PROPERTY_SLIDER_MODE*/
    obj.set_mode(ui, SliderMode::Range);
    assert_eq!(SliderMode::Range, obj.mode(ui));

    /*LV_PROPERTY_SLIDER_LEFT_VALUE*/
    obj.set_start_value(ui, 40, ANIM_OFF);
    assert_eq!(40, obj.left_value(ui));

    /*LV_PROPERTY_SLIDER_IS_DRAGGED*/
    assert!(!obj.is_dragged(ui));

    /*LV_PROPERTY_SLIDER_IS_SYMMETRICAL*/
    assert!(!obj.is_dragged(ui));
    assert!(!obj.is_symmetrical(ui));
}

#[test]
fn slider_return_true_when_dragged() {
    let mut f = set_up();
    let slider = f.slider;
    let ui = &mut f.display.ui;
    ui.obj_set_size(slider.obj(), 200, 20);
    ui.obj_center(slider.obj());
    ui.obj_update_layout(slider.obj());

    assert!(!slider.is_dragged(ui));

    let coords = ui.obj_get_coords(slider.obj());
    f.inputs.mouse.move_to(coords.x1 + 10, coords.y1 + 10);
    f.inputs.mouse.press();
    f.display.wait(50);

    f.inputs.mouse.move_by(10, 0);
    f.display.wait(50);

    assert!(slider.is_dragged(&f.display.ui));

    f.inputs.mouse.release();
    f.display.wait(50);

    assert!(!slider.is_dragged(&f.display.ui));
}

#[test]
fn slider_range_mode_hit_test_left_knob() {
    let mut f = set_up();
    let s = f.slider_range_mode;
    let ui = &mut f.display.ui;
    ui.obj_set_size(s.obj(), 200, 20);
    ui.obj_center(s.obj());

    /* Set values to ensure separation between knobs */
    s.set_value(ui, 90, ANIM_OFF);
    s.set_start_value(ui, 10, ANIM_OFF);
    ui.obj_update_layout(s.obj());

    /* Force redraw to update knob areas */
    ui.obj_invalidate(s.obj());
    ui.refr_now(None);

    /* Point on the left knob */
    let left = s.left_knob_area(ui);
    let point = Point::new((left.x1 + left.x2) / 2, (left.y1 + left.y2) / 2);

    let mut info = HitTestInfo { res: false, point };

    send_hit_test(&mut f, s, &mut info);

    assert!(info.res);
}

#[test]
fn slider_scroll_chain_horizontal() {
    let mut f = set_up();
    let slider = f.slider;
    f.display.ui.obj_set_size(slider.obj(), 200, 20);
    f.display.ui.obj_update_layout(slider.obj());

    /* Simulate POINTER release */
    f.inputs.mouse.release();
    f.display
        .ui
        .obj_send_event(slider.obj(), EventCode::RELEASED, EventParam::None);

    /* Horizontal ptr should allow vertical scroll chain */
    assert!(f.display.ui.obj_is_scroll_chain_ver(slider.obj()));
}

#[test]
fn slider_scroll_chain_vertical() {
    let mut f = set_up();
    let slider = f.slider;
    f.display.ui.obj_set_size(slider.obj(), 20, 200);
    f.display.ui.obj_update_layout(slider.obj());

    /* Simulate POINTER release */
    f.inputs.mouse.release();
    f.display
        .ui
        .obj_send_event(slider.obj(), EventCode::RELEASED, EventParam::None);

    /* Vertical ptr should allow horizontal scroll chain */
    assert!(f.display.ui.obj_is_scroll_chain_hor(slider.obj()));
}

#[test]
fn slider_range_mode_key_decrement_left_value() {
    let mut f = set_up();
    let s = f.slider_range_mode;
    /* Set range mode and initial values */
    s.set_mode(&mut f.display.ui, SliderMode::Range);
    s.set_value(&mut f.display.ui, 80, ANIM_OFF);
    s.set_start_value(&mut f.display.ui, 20, ANIM_OFF);

    /* Focus the left knob */
    s.set_left_knob_focus(&mut f.display.ui, true);

    /* Simulate LEFT key press */
    let mut key = KEY_LEFT;
    send_key(&mut f, s, key);

    /* Left value should decrement by 1 */
    assert_eq!(19, s.left_value(&f.display.ui));

    /* Simulate DOWN key press */
    key = KEY_DOWN;
    send_key(&mut f, s, key);

    /* Left value should decrement by 1 again */
    assert_eq!(18, s.left_value(&f.display.ui));
}

#[test]
fn slider_rotary_event_handling() {
    let mut f = set_up();
    let normal = f.slider_normal_mode;
    let range = f.slider_range_mode;
    /* Setup group and encoder indev */
    f.display.ui.group_add_obj(f.g, normal.obj());
    f.display.ui.group_set_editing(f.g, true);

    /* Normal mode (right knob focus) */
    normal.set_value(&mut f.display.ui, 50, ANIM_OFF);

    /* Simulate rotary right (+1) */
    f.inputs.encoder_turn(&mut f.display, 1);
    assert_eq!(51, normal.value(&f.display.ui));

    /* Simulate rotary left (-1) */
    f.inputs.encoder_turn(&mut f.display, -1);
    assert_eq!(50, normal.value(&f.display.ui));

    /* Range mode (left knob focus) */
    range.set_mode(&mut f.display.ui, SliderMode::Range);
    f.display.ui.group_add_obj(f.g, range.obj());

    /* Manually focus left knob to hit the else branch */
    range.set_left_knob_focus(&mut f.display.ui, true);

    range.set_value(&mut f.display.ui, 80, ANIM_OFF);
    range.set_start_value(&mut f.display.ui, 20, ANIM_OFF);

    /* Simulate rotary event directly to ensure coverage of the specific branch */
    let diff = 1;
    f.display
        .ui
        .obj_send_event(range.obj(), EventCode::ROTARY, EventParam::RotaryDiff(diff));
    assert_eq!(21, range.left_value(&f.display.ui));
}

/// `assert_symmetrical_slider_knob_drawn()`
fn assert_symmetrical_slider_knob_drawn(f: &mut Fixture, is_hor: bool, value: i32) {
    let ui = &mut f.display.ui;
    let screen = ui.display_get_screen_active(None).unwrap();
    ui.obj_clean(screen);
    let obj = Slider::create(ui, screen);
    ui.obj_center(obj.obj());
    if is_hor {
        ui.obj_set_size(obj.obj(), 200, 20);
    } else {
        ui.obj_set_size(obj.obj(), 20, 200);
    }

    obj.set_mode(ui, SliderMode::Symmetrical);
    obj.set_range(ui, -100, 100);
    obj.set_value(ui, value, ANIM_OFF);
    ui.obj_update_layout(obj.obj());

    let ref_screenshot_name = format!(
        "widgets/slider_sym_{}_value_{}_{}.png",
        if is_hor { "hor" } else { "ver" },
        if value >= 0 { "positive" } else { "negative" },
        value.abs()
    );
    f.display.screenshot_compare(&ref_screenshot_name).unwrap();

    let area = obj.right_knob_area(&f.display.ui);
    if is_hor {
        assert_ne!(0, area.x1);
        assert_ne!(0, area.x2);
    } else {
        assert_ne!(0, area.y1);
        assert_ne!(0, area.y2);
    }
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn slider_symmetrical_mode_knob_drawing() {
    let mut f = set_up();
    /* Horizontal Case (Negative) */
    assert_symmetrical_slider_knob_drawn(&mut f, true, -50);

    /* Vertical Case (Negative) */
    assert_symmetrical_slider_knob_drawn(&mut f, false, -50);

    /* Vertical Case (Positive) */
    assert_symmetrical_slider_knob_drawn(&mut f, false, 50);
}

#[test]
fn slider_range_mode_encoder_exit_edit_with_left_knob_focus() {
    let mut f = set_up();
    let s = f.slider_range_mode;
    s.set_mode(&mut f.display.ui, SliderMode::Range);

    /* Setup group and encoder indev */
    f.display.ui.group_add_obj(f.g, s.obj());
    f.display.ui.group_set_editing(f.g, true);

    /* Manually focus left knob */
    s.set_left_knob_focus(&mut f.display.ui, true);

    /* Simulate encoder click (Release event) */
    f.inputs.encoder_click(&mut f.display);

    assert!(!s.left_knob_focus(&f.display.ui));
    assert!(!f.display.ui.group_get_editing(f.g));
}

/// `assert_slider_drag_start_selection()`. `expected_focus < 0` skips the
/// `left_knob_focus` check.
fn assert_slider_drag_start_selection(
    f: &mut Fixture,
    obj: Slider,
    x: i32,
    y: i32,
    expected_value: SliderValueTarget,
    expected_focus: i32,
) {
    f.inputs.mouse.move_to(x, y);
    f.inputs.mouse.press();
    f.display.wait(50);

    /* Move slightly to trigger drag (exceed scroll_limit) */
    f.inputs.mouse.move_by(10, 10);
    f.display.wait(50);

    let ui = &f.display.ui;
    assert!(obj.is_dragged(ui));
    assert_eq!(Some(expected_value), obj.value_to_set(ui));
    if expected_focus >= 0 {
        assert_eq!(expected_focus, i32::from(obj.left_knob_focus(ui)));
    }

    f.inputs.mouse.release();
    f.display.wait(50);
}

/// The common scene setup of the four `*_drag_start_value_selection` tests.
fn range_slider_scene(f: &mut Fixture, obj: Slider, w: i32, h: i32, rtl: bool) {
    let ui = &mut f.display.ui;
    ui.obj_set_size(obj.obj(), w, h);
    ui.obj_center(obj.obj());
    obj.set_mode(ui, SliderMode::Range);
    if rtl {
        ui.obj_set_style_base_dir(obj.obj(), BaseDir::Rtl, MAIN);
    }

    obj.set_value(ui, 80, ANIM_OFF);
    obj.set_start_value(ui, 20, ANIM_OFF);
    ui.obj_update_layout(obj.obj());

    /* Force redraw to update knob areas */
    ui.obj_invalidate(obj.obj());
    ui.refr_now(None);
}

#[test]
fn slider_range_mode_drag_start_value_selection() {
    let mut f = set_up();
    let s = f.slider_range_mode;
    /* Right knob at ~80%, left knob at ~20% */
    range_slider_scene(&mut f, s, 200, 20, false);

    let right_knob = s.right_knob_area(&f.display.ui);
    let left_knob = s.left_knob_area(&f.display.ui);

    /* Click to the right of the right knob (Horizontal) */
    assert_slider_drag_start_selection(
        &mut f,
        s,
        right_knob.x2 + 10,
        (right_knob.y1 + right_knob.y2) / 2,
        SliderValueTarget::CurValue,
        -1,
    );

    /* Click to the left of the left knob (Horizontal) */
    assert_slider_drag_start_selection(
        &mut f,
        s,
        left_knob.x1 - 10,
        (left_knob.y1 + left_knob.y2) / 2,
        SliderValueTarget::StartValue,
        -1,
    );

    /* Click between knobs, closer to right knob (Horizontal) */
    assert_slider_drag_start_selection(
        &mut f,
        s,
        right_knob.x1 - 5,
        (right_knob.y1 + right_knob.y2) / 2,
        SliderValueTarget::CurValue,
        0,
    );

    /* Click between knobs, closer to left knob (Horizontal) */
    assert_slider_drag_start_selection(
        &mut f,
        s,
        left_knob.x2 + 5,
        (left_knob.y1 + left_knob.y2) / 2,
        SliderValueTarget::StartValue,
        1,
    );
}

#[test]
fn slider_vertical_range_mode_drag_start_value_selection() {
    let mut f = set_up();
    let slider_ver = Slider::create(&mut f.display.ui, f.active_screen);
    range_slider_scene(&mut f, slider_ver, 20, 200, false);

    let top_knob = slider_ver.right_knob_area(&f.display.ui);
    let bottom_knob = slider_ver.left_knob_area(&f.display.ui);

    /* Click above the top knob (Vertical) */
    assert_slider_drag_start_selection(
        &mut f,
        slider_ver,
        (top_knob.x1 + top_knob.x2) / 2,
        top_knob.y1 - 10,
        SliderValueTarget::CurValue,
        -1,
    );

    /* Click below the bottom knob (Vertical) */
    assert_slider_drag_start_selection(
        &mut f,
        slider_ver,
        (bottom_knob.x1 + bottom_knob.x2) / 2,
        bottom_knob.y2 + 10,
        SliderValueTarget::StartValue,
        -1,
    );

    /* Click between knobs, closer to top/right knob (Vertical) */
    assert_slider_drag_start_selection(
        &mut f,
        slider_ver,
        (top_knob.x1 + top_knob.x2) / 2,
        top_knob.y2 + 5,
        SliderValueTarget::CurValue,
        0,
    );

    /* Click between knobs, closer to bottom/left knob (Vertical) */
    assert_slider_drag_start_selection(
        &mut f,
        slider_ver,
        (bottom_knob.x1 + bottom_knob.x2) / 2,
        bottom_knob.y1 - 5,
        SliderValueTarget::StartValue,
        1,
    );
}

#[test]
fn slider_range_mode_horizontal_rtl_drag_start_value_selection() {
    let mut f = set_up();
    let s = f.slider_range_mode;
    range_slider_scene(&mut f, s, 200, 20, true);

    let left_knob = s.left_knob_area(&f.display.ui);

    assert_slider_drag_start_selection(
        &mut f,
        s,
        left_knob.x2 + 10,
        (left_knob.y1 + left_knob.y2) / 2,
        SliderValueTarget::StartValue,
        -1,
    );
}

#[test]
fn slider_range_mode_vertical_rtl_drag_start_value_selection() {
    let mut f = set_up();
    let slider_ver = Slider::create(&mut f.display.ui, f.active_screen);
    range_slider_scene(&mut f, slider_ver, 20, 200, true);

    let right_knob = slider_ver.right_knob_area(&f.display.ui);

    /* Click above the top/right knob */
    assert_slider_drag_start_selection(
        &mut f,
        slider_ver,
        (right_knob.x1 + right_knob.x2) / 2,
        right_knob.y1 - 20,
        SliderValueTarget::CurValue,
        -1,
    );
}

#[test]
fn slider_large_range_drag_no_int32_overflow_horiz() {
    let mut f = set_up();
    let slider = f.slider;
    /* Prevent int32 overflow during horizontal drag.
     * 600px slider x range 20M → pixel * range > INT32_MAX for pixel > ~107. */
    let ui = &mut f.display.ui;
    ui.obj_set_size(slider.obj(), 600, 20);
    ui.obj_center(slider.obj());
    slider.set_range(ui, 0, 20_000_000);
    slider.set_value(ui, 0, ANIM_OFF);
    ui.obj_update_layout(slider.obj());
    ui.refr_now(None);

    /* Drag from center to right: pixel ~300 → value should be ~10M (not overflowed) */
    f.inputs.mouse.move_to_obj(&mut f.display.ui, slider.obj());
    f.inputs.mouse.press();
    f.display.wait(50);
    f.inputs.mouse.move_by(150, 0); /* drag right ~50% more */
    f.display.wait(50);
    f.inputs.mouse.release();
    f.display.wait(50);

    let val = slider.value(&f.display.ui);
    /* Value must NOT be 0 (overflow collapse). Should be in upper half of range. */
    assert!(val > 5_000_000);
    assert!(val <= 20_000_000);
}

#[test]
fn slider_large_range_drag_no_int32_overflow_vert() {
    let mut f = set_up();
    let slider = f.slider;
    /* Prevent int32 overflow during vertical drag. */
    let ui = &mut f.display.ui;
    ui.obj_set_size(slider.obj(), 20, 400);
    ui.obj_center(slider.obj());
    slider.set_range(ui, 0, 20_000_000);
    slider.set_value(ui, 0, ANIM_OFF);
    ui.obj_update_layout(slider.obj());
    ui.refr_now(None);

    f.inputs.mouse.move_to_obj(&mut f.display.ui, slider.obj());
    f.inputs.mouse.press();
    f.display.wait(50);
    f.inputs.mouse.move_by(0, -100);
    f.display.wait(50);
    f.inputs.mouse.release();
    f.display.wait(50);

    let val = slider.value(&f.display.ui);
    assert!(val > 5_000_000);
    assert!(val <= 20_000_000);
}

#[test]
fn slider_large_range_key_up_increments() {
    let mut f = set_up();
    let slider = f.slider;
    let ui = &mut f.display.ui;
    ui.obj_set_size(slider.obj(), 600, 20);
    slider.set_range(ui, 0, 20_000_000);
    slider.set_value(ui, 10_000_000, ANIM_OFF);

    let key = KEY_RIGHT;
    send_key(&mut f, slider, key);

    let val = slider.value(&f.display.ui);
    assert!(val > 10_000_000);
}

#[test]
fn slider_knob_follows_drag_while_parent_is_scrolled() {
    let mut f = set_up();
    /* A slider dragged to its end doesn't change value, so it keeps its vertical scroll
     * chain and a vertical move can make the parent the indev's scroll object.
     * The knob must keep following the pointer anyway because the drag is already ongoing. */
    let ui = &mut f.display.ui;
    let cont = ui.obj_create(Some(f.active_screen));
    ui.obj_set_size(cont, 300, 160);
    ui.obj_center(cont);
    ui.obj_set_scroll_dir(cont, Dir::VER);
    ui.obj_set_style_pad_all(cont, 0, MAIN);
    /* No momentum so the scroll ends with the release */
    ui.obj_set_scroll_momentum(cont, false);

    let slider_in_cont = Slider::create(ui, cont);
    ui.obj_set_size(slider_in_cont.obj(), 200, 20);
    ui.obj_set_pos(slider_in_cont.obj(), 0, 60);
    slider_in_cont.set_value(ui, 100, ANIM_OFF);

    let value_changed_cnt = Rc::new(Cell::new(0u32));
    let counter = value_changed_cnt.clone();
    ui.obj_add_event_cb(
        slider_in_cont.obj(),
        Rc::new(move |_, _| counter.set(counter.get() + 1)),
        EventCode::VALUE_CHANGED,
        None,
    );

    /* Make the container really scrollable downwards */
    let spacer = ui.obj_create(Some(cont));
    ui.obj_set_size(spacer, 10, 100);
    ui.obj_set_pos(spacer, 0, 200);

    ui.obj_update_layout(cont);
    ui.refr_now(None);
    assert!(ui.obj_get_scroll_bottom(cont) > 0);

    let coords = ui.obj_get_coords(slider_in_cont.obj());
    let press_y = (coords.y1 + coords.y2) / 2;
    let target_x = (coords.x1 + coords.x2) / 2;

    /* Press at the right end and drag further right: dragging starts but the value
     * stays clamped at the maximum */
    f.inputs.mouse.move_to(coords.x2 - 2, press_y);
    f.inputs.mouse.press();
    f.display.wait(50);
    f.inputs.mouse.move_by(20, 0);
    f.display.wait(50);

    assert!(slider_in_cont.is_dragged(&f.display.ui));
    assert_eq!(100, slider_in_cont.value(&f.display.ui));
    assert_eq!(0, value_changed_cnt.get());

    /* Move up so the indev starts scrolling the container */
    f.inputs.mouse.move_by(0, -40);
    f.display.wait(50);

    let mouse = f.inputs.mouse.id();
    assert_eq!(Some(cont), f.display.ui.indev_get_scroll_obj(mouse));
    assert!(slider_in_cont.is_dragged(&f.display.ui));

    /* Drag back to the middle. The knob must follow even though a scroll object is active */
    f.inputs.mouse.move_by(target_x - (coords.x2 - 2 + 20), 0);
    f.display.wait(50);

    let value = slider_in_cont.value(&f.display.ui);
    assert!((value - 50).abs() <= 5, "value {value} not within 5 of 50");
    assert!(value_changed_cnt.get() > 0);

    f.inputs.mouse.release();
    f.display.wait(200);
}
