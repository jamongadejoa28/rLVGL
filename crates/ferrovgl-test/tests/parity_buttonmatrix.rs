//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_btnmatrix.c`.
//!
//! Every upstream test function, in upstream order, with its values and
//! screenshot names. `setUp` creates a button matrix on the active screen of
//! a fresh default-theme display with the upstream pointer/keypad/encoder test
//! devices; `tearDown`'s `lv_obj_clean` is the display drop. No test of this
//! file changes the screen itself (styles, flags, scroll), so a fresh screen
//! is the same as the cleaned shared screen upstream. The upstream test input
//! devices are shared by every test of the file, though, so the pointer keeps
//! its last point across tests; a test that depends on that inherited indev
//! state reproduces it explicitly (see
//! `button_matrix_pressing_event_works`). Direct reads and writes of
//! `lv_buttonmatrix_t` fields go through the matching [`Buttonmatrix`]
//! accessors. The tests after the upstream ones are regression tests of
//! upstream behaviour that no upstream test covers.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::draw::DrawDsc;
use ferrovgl_core::group::{KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_UP};
use ferrovgl_core::misc::style::{SharedRef, SharedStr};
use ferrovgl_core::{Area, ColorFormat, EventCode, EventParam, Obj, Part, StyleSelector};
use ferrovgl_test::{TestDisplay, TestInputs};
use ferrovgl_widgets::{BUTTONMATRIX_BUTTON_NONE, Buttonmatrix, ButtonmatrixCtrl, ButtonmatrixMap};

const ANIM_OFF: bool = false;

/// The globals of `test_btnmatrix.c` set by `setUp()`.
struct Fixture {
    display: TestDisplay,
    inputs: TestInputs,
    active_screen: Obj,
    btnm: Buttonmatrix,
    /// `event_triggered`
    event_triggered: Rc<Cell<bool>>,
    /// `exp_evt_code`
    exp_evt_code: Rc<Cell<EventCode>>,
}

/// `setUp()`
fn set_up() -> Fixture {
    set_up_with(|_, _| {})
}

/// `setUp()` after `inherit` recreated input device state that upstream
/// carries over from an earlier test of the file (the indevs are shared).
fn set_up_with(inherit: impl FnOnce(&mut TestDisplay, &TestInputs)) -> Fixture {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let inputs = display.create_inputs();
    inherit(&mut display, &inputs);
    let active_screen = display.screen().unwrap();
    let btnm = Buttonmatrix::create(&mut display.ui, active_screen);
    assert!(display.ui.obj_is_valid(btnm.obj()));
    Fixture {
        display,
        inputs,
        active_screen,
        btnm,
        event_triggered: Rc::new(Cell::new(false)),
        exp_evt_code: Rc::new(Cell::new(EventCode(0))),
    }
}

