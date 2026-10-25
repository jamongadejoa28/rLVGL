//! Ports of LVGL v9.6.0 `tests/src/test_cases/widgets/test_checkbox.c`.
//! Run with `--include-ignored` after `cargo xtask refimgs sync`. References
//! are immutable upstream PNGs; actual pixels come from the Rust object pipeline.
//!
//! `test_checkbox_rtl` (`widgets/checkbox_rtl_1.png`) is not ported as a
//! runnable test: it needs `lv_font_dejavu_16_persian_hebrew` and the BiDi
//! paragraph algorithm (`LV_USE_BIDI`), neither of which exists in the core
//! yet. The scene is kept below as documentation for when they land; until
//! then [`checkbox_rtl_geometry`] covers the RTL marker/text placement of
//! `lv_checkbox_draw()` without pixels.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::draw::DrawDsc;
use ferrovgl_core::font;
use ferrovgl_core::misc::color::{ColorFormat, OPA_0};
use ferrovgl_core::misc::event::EventCallback;
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::{Area, EventCode, Obj, Part, State, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::Checkbox;

/// `setUp()` is empty; `tearDown()` cleans the active screen, which a fresh
/// display per test provides.
fn scene() -> TestDisplay {
    TestDisplay::with_default_theme().unwrap()
}

/// `test_checkbox_creation_successful`
#[test]
fn checkbox_creation_successful() {
    let mut display = scene();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);
    assert!(display.ui.obj_is_valid(checkbox.obj()));
    assert_eq!(Checkbox::from_obj(&display.ui, checkbox.obj()), Some(checkbox));
}

/// `test_checkbox_should_call_event_handler_on_click_when_enabled`
#[test]
fn checkbox_should_call_event_handler_on_click_when_enabled() {
    let mut display = scene();
    let inputs = display.create_inputs();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);

    display.ui.obj_add_state(checkbox.obj(), State::CHECKED);
    let event_called = Rc::new(Cell::new(false));
    let flag = event_called.clone();
    display.ui.obj_add_event_cb(
        checkbox.obj(),
        Rc::new(move |_, event| {
            if event.code == EventCode::VALUE_CHANGED {
                flag.set(true);
            }
        }),
        EventCode::ALL,
        None,
    );

    let coords = display.ui.obj_get_coords(checkbox.obj());
    inputs.mouse_click_at(&mut display, coords.x1, coords.y1);

    assert!(event_called.get());
    // The click toggled the checkable object off.
    assert!(!display.ui.obj_has_state(checkbox.obj(), State::CHECKED));
}

/// `test_checkbox_should_have_default_text_when_created`
#[test]
fn checkbox_should_have_default_text_when_created() {
    let default_text = "Check box";
    let mut display = scene();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);

    assert_eq!(checkbox.text(&display.ui), default_text);
}

/// `test_checkbox_should_return_dynamically_allocated_text`
#[test]
fn checkbox_should_return_dynamically_allocated_text() {
    let message = "Hello World!";
    let mut display = scene();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);

    checkbox.set_text(&mut display.ui, message);

    assert_eq!(checkbox.text(&display.ui), message);
    // The text was copied, not borrowed.
    assert_ne!(checkbox.text(&display.ui).as_ptr(), message.as_ptr());
}

/// `test_checkbox_should_allocate_memory_for_static_text`. Upstream compares
/// `lv_mem_monitor()` free sizes, which have no counterpart in Rust's
/// allocator; the observable equivalent is that the static text is used in
/// place instead of copied.
#[test]
fn checkbox_should_allocate_memory_for_static_text() {
    static STATIC_TEXT: &str = "Keep me while you exist";
    let mut display = scene();
    let active_screen = display.screen().unwrap();
    let checkbox = Checkbox::create(&mut display.ui, active_screen);

    checkbox.set_text_static(&mut display.ui, STATIC_TEXT);

    assert_eq!(checkbox.text(&display.ui).as_ptr(), STATIC_TEXT.as_ptr());
    assert_eq!(checkbox.text(&display.ui), STATIC_TEXT);
}

