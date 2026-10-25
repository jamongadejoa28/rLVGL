//! Scene ports from LVGL v9.6.0 `tests/src/test_cases/widgets/test_obj_pos.c`
//! and `tests/src/test_cases/widgets/test_obj_flags.c`.
//!
//! `test_margin_align.c::test_align` lives in `parity_style.rs`;
//! `test_chaining_invalidation_layout` needs the Dropdown widget, which is not
//! ported. `test_style_min_size` is ported here with all of its assertions
//! ([`style_min_size`]); `parity_button_theme.rs` keeps an older partial port
//! of the same scene. Both upstream files' `tearDown()` only cleans the active screen and
//! none of the remaining tests depends on input or tick state inherited from an
//! earlier test of the same file, so each test starts on a fresh
//! [`TestDisplay`]. The one exception, the pointer position left behind by
//! `test_obj_flag_overflow_visible_1`, is reproduced in
//! [`obj_flag_radio_button`].

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::group::{KEY_DOWN, KEY_LEFT, KEY_RIGHT, KEY_UP};
use ferrovgl_core::indev::IndevType;
use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Align, EventCode, Obj, State, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Button, Checkbox, Label};

const DEFAULT: StyleSelector = StyleSelector::DEFAULT;

fn counter_cb(ui: &mut Ui, obj: Obj, code: EventCode, counter: &Rc<Cell<u32>>) {
    let counter = counter.clone();
    ui.obj_add_event_cb(
        obj,
        Rc::new(move |_, _| counter.set(counter.get() + 1)),
        code,
        None,
    );
}

/// `test_obj_pos.c:21..52` (`test_content_parent_pct_child_pos_1`, issue 6837).
#[test]
fn content_parent_pct_child_pos_1() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let ui = &mut display.ui;
    let screen = ui.screen_active();
    let parent = ui.obj_create(screen);
    ui.obj_set_pos(parent, 20, 20);
    ui.obj_set_size(parent, coord::SIZE_CONTENT, coord::SIZE_CONTENT);
    ui.obj_set_style_pad_all(parent, 10, DEFAULT);

    let child1 = ui.obj_create(Some(parent));
    ui.obj_set_pos(child1, 100, 50);
    ui.obj_set_size(child1, 100, 100);

    let child2 = ui.obj_create(Some(parent));

    /*Simple case*/
    ui.obj_set_size(child2, 50, 50);
    ui.obj_set_pos(child2, 0, 0);
    ui.obj_update_layout(child2);
    assert_eq!(ui.obj_get_x(child2), 0);
    assert_eq!(ui.obj_get_y(child2), 0);

    /*Simple case*/
    ui.obj_set_pos(child2, 30, 200);
    ui.obj_update_layout(child2);
    assert_eq!(ui.obj_get_x(child2), 30);
    assert_eq!(ui.obj_get_y(child2), 200);

    /*x and y should be 0 to avoid circural dependency*/
    ui.obj_set_pos(child2, coord::pct(10), coord::pct(50));
    ui.obj_update_layout(child2);
    assert_eq!(ui.obj_get_x(child2), 0);
    assert_eq!(ui.obj_get_y(child2), 0);
}

/// `test_obj_pos.c:53..84` (`test_style_min_size`): a `100%` button inside a
/// content-sized parent evaluates to 0 and is then held at its fixed or
/// content minimum size.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn style_min_size() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen();
    let parent = display.ui.obj_create(screen);
    display
        .ui
        .obj_set_size(parent, coord::SIZE_CONTENT, coord::SIZE_CONTENT);
    // lv_obj_set_style_pad_all(parent, 10, 0);

    let child = Button::create(&mut display.ui, parent).obj();
    let label = Label::create(&mut display.ui, child);
    label.set_text(&mut display.ui, "Button");
    display.ui.obj_set_size(child, coord::pct(100), coord::pct(100)); // will evaluate to 0
    display
        .ui
        .obj_set_size(label.obj(), coord::SIZE_CONTENT, coord::SIZE_CONTENT);
    display
        .screenshot_compare("widgets/obj_pos_no_min_size.png")
        .unwrap();
    assert!(display.ui.obj_is_width_min(child));
    assert!(display.ui.obj_is_height_min(child));
    assert_eq!(display.ui.obj_get_style_clamped_width(child), 0);
    assert_eq!(display.ui.obj_get_style_clamped_height(child), 0);

    display.ui.obj_set_style_min_width(child, 200, DEFAULT);
    display.ui.obj_set_style_min_height(child, 300, DEFAULT);
    display
        .screenshot_compare("widgets/obj_pos_fixed_min_size.png")
        .unwrap();
    assert_eq!(display.ui.obj_get_width(child), 200);
    assert_eq!(display.ui.obj_get_height(child), 300);
    assert!(display.ui.obj_is_width_min(child));
    assert!(display.ui.obj_is_height_min(child));
    assert_eq!(display.ui.obj_get_style_clamped_width(child), 200);
    assert_eq!(display.ui.obj_get_style_clamped_height(child), 300);

    display
        .ui
        .obj_set_style_min_width(child, coord::SIZE_CONTENT, DEFAULT);
    display
        .ui
        .obj_set_style_min_height(child, coord::SIZE_CONTENT, DEFAULT);
    display
        .screenshot_compare("widgets/obj_pos_content_min_size.png")
        .unwrap();
}