/// A `static const char * map[]` of the test.
fn map(entries: &[&'static str]) -> ButtonmatrixMap {
    SharedRef::Shared(
        entries
            .iter()
            .map(|s| SharedRef::Static(*s))
            .collect::<Rc<[SharedStr]>>(),
    )
}

/// The `{"A", "B", "\n", "C", "D", ""}` map most tests use.
fn abcd_map() -> ButtonmatrixMap {
    map(&["A", "B", "\n", "C", "D", ""])
}

/// `lv_obj_add_event_cb(btnm, event_handler, code, NULL)` with the common
/// `event_handler()`.
fn add_event_handler(f: &mut Fixture, code: EventCode) {
    let triggered = f.event_triggered.clone();
    let exp = f.exp_evt_code.clone();
    f.display.ui.obj_add_event_cb(
        f.btnm.obj(),
        Rc::new(move |_ui, e| {
            if e.code() == exp.get() {
                triggered.set(true);
            }
        }),
        code,
        None,
    );
}

/// `lv_obj_send_event(btnm, LV_EVENT_KEY, &keyCode)`
fn send_key(f: &mut Fixture, key: u32) {
    f.display
        .ui
        .obj_send_event(f.btnm.obj(), EventCode::KEY, EventParam::Key(key));
}

#[test]
fn button_matrix_creation() {
    let f = set_up();
    /* Verify the default map. */
    let map = f.btnm.map(&f.display.ui).unwrap();
    assert_eq!(&*map[0], "Btn1");
    assert_eq!(&*map[1], "Btn2");
    assert_eq!(&*map[2], "Btn3");
    assert_eq!(&*map[3], "\n");
    assert_eq!(&*map[4], "Btn4");
    assert_eq!(&*map[5], "Btn5");
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn button_matrix_set_map_works() {
    let mut f = set_up();
    let exp_map = abcd_map();

    f.btnm.set_map(&mut f.display.ui, exp_map.clone());
    /* Verify if the map was set correctly. */
    let ret_map = f.btnm.map(&f.display.ui).unwrap();
    assert_eq!(ret_map, exp_map);
    for i in 0..6 {
        assert_eq!(&*exp_map[i], &*ret_map[i]);
    }

    f.display.screenshot_compare("widgets/btnm_1.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn button_matrix_render_2() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let mut btnm = f.btnm;
    btnm.set_map(ui, abcd_map());
    btnm.set_button_ctrl_all(ui, ButtonmatrixCtrl::CHECKABLE);
    btnm.set_button_ctrl(ui, 0, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 3, ButtonmatrixCtrl::CHECKED);
    ui.obj_set_pos(btnm.obj(), 10, 10);

    let btn_map2 = map(&["A", "\n", "B", "\n", "C", "\n", "D", ""]);
    btnm = Buttonmatrix::create(ui, f.active_screen);
    assert!(ui.obj_is_valid(btnm.obj()));
    ui.obj_set_width(btnm.obj(), 150);
    ui.obj_set_height(btnm.obj(), 250);
    btnm.set_map(ui, btn_map2);
    btnm.set_button_ctrl_all(ui, ButtonmatrixCtrl::CHECKABLE);
    btnm.set_one_checked(ui, true);
    btnm.set_button_ctrl(ui, 1, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 2, ButtonmatrixCtrl::CHECKED);
    ui.obj_set_pos(btnm.obj(), 10, 160);

    let btn_map3 = map(&["A", "B", "C", "\n", "D", "E", "F", ""]);
    btnm = Buttonmatrix::create(ui, f.active_screen);
    assert!(ui.obj_is_valid(btnm.obj()));
    btnm.set_map(ui, btn_map3);
    ui.obj_set_width(btnm.obj(), 400);
    btnm.set_button_width(ui, 0, 3);
    btnm.set_button_width(ui, 1, 2);
    btnm.set_button_width(ui, 2, 1);
    btnm.set_button_width(ui, 3, 1);
    btnm.set_button_width(ui, 4, 2);
    btnm.set_button_width(ui, 5, 3);
    btnm.set_button_ctrl_all(ui, ButtonmatrixCtrl::CHECKABLE);
    btnm.set_button_ctrl(ui, 1, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 4, ButtonmatrixCtrl::CHECKED);
    ui.obj_set_pos(btnm.obj(), 300, 10);

    let btn_map4 = map(&[
        "A", "B", "C", "D", "E", "F", "G", "\n", "G", "F", "E", "D", "C", "B", "A", "",
    ]);
    btnm = Buttonmatrix::create(ui, f.active_screen);
    btnm.set_map(ui, btn_map4);
    ui.obj_set_width(btnm.obj(), 600);
    ui.obj_set_height(btnm.obj(), 150);
    btnm.set_button_width(ui, 0, 1);
    btnm.set_button_width(ui, 1, 2);
    btnm.set_button_width(ui, 2, 3);
    btnm.set_button_width(ui, 3, 4);
    btnm.set_button_width(ui, 4, 5);
    btnm.set_button_width(ui, 5, 6);
    btnm.set_button_width(ui, 6, 7);

    btnm.set_button_width(ui, 7, 7);
    btnm.set_button_width(ui, 8, 6);
    btnm.set_button_width(ui, 9, 5);
    btnm.set_button_width(ui, 10, 4);
    btnm.set_button_width(ui, 11, 3);
    btnm.set_button_width(ui, 12, 2);
    btnm.set_button_width(ui, 13, 1);

    btnm.set_button_ctrl_all(ui, ButtonmatrixCtrl::CHECKABLE);
    btnm.set_button_ctrl(ui, 1, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 3, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 5, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 7, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 9, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 11, ButtonmatrixCtrl::CHECKED);
    ui.obj_set_pos(btnm.obj(), 180, 160);

    f.display.screenshot_compare("widgets/btnm_2.png").unwrap();
}

#[test]
fn button_matrix_set_ctrl_map_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    let ctrl_map = [
        ButtonmatrixCtrl(1) | ButtonmatrixCtrl::DISABLED,
        ButtonmatrixCtrl(1) | ButtonmatrixCtrl::CHECKABLE | ButtonmatrixCtrl::CHECKED,
        ButtonmatrixCtrl(1) | ButtonmatrixCtrl::HIDDEN,
        ButtonmatrixCtrl(1) | ButtonmatrixCtrl::CHECKABLE,
    ];
    btnm.set_ctrl_map(ui, &ctrl_map);

    assert_basic_ctrl_flags(ui, btnm);
}

/// The shared assertions of the `set_ctrl_map` and `set_button_ctrl` tests.
fn assert_basic_ctrl_flags(ui: &ferrovgl_core::Ui, btnm: Buttonmatrix) {
    /* Verify if the ctrl map was set correctly. */
    assert!(btnm.has_button_ctrl(ui, 0, ButtonmatrixCtrl::DISABLED));
    assert!(btnm.has_button_ctrl(ui, 1, ButtonmatrixCtrl::CHECKABLE | ButtonmatrixCtrl::CHECKED));
    assert!(btnm.has_button_ctrl(ui, 2, ButtonmatrixCtrl::HIDDEN));
    assert!(btnm.has_button_ctrl(ui, 3, ButtonmatrixCtrl::CHECKABLE));

    /* Also checking randomly that no other flags are set. */
    assert!(!btnm.has_button_ctrl(ui, 0, ButtonmatrixCtrl::CHECKABLE));
    assert!(!btnm.has_button_ctrl(ui, 1, ButtonmatrixCtrl::DISABLED));
    assert!(!btnm.has_button_ctrl(ui, 3, ButtonmatrixCtrl::CHECKED));
    assert!(!btnm.has_button_ctrl(ui, 4, ButtonmatrixCtrl::HIDDEN));
}

/// Set the control flags of the four buttons with the individual APIs.
fn set_ctrls_individually(ui: &mut ferrovgl_core::Ui, btnm: Buttonmatrix) {
    btnm.set_button_ctrl(ui, 0, ButtonmatrixCtrl(1) | ButtonmatrixCtrl::DISABLED);
    btnm.set_button_ctrl(
        ui,
        1,
        ButtonmatrixCtrl(1) | ButtonmatrixCtrl::CHECKABLE | ButtonmatrixCtrl::CHECKED,
    );
    btnm.set_button_ctrl(ui, 2, ButtonmatrixCtrl(1) | ButtonmatrixCtrl::HIDDEN);
    btnm.set_button_ctrl(ui, 3, ButtonmatrixCtrl(1) | ButtonmatrixCtrl::CHECKABLE);
}

#[test]
fn button_matrix_set_button_ctrl_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    /* Set btn control map using individual APIs. */
    set_ctrls_individually(ui, btnm);

    assert_basic_ctrl_flags(ui, btnm);
}

#[test]
fn button_matrix_clear_button_ctrl_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    /* Set btn control map using individual APIs. */
    set_ctrls_individually(ui, btnm);

    btnm.clear_button_ctrl(ui, 0, ButtonmatrixCtrl::DISABLED);
    assert!(!btnm.has_button_ctrl(ui, 0, ButtonmatrixCtrl::DISABLED));
    btnm.clear_button_ctrl(ui, 1, ButtonmatrixCtrl::CHECKED);
    assert!(!btnm.has_button_ctrl(ui, 1, ButtonmatrixCtrl::CHECKED));
    btnm.clear_button_ctrl(ui, 2, ButtonmatrixCtrl::HIDDEN);
    assert!(!btnm.has_button_ctrl(ui, 2, ButtonmatrixCtrl::HIDDEN));
    btnm.clear_button_ctrl(ui, 3, ButtonmatrixCtrl::CHECKABLE);
    assert!(!btnm.has_button_ctrl(ui, 3, ButtonmatrixCtrl::CHECKABLE));
}

