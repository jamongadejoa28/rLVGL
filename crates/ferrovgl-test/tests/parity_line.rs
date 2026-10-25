//! Primitive-only reproduction of LVGL v9.6.0 `widgets/test_line.c` scenes.
//!
//! This verifies line task creation/rasterization, not the unported line widget.
//! Coordinates are derived from the C test and `lv_line.c:215-276`: content size
//! is the maximum point coordinate, and DRAW_MAIN adds object x/y to each point.
//! No coordinates or colors are fitted to the reference pixels.

use ferrovgl_core::draw::LineDsc;
use ferrovgl_core::{Color, EventCode, Point};
use ferrovgl_test::TestDisplay;
use std::rc::Rc;

fn scene() -> TestDisplay {
    let mut display = TestDisplay::with_default_theme().unwrap();
    display.ui.obj_clean(display.screen().unwrap());
    display
}

fn add_path(
    display: &mut TestDisplay,
    points: &[(i32, i32)],
    offset: Point,
    width: i32,
    dash_width: i32,
    dash_gap: i32,
) {
    // lv_theme_default.c:434-436 sets width=1 and color=theme->color_text.
    // The light theme text is palette GREY darken4 (#212121).
    let paths: Vec<_> = points
        .windows(2)
        .map(|pair| LineDsc {
            p1: Point::new(pair[0].0 + offset.x, pair[0].1 + offset.y).to_precise(),
            p2: Point::new(pair[1].0 + offset.x, pair[1].1 + offset.y).to_precise(),
            color: Color::make(0x21, 0x21, 0x21),
            width,
            dash_width,
            dash_gap,
            ..LineDsc::new()
        })
        .collect();
    let screen = display.screen().unwrap();
    display.ui.obj_add_event_cb(
        screen,
        Rc::new(move |ui, event| {
            let layer = event.layer().unwrap();
            for line in &paths {
                ui.draw_line(layer, line);
            }
        }),
        EventCode::DRAW_MAIN,
        None,
    );
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn line_primitives_basic_render() {
    // tests/src/test_cases/widgets/test_line.c:92-110 (test_line_basic_render).
    let mut display = scene();
    let points = [(5, 5), (100, 5), (100, 100), (120, 5), (200, 20)];
    add_path(&mut display, &points, Point::new(10, 10), 1, 0, 0);
    add_path(&mut display, &points, Point::new(400, 0), 5, 10, 3);
    display.screenshot_compare("widgets/line_1.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn line_primitives_dash_gap() {
    // test_line.c:113-145. Each path has content size250x250, derived from
    // lv_line.c:215-236. LEFT_MID/CENTER/RIGHT_MID give x0/(800-250)/2/800-250
    // and y(480-250)/2; all coordinates below follow that alignment arithmetic.
    let mut display = scene();
    let first = [(50, 50), (250, 50), (250, 250)];
    let other = [(50, 250), (50, 50), (250, 50)];
    let y = (480 - 250) / 2;
    add_path(&mut display, &first, Point::new(0, y), 1, 1, 1);
    add_path(&mut display, &other, Point::new((800 - 250) / 2, y), 2, 2, 2);
    add_path(&mut display, &other, Point::new(800 - 250, y), 5, 3, 1);
    display.screenshot_compare("widgets/line_2.png").unwrap();
}
