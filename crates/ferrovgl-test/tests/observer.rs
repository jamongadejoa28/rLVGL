//! Ported from LVGL v9.6.0: `tests/src/test_cases/test_observer.c`.
//!
//! Also ports `tests/src/test_cases/public_api/test_observer_args.c` (the
//! `test_observer_args_*` / `*_invalid*` functions below).
//!
//! Every upstream test function whose widgets and bindings exist, in upstream
//! order, with its values. None of them takes a screenshot (logic tests).
//! `setUp` creates a fresh default-theme display (upstream shares one screen,
//! but these tests only use objects they create, and `tearDown` cleans the
//! screen and deletes the subjects, so nothing is inherited); `tearDown` is
//! [`tear_down`]. The C file-scope statics (`observer_called`, `prev_v`,
//! `captured_*`, ...) are thread-locals because each Rust test runs on its own
//! thread.
//!
//! Skipped (the widget is not ported): `test_observer_roller_value`,
//! `test_observer_dropdown_value`, `test_observer_scale_line_needle_value`,
//! `test_observer_scale_image_needle_value` (no roller / dropdown / scale
//! widget).
//!
//! The C string literals passed to `lv_subject_set_pointer()` are
//! `Rc<&'static str>` payloads, the `const char *` that
//! `lv_label_bind_text()` shows.
//!
//! `TEST_ASSERT_MEM_LEAK_LESS_THAN` / `lv_test_get_free_mem()` have no
//! allocator to inspect here; they are replaced by `Rc` strong counts (for
//! "the observer freed / did not free the data") and by the arena sizes of
//! the observer state, event descriptors and objects (for "no leak").

#![allow(deprecated)] // `LV_DEPRECATIONS_IGNORE_BEGIN`: the tests exercise the deprecated API.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ferrovgl_core::core::observer::{ObjSetBoolCb, ObserverId, SubjectId, SubjectType};
use ferrovgl_core::misc::style::{ImageSrc, SharedRef};
use ferrovgl_core::{Color, EventCode, Obj, ObjFlags, State, Ui};
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::{Arc, Bar, Button, Image, Label, Slider};

const ANIM_OFF: bool = false;

thread_local! {
    /// `static uint32_t observer_called`
    static OBSERVER_CALLED: Cell<u32> = const { Cell::new(0) };
    /// `static int32_t prev_v`
    static PREV_V: Cell<i32> = const { Cell::new(0) };
    /// `static int32_t current_v`
    static CURRENT_V: Cell<i32> = const { Cell::new(0) };
    /// `static int32_t group_observer_called`
    static GROUP_OBSERVER_CALLED: Cell<i32> = const { Cell::new(0) };
    /// `static int32_t group_value_sum`
    static GROUP_VALUE_SUM: Cell<i32> = const { Cell::new(0) };
    /// `static int32_t captured_int`
    static CAPTURED_INT: Cell<i32> = const { Cell::new(0) };
    /// `static float captured_float`
    static CAPTURED_FLOAT: Cell<f32> = const { Cell::new(0.0) };
    /// `static const char * captured_string`
    static CAPTURED_STRING: RefCell<Option<String>> = const { RefCell::new(None) };
    /// `static lv_color_t captured_color`
    static CAPTURED_COLOR: Cell<Color> = const { Cell::new(Color::black()) };
    /// `static const void * captured_pointer`
    static CAPTURED_POINTER: RefCell<Option<Rc<dyn Any>>> = const { RefCell::new(None) };
}

/// The per-test state of `test_observer.c`: the shared display plus the
/// `subjects[16]` pool that `tearDown()` deletes.
struct Fixture {
    display: TestDisplay,
    inputs: TestInputs,
    subjects: [Option<SubjectId>; 16],
}

impl Fixture {
    fn ui(&mut self) -> &mut Ui {
        &mut self.display.ui
    }

    fn screen(&self) -> Obj {
        self.display.screen().unwrap()
    }
}

/// `setUp()`
fn set_up() -> Fixture {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    OBSERVER_CALLED.set(0);
    /* Function run before every test */
    Fixture {
        display,
        inputs,
        subjects: [None; 16],
    }
}

/// `tearDown()`
fn tear_down(mut f: Fixture) {
    /* Function run after every test */
    let screen = f.screen();
    f.ui().obj_clean(screen);

    /* Group Subjects first: deleting a group dereferences its elements, so the
     * elements have to outlive it. */
    for i in 0..f.subjects.len() {
        if let Some(s) = f.subjects[i]
            && f.display.ui.observer.subjects.get(s.0).unwrap().type_ == SubjectType::Group
        {
            f.display.ui.subject_delete(Some(s));
            f.subjects[i] = None;
        }
    }
    for i in 0..f.subjects.len() {
        f.display.ui.subject_delete(f.subjects[i]);
        f.subjects[i] = None;
    }
    assert!(f.display.ui.observer.subjects.is_empty());
    assert!(f.display.ui.observer.observers.is_empty());
}

/// Create a Subject and register it for automatic deletion in `tearDown()`.
fn subject_create(f: &mut Fixture, type_: SubjectType) -> SubjectId {
    for i in 0..f.subjects.len() {
        if f.subjects[i].is_none() {
            let s = f.display.ui.subject_create(type_);
            assert!(s.is_some());
            f.subjects[i] = s;
            return s.unwrap();
        }
    }
    panic!("subject pool exhausted");
}

/// Delete a registered Subject early, so `tearDown()` doesn't delete it twice.
fn subject_delete(f: &mut Fixture, subject: SubjectId) {
    for s in f.subjects.iter_mut() {
        if *s == Some(subject) {
            *s = None;
        }
    }
    f.display.ui.subject_delete(Some(subject));
}

/// `observer_basic()`
fn observer_basic(_ui: &mut Ui, _observer: ObserverId, _subject: SubjectId) {
    OBSERVER_CALLED.set(OBSERVER_CALLED.get() + 1);
}

/// `observer_int()`
fn observer_int(ui: &mut Ui, _observer: ObserverId, subject: SubjectId) {
    PREV_V.set(ui.subject_get_previous_int(subject));
    CURRENT_V.set(ui.subject_get_int(subject));
}

/// `lv_ll_get_len(&subject->subs_ll)`
fn subs_len(ui: &Ui, subject: SubjectId) -> usize {
    ui.observer.subjects.get(subject.0).unwrap().subs.len()
}

#[test]
fn test_observer_add_remove() {
    let mut f = set_up();
    /* static lv_subject_t subject; lv_subject_init_int(&subject, 5); */
    let subject = f.ui().subject_alloc_static();
    f.ui().subject_init_int(subject, 5);
    let created = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(created, 5);
    let test_subjects = [subject, created];

    for sub in test_subjects {
        let observer = f.ui().subject_add_observer(sub, Rc::new(observer_int), None);

        CURRENT_V.set(0);
        f.ui().subject_set_int(sub, 10);
        assert_eq!(10, f.ui().subject_get_int(sub));
        assert_eq!(10, CURRENT_V.get());

        f.ui().observer_delete(observer);

        f.ui().subject_set_int(sub, 15);
        assert_eq!(15, f.ui().subject_get_int(sub));
        assert_eq!(10, CURRENT_V.get()); /*The observer cb is not called*/
    }

    f.ui().subject_deinit(Some(subject));
    /* The static storage is not part of `subject_ll`; release it for tearDown's leak check. */
    f.ui().observer.subjects.remove(subject.0);
    tear_down(f);
}