#[test]
fn button_matrix_set_selected_button_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    btnm.set_selected_button(ui, 2);
    assert_eq!(2, btnm.selected_button(ui) as u16);

    btnm.set_selected_button(ui, 0);
    assert_eq!(0, btnm.selected_button(ui) as u16);

    btnm.set_selected_button(ui, 3);
    assert_eq!(3, btnm.selected_button(ui) as u16);

    btnm.set_selected_button(ui, 1);
    assert_eq!(1, btnm.selected_button(ui) as u16);
}

#[test]
fn button_matrix_set_button_ctrl_all_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    btnm.set_button_ctrl_all(ui, ButtonmatrixCtrl::HIDDEN);

    for i in 0..4 {
        assert!(btnm.has_button_ctrl(ui, i, ButtonmatrixCtrl::HIDDEN));
    }
}

#[test]
fn button_matrix_clear_button_ctrl_all_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    btnm.set_button_ctrl_all(ui, ButtonmatrixCtrl::HIDDEN);
    btnm.clear_button_ctrl_all(ui, ButtonmatrixCtrl::HIDDEN);

    for i in 0..4 {
        assert!(!btnm.has_button_ctrl(ui, i, ButtonmatrixCtrl::HIDDEN));
    }
}

#[test]
fn button_matrix_set_button_width_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    btnm.set_button_width(ui, 1, 3);
    btnm.set_button_width(ui, 2, 2);

    assert!(btnm.has_button_ctrl(ui, 1, ButtonmatrixCtrl(3)));
    assert!(btnm.has_button_ctrl(ui, 2, ButtonmatrixCtrl(2)));
}