/// Shared part of `test_circular_height_dependency` and
/// `test_circular_width_dependency`: a red flex container and three items
/// (green, blue, green), returned in creation order.
fn circular_scene(ui: &mut Ui, flow: FlexFlow) -> (Obj, [Obj; 3]) {
    let screen = ui.screen_active();
    let cont = ui.obj_create(screen);
    ui.obj_set_name(cont, Some("cont".into()));
    if flow == FlexFlow::Row {
        ui.obj_set_size(cont, coord::pct(100), coord::SIZE_CONTENT);
    } else {
        ui.obj_set_size(cont, coord::SIZE_CONTENT, coord::pct(100));
    }
    ui.obj_set_style_bg_color(cont, palette::main(Palette::Red), DEFAULT);
    ui.obj_set_style_bg_opa(cont, 255, DEFAULT);
    ui.obj_set_flex_flow(cont, flow);

    let set_main = |ui: &mut Ui, obj: Obj, v: i32| {
        if flow == FlexFlow::Row {
            ui.obj_set_height(obj, v);
        } else {
            ui.obj_set_width(obj, v);
        }
    };

    let item1 = ui.obj_create(Some(cont));
    ui.obj_set_name(item1, Some("item1".into()));
    ui.obj_set_style_bg_color(item1, palette::main(Palette::Green), DEFAULT);
    ui.obj_set_style_bg_opa(item1, 255, DEFAULT);
    set_main(ui, item1, coord::pct(100));
    ui.obj_set_flex_grow(item1, 1);

    let item2 = ui.obj_create(Some(cont));
    ui.obj_set_name(item2, Some("item2".into()));
    ui.obj_set_style_bg_color(item2, palette::main(Palette::Blue), DEFAULT);
    ui.obj_set_style_bg_opa(item2, 255, DEFAULT);
    set_main(ui, item2, 100); // fixed size
    ui.obj_set_flex_grow(item2, 1);

    let item3 = ui.obj_create(Some(cont));
    ui.obj_set_name(item3, Some("item3".into()));
    ui.obj_set_style_bg_color(item3, palette::main(Palette::Green), DEFAULT);
    ui.obj_set_style_bg_opa(item3, 255, DEFAULT);
    set_main(ui, item3, coord::pct(100)); // same as item1 but checking if child order matters
    ui.obj_set_flex_grow(item3, 1);

    (cont, [item1, item2, item3])
}

/// `test_obj_pos.c:120..179` (`test_circular_height_dependency`).
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn circular_height_dependency() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let (_, [item1, item2, item3]) = circular_scene(&mut display.ui, FlexFlow::Row);

    display
        .screenshot_compare("widgets/obj_circular_height.png")
        .unwrap();
    let ui = &mut display.ui;
    assert_eq!(ui.obj_get_height(item2), 100);
    assert_eq!(ui.obj_get_height(item1), ui.obj_get_height(item2));
    assert_eq!(ui.obj_get_height(item3), ui.obj_get_height(item2));

    /*Decreasing item2 height should also decrease item1 and item3 height*/
    ui.obj_set_height(item2, 40);
    ui.refr_now(None);
    assert_eq!(ui.obj_get_height(item2), 40);
    assert_eq!(ui.obj_get_height(item1), ui.obj_get_height(item2));
    assert_eq!(ui.obj_get_height(item3), ui.obj_get_height(item2));

    ui.obj_set_height(item2, coord::pct(100));
    ui.refr_now(None);
    assert_eq!(ui.obj_get_height(item1), 0);
    assert_eq!(ui.obj_get_height(item2), 0);
    assert_eq!(ui.obj_get_height(item3), 0);

    ui.obj_set_style_min_height(item2, 50, DEFAULT);
    ui.refr_now(None);
    assert_eq!(ui.obj_get_height(item2), 50);
    assert_eq!(ui.obj_get_height(item1), ui.obj_get_height(item2));
    assert_eq!(ui.obj_get_height(item3), ui.obj_get_height(item2));
}