#[test]
fn test_object_observer_add_remove() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    let observer = f.ui().obj_bind_flag_if_eq(obj, subject, ObjFlags::HIDDEN, 5);

    assert!(!f.ui().obj_is_hidden(obj));
    f.ui().subject_set_int(subject, 5);
    assert!(f.ui().obj_is_hidden(obj));
    f.ui().observer_delete(observer);
    f.ui().subject_set_int(subject, 1);

    /* This shouldn't get updated */
    assert!(f.ui().obj_is_hidden(obj));
    f.ui().obj_delete(obj);
    /* We shouldn't crash here */
    tear_down(f);
}

/// `get_event_delete_from_obj()`: the index of the `LV_EVENT_DELETE`
/// descriptor (the remove event is a event callback using the observer as
/// the user data).
fn get_event_delete_from_obj(ui: &Ui, obj: Obj) -> Option<u32> {
    let event_cnt = ui.obj_get_event_count(obj);
    for i in 0..event_cnt {
        let event = ui.obj_get_event_dsc(obj, i);
        assert!(event.is_some());
        if event.unwrap().filter == u32::from(EventCode::DELETE.0) {
            return Some(i);
        }
    }
    None
}

#[test]
fn test_obj_remove_from_subject_removes_delete_event() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);
    let _ = f
        .ui()
        .subject_add_observer_obj(subject, Rc::new(observer_basic), Some(obj), None);

    {
        /*
         * We expect the event delete to be added to the object allowing the observer
         * to be deleted when the object is deleted
         */
        assert!(f.ui().obj(obj).unwrap().spec.is_some());
        assert_eq!(f.ui().obj_get_event_count(obj), 1);
        let delete_event = get_event_delete_from_obj(f.ui(), obj);
        assert!(delete_event.is_some());
    }
    {
        /* Removing the object from the subject should remove the delete event entry */
        f.ui().obj_remove_from_subject(obj, Some(subject));
        let delete_event = get_event_delete_from_obj(f.ui(), obj);
        assert!(delete_event.is_none());
    }
    tear_down(f);
}

/// Not upstream: `LV_CHECK_ARG(obj != NULL, return)` of
/// `lv_obj_remove_from_subject()` (a deleted object stands for `NULL`).
#[test]
fn obj_remove_from_subject_checks_obj() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));
    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().obj_delete(obj);

    let ui = &mut f.display.ui;
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ui.obj_remove_from_subject(obj, Some(subject));
    }));
    /*The check fails (and asserts under LV_CHECK_ARG_ASSERT_ON_FAIL) before anything is touched*/
    assert_eq!(check_arg_asserts(), res.is_err());
    tear_down(f);
}

#[test]
fn test_observer_remove_removes_obj_callback() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);
    let observer = f
        .ui()
        .subject_add_observer_obj(subject, Rc::new(observer_basic), Some(obj), None);

    {
        /*
         * We expect the event delete to be added to the object allowing the observer
         * to be deleted when the object is deleted
         */
        assert!(f.ui().obj(obj).unwrap().spec.is_some());
        assert_eq!(f.ui().obj_get_event_count(obj), 1);
        let delete_event = get_event_delete_from_obj(f.ui(), obj);
        assert!(delete_event.is_some());
    }
    {
        /* Removing the observer associated with the object should remove the delete event entry */
        f.ui().observer_delete(observer);
        let delete_event = get_event_delete_from_obj(f.ui(), obj);
        assert!(delete_event.is_none());
    }
    tear_down(f);
}

#[test]
fn test_observer_int() {
    let mut f = set_up();
    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 5);
    let basic_observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);

    /*A new Subject starts at 0, so the first set leaves 0 as the previous value*/
    assert_eq!(5, f.ui().subject_get_int(subject));
    assert_eq!(0, f.ui().subject_get_previous_int(subject));
    assert_eq!(1, OBSERVER_CALLED.get());

    f.ui().subject_set_int(subject, 10);
    assert_eq!(10, f.ui().subject_get_int(subject));
    assert_eq!(5, f.ui().subject_get_previous_int(subject));
    assert_eq!(2, OBSERVER_CALLED.get());

    f.ui().subject_set_int(subject, 15);
    assert_eq!(15, f.ui().subject_get_int(subject));
    assert_eq!(10, f.ui().subject_get_previous_int(subject));
    assert_eq!(3, OBSERVER_CALLED.get());

    /* Observer shouldn't be called if value is the same */
    f.ui().subject_set_int(subject, 15);
    assert_eq!(15, f.ui().subject_get_int(subject));
    assert_eq!(15, f.ui().subject_get_previous_int(subject));
    assert_eq!(3, OBSERVER_CALLED.get());

    f.ui().observer_delete(basic_observer);
    tear_down(f);
}

#[test]
fn test_observer_float() {
    let mut f = set_up();
    let subject = subject_create(&mut f, SubjectType::Float);
    f.ui().subject_set_float(subject, 5.25);
    let basic_observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);

    /*A new Subject starts at 0, so the first set leaves 0 as the previous value*/
    assert_eq!(5.25, f.ui().subject_get_float(subject));
    assert_eq!(0.0, f.ui().subject_get_previous_float(subject));
    assert_eq!(1, OBSERVER_CALLED.get());

    f.ui().subject_set_float(subject, 10.5);
    assert_eq!(10.5, f.ui().subject_get_float(subject));
    assert_eq!(5.25, f.ui().subject_get_previous_float(subject));
    assert_eq!(2, OBSERVER_CALLED.get());

    f.ui().subject_set_float(subject, 15.75);
    assert_eq!(15.75, f.ui().subject_get_float(subject));
    assert_eq!(10.5, f.ui().subject_get_previous_float(subject));
    assert_eq!(3, OBSERVER_CALLED.get());

    /* Observer shouldn't be called if value is the same */
    f.ui().subject_set_float(subject, 15.75);
    assert_eq!(15.75, f.ui().subject_get_float(subject));
    assert_eq!(15.75, f.ui().subject_get_previous_float(subject));
    assert_eq!(3, OBSERVER_CALLED.get());

    f.ui().observer_delete(basic_observer);
    tear_down(f);
}

