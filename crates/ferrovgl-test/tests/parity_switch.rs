//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_switch.c`.
//!
//! Every upstream test function is ported with its original scenario and
//! values. Each Rust test starts from upstream's `setUp()` (a switch created on
//! the active screen of a default-themed display). Upstream runs all functions
//! in one process without cleaning the screen in `tearDown()`; the earlier
//! switches stay below the new one at the same position, so the click target
//! and every asserted value are the same in a fresh display.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::{EventCode, Obj, Part, State, Style};
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::switch::SWITCH_KNOB_EXT_AREA_CORRECTION;
use ferrovgl_widgets::{Switch, SwitchOrientation};

/// `SWITCHES_CNT`
const SWITCHES_CNT: usize = 10;

/// `LV_SWITCH_ANIM_STATE_INV` (private in `lv_switch.c`).
const SWITCH_ANIM_STATE_INV: i32 = -1;

/// `setUp()`: `scr = lv_screen_active(); sw = lv_switch_create(scr);`
fn set_up() -> (TestDisplay, TestInputs, Obj, Switch) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    let scr = display.screen().unwrap();
    let sw = Switch::create(&mut display.ui, scr);
    (display, inputs, scr, sw)
}

/// `mouse_click_on_switch()`: click the switch's top-left coordinate.
fn mouse_click_on_switch(display: &mut TestDisplay, inputs: &TestInputs, sw: Switch) {
    let coords = display.ui.obj_get_coords(sw.obj());
    inputs.mouse_click_at(display, coords.x1, coords.y1);
}

/// `test_switch_should_have_default_state_after_being_created`
#[test]
fn switch_should_have_default_state_after_being_created() {
    let (display, _inputs, _scr, sw) = set_up();
    let state = display.ui.obj_get_state(sw.obj());
    assert_eq!(state, State::DEFAULT);
}

/// `test_switch_should_not_leak_memory_after_deletion`. `lv_test_get_free_mem`
/// has no counterpart in the arena allocator; the port checks that every
/// created switch is deleted and the screen's child list is restored.
#[test]
fn switch_should_not_leak_memory_after_deletion() {
    let (mut display, _inputs, scr, _sw) = set_up();
    let initial_child_cnt = display.ui.obj_get_child_count(scr);
    let mut switches = [None; SWITCHES_CNT];

    for slot in &mut switches {
        *slot = Some(Switch::create(&mut display.ui, scr));
    }

    for sw in switches.iter().flatten() {
        display.ui.obj_delete(sw.obj());
    }

    for sw in switches.iter().flatten() {
        assert!(!display.ui.obj_is_valid(sw.obj()));
    }
    assert_eq!(display.ui.obj_get_child_count(scr), initial_child_cnt);
}

/// `test_switch_animation`
#[test]
fn switch_animation() {
    let (mut display, inputs, _scr, sw) = set_up();
    let initial_anim_state = sw.anim_state(&display.ui);
    assert_eq!(initial_anim_state, SWITCH_ANIM_STATE_INV);

    /* Trigger animation */
    mouse_click_on_switch(&mut display, &inputs, sw);
    /* Wait some time  */
    display.wait(50);

    let checked_anim_state = sw.anim_state(&display.ui);
    assert!(initial_anim_state < checked_anim_state);
    assert!(display.ui.obj_has_state(sw.obj(), State::CHECKED));

    mouse_click_on_switch(&mut display, &inputs, sw);
    display.wait(50);

    assert!(checked_anim_state > sw.anim_state(&display.ui));
    assert!(!display.ui.obj_has_state(sw.obj(), State::CHECKED));
}

/// `test_switch_should_not_have_extra_draw_size_at_creation`
#[test]
fn switch_should_not_have_extra_draw_size_at_creation() {
    let (display, _inputs, _scr, sw) = set_up();
    let extra_size = display.ui.obj_get_ext_draw_size(sw.obj());

    assert_eq!(0, extra_size);
}