/// `test_checkbox_style_opa`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn checkbox_style_opa() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let obj = Checkbox::create(&mut display.ui, screen).obj();
    display
        .ui
        .obj_set_style_opa(obj, OPA_0, StyleSelector::from(Part::INDICATOR));
    display.ui.obj_center(obj);

    display.screenshot_compare("widgets/checkbox_1.png").unwrap();
}

/// `test_checkbox_properties`: its body is compiled only with
/// `LV_USE_OBJ_PROPERTY`, which the parity configuration (`full.defconfig`)
/// disables, so upstream runs it as an empty test. The setter/getter pair it
/// would check through the property table is exercised directly here.
#[test]
fn checkbox_properties() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let obj = Checkbox::create(&mut display.ui, screen);
    obj.set_text(&mut display.ui, "Test Checkbox");
    assert_eq!(obj.text(&display.ui), "Test Checkbox");
    display.ui.obj_delete(obj.obj());
}

/// Areas of the indicator border task (drawn on the full marker area, unlike
/// the fill, which `lv_draw_rect()` may shrink by 1 px) and the text task of one
/// `lv_checkbox_draw()`, recorded from `LV_EVENT_DRAW_TASK_ADDED`. When
/// `move_by` is non-zero, the handler of the indicator border task scrolls the
/// parent by that many pixels (`lv_obj_scroll_by(parent, 0, dy, LV_ANIM_OFF)`).
fn checkbox_draw_areas(ui: &mut Ui, cb: Obj, move_by: i32) -> (Area, Area) {
    let marker = Rc::new(Cell::new(None));
    let label = Rc::new(Cell::new(None));
    let move_by = Rc::new(Cell::new(move_by));
    let (m, l, dy) = (marker.clone(), label.clone(), move_by.clone());
    let parent = ui.obj_get_parent(cb).unwrap();
    ui.obj_set_send_draw_task_events(cb, true);
    let handler: EventCallback = Rc::new(move |ui, event| {
        let Some(task) = event.draw_task().and_then(|t| ui.draw.task(t)) else {
            return;
        };
        let area = task.area;
        match &task.dsc {
            DrawDsc::Border(d) if d.base.part == Part::INDICATOR => {
                m.set(Some(area));
                let dy = dy.replace(0);
                if dy != 0 {
                    ui.obj_scroll_by(parent, 0, dy, false);
                }
            }
            DrawDsc::Label(d) if d.base.part == Part::MAIN => l.set(Some(area)),
            _ => {}
        }
    });
    ui.obj_add_event_cb(cb, handler.clone(), EventCode::DRAW_TASK_ADDED, None);
    assert!(ui.snapshot_take(cb, ColorFormat::Argb8888).is_some());
    assert_eq!(ui.obj_remove_event_cb(cb, &handler), 1);
    ui.obj_set_send_draw_task_events(cb, false);
    (marker.get().unwrap(), label.get().unwrap())
}

/// `lv_checkbox_draw()` reads `obj->coords` for the marker before
/// `lv_draw_rect()` and again for `txt_area.y1` after it, so a draw task
/// handler that moves the checkbox shifts only the text.
#[test]
fn checkbox_text_y_uses_coords_after_marker_draw() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let cb = Checkbox::create(ui, screen).obj();
    ui.obj_center(cb);
    ui.obj_update_layout(cb);
    let y1 = ui.obj_get_coords(cb).y1;

    let (marker, label) = checkbox_draw_areas(ui, cb, 0);
    assert_eq!(ui.obj_get_coords(cb).y1, y1);
    let (moved_marker, moved_label) = checkbox_draw_areas(ui, cb, 10);
    assert_eq!(ui.obj_get_coords(cb).y1, y1 + 10);
    assert_eq!(moved_marker, marker);
    assert_eq!(moved_label.y1, label.y1 + 10);
    assert_eq!(moved_label.y2, label.y2 + 10);
    assert_eq!((moved_label.x1, moved_label.x2), (label.x1, label.x2));
}