#[test]
fn test_observer_string() {
    let mut f = set_up();
    /* char buf_current[32]; char buf_previous[32]; */
    let subject = subject_create(&mut f, SubjectType::String);
    f.ui().subject_set_string_buffer_static(subject, 32, true);
    f.ui().subject_set_string(subject, "hello");

    let basic_observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);

    /*The buffers start empty, so the first copy leaves "" as the previous value*/
    assert_eq!(Some("hello"), f.ui().subject_get_string(subject));
    assert_eq!(Some(""), f.ui().subject_get_previous_string(subject));
    assert_eq!(1, OBSERVER_CALLED.get());

    f.ui().subject_set_string(subject, "my name is John");
    assert_eq!(Some("my name is John"), f.ui().subject_get_string(subject));
    assert_eq!(Some("hello"), f.ui().subject_get_previous_string(subject));
    assert_eq!(2, OBSERVER_CALLED.get());

    f.ui().subject_set_string(subject, "how are you?");
    assert_eq!(Some("how are you?"), f.ui().subject_get_string(subject));
    assert_eq!(
        Some("my name is John"),
        f.ui().subject_get_previous_string(subject)
    );
    assert_eq!(3, OBSERVER_CALLED.get());

    /* Observer shouldn't be called with same value */
    f.ui().subject_set_string(subject, "how are you?");
    assert_eq!(Some("how are you?"), f.ui().subject_get_string(subject));
    assert_eq!(Some("how are you?"), f.ui().subject_get_previous_string(subject));
    assert_eq!(3, OBSERVER_CALLED.get());

    f.ui()
        .subject_snprintf(subject, format_args!("I ate {} pizzas", 10));
    assert_eq!(Some("I ate 10 pizzas"), f.ui().subject_get_string(subject));
    assert_eq!(Some("how are you?"), f.ui().subject_get_previous_string(subject));
    assert_eq!(4, OBSERVER_CALLED.get());

    f.ui()
        .subject_snprintf(subject, format_args!("{}: {}", 1, "Coding is fun !"));
    assert_eq!(Some("1: Coding is fun !"), f.ui().subject_get_string(subject));
    assert_eq!(
        Some("I ate 10 pizzas"),
        f.ui().subject_get_previous_string(subject)
    );
    assert_eq!(5, OBSERVER_CALLED.get());

    /* Observer shouldn't be called with same value */
    f.ui()
        .subject_snprintf(subject, format_args!("{}: {}", 1, "Coding is fun !"));
    assert_eq!(Some("1: Coding is fun !"), f.ui().subject_get_string(subject));
    assert_eq!(
        Some("1: Coding is fun !"),
        f.ui().subject_get_previous_string(subject)
    );
    assert_eq!(5, OBSERVER_CALLED.get());

    /*Clip long text*/
    f.ui()
        .subject_set_string(subject, "text to be clipped to 32 chars.this should be clipped");
    assert_eq!(
        Some("text to be clipped to 32 chars."),
        f.ui().subject_get_string(subject)
    );
    assert_eq!(
        Some("1: Coding is fun !"),
        f.ui().subject_get_previous_string(subject)
    );
    assert_eq!(6, OBSERVER_CALLED.get());

    /*Check if the previous string is clipped correctly*/
    f.ui().subject_set_string(subject, "a");
    assert_eq!(Some("a"), f.ui().subject_get_string(subject));
    assert_eq!(
        Some("text to be clipped to 32 chars."),
        f.ui().subject_get_previous_string(subject)
    );
    assert_eq!(7, OBSERVER_CALLED.get());

    f.ui().observer_delete(basic_observer);
    tear_down(f);
}

/// `TEST_ASSERT_EQUAL_PTR(a, b)` for nullable pointers.
fn assert_ptr_eq(a: Option<&Rc<dyn Any>>, b: Option<Rc<dyn Any>>) {
    match (a, b.as_ref()) {
        (None, None) => {}
        (Some(a), Some(b)) => assert!(Rc::ptr_eq(a, b)),
        _ => panic!("pointers differ"),
    }
}

#[test]
fn test_observer_pointer() {
    let mut f = set_up();
    /* static int32_t a[3] = { 0 }; */
    let a: [Rc<dyn Any>; 3] = [Rc::new(0i32), Rc::new(0i32), Rc::new(0i32)];

    let subject = subject_create(&mut f, SubjectType::Pointer);
    f.ui().subject_set_pointer(subject, Some(a[0].clone()));

    let basic_observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);

    assert_eq!(1, OBSERVER_CALLED.get());

    /*A new Subject starts at NULL, so the first set leaves NULL as the previous value*/
    assert_ptr_eq(Some(&a[0]), f.ui().subject_get_pointer(subject));
    assert_ptr_eq(None, f.ui().subject_get_previous_pointer(subject));

    f.ui().subject_set_pointer(subject, Some(a[1].clone()));
    assert_ptr_eq(Some(&a[1]), f.ui().subject_get_pointer(subject));
    assert_ptr_eq(Some(&a[0]), f.ui().subject_get_previous_pointer(subject));
    assert_eq!(2, OBSERVER_CALLED.get());

    f.ui().subject_set_pointer(subject, Some(a[2].clone()));
    assert_ptr_eq(Some(&a[2]), f.ui().subject_get_pointer(subject));
    assert_ptr_eq(Some(&a[1]), f.ui().subject_get_previous_pointer(subject));
    assert_eq!(3, OBSERVER_CALLED.get());

    /*
     * Even if pointer is the same, the observer should still get called as we shouldn't assume
     * what the pointer is indicating
     */
    f.ui().subject_set_pointer(subject, Some(a[2].clone()));
    assert_ptr_eq(Some(&a[2]), f.ui().subject_get_pointer(subject));
    assert_ptr_eq(Some(&a[2]), f.ui().subject_get_previous_pointer(subject));
    assert_eq!(4, OBSERVER_CALLED.get());

    f.ui().observer_delete(basic_observer);
    tear_down(f);
}

#[test]
fn test_observer_color() {
    let mut f = set_up();
    let subject = subject_create(&mut f, SubjectType::Color);
    f.ui().subject_set_color(subject, Color::hex3(0x123));
    let basic_observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);

    /*A new Subject starts at black, so the first set leaves black as the previous value*/
    assert_eq!(Color::hex3(0x123), f.ui().subject_get_color(subject));
    assert_eq!(Color::black(), f.ui().subject_get_previous_color(subject));
    assert_eq!(1, OBSERVER_CALLED.get());

    f.ui().subject_set_color(subject, Color::hex3(0x456));
    assert_eq!(Color::hex3(0x456), f.ui().subject_get_color(subject));
    assert_eq!(Color::hex3(0x123), f.ui().subject_get_previous_color(subject));
    assert_eq!(2, OBSERVER_CALLED.get());

    f.ui().subject_set_color(subject, Color::hex3(0xabc));
    assert_eq!(Color::hex3(0xabc), f.ui().subject_get_color(subject));
    assert_eq!(Color::hex3(0x456), f.ui().subject_get_previous_color(subject));
    assert_eq!(3, OBSERVER_CALLED.get());

    /* Observer shouldn't be called if value is the same */
    f.ui().subject_set_color(subject, Color::hex3(0xabc));
    assert_eq!(Color::hex3(0xabc), f.ui().subject_get_color(subject));
    assert_eq!(Color::hex3(0xabc), f.ui().subject_get_previous_color(subject));
    assert_eq!(3, OBSERVER_CALLED.get());

    f.ui().observer_delete(basic_observer);
    tear_down(f);
}

/// `group_observer_cb()`
fn group_observer_cb(ui: &mut Ui, _observer: ObserverId, subject: SubjectId) {
    GROUP_OBSERVER_CALLED.set(GROUP_OBSERVER_CALLED.get() + 1);

    GROUP_VALUE_SUM.set(0);
    let s = ui.observer.subjects.get(subject.0).unwrap();
    let (size, list) = (s.size, s.value.group().unwrap());
    for i in 0..size {
        let sub = list[i as usize];
        GROUP_VALUE_SUM.set(GROUP_VALUE_SUM.get() + ui.subject_get_int(sub));
    }
}

#[test]
fn test_observer_group() {
    let mut f = set_up();
    /*The group keeps the list by pointer, so it has to outlive the group Subject*/
    let mut list = [None; 5];
    for (i, item) in list.iter_mut().enumerate() {
        let s = subject_create(&mut f, SubjectType::Int);
        f.ui().subject_set_int(s, i as i32 + 1);
        *item = Some(s);
    }
    let list: Vec<SubjectId> = list.iter().map(|s| s.unwrap()).collect();

    let group_subject = subject_create(&mut f, SubjectType::Group);
    f.ui()
        .subject_set_group_list_static(group_subject, Rc::from(&list[..2]));
    assert_eq!(Some(list[0]), f.ui().subject_get_group_element(group_subject, 0));
    assert_eq!(Some(list[1]), f.ui().subject_get_group_element(group_subject, 1));

    GROUP_OBSERVER_CALLED.set(0);
    f.ui()
        .subject_add_observer(group_subject, Rc::new(group_observer_cb), None);
    assert_eq!(1, GROUP_OBSERVER_CALLED.get());
    assert_eq!(1 + 2, GROUP_VALUE_SUM.get());

    f.ui().subject_set_int(list[0], 10);
    assert_eq!(2, GROUP_OBSERVER_CALLED.get());
    assert_eq!(10 + 2, GROUP_VALUE_SUM.get());

    f.ui().subject_set_int(list[1], 20);
    assert_eq!(3, GROUP_OBSERVER_CALLED.get());
    assert_eq!(10 + 20, GROUP_VALUE_SUM.get());
    tear_down(f);
}