/// `test_switch_should_update_extra_draw_size_after_editing_padding`
#[test]
fn switch_should_update_extra_draw_size_after_editing_padding() {
    let (mut display, _inputs, _scr, sw) = set_up();
    let pad = 6;
    let expected = pad + SWITCH_KNOB_EXT_AREA_CORRECTION;

    let mut style_knob = Style::new();
    style_knob.set_pad_all(pad);
    let style_knob = display.ui.style_create(style_knob);

    display.ui.obj_remove_style_all(sw.obj());
    display.ui.obj_add_style(sw.obj(), style_knob, Part::KNOB.into());
    display.ui.obj_center(sw.obj());

    /* Get extra draw size */
    let actual = display.ui.obj_get_ext_draw_size(sw.obj());

    assert_eq!(expected, actual);
}

/// `test_switch_should_trigger_value_changed_event_only_once` (see #2330).
#[test]
fn switch_should_trigger_value_changed_event_only_once() {
    let (mut display, inputs, _scr, sw) = set_up();
    let value_changed_event_cnt = Rc::new(Cell::new(0u8));
    let counter = value_changed_event_cnt.clone();
    display.ui.obj_add_event_cb(
        sw.obj(),
        Rc::new(move |_, e| {
            if e.code() == EventCode::VALUE_CHANGED {
                counter.set(counter.get() + 1);
            }
        }),
        EventCode::ALL,
        None,
    );
    mouse_click_on_switch(&mut display, &inputs, sw);

    assert_eq!(1, value_changed_event_cnt.get());
}

/// `test_switch_should_state_change_when_event_bubbling_is_enabled` (see #2785).
#[test]
fn switch_should_state_change_when_event_bubbling_is_enabled() {
    let (mut display, inputs, _scr, sw) = set_up();
    display.ui.obj_set_event_bubble(sw.obj(), true);
    mouse_click_on_switch(&mut display, &inputs, sw);

    assert!(display.ui.obj_has_state(sw.obj(), State::CHECKED));
}

/// `test_screeshots` (sic): six switches in a centered flex column.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn switch_screeshots() {
    let (mut display, _inputs, scr, _sw) = set_up();
    let active = display.screen().unwrap();
    display.ui.obj_set_flex_flow(active, FlexFlow::Column);
    display
        .ui
        .obj_set_flex_align(active, FlexAlign::Center, FlexAlign::Center, FlexAlign::Center);
    display.ui.obj_clean(scr);

    let _sw = Switch::create(&mut display.ui, active);

    let sw = Switch::create(&mut display.ui, active);
    display.ui.obj_add_state(sw.obj(), State::CHECKED);

    let _sw = Switch::create(&mut display.ui, active);

    let sw = Switch::create(&mut display.ui, active);
    display
        .ui
        .obj_add_state(sw.obj(), State::CHECKED | State::DISABLED);

    let sw = Switch::create(&mut display.ui, active);
    sw.set_orientation(&mut display.ui, SwitchOrientation::Vertical);
    display.ui.obj_set_size(sw.obj(), 50, 100);

    let sw = Switch::create(&mut display.ui, active);
    sw.set_orientation(&mut display.ui, SwitchOrientation::Vertical);
    display.ui.obj_add_state(sw.obj(), State::CHECKED);
    display.ui.obj_set_size(sw.obj(), 50, 100);

    display.wait(1000); /*Wait for the transitions*/

    display.screenshot_compare("widgets/switch_1.png").unwrap();
}

/// `test_switch_properties`. `tests/configs/full.defconfig` leaves
/// `LV_USE_OBJ_PROPERTY` off, so the upstream body is compiled out there. The
/// `lv_switch_get_orientation()` half of the scenario is kept through the
/// public setter, which the property setter forwards to.
#[test]
fn switch_properties() {
    let (mut display, _inputs, _scr, _sw) = set_up();
    let active = display.screen().unwrap();
    let obj = Switch::create(&mut display.ui, active);

    /* Test ORIENTATION property */
    obj.set_orientation(&mut display.ui, SwitchOrientation::Vertical);
    assert_eq!(SwitchOrientation::Vertical, obj.orientation(&display.ui));

    obj.set_orientation(&mut display.ui, SwitchOrientation::Horizontal);
    assert_eq!(SwitchOrientation::Horizontal, obj.orientation(&display.ui));

    display.ui.obj_delete(obj.obj());
    assert!(!display.ui.obj_is_valid(obj.obj()));
}
