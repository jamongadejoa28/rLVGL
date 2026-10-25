//! Real input/timer/group/style/renderer integration, without a window server.

use std::cell::RefCell;
use std::rc::Rc;

use ferrovgl_core::group::{KEY_NEXT, KEY_PREV};
use ferrovgl_core::indev::IndevType;
use ferrovgl_core::{Color, EventCode, Obj, ObjFlags, Part, Point, State, StyleSelector};
use ferrovgl_test::{SyntheticInput, TestDisplay};

fn rectangle(display: &mut TestDisplay, x: i32, y: i32, w: i32, h: i32) -> Obj {
    let obj = display.ui.obj_create(display.screen());
    display.ui.obj_set_pos(obj, x, y);
    display.ui.obj_set_size(obj, w, h);
    display.ui.obj_set_style_bg_opa(obj, 255, StyleSelector::DEFAULT);
    display
        .ui
        .obj_set_style_bg_color(obj, Color::hex(0x123456), StyleSelector::DEFAULT);
    obj
}

#[test]
fn timed_pointer_press_changes_pixels_and_release_delivers_one_click() {
    let mut display = TestDisplay::with_resolution(96, 64).unwrap();
    let obj = rectangle(&mut display, 10, 10, 50, 30);
    display.ui.obj_set_style_bg_color(
        obj,
        Color::hex(0xabcdef),
        StyleSelector::new(Part::MAIN, State::PRESSED),
    );
    let inputs = display.create_inputs();
    let log = Rc::new(RefCell::new(Vec::new()));
    let events = log.clone();
    display.ui.obj_add_event_cb(
        obj,
        Rc::new(move |_, e| {
            if [
                EventCode::PRESSED,
                EventCode::RELEASED,
                EventCode::SHORT_CLICKED,
                EventCode::CLICKED,
            ]
            .contains(&e.code)
            {
                events.borrow_mut().push(e.code);
            }
        }),
        EventCode::ALL,
        None,
    );
    assert_eq!(
        display.snapshot().unwrap().pixel(30, 20),
        Some([0x12, 0x34, 0x56])
    );
    inputs.mouse.move_to(30, 20);
    inputs.mouse.press();
    display.wait(50);
    assert!(display.ui.obj_has_state(obj, State::PRESSED));
    assert_eq!(
        display.snapshot().unwrap().pixel(30, 20),
        Some([0xab, 0xcd, 0xef])
    );
    inputs.mouse.release();
    display.wait(50);
    assert!(!display.ui.obj_has_state(obj, State::PRESSED));
    assert_eq!(
        display.snapshot().unwrap().pixel(30, 20),
        Some([0x12, 0x34, 0x56])
    );
    assert_eq!(
        *log.borrow(),
        [
            EventCode::PRESSED,
            EventCode::RELEASED,
            EventCode::SHORT_CLICKED,
            EventCode::CLICKED
        ]
    );
    display.ui.deinit();
    assert!(display.ui.indevs.is_empty());
}

#[test]
fn keypad_and_encoder_use_focus_group_and_consume_rotation_once() {
    let mut display = TestDisplay::with_resolution(160, 80).unwrap();
    let a = rectangle(&mut display, 0, 0, 40, 40);
    let b = rectangle(&mut display, 50, 0, 40, 40);
    let c = rectangle(&mut display, 100, 0, 40, 40);
    let group = display.ui.group_create();
    for obj in [a, b, c] {
        display.ui.group_add_obj(group, obj);
    }
    let inputs = display.create_inputs();
    inputs.set_group(&mut display.ui, Some(group));
    inputs.key_hit(&mut display, KEY_NEXT);
    assert_eq!(display.ui.group_get_focused(group), Some(b));
    assert!(display.ui.obj_has_state(b, State::FOCUS_KEY));
    inputs.key_hit(&mut display, KEY_PREV);
    assert_eq!(display.ui.group_get_focused(group), Some(a));
    inputs.encoder_turn(&mut display, 2);
    assert_eq!(display.ui.group_get_focused(group), Some(c));
    display.wait(200);
    assert_eq!(display.ui.group_get_focused(group), Some(c));
    display.ui.group_delete(group);
    assert_eq!(display.ui.indev_get_group(inputs.keypad.id()), None);
    assert_eq!(display.ui.indev_get_group(inputs.encoder.id()), None);
}

#[test]
fn button_input_uses_configured_point_and_click_deletion_is_safe() {
    let mut display = TestDisplay::with_resolution(80, 60).unwrap();
    let obj = rectangle(&mut display, 10, 10, 40, 30);
    let id = display.display();
    let button = SyntheticInput::new(&mut display.ui, id, IndevType::Button);
    display
        .ui
        .indev_set_button_points(button.id(), vec![Point::new(20, 20)]);
    display.ui.obj_add_event_cb(
        obj,
        Rc::new(move |ui, _| ui.obj_delete(obj)),
        EventCode::CLICKED,
        None,
    );
    display.refresh();
    button.set_button(0);
    button.press();
    display.wait(50);
    button.release();
    display.wait(50);
    assert!(!display.ui.obj_is_valid(obj));
    assert_eq!(display.ui.indev_get_active_obj(), None);
    display.wait(500);
    display.ui.deinit();
}

#[test]
fn pointer_drag_scrolls_overflow_and_does_not_click() {
    let mut display = TestDisplay::with_resolution(140, 120).unwrap();
    let parent = rectangle(&mut display, 10, 10, 100, 80);
    let child = display.ui.obj_create(Some(parent));
    display.ui.obj_set_size(child, 80, 250);
    display
        .ui
        .obj_remove_flag(child, ObjFlags::CLICKABLE | ObjFlags::SCROLLABLE);
    let inputs = display.create_inputs();
    let clicks = Rc::new(RefCell::new(0));
    let count = clicks.clone();
    display.ui.obj_add_event_cb(
        parent,
        Rc::new(move |_, _| *count.borrow_mut() += 1),
        EventCode::CLICKED,
        None,
    );
    display.refresh();
    inputs.mouse.move_to(50, 70);
    inputs.mouse.press();
    display.wait(50);
    for y in [55, 40, 25] {
        inputs.mouse.move_to(50, y);
        display.wait(40);
    }
    inputs.mouse.release();
    display.wait(50);
    assert!(display.ui.obj_get_scroll_y(parent) > 0);
    assert_eq!(*clicks.borrow(), 0);
    display.wait(500);
    display.ui.deinit();
}
