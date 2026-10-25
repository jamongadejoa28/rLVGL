use super::*;
use crate::core::UiConfig;
use crate::core::obj::ScrollSnap;
use crate::group::{KEY_ENTER, KEY_LEFT, KEY_NEXT, KEY_RIGHT};
use crate::misc::style::{State, StyleSelector};
use alloc::collections::VecDeque;
use alloc::vec;
use core::cell::{Cell, RefCell};

struct SampleDriver(Rc<RefCell<IndevReadData>>);
impl IndevDriver for SampleDriver {
    fn read(&mut self, _: IndevId, data: &mut IndevReadData) {
        *data = self.0.borrow().clone();
    }
}

fn setup(ty: IndevType) -> (Ui, Obj, Obj, IndevId, Rc<RefCell<IndevReadData>>) {
    let mut ui = Ui::new(UiConfig::default());
    ui.display_create(320, 240);
    let screen = ui.screen_active().unwrap();
    let obj = ui.obj_create(Some(screen));
    ui.obj_set_pos(obj, 10, 10);
    ui.obj_set_size(obj, 100, 80);
    ui.obj_update_layout(screen);
    let indev = ui.indev_create();
    ui.indev_set_type(indev, ty);
    let sample = Rc::new(RefCell::new(IndevReadData {
        point: Point::new(30, 30),
        key: KEY_ENTER,
        ..Default::default()
    }));
    ui.indev_set_driver(indev, Some(Box::new(SampleDriver(sample.clone()))));
    (ui, screen, obj, indev, sample)
}

fn feed(ui: &mut Ui, indev: IndevId, sample: &RefCell<IndevReadData>, state: IndevState, time: u32) {
    sample.borrow_mut().state = state;
    sample.borrow_mut().timestamp = time;
    ui.indev_read(indev);
}

fn input_code(code: EventCode) -> bool {
    matches!(
        code,
        EventCode::PRESSED
            | EventCode::PRESSING
            | EventCode::RELEASED
            | EventCode::SHORT_CLICKED
            | EventCode::SINGLE_CLICKED
            | EventCode::DOUBLE_CLICKED
            | EventCode::TRIPLE_CLICKED
            | EventCode::CLICKED
            | EventCode::LONG_PRESSED
            | EventCode::LONG_PRESSED_REPEAT
    )
}

#[test]
fn pointer_press_release_streak_and_long_press_event_order() {
    let (mut ui, _, obj, indev, sample) = setup(IndevType::Pointer);
    let events = Rc::new(RefCell::new(Vec::new()));
    let log = events.clone();
    ui.indev_add_event_cb(
        indev,
        Rc::new(move |ui, e| {
            if input_code(e.code) {
                assert_eq!(ui.indev_active(), Some(indev));
                log.borrow_mut().push((0, e.code));
            }
        }),
        EventCode::ALL,
        None,
    );
    let log = events.clone();
    ui.obj_add_event_cb(
        obj,
        Rc::new(move |_, e| {
            if input_code(e.code) {
                log.borrow_mut().push((1, e.code));
            }
        }),
        EventCode::ALL,
        None,
    );
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    assert!(ui.obj_has_state(obj, State::PRESSED));
    feed(&mut ui, indev, &sample, IndevState::Released, 110);
    assert!(!ui.obj_has_state(obj, State::PRESSED));
    assert_eq!(
        *events.borrow(),
        vec![
            (0, EventCode::PRESSED),
            (1, EventCode::PRESSED),
            (1, EventCode::PRESSING),
            (0, EventCode::RELEASED),
            (1, EventCode::RELEASED),
            (0, EventCode::SHORT_CLICKED),
            (1, EventCode::SHORT_CLICKED),
            (1, EventCode::SINGLE_CLICKED),
            (0, EventCode::CLICKED),
            (1, EventCode::CLICKED)
        ]
    );
    for (press, release, expected) in [
        (200, 210, EventCode::DOUBLE_CLICKED),
        (300, 310, EventCode::TRIPLE_CLICKED),
    ] {
        events.borrow_mut().clear();
        feed(&mut ui, indev, &sample, IndevState::Pressed, press);
        feed(&mut ui, indev, &sample, IndevState::Released, release);
        assert!(events.borrow().contains(&(1, expected)));
    }
    events.borrow_mut().clear();
    feed(&mut ui, indev, &sample, IndevState::Pressed, 500);
    feed(&mut ui, indev, &sample, IndevState::Pressed, 900);
    feed(&mut ui, indev, &sample, IndevState::Pressed, 1000);
    feed(&mut ui, indev, &sample, IndevState::Released, 1010);
    let log = events.borrow();
    assert!(log.contains(&(1, EventCode::LONG_PRESSED)));
    assert!(log.contains(&(1, EventCode::LONG_PRESSED_REPEAT)));
    assert!(log.contains(&(1, EventCode::CLICKED)));
    assert!(!log.contains(&(1, EventCode::SHORT_CLICKED)));
    assert_eq!(ui.indev_active(), None);
    assert_eq!(ui.indev_get_active_obj(), None);
}