#[test]
fn button_matrix_set_one_checked_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    btnm.set_button_ctrl_all(ui, ButtonmatrixCtrl::CHECKABLE);
    btnm.set_one_checked(ui, true);

    btnm.set_button_ctrl(ui, 0, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 1, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 2, ButtonmatrixCtrl::CHECKED);
    btnm.set_button_ctrl(ui, 3, ButtonmatrixCtrl::CHECKED);

    assert!(btnm.one_checked(ui));

    assert!(!btnm.has_button_ctrl(ui, 0, ButtonmatrixCtrl::CHECKED));
    assert!(!btnm.has_button_ctrl(ui, 1, ButtonmatrixCtrl::CHECKED));
    assert!(!btnm.has_button_ctrl(ui, 2, ButtonmatrixCtrl::CHECKED));
    assert!(btnm.has_button_ctrl(ui, 3, ButtonmatrixCtrl::CHECKED));
}

#[test]
fn button_matrix_get_button_text_works() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    btnm.set_map(ui, abcd_map());

    assert_eq!(Some("A"), btnm.button_text(ui, 0));
    assert_eq!(Some("B"), btnm.button_text(ui, 1));
    assert_eq!(Some("C"), btnm.button_text(ui, 2));
    assert_eq!(Some("D"), btnm.button_text(ui, 3));
}

#[test]
fn button_matrix_pressed_event_works() {
    let mut f = set_up();
    f.btnm.set_map(&mut f.display.ui, abcd_map());
    add_event_handler(&mut f, EventCode::PRESSED);
    /* Set expected event code before the event is raised. */
    f.exp_evt_code.set(EventCode::PRESSED);
    /* Click button index 0. */
    f.inputs.mouse_click_at(&mut f.display, 10, 10);
    assert!(f.event_triggered.get());
}

#[test]
fn button_matrix_release_event_works() {
    let mut f = set_up();
    f.btnm.set_map(&mut f.display.ui, abcd_map());
    f.btnm
        .set_button_ctrl_all(&mut f.display.ui, ButtonmatrixCtrl::CHECKABLE);
    add_event_handler(&mut f, EventCode::RELEASED);
    /* Set expected event code before the event is raised. */
    f.exp_evt_code.set(EventCode::RELEASED);

    /* Click button index 0. */
    f.inputs.mouse_click_at(&mut f.display, 65, 35);
    assert!(f.event_triggered.get());
    // The click really reached button 0 and toggled it.
    assert!(
        f.btnm
            .has_button_ctrl(&f.display.ui, 0, ButtonmatrixCtrl::CHECKED)
    );
    f.event_triggered.set(false);
    /* This will increase test coverage by unchecking the
    same button. */
    f.btnm
        .set_button_ctrl(&mut f.display.ui, 0, ButtonmatrixCtrl::POPOVER);
    /* Click button index 0. */
    f.inputs.mouse_click_at(&mut f.display, 65, 35);
    assert!(f.event_triggered.get());
    assert!(
        !f.btnm
            .has_button_ctrl(&f.display.ui, 0, ButtonmatrixCtrl::CHECKED)
    );
}