#[test]
fn test_observer_group_deinit_unsubscribes_from_elements() {
    let mut f = set_up();
    let l0 = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(l0, 1);
    let l1 = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(l1, 2);
    let list = [l0, l1];

    let group_subject = subject_create(&mut f, SubjectType::Group);
    f.ui()
        .subject_set_group_list_static(group_subject, Rc::from(&list[..]));

    /* The group subscribed to both of its elements. */
    assert_eq!(1, subs_len(f.ui(), list[0]));
    assert_eq!(1, subs_len(f.ui(), list[1]));

    GROUP_OBSERVER_CALLED.set(0);
    f.ui()
        .subject_add_observer(group_subject, Rc::new(group_observer_cb), None);
    assert_eq!(1, GROUP_OBSERVER_CALLED.get());

    /* Deleting the group must unsubscribe it from all its elements,
     * otherwise they keep a dangling observer pointing to the group. */
    subject_delete(&mut f, group_subject);

    assert_eq!(0, subs_len(f.ui(), list[0]));
    assert_eq!(0, subs_len(f.ui(), list[1]));

    GROUP_OBSERVER_CALLED.set(0);
    /* Would notify the freed group subject if the observers were left behind. */
    f.ui().subject_set_int(list[0], 10);
    f.ui().subject_set_int(list[1], 20);
    assert_eq!(0, GROUP_OBSERVER_CALLED.get());
    tear_down(f);
}

#[test]
fn test_observer_obj_flag_eq() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    f.ui().obj_bind_flag_if_eq(obj, subject, ObjFlags::HIDDEN, 5);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui()
        .obj_bind_flag_if_not_eq(obj, subject, ObjFlags::CHECKABLE, 10);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_is_hidden(obj));
    assert!(f.ui().obj_is_checkable(obj));

    f.ui().subject_set_int(subject, 5);
    assert!(f.ui().obj_is_hidden(obj));
    assert!(f.ui().obj_is_checkable(obj));

    f.ui().subject_set_int(subject, 10);
    assert!(!f.ui().obj_is_hidden(obj));
    assert!(!f.ui().obj_is_checkable(obj));
    tear_down(f);
}

#[test]
fn test_observer_obj_flag_ge() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    f.ui().obj_bind_flag_if_ge(obj, subject, ObjFlags::HIDDEN, 5);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 5);
    assert!(f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 4);
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 6);
    assert!(f.ui().obj_is_hidden(obj));
    tear_down(f);
}

#[test]
fn test_observer_obj_flag_gt() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    f.ui().obj_bind_flag_if_gt(obj, subject, ObjFlags::HIDDEN, 5);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 5);
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 6);
    assert!(f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 4);
    assert!(!f.ui().obj_is_hidden(obj));
    tear_down(f);
}

#[test]
fn test_observer_obj_flag_le() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 7);

    f.ui().obj_bind_flag_if_le(obj, subject, ObjFlags::HIDDEN, 5);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 5);
    assert!(f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 6);
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 4);
    assert!(f.ui().obj_is_hidden(obj));
    tear_down(f);
}

#[test]
fn test_observer_obj_flag_lt() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 7);

    f.ui().obj_bind_flag_if_lt(obj, subject, ObjFlags::HIDDEN, 5);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 4);
    assert!(f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 5);
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 3);
    assert!(f.ui().obj_is_hidden(obj));
    tear_down(f);
}

#[test]
fn test_observer_obj_state_eq() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    f.ui().obj_bind_state_if_eq(obj, subject, State::CHECKED, 5);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().obj_bind_state_if_not_eq(obj, subject, State::DISABLED, 10);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));
    assert!(f.ui().obj_has_state(obj, State::DISABLED));

    f.ui().subject_set_int(subject, 5);
    assert!(f.ui().obj_has_state(obj, State::CHECKED));
    assert!(f.ui().obj_has_state(obj, State::DISABLED));

    f.ui().subject_set_int(subject, 10);
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));
    assert!(!f.ui().obj_has_state(obj, State::DISABLED));
    tear_down(f);
}

#[test]
fn test_observer_obj_state_gt() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    f.ui().obj_bind_state_if_gt(obj, subject, State::CHECKED, 5);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 6);
    assert!(f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 5);
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 7);
    assert!(f.ui().obj_has_state(obj, State::CHECKED));
    tear_down(f);
}

#[test]
fn test_observer_obj_state_ge() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    f.ui().obj_bind_state_if_ge(obj, subject, State::CHECKED, 5);
    /*Should be applied immediately*/
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 6);
    assert!(f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 4);
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 5);
    assert!(f.ui().obj_has_state(obj, State::CHECKED));
    tear_down(f);
}

#[test]
fn test_observer_obj_state_le() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    f.ui().obj_bind_state_if_le(obj, subject, State::CHECKED, 5);
    /*Should be applied immediately*/
    assert!(f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 6);
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 4);
    assert!(f.ui().obj_has_state(obj, State::CHECKED));
    tear_down(f);
}

#[test]
fn test_observer_obj_state_lt() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);

    f.ui().obj_bind_state_if_lt(obj, subject, State::CHECKED, 5);
    /*Should be applied immediately*/
    assert!(f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 5);
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 4);
    assert!(f.ui().obj_has_state(obj, State::CHECKED));
    tear_down(f);
}

/* Recommended replacement for lv_obj_bind_flag_if_*: bind a flag from a boolean
 * subject by passing a dedicated per-flag setter directly to lv_obj_bind_bool. */
#[test]
fn test_observer_obj_bind_bool_flag() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 0);

    let observer = f.ui().obj_bind_bool(
        obj,
        subject,
        Rc::new(|ui: &mut Ui, obj, en| ui.obj_set_hidden(obj, en)),
    );
    assert!(observer.is_some());

    /*Applied immediately: 0 -> not hidden*/
    assert!(!f.ui().obj_is_hidden(obj));

    /*Any non-zero value -> hidden*/
    f.ui().subject_set_int(subject, 1);
    assert!(f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 5);
    assert!(f.ui().obj_is_hidden(obj));

    f.ui().subject_set_int(subject, 0);
    assert!(!f.ui().obj_is_hidden(obj));

    /*After removing the observer the widget is no longer updated*/
    f.ui().observer_delete(observer);
    f.ui().subject_set_int(subject, 1);
    assert!(!f.ui().obj_is_hidden(obj));

    f.ui().obj_delete(obj);
    tear_down(f);
}

/* The remaining lv_obj_bind_<type>() functions just forward the subject's value
 * to the setter; these tests confirm the value arrives on subscribe and update. */

/// `capture_int_cb()`
fn capture_int_cb(_ui: &mut Ui, _obj: Obj, v: i32) {
    CAPTURED_INT.set(v);
}

#[test]
fn test_observer_obj_bind_int() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 10);

    assert!(
        f.ui()
            .obj_bind_int(obj, subject, Rc::new(capture_int_cb))
            .is_some()
    );
    assert_eq!(10, CAPTURED_INT.get());
    f.ui().subject_set_int(subject, 42);
    assert_eq!(42, CAPTURED_INT.get());

    f.ui().obj_delete(obj);
    tear_down(f);
}