#[test]
fn consuming_device_callback_skips_only_the_corresponding_object_event() {
    let (mut ui, _, obj, indev, sample) = setup(IndevType::Pointer);
    ui.indev_add_event_cb(
        indev,
        Rc::new(move |ui, _| ui.indev_stop_processing(indev)),
        EventCode::PRESSED,
        None,
    );
    let released = Rc::new(Cell::new(false));
    let flag = released.clone();
    ui.obj_add_event_cb(
        obj,
        Rc::new(move |_, _| flag.set(true)),
        EventCode::RELEASED,
        None,
    );
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    assert!(!ui.obj_has_state(obj, State::PRESSED));
    feed(&mut ui, indev, &sample, IndevState::Released, 110);
    assert!(released.get());
    assert!(!ui.indev(indev).unwrap().stop_processing_query);
}

#[test]
fn object_and_device_deletion_during_press_stop_dispatch() {
    let (mut ui, _, obj, indev, sample) = setup(IndevType::Pointer);
    ui.obj_add_event_cb(
        obj,
        Rc::new(move |ui, _| ui.obj_delete(obj)),
        EventCode::PRESSED,
        None,
    );
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    assert!(!ui.obj_is_valid(obj));
    assert_eq!(ui.indev(indev).unwrap().pointer.act_obj, None);
    feed(&mut ui, indev, &sample, IndevState::Released, 110);
    let timer = ui.indev_get_read_timer(indev).unwrap();
    let delete_count = Rc::new(Cell::new(0));
    let count = delete_count.clone();
    ui.indev_add_event_cb(
        indev,
        Rc::new(move |ui, _| {
            count.set(count.get() + 1);
            ui.indev_delete(indev);
        }),
        EventCode::DELETE,
        None,
    );
    ui.indev_add_event_cb(
        indev,
        Rc::new(move |ui, _| ui.indev_delete(indev)),
        EventCode::PRESSED,
        None,
    );
    feed(&mut ui, indev, &sample, IndevState::Pressed, 200);
    assert!(ui.indev(indev).is_none());
    assert!(ui.timer(timer).is_none());
    assert_eq!(delete_count.get(), 1);
    assert_eq!(ui.indev_active(), None);
}

