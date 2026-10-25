//! Ported from LVGL v9.6.0: `tests/src/test_cases/widgets/test_line.c`.
//!
//! Every upstream test function, in upstream order, with its values and
//! screenshot names. `setUp` creates `line` on the active screen of a fresh
//! default-theme display; `tearDown`'s `lv_obj_clean` is the display drop.
//! No test of this file changes the screen itself, so there is no inherited
//! screen state to reproduce.

use std::cell::RefCell;
use std::rc::Rc;

use ferrovgl_core::misc::area::PointPrecise;
use ferrovgl_core::misc::style::SharedRef;
use ferrovgl_core::misc::types::ValuePrecise;
use ferrovgl_core::{Align, EventCode, Obj, StyleSelector};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Line, LinePoints};

const MAIN: StyleSelector = StyleSelector::DEFAULT;

const DEFAULT_POINT_NUM: u16 = 0;
const INITIAL_EXTRA_DRAW_SIZE: i32 = 5;
const FINAL_EXTRA_DRAW_SIZE: i32 = 10;

/// `{x, y}` initializer of an `lv_point_precise_t`.
#[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
const fn pp(x: i32, y: i32) -> PointPrecise {
    PointPrecise::new(x as ValuePrecise, y as ValuePrecise)
}

/// The globals of `test_line.c` set by `setUp()`.
struct Fixture {
    display: TestDisplay,
    active_screen: Obj,
    line: Line,
}

/// `setUp()`
fn set_up() -> Fixture {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let active_screen = display.screen().unwrap();
    let line = Line::create(&mut display.ui, active_screen);
    Fixture {
        display,
        active_screen,
        line,
    }
}

#[test]
fn line_should_have_valid_documented_default_values() {
    let mut f = set_up();
    let line = f.line;
    let ui = &mut f.display.ui;
    assert_eq!(u32::from(DEFAULT_POINT_NUM), line.point_count(ui));
    assert!(line.points(ui).is_none());
    assert!(!line.y_invert(ui));
    assert!(!ui.obj_is_clickable(line.obj()));
    /* line doesn't have any points, so it's 0,0 in size */
    assert_eq!(0, ui.obj_get_self_width(line.obj()));
    assert_eq!(0, ui.obj_get_self_height(line.obj()));
}

#[test]
fn line_should_return_valid_y_invert() {
    let mut f = set_up();
    f.line.set_y_invert(&mut f.display.ui, true);
    assert!(f.line.y_invert(&f.display.ui));
}

#[test]
#[allow(clippy::unnecessary_cast)] // `ValuePrecise` is `i32` without the `float` feature
fn line_size_should_be_updated_after_adding_points() {
    static POINTS: [PointPrecise; 1] = [pp(5, 5)];
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let point_cnt = POINTS.len() as u16;
    f.line
        .set_points(ui, Some(SharedRef::Static(&POINTS)), u32::from(point_cnt));

    let mut calculated_width: i32 = 0;
    let mut calculated_height: i32 = 0;

    /* Get the biggest coordinate on both axis */
    for p in &POINTS[..usize::from(point_cnt)] {
        calculated_width = (p.x as i32).max(calculated_width);
        calculated_height = (p.y as i32).max(calculated_height);
    }

    assert_eq!(calculated_width, ui.obj_get_self_width(f.line.obj()));
    assert_eq!(calculated_height, ui.obj_get_self_height(f.line.obj()));
}

