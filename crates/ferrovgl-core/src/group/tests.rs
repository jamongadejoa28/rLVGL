use super::*;
use crate::core::UiConfig;
use crate::core::obj::ObjFlags;
use core::cell::RefCell;

fn scene() -> (Ui, GroupId, [Obj; 3]) {
    let mut ui = Ui::new(UiConfig::default());
    let display = ui.display_create(320, 240);
    let screen = ui.display_get_screen_active(Some(display)).unwrap();
    let objects = core::array::from_fn(|_| ui.obj_create(Some(screen)));
    let group = ui.group_create();
    for obj in objects {
        ui.group_add_obj(group, obj);
    }
    (ui, group, objects)
}

#[test]
fn creation_order_focus_and_refocus_match_upstream() {
    let (mut ui, group, [a, b, c]) = scene();
    let second = ui.group_create();
    assert_eq!(ui.group_by_index(0), Some(group));
    assert_eq!(ui.group_by_index(1), Some(second));
    assert_eq!(ui.group_get_focused(group), Some(a));
    assert_eq!(ui.group(group).unwrap().refocus_policy, GroupRefocusPolicy::Prev);
    ui.group_focus_prev(group);
    assert_eq!(ui.group_get_focused(group), Some(c));
    ui.group_set_wrap(group, false);
    ui.group_focus_next(group);
    assert_eq!(ui.group_get_focused(group), Some(c));
    ui.group_focus_freeze(group, true);
    ui.group_remove_obj(c);
    assert_eq!(ui.group_get_focused(group), Some(b));
    assert!(!ui.group(group).unwrap().frozen);
    assert!(!ui.group_get_wrap(group));
    ui.group_set_refocus_policy(group, GroupRefocusPolicy::Next);
    ui.obj_delete(b);
    assert_eq!(ui.group_get_focused(group), Some(a));
    ui.obj_delete(a);
    assert_eq!(ui.group_get_focused(group), None);
    assert_eq!(ui.group_get_obj_count(group), 0);
}

#[test]
fn traversal_skips_disabled_and_hidden_ancestors_and_reports_edges() {
    let (mut ui, group, [a, b, c]) = scene();
    let parent = ui.obj_create(ui.obj_get_parent(a));
    ui.obj_set_parent(b, parent);
    ui.obj_add_flag(parent, ObjFlags::HIDDEN);
    ui.obj_add_state(c, State::DISABLED);
    let edges = Rc::new(RefCell::new(Vec::new()));
    let output = edges.clone();
    ui.group_set_edge_cb(
        group,
        Some(Rc::new(move |_, _, next| output.borrow_mut().push(next))),
    );
    ui.group_focus_next(group);
    ui.group_focus_prev(group);
    assert_eq!(ui.group_get_focused(group), Some(a));
    assert_eq!(*edges.borrow(), [true, false]);
    ui.obj_remove_state(c, State::DISABLED);
    ui.group_focus_next(group);
    assert_eq!(ui.group_get_focused(group), Some(c));
    ui.group_remove_obj(c);
    ui.group_remove_obj(a);
    assert_eq!(ui.group_get_focused(group), None);
    assert_eq!(ui.group_get_obj_count(group), 1);
}

#[test]
fn explicit_and_traversal_focus_have_distinct_callback_order() {
    let (mut ui, group, [a, b, c]) = scene();
    let log = Rc::new(RefCell::new(Vec::new()));
    for obj in [a, b, c] {
        let log = log.clone();
        ui.obj_add_event_cb(
            obj,
            Rc::new(move |_, e| {
                if e.code == EventCode::FOCUSED {
                    log.borrow_mut().push("focused");
                }
                if e.code == EventCode::DEFOCUSED {
                    log.borrow_mut().push("defocused");
                }
            }),
            EventCode::ALL,
            None,
        );
    }
    let output = log.clone();
    ui.group_set_focus_cb(
        group,
        Some(Rc::new(move |_, _| output.borrow_mut().push("callback"))),
    );
    ui.group_focus_obj(b);
    assert_eq!(*log.borrow(), ["defocused", "callback", "focused"]);
    log.borrow_mut().clear();
    ui.group_focus_next(group);
    assert_eq!(*log.borrow(), ["defocused", "focused", "callback"]);
    ui.group_set_editing(group, true);
    assert!(ui.obj_has_state(c, State::EDITED));
    ui.group_focus_obj(c);
    assert!(!ui.group_get_editing(group));
    assert!(!ui.obj_has_state(c, State::EDITED));
}

#[test]
fn swap_preserves_focused_object_and_key_dispatch_checks_disabled() {
    let (mut ui, group, [a, b, _]) = scene();
    ui.group_swap_obj(a, b);
    assert_eq!(ui.group_get_obj_by_index(group, 0), Some(b));
    assert_eq!(ui.group_get_focused(group), Some(a));
    let keys = Rc::new(RefCell::new(Vec::new()));
    let output = keys.clone();
    ui.obj_add_event_cb(
        a,
        Rc::new(move |_, e| output.borrow_mut().push(e.key())),
        EventCode::KEY,
        None,
    );
    assert_eq!(ui.group_send_data(group, 'x' as u32), LvResult::Ok);
    ui.obj_add_state(a, State::DISABLED);
    assert_eq!(ui.group_send_data(group, 'y' as u32), LvResult::Ok);
    assert_eq!(*keys.borrow(), ['x' as u32]);
}