#[test]
fn read_callback_can_delete_or_detach_itself_and_recursive_read_is_ignored() {
    struct CallbackDriver {
        delete: bool,
        calls: Rc<Cell<u32>>,
    }
    impl IndevDriver for CallbackDriver {
        fn read(&mut self, _: IndevId, _: &mut IndevReadData) {
            unreachable!()
        }
        fn read_with_ui(&mut self, ui: &mut Ui, id: IndevId, _: &mut IndevReadData) {
            self.calls.set(self.calls.get() + 1);
            ui.indev_read(id);
            if self.delete {
                ui.indev_delete(id);
            } else {
                ui.indev_set_driver(id, None);
            }
        }
    }
    for delete in [false, true] {
        let (mut ui, _, _, indev, _) = setup(IndevType::Pointer);
        let calls = Rc::new(Cell::new(0));
        ui.indev_set_driver(
            indev,
            Some(Box::new(CallbackDriver {
                delete,
                calls: calls.clone(),
            })),
        );
        ui.indev_read(indev);
        assert_eq!(calls.get(), 1);
        assert!(ui.indev(indev).is_none_or(|d| d.driver.is_none()));
        assert_eq!(ui.indev_active(), None);
    }
}

#[test]
fn event_mode_timer_wait_release_and_timestamp_wrap() {
    let (mut ui, _, obj, indev, sample) = setup(IndevType::Pointer);
    ui.indev_set_mode(indev, IndevMode::Event);
    let timer = ui.indev_get_read_timer(indev).unwrap();
    assert!(ui.timer_get_paused(timer));
    feed(&mut ui, indev, &sample, IndevState::Pressed, u32::MAX - 200);
    assert!(!ui.timer_get_paused(timer));
    feed(&mut ui, indev, &sample, IndevState::Pressed, 200);
    assert!(ui.indev(indev).unwrap().long_pr_sent);
    ui.indev_wait_release(indev);
    feed(&mut ui, indev, &sample, IndevState::Released, 210);
    assert!(ui.timer_get_paused(timer));
    assert!(!ui.obj_has_state(obj, State::PRESSED));
    assert!(!ui.indev(indev).unwrap().wait_until_release);
}

#[test]
fn pointer_raw_position_rotation_and_missing_release_coordinates() {
    struct ReleaseDriver;
    impl IndevDriver for ReleaseDriver {
        fn read(&mut self, _: IndevId, _: &mut IndevReadData) {}
    }
    let (mut ui, _, _, indev, sample) = setup(IndevType::Pointer);
    let display = ui.indev_get_display(indev).unwrap();
    ui.display_set_rotation(display, crate::display::Rotation::R90);
    sample.borrow_mut().point = Point::new(20, 30);
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    let transformed = ui.indev_get_point(indev);
    assert_ne!(transformed, Point::new(20, 30));
    assert_eq!(
        ui.indev(indev).unwrap().pointer.last_raw_point,
        Point::new(20, 30)
    );
    ui.indev_set_driver(indev, Some(Box::new(ReleaseDriver)));
    ui.indev_read(indev);
    assert_eq!(ui.indev_get_point(indev), transformed);
}

#[test]
fn search_respects_z_order_hidden_transform_and_deleted_hit_target() {
    let (mut ui, screen, low, _, _) = setup(IndevType::Pointer);
    let high = ui.obj_create(Some(screen));
    ui.obj_set_pos(high, 10, 10);
    ui.obj_set_size(high, 100, 80);
    ui.obj_update_layout(screen);
    assert_eq!(ui.indev_search_obj(screen, Point::new(30, 30)), Some(high));
    ui.obj_add_flag(high, ObjFlags::HIDDEN);
    assert_eq!(ui.indev_search_obj(screen, Point::new(30, 30)), Some(low));
    ui.obj_remove_flag(high, ObjFlags::HIDDEN);
    ui.obj_add_flag(high, ObjFlags::ADV_HITTEST);
    ui.obj_add_event_cb(
        high,
        Rc::new(move |ui, _| ui.obj_delete(high)),
        EventCode::HIT_TEST,
        None,
    );
    assert_eq!(ui.indev_search_obj(screen, Point::new(30, 30)), Some(low));
    ui.obj_set_style_transform_scale(low, 512, StyleSelector::DEFAULT);
    ui.obj_update_layout(screen);
    assert_eq!(ui.indev_search_obj(screen, Point::new(150, 50)), Some(low));
}