/// `test_obj_pos.c:181..240` (`test_circular_width_dependency`).
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn circular_width_dependency() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let (_, [item1, item2, item3]) = circular_scene(&mut display.ui, FlexFlow::Column);

    display
        .screenshot_compare("widgets/obj_circular_width.png")
        .unwrap();
    let ui = &mut display.ui;
    assert_eq!(ui.obj_get_width(item2), 100);
    assert_eq!(ui.obj_get_width(item1), ui.obj_get_width(item2));
    assert_eq!(ui.obj_get_width(item3), ui.obj_get_width(item2));

    /*Decreasing item2 width should also decrease item1 and item3 width*/
    ui.obj_set_width(item2, 40);
    ui.refr_now(None);
    assert_eq!(ui.obj_get_width(item2), 40);
    assert_eq!(ui.obj_get_width(item1), ui.obj_get_width(item2));
    assert_eq!(ui.obj_get_width(item3), ui.obj_get_width(item2));

    ui.obj_set_width(item2, coord::pct(100));
    ui.refr_now(None);
    assert_eq!(ui.obj_get_width(item1), 0);
    assert_eq!(ui.obj_get_width(item2), 0);
    assert_eq!(ui.obj_get_width(item3), 0);

    ui.obj_set_style_min_width(item2, 50, DEFAULT);
    ui.refr_now(None);
    assert_eq!(ui.obj_get_width(item2), 50);
    assert_eq!(ui.obj_get_width(item1), ui.obj_get_width(item2));
    assert_eq!(ui.obj_get_width(item3), ui.obj_get_width(item2));
}

/// `cont_create()` of `test_obj_pos.c:242..274`.
fn cont_create(ui: &mut Ui, parent: Obj, text: &str) -> Obj {
    let cont = ui.obj_create(Some(parent));
    ui.obj_remove_style(cont, None, DEFAULT);
    ui.obj_set_style_border_width(cont, 1, DEFAULT);
    ui.obj_set_size(cont, 150, 30);

    let label = Label::create(ui, cont);
    label.set_text(ui, text);

    for (align, color) in [
        (Align::TopRight, Palette::Blue),
        (Align::BottomLeft, Palette::Green),
        (Align::BottomRight, Palette::Red),
    ] {
        let rect = ui.obj_create(Some(cont));
        ui.obj_remove_style(rect, None, DEFAULT);
        ui.obj_set_size(rect, 10, 10);
        ui.obj_set_align(rect, align);
        ui.obj_set_style_bg_color(rect, palette::main(color), DEFAULT);
        ui.obj_set_style_bg_opa(rect, 255, DEFAULT);
    }

    cont
}

/// The full-screen container of the RTL/alignment tests.
fn base_cont(ui: &mut Ui, dir: BaseDir) -> Obj {
    let screen = ui.screen_active();
    let cont = ui.obj_create(screen);
    ui.obj_set_size(cont, coord::pct(100), coord::pct(100));
    ui.obj_set_style_base_dir(cont, dir, DEFAULT);
    cont
}

/// `test_obj_pos.c:282..296` (`test_rtl_pos_x_y`).
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn rtl_pos_x_y() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let ui = &mut display.ui;
    let cont = base_cont(ui, BaseDir::Rtl);
    for (text, x, y) in [
        ("(0,0)", 0, 0),
        ("(50,50)", 50, 50),
        ("(100,100)", 100, 100),
        ("(150,150)", 150, 150),
        ("(100,200)", 100, 200),
    ] {
        /*cont_create_x_y()*/
        let c = cont_create(ui, cont, text);
        ui.obj_set_pos(c, x, y);
    }
    display.screenshot_compare("widgets/rtl_obj_pos_x_y.png").unwrap();
}