#[test]
fn button_matrix_key_event_works() {
    let mut f = set_up();
    f.btnm.set_map(&mut f.display.ui, abcd_map());
    f.btnm
        .set_button_ctrl_all(&mut f.display.ui, ButtonmatrixCtrl::CHECKABLE);
    f.display.ui.obj_update_layout(f.btnm.obj()); /*The force calculating the button areas*/
    add_event_handler(&mut f, EventCode::KEY);
    /* Set expected event code before the event is raised. */
    f.exp_evt_code.set(EventCode::KEY);

    /* Select the first button. */
    f.btnm.set_selected_button(&mut f.display.ui, 0);
    send_key(&mut f, KEY_RIGHT);
    assert!(f.event_triggered.get());
    let btn_id = f.btnm.selected_button(&f.display.ui) as u16;
    assert_eq!(1, btn_id);

    f.event_triggered.set(false);
    send_key(&mut f, KEY_LEFT);
    assert!(f.event_triggered.get());
    let btn_id = f.btnm.selected_button(&f.display.ui) as u16;
    assert_eq!(0, btn_id);

    f.event_triggered.set(false);
    send_key(&mut f, KEY_DOWN);
    assert!(f.event_triggered.get());
    let btn_id = f.btnm.selected_button(&f.display.ui) as u16;
    assert_eq!(2, btn_id);

    f.event_triggered.set(false);
    send_key(&mut f, KEY_UP);
    assert!(f.event_triggered.get());
    let btn_id = f.btnm.selected_button(&f.display.ui) as u16;
    assert_eq!(0, btn_id);

    /* Added this code to increase code coverage. */
    f.btnm.set_btn_id_sel(&mut f.display.ui, BUTTONMATRIX_BUTTON_NONE);
    f.btnm
        .set_button_ctrl(&mut f.display.ui, 0, ButtonmatrixCtrl::HIDDEN);
    send_key(&mut f, KEY_DOWN);
    assert!(f.event_triggered.get());
    f.event_triggered.set(false);
    // The first visible button is selected.
    assert_eq!(1, f.btnm.selected_button(&f.display.ui));
}

#[test]
fn button_matrix_pressing_event_works() {
    // The shared upstream mouse indev still has its point at (65, 35) from
    // the last `lv_test_mouse_click_at(65, 35)` of
    // `test_button_matrix_release_event_works` (`test_button_matrix_key_event_works`
    // in between does not read it). That click is replayed before `setUp()`
    // creates the button matrix, so it only reaches the (cleaned) screen,
    // like upstream where its target was deleted by `tearDown()`.
    let mut f = set_up_with(|display, inputs| inputs.mouse_click_at(display, 65, 35));
    let btnm_obj = f.btnm;
    f.btnm.set_map(&mut f.display.ui, abcd_map());
    f.btnm
        .set_button_ctrl_all(&mut f.display.ui, ButtonmatrixCtrl::CHECKABLE);
    add_event_handler(&mut f, EventCode::PRESSING);
    /* Set expected event code before the event is raised. */
    f.exp_evt_code.set(EventCode::PRESSING);
    /* Select a button before raising a simulated event.
     * This is done to increase code coverage. */
    btnm_obj.set_btn_id_sel(&mut f.display.ui, 3);
    /* Send a dummy lv_indev_t object as param to avoid crashing during build. */
    let indev = f.inputs.mouse.id();
    f.display
        .ui
        .obj_send_event(f.btnm.obj(), EventCode::PRESSING, EventParam::Indev(indev));
    assert!(f.event_triggered.get());
    // (65, 35) is not on button 3, so the selection was dropped.
    assert_eq!(
        f.display.ui.indev_get_point(indev),
        ferrovgl_core::Point::new(65, 35)
    );
    assert_eq!(BUTTONMATRIX_BUTTON_NONE, f.btnm.selected_button(&f.display.ui));
}