/// `capture_float_cb()`
fn capture_float_cb(_ui: &mut Ui, _obj: Obj, v: f32) {
    CAPTURED_FLOAT.set(v);
}

#[test]
fn test_observer_obj_bind_float() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Float);
    f.ui().subject_set_float(subject, 1.5);

    assert!(
        f.ui()
            .obj_bind_float(obj, subject, Rc::new(capture_float_cb))
            .is_some()
    );
    assert_eq!(1.5, CAPTURED_FLOAT.get());
    f.ui().subject_set_float(subject, 2.5);
    assert_eq!(2.5, CAPTURED_FLOAT.get());

    f.ui().obj_delete(obj);
    tear_down(f);
}

/// `capture_string_cb()`
fn capture_string_cb(_ui: &mut Ui, _obj: Obj, v: &str) {
    CAPTURED_STRING.set(Some(v.to_string()));
}

#[test]
fn test_observer_obj_bind_string() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));
    /* static char buf[32]; static char prev_buf[32]; */

    let subject = subject_create(&mut f, SubjectType::String);
    f.ui().subject_set_string_buffer_static(subject, 32, true);
    f.ui().subject_set_string(subject, "hello");

    assert!(
        f.ui()
            .obj_bind_string(obj, subject, Rc::new(capture_string_cb))
            .is_some()
    );
    assert_eq!(
        Some("hello"),
        CAPTURED_STRING.with_borrow(|s| s.clone()).as_deref()
    );
    f.ui().subject_set_string(subject, "world");
    assert_eq!(
        Some("world"),
        CAPTURED_STRING.with_borrow(|s| s.clone()).as_deref()
    );

    f.ui().obj_delete(obj);
    tear_down(f);
}

/// `capture_color_cb()`
fn capture_color_cb(_ui: &mut Ui, _obj: Obj, v: Color) {
    CAPTURED_COLOR.set(v);
}

#[test]
fn test_observer_obj_bind_color() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Color);
    f.ui().subject_set_color(subject, Color::hex(0x123456));

    assert!(
        f.ui()
            .obj_bind_color(obj, subject, Rc::new(capture_color_cb))
            .is_some()
    );
    assert_eq!(Color::hex(0x123456), CAPTURED_COLOR.get());
    f.ui().subject_set_color(subject, Color::hex(0xabcdef));
    assert_eq!(Color::hex(0xabcdef), CAPTURED_COLOR.get());

    f.ui().obj_delete(obj);
    tear_down(f);
}

/// `capture_pointer_cb()`
fn capture_pointer_cb(_ui: &mut Ui, _obj: Obj, v: Option<Rc<dyn Any>>) {
    CAPTURED_POINTER.set(v);
}

#[test]
fn test_observer_obj_bind_pointer() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));
    /* static const char * a = "a"; static const char * b = "b"; */
    let a: Rc<dyn Any> = Rc::new("a");
    let b: Rc<dyn Any> = Rc::new("b");

    let subject = subject_create(&mut f, SubjectType::Pointer);
    f.ui().subject_set_pointer(subject, Some(a.clone()));

    assert!(
        f.ui()
            .obj_bind_pointer(obj, subject, Rc::new(capture_pointer_cb))
            .is_some()
    );
    assert_ptr_eq(Some(&a), CAPTURED_POINTER.with_borrow(|p| p.clone()));
    f.ui().subject_set_pointer(subject, Some(b.clone()));
    assert_ptr_eq(Some(&b), CAPTURED_POINTER.with_borrow(|p| p.clone()));

    f.ui().obj_delete(obj);
    tear_down(f);
}

/* Recommended replacement for lv_obj_bind_state_if_*: toggle a state from a
 * custom observer added with lv_subject_add_observer_obj. */

/// `set_disabled_observer_cb()`
fn set_disabled_observer_cb(ui: &mut Ui, observer: ObserverId, subject: SubjectId) {
    let target = ui.observer_get_target_obj(observer).unwrap();
    let v = ui.subject_get_int(subject) != 0;
    ui.obj_set_state(target, State::DISABLED, v);
}

#[test]
fn test_observer_obj_state_via_observer() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 0);

    let observer =
        f.ui()
            .subject_add_observer_obj(subject, Rc::new(set_disabled_observer_cb), Some(obj), None);
    assert!(observer.is_some());

    /*Applied immediately*/
    assert!(!f.ui().obj_has_state(obj, State::DISABLED));

    f.ui().subject_set_int(subject, 1);
    assert!(f.ui().obj_has_state(obj, State::DISABLED));

    f.ui().subject_set_int(subject, 0);
    assert!(!f.ui().obj_has_state(obj, State::DISABLED));

    f.ui().obj_delete(obj);
    tear_down(f);
}

#[test]
fn test_observer_button_checked() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = Button::create(f.ui(), screen).obj();
    f.ui().obj_set_size(obj, 100, 100);
    f.ui().obj_set_checkable(obj, true);
    f.ui().obj_update_layout(obj);

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 1);
    f.ui().obj_bind_checked(obj, subject);

    assert!(f.ui().obj_has_state(obj, State::CHECKED));

    f.ui().subject_set_int(subject, 0);
    assert!(!f.ui().obj_has_state(obj, State::CHECKED));

    f.inputs.mouse_click_at(&mut f.display, 10, 10);
    assert!(f.ui().obj_has_state(obj, State::CHECKED));
    assert_eq!(1, f.ui().subject_get_int(subject));
    tear_down(f);
}

#[test]
fn test_observer_label_text_normal() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = Label::create(f.ui(), screen);

    /*Cannot bind color*/
    let subject_color = subject_create(&mut f, SubjectType::Color);
    let observer = obj.bind_text(f.ui(), subject_color, None);
    assert!(observer.is_none());

    /*Bind it with "%d" if NULL is passed*/
    let subject_int = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject_int, 10);
    let observer = obj.bind_text(f.ui(), subject_int, None);
    assert!(observer.is_some());
    assert_eq!("10", obj.text(f.ui()));

    /*Bind it with "%0.1f" if NULL is passed*/
    let subject_float = subject_create(&mut f, SubjectType::Float);
    f.ui().subject_set_float(subject_float, 10.5);
    let observer = obj.bind_text(f.ui(), subject_float, None);
    assert!(observer.is_some());
    assert_eq!("10.5", obj.text(f.ui()));

    /*An explicit format string is used as-is*/
    f.ui().subject_set_float(subject_float, 81.5);
    let observer = obj.bind_text(f.ui(), subject_float, Some("Value: %0.2f"));
    assert!(observer.is_some());
    assert_eq!("Value: 81.50", obj.text(f.ui()));

    /*Bind to string*/
    /* static char buf[32]; */
    let subject_string = subject_create(&mut f, SubjectType::String);
    f.ui().subject_set_string_buffer_static(subject_string, 32, false);
    f.ui().subject_set_string(subject_string, "hello");
    obj.bind_text(f.ui(), subject_string, None);
    assert_eq!("hello", obj.text(f.ui()));

    f.ui().subject_set_string(subject_string, "world");
    assert_eq!("world", obj.text(f.ui()));

    /*Remove the label from the subject*/
    f.ui().obj_remove_from_subject(obj.obj(), Some(subject_string));
    f.ui().subject_set_string(subject_string, "nothing");
    assert_eq!("world", obj.text(f.ui()));

    /*Bind to pointer*/
    let subject_pointer = subject_create(&mut f, SubjectType::Pointer);
    f.ui()
        .subject_set_pointer(subject_pointer, Some(Rc::new("HELLO")));
    obj.bind_text(f.ui(), subject_pointer, None);
    assert_eq!("HELLO", obj.text(f.ui()));

    f.ui()
        .subject_set_pointer(subject_pointer, Some(Rc::new("WORLD")));
    assert_eq!("WORLD", obj.text(f.ui()));

    /*Remove the label from the subject*/
    f.ui().obj_remove_from_subject(obj.obj(), Some(subject_pointer));
    f.ui()
        .subject_set_pointer(subject_pointer, Some(Rc::new("NOTHING")));
    assert_eq!("WORLD", obj.text(f.ui()));
    tear_down(f);
}