/// `cont_create_align()`.
fn align_scene(display: &mut TestDisplay, dir: BaseDir, items: [(&str, Align); 3], name: &str) {
    let ui = &mut display.ui;
    let cont = base_cont(ui, dir);
    for (text, align) in items {
        let c = cont_create(ui, cont, text);
        ui.obj_set_align(c, align);
    }
    display.screenshot_compare(name).unwrap();
}

/// `cont_create_align_offset()` with `(150, 0)`.
fn align_offset_scene(display: &mut TestDisplay, dir: BaseDir, name: &str) {
    let ui = &mut display.ui;
    let cont = base_cont(ui, dir);
    for (text, align) in [
        ("TOP_MID (150,0)", Align::TopMid),
        ("CENTER (150,0)", Align::Center),
        ("BOTTOM_MID (150,0)", Align::BottomMid),
    ] {
        let c = cont_create(ui, cont, text);
        ui.obj_align(c, align, 150, 0);
    }
    display.screenshot_compare(name).unwrap();
}

const LEFT: [(&str, Align); 3] = [
    ("TOP_LEFT", Align::TopLeft),
    ("LEFT_MID", Align::LeftMid),
    ("BOTTOM_LEFT", Align::BottomLeft),
];
const RIGHT: [(&str, Align); 3] = [
    ("TOP_RIGHT", Align::TopRight),
    ("RIGHT_MID", Align::RightMid),
    ("BOTTOM_RIGHT", Align::BottomRight),
];

/// `test_obj_pos.c` `test_align_left`.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn align_left() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    align_scene(&mut display, BaseDir::Ltr, LEFT, "widgets/obj_align_left.png");
}

/// `test_obj_pos.c` `test_align_right`.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn align_right() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    align_scene(&mut display, BaseDir::Ltr, RIGHT, "widgets/obj_align_right.png");
}

/// `test_obj_pos.c` `test_align_center`.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn align_center() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    align_offset_scene(&mut display, BaseDir::Ltr, "widgets/obj_align_center.png");
}

/// `test_obj_pos.c` `test_rtl_align_left`.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn rtl_align_left() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    align_scene(&mut display, BaseDir::Rtl, LEFT, "widgets/rtl_obj_align_left.png");
}

/// `test_obj_pos.c` `test_rtl_align_right`.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn rtl_align_right() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    align_scene(
        &mut display,
        BaseDir::Rtl,
        RIGHT,
        "widgets/rtl_obj_align_right.png",
    );
}

/// `test_obj_pos.c` `test_rtl_align_center`.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn rtl_align_center() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    align_offset_scene(&mut display, BaseDir::Rtl, "widgets/rtl_obj_align_center.png");
}