#[test]
fn keypad_focus_remap_enter_and_encoder_navigation_editing() {
    let (mut ui, screen, first, indev, sample) = setup(IndevType::Keypad);
    let second = ui.obj_create(Some(screen));
    let group = ui.group_create();
    ui.group_add_obj(group, first);
    ui.group_add_obj(group, second);
    ui.indev_set_group(indev, Some(group));
    sample.borrow_mut().key = KEY_NEXT;
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    assert_eq!(ui.group_get_focused(group), Some(second));
    feed(&mut ui, indev, &sample, IndevState::Released, 110);
    ui.indev_set_key_remap_cb(indev, Some(Rc::new(|_, _, _| KEY_ENTER)));
    sample.borrow_mut().key = 65;
    feed(&mut ui, indev, &sample, IndevState::Pressed, 200);
    assert_eq!(ui.indev_get_key(indev), KEY_ENTER);
    assert!(ui.obj_has_state(second, State::PRESSED));
    feed(&mut ui, indev, &sample, IndevState::Released, 210);
    assert!(!ui.obj_has_state(second, State::PRESSED));
    ui.indev_set_type(indev, IndevType::Encoder);
    sample.borrow_mut().key = KEY_ENTER;
    sample.borrow_mut().enc_diff = -1;
    feed(&mut ui, indev, &sample, IndevState::Released, 300);
    assert_eq!(ui.group_get_focused(group), Some(first));
    sample.borrow_mut().enc_diff = 0;
    feed(&mut ui, indev, &sample, IndevState::Pressed, 310);
    feed(&mut ui, indev, &sample, IndevState::Released, 320);
    assert!(ui.group_get_editing(group));
    let keys = Rc::new(RefCell::new(Vec::new()));
    let out = keys.clone();
    ui.obj_add_event_cb(
        first,
        Rc::new(move |_, e| {
            if let EventParam::Key(key) = e.param {
                out.borrow_mut().push(key);
            }
        }),
        EventCode::KEY,
        None,
    );
    sample.borrow_mut().enc_diff = 2;
    feed(&mut ui, indev, &sample, IndevState::Released, 330);
    sample.borrow_mut().enc_diff = -1;
    feed(&mut ui, indev, &sample, IndevState::Released, 340);
    assert_eq!(*keys.borrow(), vec![KEY_RIGHT, KEY_RIGHT, KEY_LEFT]);
}

#[test]
fn button_points_change_releases_previous_object_and_invalid_index_is_ignored() {
    let (mut ui, screen, first, indev, sample) = setup(IndevType::Button);
    let second = ui.obj_create(Some(screen));
    ui.obj_set_pos(second, 150, 10);
    ui.obj_set_size(second, 100, 80);
    ui.obj_update_layout(screen);
    ui.indev_set_button_points(indev, vec![Point::new(30, 30), Point::new(170, 30)]);
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    assert_eq!(ui.indev(indev).unwrap().pointer.act_obj, Some(first));
    sample.borrow_mut().btn_id = 1;
    feed(&mut ui, indev, &sample, IndevState::Pressed, 200);
    assert!(!ui.obj_has_state(first, State::PRESSED));
    assert_eq!(ui.indev(indev).unwrap().pointer.act_obj, Some(second));
    feed(&mut ui, indev, &sample, IndevState::Released, 210);
    sample.borrow_mut().btn_id = 99;
    feed(&mut ui, indev, &sample, IndevState::Pressed, 220);
    assert_eq!(ui.indev(indev).unwrap().pointer.act_obj, None);
}