#[test]
fn test_observer_label_text_formatted() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = Label::create(f.ui(), screen);

    /*Cannot bind color*/
    let subject_color = subject_create(&mut f, SubjectType::Color);
    let observer = obj.bind_text(f.ui(), subject_color, None);
    assert!(observer.is_none());

    /*Bind to int*/
    let subject_int = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject_int, 10);
    obj.bind_text(f.ui(), subject_int, Some("value: %d"));
    assert_eq!("value: 10", obj.text(f.ui()));

    f.ui().subject_set_int(subject_int, -20);
    assert_eq!("value: -20", obj.text(f.ui()));

    /*Remove the label from the subject*/
    f.ui().obj_remove_from_subject(obj.obj(), Some(subject_int));
    f.ui().subject_set_int(subject_int, 100);
    assert_eq!("value: -20", obj.text(f.ui()));

    /*Bind to string*/
    /* static char buf[32]; */
    let subject_string = subject_create(&mut f, SubjectType::String);
    f.ui().subject_set_string_buffer_static(subject_string, 32, false);
    f.ui().subject_set_string(subject_string, "hello");
    obj.bind_text(f.ui(), subject_string, Some("text: %s"));
    assert_eq!("text: hello", obj.text(f.ui()));

    f.ui().subject_set_string(subject_string, "world");
    assert_eq!("text: world", obj.text(f.ui()));

    /*Remove the label from the subject*/
    f.ui().obj_remove_from_subject(obj.obj(), Some(subject_string));
    f.ui().subject_set_string(subject_string, "nothing");
    assert_eq!("text: world", obj.text(f.ui()));

    /*Bind to pointer*/
    let subject_pointer = subject_create(&mut f, SubjectType::Pointer);
    f.ui()
        .subject_set_pointer(subject_pointer, Some(Rc::new("HELLO")));
    obj.bind_text(f.ui(), subject_pointer, Some("pointer: %s"));
    assert_eq!("pointer: HELLO", obj.text(f.ui()));

    f.ui()
        .subject_set_pointer(subject_pointer, Some(Rc::new("WORLD")));
    assert_eq!("pointer: WORLD", obj.text(f.ui()));

    /*Remove the label from the subject*/
    f.ui().obj_remove_from_subject(obj.obj(), Some(subject_pointer));
    f.ui()
        .subject_set_pointer(subject_pointer, Some(Rc::new("pointer: NOTHING")));
    assert_eq!("pointer: WORLD", obj.text(f.ui()));
    tear_down(f);
}

#[test]
fn test_observer_arc_value() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = Arc::create(f.ui(), screen);

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 30);
    obj.bind_value(f.ui(), subject);

    assert_eq!(30, obj.value(f.ui()));

    f.ui().subject_set_int(subject, 40);
    assert_eq!(40, obj.value(f.ui()));

    f.ui().obj_update_layout(obj.obj());
    f.inputs.mouse.release();
    f.display.wait(100);

    f.inputs.mouse.move_to(65, 10);
    f.inputs.mouse.press();
    f.display.wait(100);
    f.inputs.mouse.release();

    assert_eq!(50, obj.value(f.ui()));
    assert_eq!(50, f.ui().subject_get_int(subject));
    tear_down(f);
}

#[test]
fn test_observer_slider_value() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = Slider::create(f.ui(), screen);

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 30);
    obj.bind_value(f.ui(), subject);

    assert_eq!(30, obj.value(f.ui()));

    f.ui().subject_set_int(subject, 40);
    assert_eq!(40, obj.value(f.ui()));

    f.ui().obj_update_layout(obj.obj());
    f.inputs.mouse.release();
    f.display.wait(100);

    f.inputs.mouse.move_to(65, 10);
    f.inputs.mouse.press();
    f.display.wait(100);

    f.inputs.mouse.move_to(75, 10);
    f.inputs.mouse.press();
    f.display.wait(100);
    f.inputs.mouse.release();

    assert_eq!(29, obj.value(f.ui()));
    assert_eq!(29, f.ui().subject_get_int(subject));
    tear_down(f);
}

/// Not upstream (no `test_observer.c` case): `lv_image_bind_src()` sets the
/// pointer subject's value as the source and rejects other subject types.
#[test]
fn image_bind_src() {
    let mut f = set_up();
    let screen = f.screen();
    let obj = Image::create(f.ui(), screen);

    let src_a = ImageSrc::Symbol(SharedRef::Static("\u{f03e}"));
    let src_b = ImageSrc::Symbol(SharedRef::Static("\u{f00c}"));
    let subject = subject_create(&mut f, SubjectType::Pointer);
    f.ui().subject_set_pointer(subject, Some(Rc::new(src_a.clone())));
    assert!(obj.bind_src(f.ui(), subject).is_some());
    assert!(obj.src(f.ui()) == src_a);

    f.ui().subject_set_pointer(subject, Some(Rc::new(src_b.clone())));
    assert!(obj.src(f.ui()) == src_b);

    f.ui().subject_set_pointer(subject, None);
    assert!(obj.src(f.ui()) == ImageSrc::None);

    if !check_arg_asserts() {
        let subject_int = subject_create(&mut f, SubjectType::Int);
        assert!(obj.bind_src(f.ui(), subject_int).is_none());
    }
    tear_down(f);
}

#[test]
fn test_observer_set_user_data() {
    let mut f = set_up();

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 5);

    let observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);
    assert!(observer.is_some());
    let observer = observer.unwrap();

    /* Initially NULL */
    assert_ptr_eq(None, f.ui().observer_get_user_data(observer));

    /* static int32_t a; static int32_t b; */
    let a: Rc<dyn Any> = Rc::new(0i32);
    let b: Rc<dyn Any> = Rc::new(0i32);
    f.ui().observer_set_user_data(observer, Some(a.clone()));
    assert_ptr_eq(Some(&a), f.ui().observer_get_user_data(observer));

    f.ui().observer_set_user_data(observer, Some(b.clone()));
    assert_ptr_eq(Some(&b), f.ui().observer_get_user_data(observer));

    f.ui().observer_delete(Some(observer));
    tear_down(f);
}

#[test]
fn test_observer_set_user_data_user_owned() {
    let mut f = set_up();

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 5);

    let observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);
    assert!(observer.is_some());
    let observer = observer.unwrap();

    /* The observer must not free user-owned data on removal. */
    let data: Rc<dyn Any> = Rc::new([0u8; 64]);
    f.ui().observer_set_user_data(observer, Some(data.clone()));
    assert_ptr_eq(Some(&data), f.ui().observer_get_user_data(observer));

    f.ui().observer_delete(Some(observer));
    /* TEST_ASSERT_MEM_LEAK_LESS_THAN: only the caller's reference is left */
    assert_eq!(1, Rc::strong_count(&data));
    assert!(f.ui().observer.observers.is_empty());
    tear_down(f);
}