#[test]
fn line_should_update_extra_draw_size_based_on_style() {
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let line = f.line.obj();
    /* Setup an event handler for line extra draw size event */
    ui.obj_add_event_cb(
        line,
        Rc::new(|_ui, e| {
            /* `line_event_cb()` */
            if e.code() == EventCode::REFR_EXT_DRAW_SIZE {
                /* Set the new line extra draw size */
                e.set_ext_draw_size(INITIAL_EXTRA_DRAW_SIZE);
            }
        }),
        EventCode::ALL,
        None,
    );
    /* Trigger the extra draw size event */
    ui.obj_refresh_ext_draw_size(line);

    assert_eq!(INITIAL_EXTRA_DRAW_SIZE, ui.obj_get_ext_draw_size(line));

    /* Update line width style, the event handler should set the extra draw size
     * to the line width */
    ui.obj_set_style_line_width(line, FINAL_EXTRA_DRAW_SIZE, MAIN);

    /* Trigger the extra draw size event */
    ui.obj_refresh_ext_draw_size(line);

    assert_eq!(FINAL_EXTRA_DRAW_SIZE, ui.obj_get_ext_draw_size(line));
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn line_basic_render() {
    static POINTS: [PointPrecise; 5] = [
        pp(5, 5),
        pp(100, 5),   /*Horizontal*/
        pp(100, 100), /*Vertical*/
        pp(120, 5),   /*Steep*/
        pp(200, 20),  /*Flat*/
    ];
    let mut f = set_up();
    let ui = &mut f.display.ui;
    let point_cnt = POINTS.len() as u32;
    let mut line = f.line;
    line.set_points(ui, Some(SharedRef::Static(&POINTS)), point_cnt);
    ui.obj_set_pos(line.obj(), 10, 10);

    line = Line::create(ui, f.active_screen);
    line.set_points(ui, Some(SharedRef::Static(&POINTS)), point_cnt);
    ui.obj_set_pos(line.obj(), 400, 0);
    ui.obj_set_style_line_width(line.obj(), 5, MAIN);
    ui.obj_set_style_line_dash_gap(line.obj(), 3, MAIN);
    ui.obj_set_style_line_dash_width(line.obj(), 10, MAIN);

    f.display.screenshot_compare("widgets/line_1.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn line_dash_gap() {
    static LINE_POINTS1: [PointPrecise; 3] = [pp(50, 50), pp(250, 50), pp(250, 250)];
    static LINE_POINTS2: [PointPrecise; 3] = [pp(50, 250), pp(50, 50), pp(250, 50)];

    let mut f = set_up();
    let screen = f.display.screen().unwrap();
    let ui = &mut f.display.ui;

    let line1 = Line::create(ui, screen);
    line1.set_points(ui, Some(SharedRef::Static(&LINE_POINTS1)), 3);
    ui.obj_set_style_line_width(line1.obj(), 1, MAIN);
    ui.obj_set_style_line_dash_width(line1.obj(), 1, MAIN);
    ui.obj_set_style_line_dash_gap(line1.obj(), 1, MAIN);

    ui.obj_align(line1.obj(), Align::LeftMid, 0, 0);

    let line2 = Line::create(ui, screen);
    line2.set_points(ui, Some(SharedRef::Static(&LINE_POINTS2)), 3);
    ui.obj_set_style_line_width(line2.obj(), 2, MAIN);
    ui.obj_set_style_line_dash_width(line2.obj(), 2, MAIN);
    ui.obj_set_style_line_dash_gap(line2.obj(), 2, MAIN);

    ui.obj_center(line2.obj());

    let line3 = Line::create(ui, screen);
    line3.set_points(ui, Some(SharedRef::Static(&LINE_POINTS2)), 3);
    ui.obj_set_style_line_width(line3.obj(), 5, MAIN);
    ui.obj_set_style_line_dash_width(line3.obj(), 3, MAIN);
    ui.obj_set_style_line_dash_gap(line3.obj(), 1, MAIN);

    ui.obj_align(line3.obj(), Align::RightMid, 0, 0);

    f.display.screenshot_compare("widgets/line_2.png").unwrap();
}

#[test]
fn line_point_array_getters_and_setters() {
    let mut f = set_up();
    let line = f.line;
    let ui = &mut f.display.ui;

    let points: Rc<[PointPrecise]> = Rc::new([pp(10, 20), pp(30, 40), pp(50, 60)]);
    let points = SharedRef::Shared(points);
    line.set_points(ui, Some(points.clone()), 3);
    assert_eq!(3, line.point_count(ui));
    assert!(!line.is_point_array_mutable(ui));
    match line.points(ui) {
        Some(LinePoints::Const(p)) => assert_eq!(points.addr(), p.addr()),
        other => panic!("expected the constant array, got {other:?}"),
    }
    assert!(line.points_mutable(ui).is_none());

    let points_mutable = Rc::new(RefCell::new(vec![pp(10, 20), pp(30, 40)]));
    line.set_points_mutable(ui, Some(points_mutable.clone()), 2);
    assert_eq!(2, line.point_count(ui));
    assert!(line.is_point_array_mutable(ui));
    match line.points(ui) {
        Some(LinePoints::Mut(p)) => assert!(Rc::ptr_eq(&points_mutable, &p)),
        other => panic!("expected the mutable array, got {other:?}"),
    }
    let got = line.points_mutable(ui).expect("mutable array");
    assert!(Rc::ptr_eq(&points_mutable, &got));
}

/// `LV_USE_OBJ_PROPERTY` is disabled in the parity configuration, so the
/// upstream body is compiled out and the test is empty.
#[test]
fn line_properties() {}