#[test]
fn buffered_reads_and_disabled_devices_do_not_leave_active_context() {
    struct QueueDriver(VecDeque<IndevReadData>);
    impl IndevDriver for QueueDriver {
        fn read(&mut self, _: IndevId, data: &mut IndevReadData) {
            *data = self.0.pop_front().unwrap();
            data.continue_reading = !self.0.is_empty();
        }
    }
    let (mut ui, _, obj, indev, _) = setup(IndevType::Pointer);
    let pressed = IndevReadData {
        state: IndevState::Pressed,
        point: Point::new(30, 30),
        timestamp: 100,
        ..Default::default()
    };
    let mut released = pressed.clone();
    released.state = IndevState::Released;
    released.timestamp = 110;
    ui.indev_set_driver(
        indev,
        Some(Box::new(QueueDriver(VecDeque::from([pressed, released])))),
    );
    ui.indev_enable(Some(indev), false);
    ui.indev_read(indev);
    assert_eq!(ui.indev_active(), None);
    ui.indev_enable(Some(indev), true);
    ui.indev_read(indev);
    assert_eq!(ui.indev_get_short_click_streak(indev), 1);
    assert!(!ui.obj_has_state(obj, State::PRESSED));
    assert_eq!(ui.indev_active(), None);
}

#[test]
fn directional_gesture_is_sent_once_after_distance_threshold() {
    let (mut ui, screen, obj, indev, sample) = setup(IndevType::Pointer);
    ui.obj_remove_flag(obj, ObjFlags::SCROLLABLE | ObjFlags::GESTURE_BUBBLE);
    ui.obj_remove_flag(screen, ObjFlags::SCROLLABLE);
    ui.obj_add_flag(obj, ObjFlags::PRESS_LOCK);
    let directions = Rc::new(Cell::new(0));
    let out = directions.clone();
    ui.obj_add_event_cb(
        obj,
        Rc::new(move |_, _| out.set(out.get() + 1)),
        EventCode::GESTURE_RIGHT,
        None,
    );
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    sample.borrow_mut().point.x += 60;
    feed(&mut ui, indev, &sample, IndevState::Pressed, 110);
    sample.borrow_mut().point.x += 10;
    feed(&mut ui, indev, &sample, IndevState::Pressed, 120);
    assert_eq!(directions.get(), 1);
    assert_eq!(ui.indev_get_gesture_dir(indev), Dir::RIGHT);
    assert!(ui.indev_get_press_moved(indev));
}

#[test]
fn drag_scroll_momentum_cleanup_and_snap_distance() {
    let (mut ui, screen, obj, indev, sample) = setup(IndevType::Pointer);
    ui.obj_set_size(obj, 100, 100);
    let content = ui.obj_create(Some(obj));
    ui.obj_set_size(content, 80, 300);
    ui.obj_remove_flag(content, ObjFlags::CLICKABLE);
    ui.obj_update_layout(screen);
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    sample.borrow_mut().point.y = 10;
    feed(&mut ui, indev, &sample, IndevState::Pressed, 110);
    assert_eq!(ui.indev_get_scroll_obj(indev), Some(obj));
    assert_eq!(ui.indev_get_scroll_dir(indev), Dir::VER);
    assert_eq!(ui.obj_get_scroll_y(obj), 20);
    let predicted = ui.indev_scroll_throw_predict(indev, Dir::VER);
    assert!(predicted < -20);
    feed(&mut ui, indev, &sample, IndevState::Released, 120);
    assert!(ui.indev(indev).unwrap().scroll_throw_anim.is_some());
    for _ in 0..200 {
        ui.tick_inc(10);
        ui.anim_refr_now();
    }
    assert_eq!(ui.indev_get_scroll_obj(indev), None);
    assert_eq!(ui.indev(indev).unwrap().scroll_throw_anim, None);
    ui.obj_scroll_to_y(obj, 0, false);
    ui.obj_set_scroll_snap_y(obj, ScrollSnap::Start);
    ui.obj_set_y(content, 20);
    ui.obj_update_layout(screen);
    assert_eq!(ui.indev_scroll_get_snap_dist(obj).y, -20);
}