#[test]
fn button_matrix_long_press_repeat_event_works() {
    let mut f = set_up();
    let btnm_obj = f.btnm;

    f.btnm.set_map(&mut f.display.ui, abcd_map());
    f.btnm
        .set_button_ctrl_all(&mut f.display.ui, ButtonmatrixCtrl::CHECKABLE);
    add_event_handler(&mut f, EventCode::LONG_PRESSED_REPEAT);
    /* Set expected event code before the event is raised. */
    f.exp_evt_code.set(EventCode::LONG_PRESSED_REPEAT);
    /* Select a button before raising a simulated event.
     * This is done to increase code coverage. */
    btnm_obj.set_btn_id_sel(&mut f.display.ui, 0);
    f.display
        .ui
        .obj_send_event(f.btnm.obj(), EventCode::LONG_PRESSED_REPEAT, EventParam::None);
    assert!(f.event_triggered.get());
}

#[test]
fn button_matrix_press_lost_event_works() {
    let mut f = set_up();

    f.btnm.set_map(&mut f.display.ui, abcd_map());
    f.btnm
        .set_button_ctrl_all(&mut f.display.ui, ButtonmatrixCtrl::CHECKABLE);
    add_event_handler(&mut f, EventCode::PRESS_LOST);
    /* Set expected event code before the event is raised. */
    f.exp_evt_code.set(EventCode::PRESS_LOST);
    f.display
        .ui
        .obj_send_event(f.btnm.obj(), EventCode::PRESS_LOST, EventParam::None);
    assert!(f.event_triggered.get());
}

#[test]
fn button_matrix_defocused_event_works() {
    let mut f = set_up();
    let btnm_obj = f.btnm;

    f.btnm.set_map(&mut f.display.ui, abcd_map());
    f.btnm
        .set_button_ctrl_all(&mut f.display.ui, ButtonmatrixCtrl::CHECKABLE);
    add_event_handler(&mut f, EventCode::DEFOCUSED);
    /* Set expected event code before the event is raised. */
    f.exp_evt_code.set(EventCode::DEFOCUSED);
    /* Select a button before raising a simulated event.
     * This is done to increase code coverage. */
    btnm_obj.set_btn_id_sel(&mut f.display.ui, 0);
    f.display
        .ui
        .obj_send_event(f.btnm.obj(), EventCode::DEFOCUSED, EventParam::None);
    assert!(f.event_triggered.get());
}

#[test]
fn button_matrix_focused_event_works() {
    let mut f = set_up();

    f.btnm.set_map(&mut f.display.ui, abcd_map());
    f.btnm
        .set_button_ctrl_all(&mut f.display.ui, ButtonmatrixCtrl::CHECKABLE);
    add_event_handler(&mut f, EventCode::FOCUSED);
    /* Set expected event code before the event is raised. */
    f.exp_evt_code.set(EventCode::FOCUSED);
    f.display
        .ui
        .obj_send_event(f.btnm.obj(), EventCode::FOCUSED, EventParam::None);
    assert!(f.event_triggered.get());
}