/// `test_obj_flags.c:24..136` (`test_obj_flag_overflow_visible_1`), with or without
/// its screenshots, so later tests can inherit its input state.
fn overflow_visible_1(display: &mut TestDisplay, compare: bool) {
    let ui = &mut display.ui;
    let screen = ui.screen_active();
    let obj_main = ui.obj_create(screen);
    ui.obj_set_size(obj_main, 400, 300);
    ui.obj_set_style_bg_color(obj_main, palette::main(Palette::Red), DEFAULT);
    ui.obj_set_overflow_visible(obj_main, true);
    ui.obj_center(obj_main);
    ui.obj_set_ext_draw_size(obj_main, 100);

    let obj_child_1 = ui.obj_create(Some(obj_main));
    ui.obj_set_size(obj_child_1, 200, 200);
    ui.obj_set_style_bg_color(obj_child_1, palette::main(Palette::Purple), DEFAULT);
    ui.obj_align(obj_child_1, Align::LeftMid, -100, 0);

    let btn_1 = Button::create(ui, obj_child_1).obj();
    ui.obj_set_size(btn_1, 100, 100);
    ui.obj_align(btn_1, Align::LeftMid, -75, 0);
    ui.obj_set_scroll_on_focus(btn_1, false);
    let cnt_1 = Rc::new(Cell::new(0));
    counter_cb(ui, btn_1, EventCode::CLICKED, &cnt_1);

    let label_1 = Label::create(ui, btn_1);
    label_1.set_text(ui, "Button 1");
    ui.obj_center(label_1.obj());

    let obj_child_2 = ui.obj_create(Some(obj_main));
    ui.obj_set_size(obj_child_2, 200, 200);
    ui.obj_set_style_bg_color(obj_child_2, palette::main(Palette::Orange), DEFAULT);
    ui.obj_set_overflow_visible(obj_child_2, true);
    ui.obj_align(obj_child_2, Align::RightMid, 100, 0);
    ui.obj_set_ext_draw_size(obj_child_2, 100);

    let btn_2 = Button::create(ui, obj_child_2).obj();
    ui.obj_set_size(btn_2, 100, 100);
    ui.obj_align(btn_2, Align::RightMid, 75, 0);
    ui.obj_set_scroll_on_focus(btn_2, false);
    let cnt_2 = Rc::new(Cell::new(0));
    counter_cb(ui, btn_2, EventCode::CLICKED, &cnt_2);

    let label_2 = Label::create(ui, btn_2);
    label_2.set_text(ui, "Button 2");
    ui.obj_center(label_2.obj());

    cnt_1.set(0);
    cnt_2.set(0);

    /*The clipped part of the left button (shouldn't trigger click event)*/
    display.mouse_click_at(100, 220);
    /*The non clipped part of the left button (should trigger click event)*/
    display.mouse_click_at(140, 220);
    /*The left part of the right button (should trigger click event)*/
    display.mouse_click_at(650, 220);
    /*The outer part of the right button (should trigger click event as obj_child_2 has LV_OBJ_FLAG_OVERFLOW_VISIBLE)*/
    display.mouse_click_at(690, 220);

    assert_eq!(cnt_1.get(), 1);
    assert_eq!(cnt_2.get(), 2);

    if compare {
        display
            .screenshot_compare("widgets/obj_flag_overflow_visible_1_1.png")
            .unwrap();
    }

    /*Test if the overflowing parts are rendered correctly after scrolling too*/
    display.ui.obj_scroll_by_bounded(obj_main, -20, 0, false);
    if compare {
        display
            .screenshot_compare("widgets/obj_flag_overflow_visible_1_2.png")
            .unwrap();
    }

    display.ui.obj_scroll_by_bounded(obj_child_2, -30, 0, false);
    if compare {
        display
            .screenshot_compare("widgets/obj_flag_overflow_visible_1_3.png")
            .unwrap();
    }

    /*Test with rotation*/
    let ui = &mut display.ui;
    for (obj, pivot_x, pivot_y) in [
        (obj_main, 200, 150),
        (obj_child_1, 100, 100),
        (obj_child_2, 100, 100),
        (btn_1, 100, 100),
        (btn_2, 100, 100),
    ] {
        ui.obj_set_style_transform_rotation(obj, 300, DEFAULT);
        ui.obj_set_style_transform_pivot_x(obj, pivot_x, DEFAULT);
        ui.obj_set_style_transform_pivot_y(obj, pivot_y, DEFAULT);
    }

    cnt_1.set(0);
    cnt_2.set(0);

    /*The clipped part of the left button (shouldn't trigger click event)*/
    display.mouse_click_at(185, 40);
    /*The non clipped part of the left button (should trigger click event)*/
    display.mouse_click_at(210, 80);
    /*The left part of the right button (should trigger click event)*/
    display.mouse_click_at(590, 370);
    /*The outer part of the right button (should trigger click event as obj_child_2 has LV_OBJ_FLAG_OVERFLOW_VISIBLE)*/
    display.mouse_click_at(600, 430);
    /*The clipped part of the right button (clipped because it's out of the red panel's ext draw size, shouldn't trigger click event)*/
    display.mouse_click_at(645, 430);

    assert_eq!(cnt_1.get(), 1);
    assert_eq!(cnt_2.get(), 2);
    if compare {
        display
            .screenshot_compare("widgets/obj_flag_overflow_visible_1_4.png")
            .unwrap();
    }
}

/// `test_obj_flags.c:24..136` (`test_obj_flag_overflow_visible_1`).
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn obj_flag_overflow_visible_1() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    overflow_visible_1(&mut display, true);
}