#[test]
fn nested_device_reads_restore_outer_event_context() {
    let (mut ui, _, obj, outer, sample) = setup(IndevType::Pointer);
    let inner = ui.indev_create();
    ui.indev_set_type(inner, IndevType::Keypad);
    let inner_sample = Rc::new(RefCell::new(IndevReadData {
        state: IndevState::Pressed,
        key: 65,
        timestamp: 100,
        ..Default::default()
    }));
    ui.indev_set_driver(inner, Some(Box::new(SampleDriver(inner_sample))));
    let contexts = Rc::new(RefCell::new(Vec::new()));
    let captured = contexts.clone();
    ui.indev_add_event_cb(
        inner,
        Rc::new(move |ui, _| {
            captured
                .borrow_mut()
                .push((ui.indev_active(), ui.indev_get_active_obj()))
        }),
        EventCode::PRESSED,
        None,
    );
    let captured = contexts.clone();
    ui.indev_add_event_cb(
        outer,
        Rc::new(move |ui, _| {
            ui.indev_read(inner);
            captured
                .borrow_mut()
                .push((ui.indev_active(), ui.indev_get_active_obj()));
        }),
        EventCode::PRESSED,
        None,
    );
    feed(&mut ui, outer, &sample, IndevState::Pressed, 100);
    assert_eq!(
        *contexts.borrow(),
        vec![(Some(inner), None), (Some(outer), Some(obj))]
    );
    assert_eq!(ui.indev_active(), None);
    assert_eq!(ui.indev_get_active_obj(), None);
}

#[test]
fn scroll_threshold_chain_and_transform_direction_are_recomputed_per_ancestor() {
    let (mut ui, screen, parent, indev, _) = setup(IndevType::Pointer);
    ui.obj_remove_flag(screen, ObjFlags::SCROLLABLE);
    ui.obj_set_size(parent, 100, 100);
    let child = ui.obj_create(Some(parent));
    ui.obj_set_size(child, 80, 300);
    ui.obj_remove_flag(child, ObjFlags::SCROLLABLE);
    ui.obj_update_layout(screen);
    let d = ui.indev_mut(indev).unwrap();
    d.pointer.act_obj = Some(child);
    d.pointer.vect = Point::new(0, -6);
    assert_eq!(ui.indev_find_scroll_obj(indev), None);
    ui.indev_mut(indev).unwrap().pointer.vect.y = -5;
    assert_eq!(ui.indev_find_scroll_obj(indev), Some(parent));
    let d = ui.indev_mut(indev).unwrap();
    d.pointer.scroll_obj = None;
    d.pointer.scroll_sum = Point::default();
    d.pointer.vect.y = -20;
    ui.obj_remove_flag(child, ObjFlags::SCROLL_CHAIN_VER);
    assert_eq!(ui.indev_find_scroll_obj(indev), None);
    ui.obj_add_flag(child, ObjFlags::SCROLL_CHAIN_VER);
    ui.obj_set_style_transform_rotation(child, 900, StyleSelector::DEFAULT);
    // Child-local movement is horizontal; the unrotated parent must still see
    // the original vertical motion. v9.6 clears both axis flags per ancestor.
    ui.indev_mut(indev).unwrap().pointer.scroll_sum = Point::default();
    assert_eq!(ui.indev_find_scroll_obj(indev), Some(parent));
    assert_eq!(ui.indev_get_scroll_dir(indev), Dir::VER);
}