#[test]
fn buttonmatrix_properties() {
    // `#if LV_USE_OBJ_PROPERTY`: disabled in the parity configuration, so
    // upstream compiles an empty test body.
    let _f = set_up();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn button_matrix_partial_clip_render() {
    let mut f = set_up();
    let ui = &mut f.display.ui;

    /*Create a small container that clips its content*/
    let cont = ui.obj_create(Some(f.active_screen));
    ui.obj_set_size(cont, 300, 120);
    ui.obj_center(cont);
    ui.obj_set_style_pad_all(cont, 0, StyleSelector::DEFAULT);

    /*Create a buttonmatrix taller than the container*/
    let clip_map = map(&[
        "A", "B", "C", "\n", "D", "E", "F", "\n", "G", "H", "I", "\n", "J", "K", "L", "",
    ]);
    let btnm_clip = Buttonmatrix::create(ui, cont);
    btnm_clip.set_map(ui, clip_map);
    ui.obj_set_size(btnm_clip.obj(), 280, 300);

    /*Scroll so the middle rows are visible, top and bottom rows clipped*/
    ui.obj_scroll_to_y(cont, 80, ANIM_OFF);

    f.display
        .screenshot_compare("widgets/btnm_partial_clip.png")
        .unwrap();
}

#[test]
fn button_matrix_draw_task_culling() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    /*Buttons farther outside the clip area than this margin must not create
     *draw tasks.  The margin must be larger than the widget's internal clip
     *margin (row/column gap with a dpi/10 floor) so buttons whose shadow or
     *outline could reach the clip area are exempt from the check.*/
    let margin: i32 = 40;

    /*Create a small container that clips its content*/
    let cont = ui.obj_create(Some(f.active_screen));
    ui.obj_set_size(cont, 300, 150);
    ui.obj_center(cont);
    ui.obj_set_style_pad_all(cont, 0, StyleSelector::DEFAULT);

    /*Create a buttonmatrix much taller than the container so entire rows
     *fall far outside the clip area on both sides*/
    let clip_map = map(&[
        "A", "B", "C", "\n", "D", "E", "F", "\n", "G", "H", "I", "\n", "J", "K", "L", "",
    ]);
    let btnm_clip = Buttonmatrix::create(ui, cont);
    btnm_clip.set_map(ui, clip_map);
    ui.obj_set_size(btnm_clip.obj(), 280, 600);
    ui.obj_set_send_draw_task_events(btnm_clip.obj(), true);
    let drawn_buttons_mask = Rc::new(Cell::new(0u32));
    let mask = drawn_buttons_mask.clone();
    // `partial_clip_draw_task_cb()`
    ui.obj_add_event_cb(
        btnm_clip.obj(),
        Rc::new(move |ui, e| {
            let Some(task) = e.draw_task().and_then(|t| ui.draw.task(t)) else {
                return;
            };
            let Some(base_dsc) = task.dsc.base() else {
                return;
            };
            if base_dsc.part != Part::ITEMS {
                return;
            }
            if !matches!(task.dsc, DrawDsc::Fill(_)) {
                return;
            }
            mask.set(mask.get() | (1u32 << base_dsc.id1));
        }),
        EventCode::DRAW_TASK_ADDED,
        None,
    );

    /*Scroll so the middle rows are visible, top and bottom rows clipped*/
    ui.obj_update_layout(f.active_screen);
    ui.obj_scroll_to_y(cont, 225, ANIM_OFF);

    drawn_buttons_mask.set(0);
    ui.obj_invalidate(f.active_screen);
    ui.refr_now(None);

    let clip_area = ui.obj_get_coords(cont);

    let obj_coords = ui.obj_get_coords(btnm_clip.obj());

    let mut culled_above = false;
    let mut culled_below = false;
    let mut drawn_cnt = 0;
    for i in 0..btnm_clip.btn_cnt(ui) {
        let mut btn_area: Area = btnm_clip.button_areas(ui)[i as usize];
        btn_area.move_by(obj_coords.x1, obj_coords.y1);
        let drawn = (drawn_buttons_mask.get() >> i) & 1 != 0;
        drawn_cnt += u32::from(drawn);

        if btn_area.is_on(&clip_area) {
            assert!(drawn, "Button intersecting the clip area was not drawn");
        } else {
            let mut enlarged = btn_area;
            enlarged.increase(margin, margin);
            if !enlarged.is_on(&clip_area) {
                assert!(!drawn, "Button far outside the clip area created draw tasks");
                if btn_area.y2 < clip_area.y1 {
                    culled_above = true;
                }
                if btn_area.y1 > clip_area.y2 {
                    culled_below = true;
                }
            }
        }
    }

    /*Self-check: the geometry must exercise culling on both sides of the
     *clip area, otherwise the assertions above are vacuous*/
    assert!(
        culled_above,
        "No button far above the clip area, adjust the test geometry"
    );
    assert!(
        culled_below,
        "No button far below the clip area, adjust the test geometry"
    );
    assert!(drawn_cnt > 0);
}