/// `test_obj_flags.c:152..259` (`test_obj_flag_radio_button`). Upstream runs it
/// after `test_obj_flag_overflow_visible_1` on the same screen and devices, so
/// that test's input sequence runs first (without its screenshots), followed by
/// `tearDown()`.
#[test]
fn obj_flag_radio_button() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    overflow_visible_1(&mut display, false);
    display.clean_screen();

    let g = display.ui.group_create();
    let keypad = display.indev_get_indev(IndevType::Keypad).unwrap();
    display.ui.indev_set_group(keypad, Some(g));

    let ui = &mut display.ui;
    let scr = ui.screen_active().unwrap();
    let mut cb = [scr; 5];
    let called: [Rc<Cell<u32>>; 5] = core::array::from_fn(|_| Rc::new(Cell::new(0)));
    let checked: [Rc<Cell<u32>>; 5] = core::array::from_fn(|_| Rc::new(Cell::new(0)));
    let unchecked: [Rc<Cell<u32>>; 5] = core::array::from_fn(|_| Rc::new(Cell::new(0)));
    for i in 0..5 {
        cb[i] = Checkbox::create(ui, scr).obj();
        ui.obj_set_y(cb[i], i as i32 * 50);
        ui.obj_set_radio_button(cb[i], true);
        ui.group_add_obj(g, cb[i]);
        counter_cb(ui, cb[i], EventCode::VALUE_CHANGED, &called[i]);
        counter_cb(ui, cb[i], EventCode::CHECKED, &checked[i]);
        counter_cb(ui, cb[i], EventCode::UNCHECKED, &unchecked[i]);
    }
    let counts = |c: &[Rc<Cell<u32>>; 5]| c.each_ref().map(|c| c.get());

    /*Click the first checkbox*/
    display.mouse_click_at(20, 5);
    assert!(display.ui.obj_has_state(cb[0], State::CHECKED));
    assert_eq!(counts(&called), [1, 0, 0, 0, 0]);
    assert_eq!(counts(&checked), [1, 0, 0, 0, 0]);
    assert_eq!(counts(&unchecked), [0, 0, 0, 0, 0]);

    /*Click the second checkbox*/
    display.mouse_click_at(20, 55);
    assert!(!display.ui.obj_has_state(cb[0], State::CHECKED));
    assert!(display.ui.obj_has_state(cb[1], State::CHECKED));
    assert_eq!(counts(&called), [2, 1, 0, 0, 0]);
    assert_eq!(counts(&checked), [1, 1, 0, 0, 0]);
    assert_eq!(counts(&unchecked), [1, 0, 0, 0, 0]);

    /*Clicking the same checkbox shouldn't change anything*/
    display.mouse_click_at(20, 55);
    assert!(!display.ui.obj_has_state(cb[0], State::CHECKED));
    assert!(display.ui.obj_has_state(cb[1], State::CHECKED));
    assert_eq!(counts(&called), [2, 1, 0, 0, 0]);
    assert_eq!(counts(&checked), [1, 1, 0, 0, 0]);
    assert_eq!(counts(&unchecked), [1, 0, 0, 0, 0]);

    /*Turn on checkbox 2 with a key*/
    display.ui.group_focus_obj(cb[2]);
    display.key_hit(KEY_UP);
    assert!(!display.ui.obj_has_state(cb[1], State::CHECKED));
    assert!(display.ui.obj_has_state(cb[2], State::CHECKED));
    assert_eq!(counts(&called), [2, 2, 1, 0, 0]);
    assert_eq!(counts(&checked), [1, 1, 1, 0, 0]);
    assert_eq!(counts(&unchecked), [1, 1, 0, 0, 0]);

    /*Nothing happen checking checkbox 2 again*/
    /*Can't check off*/
    /*Same with left*/
    for key in [KEY_RIGHT, KEY_DOWN, KEY_LEFT] {
        display.key_hit(key);
        assert!(!display.ui.obj_has_state(cb[1], State::CHECKED));
        assert!(display.ui.obj_has_state(cb[2], State::CHECKED));
        assert_eq!(counts(&called), [2, 2, 1, 0, 0]);
        assert_eq!(counts(&checked), [1, 1, 1, 0, 0]);
        assert_eq!(counts(&unchecked), [1, 1, 0, 0, 0]);
    }
}

/// `test_obj_flags.c:263..271` (`test_obj_flag_parent_hidden`, issue 10083):
/// hiding and showing the active screen must not crash.
#[test]
fn obj_flag_parent_hidden() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_set_hidden(screen, true);
    assert!(display.ui.obj_is_hidden(screen));
    display.ui.obj_set_hidden(screen, false);
    assert!(!display.ui.obj_is_hidden(screen));
}