#[test]
fn nonelastic_edges_and_scroll_one_snap_limit_actual_drag_distance() {
    let (mut ui, screen, obj, indev, _) = setup(IndevType::Pointer);
    ui.obj_set_size(obj, 100, 100);
    ui.obj_remove_flag(obj, ObjFlags::SCROLL_ELASTIC);
    let children: Vec<_> = (0..3)
        .map(|index| {
            let child = ui.obj_create(Some(obj));
            ui.obj_set_size(child, 80, 80);
            ui.obj_set_y(child, index * 100);
            child
        })
        .collect();
    ui.obj_update_layout(screen);
    // Initialize reset handling before directly exercising the scroll handler.
    ui.indev_read(indev);
    let d = ui.indev_mut(indev).unwrap();
    d.pointer.act_obj = Some(obj);
    d.pointer.vect = Point::new(0, 20);
    ui.indev_scroll_handler(indev);
    assert_eq!(
        ui.obj_get_scroll_y(obj),
        0,
        "nonelastic start edge rejects outward dragging"
    );
    ui.indev_reset(Some(indev), None);
    ui.indev_read(indev);
    ui.obj_set_scroll_snap_y(obj, ScrollSnap::Start);
    ui.obj_add_flag(obj, ObjFlags::SCROLL_ONE);
    let d = ui.indev_mut(indev).unwrap();
    d.pointer.act_obj = Some(children[0]);
    d.pointer.vect = Point::new(0, -30);
    ui.indev_scroll_handler(indev);
    assert_eq!(ui.obj_get_scroll_y(obj), 30);
    ui.indev_mut(indev).unwrap().pointer.vect.y = -200;
    ui.indev_scroll_handler(indev);
    assert_eq!(
        ui.obj_get_scroll_y(obj),
        100,
        "SCROLL_ONE limits a drag to the next snap child"
    );
    assert_eq!(ui.indev_scroll_get_snap_dist(obj).y, 0);
}

#[test]
fn encoder_long_press_toggles_editing_and_removes_pressed_state() {
    let (mut ui, screen, first, indev, sample) = setup(IndevType::Encoder);
    let second = ui.obj_create(Some(screen));
    let group = ui.group_create();
    ui.group_add_obj(group, first);
    ui.group_add_obj(group, second);
    ui.indev_set_group(indev, Some(group));
    ui.group_set_editing(group, true);
    feed(&mut ui, indev, &sample, IndevState::Pressed, 100);
    assert!(ui.obj_has_state(first, State::PRESSED));
    feed(&mut ui, indev, &sample, IndevState::Pressed, 500);
    assert!(!ui.group_get_editing(group));
    assert!(!ui.obj_has_state(first, State::PRESSED));
    feed(&mut ui, indev, &sample, IndevState::Released, 510);
    assert!(!ui.group_get_editing(group));
}

#[test]
fn release_preserves_upstream_x_based_y_scale_quirk() {
    use crate::misc::anim::Anim;
    let (mut ui, screen, obj, indev, sample) = setup(IndevType::Pointer);
    ui.indev_read(indev);
    ui.obj_set_style_transform_scale_x(obj, 512, StyleSelector::DEFAULT);
    ui.obj_set_style_transform_scale_y(obj, 256, StyleSelector::DEFAULT);
    ui.obj_update_layout(screen);
    // Keep an existing animation so the release's transformed vectors can be
    // observed before the throw handler applies momentum decay.
    let animation = ui.anim_start(&Anim::new());
    let d = ui.indev_mut(indev).unwrap();
    d.pointer.act_obj = Some(obj);
    d.pointer.scroll_obj = Some(obj);
    d.pointer.scroll_dir = Dir::VER;
    d.pointer.scroll_throw_vect = Point::new(20, 40);
    d.pointer.scroll_throw_vect_ori = Point::new(40, 80);
    d.scroll_throw_anim = Some(animation);
    feed(&mut ui, indev, &sample, IndevState::Released, 100);
    let d = ui.indev(indev).unwrap();
    // C release: X=512 and Y=updated_X*256/256=512. Dragging would use Y=256.
    assert_eq!(d.pointer.scroll_throw_vect, Point::new(10, 20));
    assert_eq!(d.pointer.scroll_throw_vect_ori, Point::new(20, 40));
}