/// Draw `btnm` with `on_fill_0` run (once) from `LV_EVENT_DRAW_TASK_ADDED`
/// for the `LV_PART_ITEMS` fill task of button 0, and return the label draw
/// tasks of the items as `(id1, text, area)`. The draw is an
/// `lv_snapshot_take()`: during a display refresh the widget setters would hit
/// the "Invalidate area is not allowed during rendering" assertion (in C too).
fn draw_items_with_fill_0_hook(
    f: &mut Fixture,
    on_fill_0: impl Fn(&mut ferrovgl_core::Ui, Buttonmatrix) + 'static,
) -> Vec<(u32, String, Area)> {
    let ui = &mut f.display.ui;
    let btnm = f.btnm;
    ui.obj_set_send_draw_task_events(btnm.obj(), true);
    let labels = Rc::new(std::cell::RefCell::new(Vec::new()));
    let labels_cb = labels.clone();
    let done = Rc::new(Cell::new(false));
    ui.obj_add_event_cb(
        btnm.obj(),
        Rc::new(move |ui, e| {
            let Some(task) = e.draw_task().and_then(|t| ui.draw.task(t)) else {
                return;
            };
            let Some(base_dsc) = task.dsc.base() else {
                return;
            };
            if base_dsc.part != Part::ITEMS {
                return;
            }
            let id1 = base_dsc.id1;
            match &task.dsc {
                DrawDsc::Fill(_) if id1 == 0 && !done.get() => {
                    done.set(true);
                    on_fill_0(ui, btnm);
                }
                DrawDsc::Label(dsc) => {
                    labels_cb
                        .borrow_mut()
                        .push((id1, String::from(&*dsc.text), task.area));
                }
                _ => {}
            }
        }),
        EventCode::DRAW_TASK_ADDED,
        None,
    );
    ui.obj_update_layout(f.active_screen);
    assert!(ui.snapshot_take(btnm.obj(), ColorFormat::Argb8888).is_some());
    labels.take()
}

#[test]
fn button_matrix_text_read_after_draw_rect() {
    // `draw_main()` reads `btnm->map_p[txt_i]` after `lv_draw_rect()`, so a
    // map set by a `LV_EVENT_DRAW_TASK_ADDED` callback during the button's
    // background is used for its label.
    let mut f = set_up();
    f.btnm.set_map(&mut f.display.ui, abcd_map());
    let new_map = map(&["X", "Y", ""]);
    let labels = draw_items_with_fill_0_hook(&mut f, move |ui, btnm| {
        btnm.set_map(ui, new_map.clone());
    });
    let first = labels.iter().find(|(id1, ..)| *id1 == 0).unwrap();
    assert_eq!(first.1, "X");
}

#[test]
fn button_matrix_popover_read_after_draw_rect() {
    // The second `LV_BUTTONMATRIX_CTRL_POPOVER` test of `draw_main()` (the
    // text push-up) reads `btnm->ctrl_bits[btn_i]` after `lv_draw_rect()`.
    let label_area_0 = |clear_in_callback: bool| {
        let mut f = set_up();
        f.btnm.set_map(&mut f.display.ui, abcd_map());
        f.btnm
            .set_button_ctrl(&mut f.display.ui, 0, ButtonmatrixCtrl::POPOVER);
        f.btnm.set_btn_id_sel(&mut f.display.ui, 0);
        f.display
            .ui
            .obj_add_state(f.btnm.obj(), ferrovgl_core::State::PRESSED);
        let labels = draw_items_with_fill_0_hook(&mut f, move |ui, btnm| {
            if clear_in_callback {
                btnm.clear_button_ctrl(ui, 0, ButtonmatrixCtrl::POPOVER);
            }
        });
        let btn_height = f.btnm.button_areas(&f.display.ui)[0].height();
        let first = labels.iter().find(|(id1, ..)| *id1 == 0).unwrap();
        (first.2, btn_height)
    };
    let (pushed, btn_height) = label_area_0(false);
    let (not_pushed, _) = label_area_0(true);
    assert_eq!(not_pushed.y1 - pushed.y1, btn_height / 2);
    assert_eq!(not_pushed.y2 - pushed.y2, btn_height / 2);
}