#[test]
fn test_observer_set_user_data_replaces_internal_data() {
    let mut f = set_up();

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 5);

    let observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);
    assert!(observer.is_some());
    let observer = observer.unwrap();

    /* Simulate an observer whose data is owned by an internal binding. */
    let old_data: Rc<dyn Any> = Rc::new([0u8; 64]);
    {
        let o = f.ui().observer.observers.get_mut(observer.0).unwrap();
        o.user_data = Some(old_data.clone());
        o.auto_free_user_data = true;
    }

    let new_data: Rc<dyn Any> = Rc::new([0u8; 64]);
    f.ui().observer_set_user_data(observer, Some(new_data.clone()));
    assert_ptr_eq(Some(&new_data), f.ui().observer_get_user_data(observer));
    assert!(
        !f.ui()
            .observer
            .observers
            .get(observer.0)
            .unwrap()
            .auto_free_user_data
    );
    /* LV_HEAP_CHECK(...): the internally-owned data was released */
    assert_eq!(1, Rc::strong_count(&old_data));

    f.ui().observer_delete(Some(observer));
    /* TEST_ASSERT_MEM_LEAK_LESS_THAN */
    assert_eq!(1, Rc::strong_count(&new_data));
    tear_down(f);
}

#[test]
fn test_observer_set_user_data_same_internal_data() {
    let mut f = set_up();

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 5);

    let observer = f
        .ui()
        .subject_add_observer(subject, Rc::new(observer_basic), None);
    assert!(observer.is_some());
    let observer = observer.unwrap();

    let data: Rc<dyn Any> = Rc::new([0u8; 64]);
    {
        let o = f.ui().observer.observers.get_mut(observer.0).unwrap();
        o.user_data = Some(data.clone());
        o.auto_free_user_data = true;
    }

    let mem_with_data = Rc::strong_count(&data);
    f.ui().observer_set_user_data(observer, Some(data.clone()));
    assert_ptr_eq(Some(&data), f.ui().observer_get_user_data(observer));
    assert!(
        !f.ui()
            .observer
            .observers
            .get(observer.0)
            .unwrap()
            .auto_free_user_data
    );
    /* TEST_ASSERT_EQUAL_UINT32(mem_with_data, lv_test_get_free_mem()) */
    assert_eq!(mem_with_data, Rc::strong_count(&data));

    f.ui().observer_delete(Some(observer));
    /* TEST_ASSERT_MEM_LEAK_LESS_THAN */
    assert_eq!(1, Rc::strong_count(&data));
    tear_down(f);
}

#[test]
fn test_observer_delete() {
    let mut f = set_up();
    /* uint32_t mem = lv_test_get_free_mem(); */
    let mem = (
        f.ui().observer.subjects.len(),
        f.ui().observer.observers.len(),
        f.ui().event_dscs.len(),
        f.ui().objs.len(),
    );
    let screen = f.screen();
    for _ in 0..64 {
        let obj1 = Slider::create(f.ui(), screen);
        let obj2 = Slider::create(f.ui(), screen);

        let subject = f.ui().subject_create(SubjectType::Int).unwrap();
        f.ui().subject_set_int(subject, 30);
        obj1.bind_value(f.ui(), subject);
        obj2.bind_value(f.ui(), subject);
        f.ui().subject_add_observer(subject, Rc::new(observer_int), None);
        f.ui().obj_delete(obj1.obj());
        f.ui().subject_delete(Some(subject));
        f.ui().obj_delete(obj2.obj());
    }

    /* TEST_ASSERT_MEM_LEAK_LESS_THAN(mem, 32) */
    let after = (
        f.ui().observer.subjects.len(),
        f.ui().observer.observers.len(),
        f.ui().event_dscs.len(),
        f.ui().objs.len(),
    );
    assert_eq!(mem, after);
    tear_down(f);
}

/*
 * `tests/src/test_cases/public_api/test_observer_args.c`
 *
 * Built only when `LV_CHECK_ARG_ASSERT_ON_FAIL` is off
 * (`lvgl_add_test_folder_if(NOT CONFIG_LV_CHECK_ARG_ASSERT_ON_FAIL test_cases/public_api)`):
 * every case below makes `LV_CHECK_ARG` fail on purpose. With
 * ferrovgl-core's `check-arg-panic` feature (unified in by
 * `cargo test --workspace --all-features`) they are skipped the same way.
 */

/// `CONFIG_LV_CHECK_ARG_ASSERT_ON_FAIL`: whether a failed `LV_CHECK_ARG`
/// panics in this build.
fn check_arg_asserts() -> bool {
    std::panic::catch_unwind(|| ferrovgl_core::misc::check_arg_failed("probe")).is_err()
}

/// `observer_int()` of `test_observer_args.c`
fn args_observer_int(_ui: &mut Ui, _observer: ObserverId, _subject: SubjectId) {
    OBSERVER_CALLED.set(OBSERVER_CALLED.get() + 1);
}

/*A setter for the wrong type leaves the value and the observers alone*/
#[test]
fn test_observer_args_int_setters_of_other_types_are_ignored() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 15);
    f.ui()
        .subject_add_observer(subject, Rc::new(args_observer_int), None);
    OBSERVER_CALLED.set(0);

    f.ui().subject_set_pointer(subject, None);
    f.ui().subject_set_color(subject, Color::black());
    f.ui().subject_set_string(subject, "hello");

    assert_eq!(15, f.ui().subject_get_int(subject));
    assert_eq!(0, OBSERVER_CALLED.get());
    tear_down(f);
}

#[test]
fn test_observer_args_float_setters_of_other_types_are_ignored() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    let subject = subject_create(&mut f, SubjectType::Float);
    f.ui().subject_set_float(subject, 15.75);
    f.ui()
        .subject_add_observer(subject, Rc::new(args_observer_int), None);
    OBSERVER_CALLED.set(0);

    f.ui().subject_set_pointer(subject, None);
    f.ui().subject_set_color(subject, Color::black());
    f.ui().subject_set_string(subject, "hello");

    assert_eq!(15.75, f.ui().subject_get_float(subject));
    assert_eq!(0, OBSERVER_CALLED.get());
    tear_down(f);
}

#[test]
fn test_observer_args_string_setters_of_other_types_are_ignored() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    /* static char buf_current[32]; static char buf_previous[32]; */
    let subject = subject_create(&mut f, SubjectType::String);
    f.ui().subject_set_string_buffer_static(subject, 32, true);
    f.ui().subject_set_string(subject, "a");
    f.ui()
        .subject_add_observer(subject, Rc::new(args_observer_int), None);
    OBSERVER_CALLED.set(0);

    f.ui().subject_set_pointer(subject, None);
    f.ui().subject_set_color(subject, Color::black());
    f.ui().subject_set_int(subject, 10);

    assert_eq!(Some("a"), f.ui().subject_get_string(subject));
    assert_eq!(0, OBSERVER_CALLED.get());
    tear_down(f);
}

#[test]
fn test_observer_args_color_setters_of_other_types_are_ignored() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    let subject = subject_create(&mut f, SubjectType::Color);
    f.ui().subject_set_color(subject, Color::hex3(0xabc));
    f.ui()
        .subject_add_observer(subject, Rc::new(args_observer_int), None);
    OBSERVER_CALLED.set(0);

    f.ui().subject_set_pointer(subject, None);
    f.ui().subject_set_int(subject, 10);
    f.ui().subject_set_string(subject, "hello");

    assert_eq!(Color::hex3(0xabc), f.ui().subject_get_color(subject));
    assert_eq!(0, OBSERVER_CALLED.get());
    tear_down(f);
}