#[test]
fn scroll_getter_order_observes_self_size_callback_changes() {
    use crate::misc::types::coord;
    for horizontal in [true, false] {
        let (mut ui, _, obj, indev, _) = setup(IndevType::Pointer);
        ui.indev_read(indev);
        ui.obj_remove_flag(obj, ObjFlags::SCROLL_ELASTIC);
        let changed = Rc::new(Cell::new(false));
        let flag = changed.clone();
        ui.obj_add_event_cb(
            obj,
            Rc::new(move |ui, e| {
                if let EventParam::SelfSize(size) = &mut e.param {
                    size.x = 1000;
                    size.y = 1000;
                }
                if !flag.replace(true) {
                    let spec = ui.obj_mut(obj).unwrap().spec.as_mut().unwrap();
                    if horizontal {
                        spec.scroll.x = -5;
                    } else {
                        spec.scroll.y = -5;
                    }
                }
            }),
            EventCode::GET_SELF_SIZE,
            None,
        );
        let d = ui.indev_mut(indev).unwrap();
        d.pointer.act_obj = Some(obj);
        d.pointer.scroll_obj = Some(obj);
        d.pointer.scroll_dir = if horizontal { Dir::HOR } else { Dir::VER };
        d.pointer.scroll_area = Area::new(coord::MIN, coord::MIN, coord::MAX, coord::MAX);
        if horizontal {
            d.pointer.vect = Point::new(10, 0);
            // Drag reads right (callback sets left=5), then left, clamps to 5.
            ui.indev_scroll_handler(indev);
            assert_eq!(ui.obj_get_scroll_x(obj), 0);
        } else {
            d.pointer.scroll_throw_vect = Point::new(0, 10);
            // Throw reads bottom (callback sets top=5), then top, clamps to 5.
            ui.indev_scroll_throw_handler(indev);
            assert_eq!(ui.obj_get_scroll_y(obj), 0);
        }
        assert!(changed.get());
    }
}

#[test]
fn self_size_callback_can_delete_device_during_scroll_extent_queries() {
    for horizontal in [true, false] {
        let (mut ui, _, obj, indev, _) = setup(IndevType::Pointer);
        ui.indev_read(indev);
        ui.obj_add_event_cb(
            obj,
            Rc::new(move |ui, _| ui.indev_delete(indev)),
            EventCode::GET_SELF_SIZE,
            None,
        );
        let d = ui.indev_mut(indev).unwrap();
        d.pointer.act_obj = Some(obj);
        d.pointer.scroll_obj = Some(obj);
        d.pointer.scroll_dir = if horizontal { Dir::HOR } else { Dir::VER };
        d.pointer.vect = Point::new(10, 0);
        d.pointer.scroll_throw_vect = Point::new(0, 10);
        if horizontal {
            ui.indev_scroll_handler(indev);
        } else {
            ui.indev_scroll_throw_handler(indev);
        }
        assert!(ui.indev(indev).is_none());
    }
}

#[test]
fn scroll_search_and_settle_stop_after_self_size_deletes_device_or_object() {
    for delete_object in [false, true] {
        for settling in [false, true] {
            let (mut ui, _, obj, indev, _) = setup(IndevType::Pointer);
            ui.indev_read(indev);
            let calls = Rc::new(Cell::new(0));
            let count = calls.clone();
            ui.obj_add_event_cb(
                obj,
                Rc::new(move |ui, _| {
                    count.set(count.get() + 1);
                    // Throw first queries horizontal extents, then vertical
                    // extents in the settling loop. Search deletes on its first.
                    if count.get() == if settling { 2 } else { 1 } {
                        if delete_object {
                            ui.obj_delete(obj);
                        } else {
                            ui.indev_delete(indev);
                        }
                    }
                }),
                EventCode::GET_SELF_SIZE,
                None,
            );
            let d = ui.indev_mut(indev).unwrap();
            d.pointer.act_obj = Some(obj);
            d.pointer.vect = Point::new(20, 0);
            if settling {
                d.pointer.scroll_obj = Some(obj);
                d.pointer.scroll_dir = Dir::HOR;
                ui.indev_scroll_throw_handler(indev);
            } else {
                assert_eq!(ui.indev_find_scroll_obj(indev), None);
            }
            assert_eq!(calls.get(), if settling { 2 } else { 1 });
            if delete_object {
                assert!(!ui.obj_is_valid(obj));
            } else {
                assert!(ui.indev(indev).is_none());
            }
        }
    }
}