#[test]
fn focus_callback_can_delete_group_without_stale_indices() {
    let (mut ui, group, [a, b, c]) = scene();
    ui.group_set_default(Some(group));
    ui.group_set_focus_cb(group, Some(Rc::new(|ui, group| ui.group_delete(group))));
    ui.group_focus_obj(b);
    assert!(ui.group(group).is_none());
    assert_eq!(ui.group_get_default(), None);
    for obj in [a, b, c] {
        assert_eq!(ui.obj_get_group(obj), None);
    }
    let reused = ui.group_create();
    assert_ne!(reused, group);
    assert_eq!(ui.group_send_data(group, 1), LvResult::Invalid);
}

#[test]
fn focused_object_deletion_during_event_and_recursive_group_delete_are_safe() {
    let (mut ui, group, [a, b, c]) = scene();
    ui.obj_add_event_cb(
        b,
        Rc::new(move |ui, _| ui.obj_delete(b)),
        EventCode::FOCUSED,
        None,
    );
    ui.group_focus_next(group);
    assert!(!ui.obj_is_valid(b));
    assert_eq!(ui.group_get_focused(group), Some(a));
    ui.obj_add_event_cb(
        a,
        Rc::new(move |ui, _| ui.group_delete(group)),
        EventCode::DEFOCUSED,
        None,
    );
    ui.group_delete(group);
    assert!(ui.group(group).is_none());
    assert_eq!(ui.obj_get_group(c), None);
}

#[test]
fn removing_all_and_moving_between_groups_clear_membership() {
    let (mut ui, group, [a, b, c]) = scene();
    let second = ui.group_create();
    ui.group_add_obj(second, b);
    assert_eq!(ui.group_get_obj_count(group), 2);
    assert_eq!(ui.obj_get_group(b), Some(second));
    ui.group_remove_all_objs(group);
    assert_eq!(ui.obj_get_group(a), None);
    assert_eq!(ui.obj_get_group(c), None);
    assert_eq!(ui.group_get_obj_count(group), 0);
    assert!(!ui.obj_has_state(a, State::FOCUSED));
}

#[test]
fn removal_defocus_observes_current_focus_and_recursive_removal_runs_once() {
    for remove_all in [false, true] {
        let (mut ui, group, [a, b, c]) = scene();
        ui.group_remove_obj(b);
        ui.group_remove_obj(c);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let output = observed.clone();
        ui.obj_add_event_cb(
            a,
            Rc::new(move |ui, _| {
                output.borrow_mut().push(ui.group_get_focused(group));
                if remove_all {
                    ui.group_remove_all_objs(group);
                } else {
                    ui.group_remove_obj(a);
                }
            }),
            EventCode::DEFOCUSED,
            None,
        );
        if remove_all {
            ui.group_remove_all_objs(group);
        } else {
            ui.group_remove_obj(a);
        }
        assert_eq!(*observed.borrow(), [Some(a)]);
        assert_eq!(ui.group_get_focused(group), None);
        assert_eq!(ui.group_get_obj_count(group), 0);
        assert_eq!(ui.obj_get_group(a), None);
    }
}

#[test]
fn defocus_callback_can_delete_its_group_or_object_without_duplicate_dispatch() {
    for delete_group in [false, true] {
        let (mut ui, group, [a, b, c]) = scene();
        let calls = Rc::new(RefCell::new(0));
        let output = calls.clone();
        ui.obj_add_event_cb(
            a,
            Rc::new(move |ui, _| {
                *output.borrow_mut() += 1;
                assert_eq!(ui.group_get_focused(group), Some(a));
                if delete_group {
                    ui.group_delete(group);
                } else {
                    ui.obj_delete(a);
                }
            }),
            EventCode::DEFOCUSED,
            None,
        );
        ui.group_remove_all_objs(group);
        assert_eq!(*calls.borrow(), 1);
        assert_eq!(ui.obj_get_group(b), None);
        assert_eq!(ui.obj_get_group(c), None);
        if delete_group {
            assert!(ui.group(group).is_none());
        } else {
            assert!(!ui.obj_is_valid(a));
        }
    }
}

#[test]
fn single_removal_defocus_can_clear_the_group_without_another_defocus() {
    let (mut ui, group, [a, b, c]) = scene();
    ui.group_remove_obj(b);
    ui.group_remove_obj(c);
    let calls = Rc::new(RefCell::new(0));
    let output = calls.clone();
    ui.obj_add_event_cb(
        a,
        Rc::new(move |ui, _| {
            *output.borrow_mut() += 1;
            assert_eq!(ui.group_get_focused(group), Some(a));
            ui.group_remove_all_objs(group);
        }),
        EventCode::DEFOCUSED,
        None,
    );
    ui.group_remove_obj(a);
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(ui.group_get_obj_count(group), 0);
    assert_eq!(ui.obj_get_group(a), None);
}