/// Geometry of `lv_checkbox_draw()` in both base directions, including the
/// upstream quirk that the main border width is added to the left padding
/// only (LTR), not to the right padding (RTL).
#[test]
fn checkbox_rtl_geometry() {
    let mut display = scene();
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let cb = Checkbox::create(ui, screen).obj();
    let main = StyleSelector::from(Part::MAIN);
    ui.obj_set_style_border_width(cb, 3, main);
    ui.obj_set_style_pad_left(cb, 5, main);
    ui.obj_set_style_pad_right(cb, 7, main);
    ui.obj_set_style_pad_top(cb, 4, main);
    ui.obj_center(cb);
    ui.obj_update_layout(cb);

    let font = ui.obj_get_style_text_font(cb, Part::MAIN).unwrap();
    let font_h = font::get_line_height(font.as_font());
    let colp = ui.obj_get_style_pad_column(cb, Part::MAIN);
    let ind = Part::INDICATOR;
    let marker_w = font_h + ui.obj_get_style_pad_left(cb, ind) + ui.obj_get_style_pad_right(cb, ind);
    let marker_h = font_h + ui.obj_get_style_pad_top(cb, ind) + ui.obj_get_style_pad_bottom(cb, ind);
    let transf_w = ui.obj_get_style_transform_width(cb, ind);
    let transf_h = ui.obj_get_style_transform_height(cb, ind);
    let coords = ui.obj_get_coords(cb);
    let marker_y1 = coords.y1 + 4 + 3;
    let text_y1 = marker_y1 + (marker_h - font_h) / 2;

    let (ltr_marker, ltr_label) = checkbox_draw_areas(ui, cb, 0);
    let x1 = coords.x1 + 5 + 3;
    let mut expected = Area::new(x1, marker_y1, x1 + marker_w - 1, marker_y1 + marker_h - 1);
    expected.increase(transf_w, transf_h);
    assert_eq!(ltr_marker, expected);
    assert_eq!(ltr_label.x1, x1 + marker_w - 1 + colp);
    assert_eq!(ltr_label.y1, text_y1);

    ui.obj_set_style_base_dir(cb, BaseDir::Rtl, main);
    ui.obj_update_layout(cb);
    assert_eq!(ui.obj_get_coords(cb), coords);
    let (rtl_marker, rtl_label) = checkbox_draw_areas(ui, cb, 0);
    let x2 = coords.x2 - 7;
    let mut expected = Area::new(x2 - marker_w + 1, marker_y1, x2, marker_y1 + marker_h - 1);
    expected.increase(transf_w, transf_h);
    assert_eq!(rtl_marker, expected);
    assert_eq!(rtl_label.x2, x2 - marker_w + 1 - colp);
    assert_eq!(rtl_label.width(), ltr_label.width());
    assert_eq!((rtl_label.y1, rtl_label.y2), (ltr_label.y1, ltr_label.y2));
}

// `test_checkbox_rtl` (not runnable yet, see the module documentation):
//
//     let message = "מעבד, או בשמו המלא יחידת עיבוד מרכזית (באנגלית: CPU - Central Processing Unit).";
//     let screen = ui.obj_create(active_screen);
//     ui.obj_remove_style_all(screen);
//     ui.obj_set_size(screen, 800, 480);
//     ui.obj_center(screen);
//     ui.obj_set_style_bg_color(screen, Color::WHITE, 0);
//     ui.obj_set_style_bg_opa(screen, OPA_100, 0);
//     ui.obj_set_style_pad_all(screen, 0, 0);
//     let test_checkbox = Checkbox::create(ui, active_screen);
//     test_checkbox.set_text(ui, message);
//     ui.obj_set_style_text_font(test_checkbox, &lv_font_dejavu_16_persian_hebrew, 0);
//     ui.obj_center(test_checkbox);
//     ui.obj_set_style_base_dir(test_checkbox, BaseDir::Rtl, 0);
//     screenshot_compare("widgets/checkbox_rtl_1.png");