#[test]
fn test_observer_args_pointer_setters_of_other_types_are_ignored() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    /* static const int a[] = {1, 2, 3}; */
    let a: [Rc<dyn Any>; 3] = [Rc::new(1i32), Rc::new(2i32), Rc::new(3i32)];
    let subject = subject_create(&mut f, SubjectType::Pointer);
    f.ui().subject_set_pointer(subject, Some(a[2].clone()));
    f.ui()
        .subject_add_observer(subject, Rc::new(args_observer_int), None);
    OBSERVER_CALLED.set(0);

    f.ui().subject_set_int(subject, 10);
    f.ui().subject_set_color(subject, Color::black());
    f.ui().subject_set_string(subject, "hello");

    assert_ptr_eq(Some(&a[2]), f.ui().subject_get_pointer(subject));
    assert_eq!(0, OBSERVER_CALLED.get());
    tear_down(f);
}

/*An index past the end of the group reports no element*/
#[test]
fn test_observer_args_group_element_out_of_range() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    let mut list = Vec::new();
    for i in 0..2 {
        let s = subject_create(&mut f, SubjectType::Int);
        f.ui().subject_set_int(s, i + 1);
        list.push(s);
    }

    let group_subject = subject_create(&mut f, SubjectType::Group);
    f.ui()
        .subject_set_group_list_static(group_subject, Rc::from(&list[..]));

    assert_eq!(None, f.ui().subject_get_group_element(group_subject, 2));
    assert_eq!(None, f.ui().subject_get_group_element(group_subject, 1000));
    assert_eq!(None, f.ui().subject_get_group_element(group_subject, -1));
    tear_down(f);
}

/*A subject that was never initialised has no type to observe*/
#[test]
fn test_observer_args_add_observer_to_uninitialized_subject() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    /* static lv_subject_t uninitialized_subject; */
    let uninitialized_subject = f.ui().subject_alloc_static();
    assert!(
        f.ui()
            .subject_add_observer(uninitialized_subject, Rc::new(args_observer_int), None)
            .is_none()
    );
    /* The static storage is not part of `subject_ll`; release it for tearDown's leak check. */
    f.ui().observer.subjects.remove(uninitialized_subject.0);
    tear_down(f);
}

/*lv_obj_bind_checked() only accepts an int subject*/
#[test]
fn test_observer_args_bind_checked_to_non_int_subject() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    let screen = f.screen();
    let obj = Button::create(f.ui(), screen).obj();
    f.ui().obj_set_checkable(obj, true);

    let subject_wrong = subject_create(&mut f, SubjectType::Pointer);
    assert!(f.ui().obj_bind_checked(obj, subject_wrong).is_none());
    tear_down(f);
}

/*A string setter on a pointer subject changes nothing, so a bound label stays put*/
#[test]
fn test_observer_args_set_string_to_pointer_subject_is_ignored() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    let screen = f.screen();
    let obj = Label::create(f.ui(), screen);
    let subject_pointer = subject_create(&mut f, SubjectType::Pointer);
    f.ui()
        .subject_set_pointer(subject_pointer, Some(Rc::new("HELLO")));
    obj.bind_text(f.ui(), subject_pointer, None);
    assert_eq!("HELLO", obj.text(f.ui()));

    f.ui().subject_set_string(subject_pointer, "NOTHING");
    assert_eq!("HELLO", obj.text(f.ui()));
    tear_down(f);
}

#[test]
fn test_observer_obj_bind_bool_invalid() {
    if check_arg_asserts() {
        return;
    }
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 0);

    let set_hidden: ObjSetBoolCb = Rc::new(|ui: &mut Ui, obj, en| ui.obj_set_hidden(obj, en));
    /* lv_obj_bind_bool(NULL, ...): a deleted object stands for NULL */
    let dead = f.ui().obj_create(Some(screen));
    f.ui().obj_delete(dead);
    assert!(f.ui().obj_bind_bool(dead, subject, set_hidden.clone()).is_none());
    /* lv_obj_bind_bool(obj, NULL, ...): a deleted subject stands for NULL */
    let dead_subject = f.ui().subject_create(SubjectType::Int).unwrap();
    f.ui().subject_delete(Some(dead_subject));
    assert!(f.ui().obj_bind_bool(obj, dead_subject, set_hidden).is_none());
    /* lv_obj_bind_bool(obj, subject, NULL): the setter cannot be NULL in Rust */

    f.ui().obj_delete(obj);
    tear_down(f);
}

#[test]
fn test_observer_obj_flag_invalid_subject() {
    if check_arg_asserts() {
        return;
    }
    type BindFlagFn = fn(&mut Ui, Obj, SubjectId, ObjFlags, i32) -> Option<ObserverId>;
    let fns: [BindFlagFn; 6] = [
        Ui::obj_bind_flag_if_eq,
        Ui::obj_bind_flag_if_not_eq,
        Ui::obj_bind_flag_if_ge,
        Ui::obj_bind_flag_if_gt,
        Ui::obj_bind_flag_if_lt,
        Ui::obj_bind_flag_if_le,
    ];

    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    /* Can only bind to int */
    let invalid = [
        subject_create(&mut f, SubjectType::Pointer),
        subject_create(&mut f, SubjectType::String),
        subject_create(&mut f, SubjectType::Color),
        subject_create(&mut f, SubjectType::Group),
    ];

    for func in fns {
        for &s in &invalid {
            assert!(func(f.ui(), obj, s, ObjFlags::HIDDEN, 5).is_none());
        }
    }
    tear_down(f);
}

#[test]
fn test_observer_obj_state_invalid_subject() {
    if check_arg_asserts() {
        return;
    }
    type BindStateFn = fn(&mut Ui, Obj, SubjectId, State, i32) -> Option<ObserverId>;
    let fns: [BindStateFn; 6] = [
        Ui::obj_bind_state_if_eq,
        Ui::obj_bind_state_if_not_eq,
        Ui::obj_bind_state_if_ge,
        Ui::obj_bind_state_if_gt,
        Ui::obj_bind_state_if_lt,
        Ui::obj_bind_state_if_le,
    ];
    let mut f = set_up();
    let screen = f.screen();
    let obj = f.ui().obj_create(Some(screen));

    /* Can only bind to int */
    let invalid = [
        subject_create(&mut f, SubjectType::Pointer),
        subject_create(&mut f, SubjectType::String),
        subject_create(&mut f, SubjectType::Color),
        subject_create(&mut f, SubjectType::Group),
    ];

    for func in fns {
        for &s in &invalid {
            assert!(func(f.ui(), obj, s, State::empty(), 5).is_none());
        }
    }
    tear_down(f);
}

/*
 * Not upstream: `lv_bar_bind_value()` has no upstream test case. Same
 * scenario as `test_observer_slider_value` without the input part.
 */
#[test]
fn bar_bind_value_follows_int_and_float_subjects() {
    let mut f = set_up();
    let screen = f.screen();
    let bar = Bar::create(f.ui(), screen);

    let subject = subject_create(&mut f, SubjectType::Int);
    f.ui().subject_set_int(subject, 30);
    assert!(bar.bind_value(f.ui(), subject).is_some());
    assert_eq!(30, bar.value(f.ui()));
    f.ui().subject_set_int(subject, 40);
    assert_eq!(40, bar.value(f.ui()));

    let subject_float = subject_create(&mut f, SubjectType::Float);
    f.ui().subject_set_float(subject_float, 12.75);
    assert!(bar.bind_value(f.ui(), subject_float).is_some());
    /* (int32_t)subject->value.float_v */
    assert_eq!(12, bar.value(f.ui()));

    /* Deleting the bar removes its observers (unsubscribe_on_delete_cb) */
    f.ui().obj_delete(bar.obj());
    assert_eq!(0, subs_len(f.ui(), subject));
    assert_eq!(0, subs_len(f.ui(), subject_float));
    bar.set_value(f.ui(), 5, ANIM_OFF);
    tear_down(f);
}
