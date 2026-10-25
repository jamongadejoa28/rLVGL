//! Source-based style tests from the pinned LVGL v9.6.0 tests.
//!
//! `test_style.c` and the two `test_obj_style_*` suites have no reference PNGs.
//! The image scenarios below are `test_margin_align.c::test_align`, which uses
//! real base objects, local margin/radius styles and the default card theme.
//! The full `theme_default_*.png` scenes require widgets not yet implemented.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ferrovgl_core::core::obj_pos::ObjPointTransformFlag;
use ferrovgl_core::misc::anim::AnimPath;
use ferrovgl_core::misc::event::EventParam;
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::{SharedRef, StyleTransitionDsc};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{
    Align, Area, Color, EventCode, FontRef, Obj, Part, State, Style, StyleProp, StyleSelector,
};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::Label;

const DEFAULT: StyleSelector = StyleSelector::DEFAULT;

fn margin(display: &mut TestDisplay, obj: Obj, left: i32, top: i32, right: i32, bottom: i32) {
    display.ui.obj_set_style_margin_left(obj, left, DEFAULT);
    display.ui.obj_set_style_margin_top(obj, top, DEFAULT);
    display.ui.obj_set_style_margin_right(obj, right, DEFAULT);
    display.ui.obj_set_style_margin_bottom(obj, bottom, DEFAULT);
}

/// Exact operation order of `test_margin_align.c:41..77`, including the layout
/// before changing margins and the first screenshot before the final radius.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn style_margin_alignment_and_radius_change() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_set_width(screen, 800);
    display.ui.obj_set_height(screen, 400);
    display.ui.obj_center(screen);
    display.ui.obj_set_style_pad_all(screen, 0, DEFAULT);
    display.ui.obj_set_style_border_width(screen, 0, DEFAULT);
    let objects = [
        Palette::Blue,
        Palette::Red,
        Palette::DeepPurple,
        Palette::Green,
        Palette::Pink,
    ]
    .map(|color| {
        let obj = display.ui.obj_create(Some(screen));
        display
            .ui
            .obj_set_style_bg_color(obj, palette::main(color), DEFAULT);
        display.ui.obj_set_size(obj, 150, 150);
        obj
    });
    display.ui.obj_align(objects[0], Align::TopMid, 0, 0);
    display
        .ui
        .obj_align_to(objects[1], objects[0], Align::OutBottomMid, 0, 0);
    display
        .ui
        .obj_align_to(objects[2], objects[1], Align::OutRightMid, 0, 0);
    display
        .ui
        .obj_align_to(objects[3], objects[2], Align::OutBottomMid, 0, 0);
    display.ui.obj_update_layout(screen);
    margin(&mut display, objects[0], 20, 50, 60, 50);
    display
        .ui
        .obj_set_style_width(objects[0], coord::pct(60), DEFAULT);
    margin(&mut display, objects[1], 15, 12, 50, 20);
    margin(&mut display, objects[2], 25, 25, 100, 20);
    margin(&mut display, objects[3], 12, 50, 100, 100);
    display
        .ui
        .obj_set_style_width(objects[3], coord::pct(100), DEFAULT);
    display.screenshot_compare("margin_align_0.png").unwrap();
    margin(&mut display, objects[4], 6, 100, 100, 100);
    display.ui.obj_set_style_radius(objects[0], 0, DEFAULT);
    display.ui.obj_align(objects[4], Align::LeftMid, 0, 0);
    display.ui.obj_update_layout(screen);
    display.screenshot_compare("margin_align_1.png").unwrap();
}

/// `test_obj_style_state_refresh.c:29..45`: callbacks on every descendant,
/// and an actual render before toggling state, to avoid an unrendered shortcut.
fn rendered_subtree() -> (TestDisplay, Obj, Rc<Cell<u32>>) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let parent = display.ui.obj_create(display.screen());
    display.ui.obj_set_size(parent, 200, 200);
    let count = Rc::new(Cell::new(0));
    let register = |display: &mut TestDisplay, obj| {
        let count = count.clone();
        display.ui.obj_add_event_cb(
            obj,
            Rc::new(move |_, _| count.set(count.get() + 1)),
            EventCode::STYLE_CHANGED,
            None,
        );
    };
    for _ in 0..3 {
        let child = display.ui.obj_create(Some(parent));
        register(&mut display, child);
        for _ in 0..2 {
            let grandchild = display.ui.obj_create(Some(child));
            register(&mut display, grandchild);
        }
    }
    display.snapshot().unwrap();
    (display, parent, count)
}

#[test]
fn style_scrollbar_redraw_does_not_cascade_to_children() {
    let (mut display, parent, count) = rendered_subtree();
    let mut style = Style::new();
    style.set_bg_opa(255);
    let style = display.ui.style_create(style);
    display
        .ui
        .obj_add_style(parent, style, Part::SCROLLBAR | State::SCROLLED);
    count.set(0);
    display.ui.obj_add_state(parent, State::SCROLLED);
    display.ui.obj_remove_state(parent, State::SCROLLED);
    assert_eq!(count.get(), 0);
}

#[test]
fn style_main_layout_state_cascades_to_children() {
    let (mut display, parent, count) = rendered_subtree();
    let mut style = Style::new();
    style.set_pad_all(20);
    let style = display.ui.style_create(style);
    display
        .ui
        .obj_add_style(parent, style, Part::MAIN | State::USER_1);
    count.set(0);
    display.ui.obj_add_state(parent, State::USER_1);
    assert!(count.get() > 0);
}

#[test]
fn style_scrollbar_layout_state_cascades_to_children() {
    let (mut display, parent, count) = rendered_subtree();
    let mut style = Style::new();
    style.set_width(30);
    let style = display.ui.style_create(style);
    display
        .ui
        .obj_add_style(parent, style, Part::SCROLLBAR | State::USER_1);
    count.set(0);
    display.ui.obj_add_state(parent, State::USER_1);
    assert!(count.get() > 0);
}

#[test]
fn style_multi_part_redraw_cascades_to_children() {
    let (mut display, parent, count) = rendered_subtree();
    let mut main = Style::new();
    main.set_bg_color(Color::hex(0x123456));
    let main = display.ui.style_create(main);
    display.ui.obj_add_style(parent, main, Part::MAIN | State::USER_1);
    let mut scrollbar = Style::new();
    scrollbar.set_bg_opa(255);
    let scrollbar = display.ui.style_create(scrollbar);
    display
        .ui
        .obj_add_style(parent, scrollbar, Part::SCROLLBAR | State::USER_1);
    count.set(0);
    display.ui.obj_add_state(parent, State::USER_1);
    assert!(count.get() > 0);
}

fn drawn_area(display: &TestDisplay, obj: Obj) -> Area {
    let mut area = display.ui.obj_get_coords(obj);
    display
        .ui
        .obj_get_transformed_area(obj, &mut area, ObjPointTransformFlag::Recursive);
    area
}

fn capture(display: &mut TestDisplay) -> Rc<RefCell<Vec<Area>>> {
    let captured = Rc::new(RefCell::new(Vec::new()));
    let copy = captured.clone();
    display.ui.display_add_event_cb(
        display.display(),
        Rc::new(move |_, event| {
            let area = match &event.param {
                EventParam::Area(area) => *area,
                EventParam::AreaMut(area) => **area,
                _ => panic!("INVALIDATE_AREA must carry an area"),
            };
            copy.borrow_mut().push(area);
        }),
        EventCode::INVALIDATE_AREA,
        None,
    );
    captured
}

fn assert_released_strip(display: &TestDisplay, obj: Obj, before: Area, captured: &[Area]) {
    let after = drawn_area(display, obj);
    assert!(
        after.y2 < before.y2 - 4,
        "drawn area must really shrink: {before:?} -> {after:?}"
    );
    let strip = Area::new(before.x1, after.y2 + 2, before.x2, before.y2);
    assert!(
        captured.iter().any(|area| strip.x1 >= area.x1
            && strip.x2 <= area.x2
            && strip.y1 >= area.y1
            && strip.y2 <= area.y2),
        "released strip {strip:?} missing from {captured:?}"
    );
}

/// The six public mutation paths from
/// `test_obj_style_transform_invalidation.c:133..213`.
#[test]
fn style_mutations_invalidate_old_transformed_area() {
    for operation in 0..6 {
        let mut display = TestDisplay::with_default_theme().unwrap();
        let mut big = Style::new();
        big.set_transform_scale_y(4 * 256);
        let big = display.ui.style_create(big);
        let mut small = Style::new();
        small.set_transform_scale_y(256);
        let small = display.ui.style_create(small);
        let obj = display.ui.obj_create(display.screen());
        display.ui.obj_remove_style_all(obj);
        display.ui.obj_set_pos(obj, 100, 100);
        display.ui.obj_set_size(obj, 60, 40);
        display.ui.obj_set_style_bg_opa(obj, 255, DEFAULT);
        display.ui.obj_add_style(obj, big, DEFAULT);
        display.snapshot().unwrap();
        let before = drawn_area(&display, obj);
        let captured = capture(&mut display);
        match operation {
            0 => display.ui.obj_set_style_transform_scale_y(obj, 256, DEFAULT),
            1 => display.ui.obj_add_style(obj, small, DEFAULT),
            2 => display.ui.obj_remove_style(obj, Some(big), DEFAULT),
            3 => display.ui.obj_remove_style_all(obj),
            4 => assert!(display.ui.obj_replace_style(obj, big, small, DEFAULT)),
            5 => display.ui.obj_set_style_enabled(obj, big, DEFAULT, false),
            _ => unreachable!(),
        }
        assert_released_strip(&display, obj, before, &captured.borrow());
    }
}

/// `test_obj_style_transform_invalidation.c:219..249`.
fn running_transition() -> (TestDisplay, Obj) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let obj = display.ui.obj_create(display.screen());
    display.ui.obj_remove_style_all(obj);
    display.ui.obj_set_pos(obj, 100, 100);
    display.ui.obj_set_size(obj, 60, 40);
    display.ui.obj_set_style_bg_opa(obj, 255, DEFAULT);
    let mut big = Style::new();
    big.set_transform_scale_y(4 * 256);
    let big = display.ui.style_create(big);
    display.ui.obj_add_style(obj, big, Part::MAIN | State::PRESSED);
    let transition = StyleTransitionDsc::new_static(
        &[StyleProp::TRANSFORM_SCALE_Y, StyleProp::INV],
        AnimPath::Linear,
        400,
        0,
    );
    display
        .ui
        .obj_set_style_transition(obj, SharedRef::Shared(Rc::new(transition)), DEFAULT);
    display.snapshot().unwrap();
    display.ui.obj_add_state(obj, State::PRESSED);
    display.wait(400);
    assert_eq!(
        display.ui.obj_get_style_transform_scale_y(obj, Part::MAIN),
        4 * 256
    );
    display.ui.obj_remove_state(obj, State::PRESSED);
    display.wait(200);
    let scale = display.ui.obj_get_style_transform_scale_y(obj, Part::MAIN);
    assert!(scale > 256 + 32 && scale < 4 * 256, "scale: {scale}");
    (display, obj)
}

#[test]
fn style_transition_step_invalidates_old_area() {
    let (mut display, obj) = running_transition();
    let before = drawn_area(&display, obj);
    let captured = capture(&mut display);
    display.ui.tick_inc(100);
    display.ui.timer_handler();
    assert_released_strip(&display, obj, before, &captured.borrow());
}

#[test]
fn style_killing_transition_invalidates_old_area() {
    let (mut display, obj) = running_transition();
    let before = drawn_area(&display, obj);
    let captured = capture(&mut display);
    let mut plain = Style::new();
    plain.set_bg_color(Color::hex(0x00ff00));
    let plain = display.ui.style_create(plain);
    display.ui.obj_add_style(obj, plain, DEFAULT);
    assert_released_strip(&display, obj, before, &captured.borrow());
}

/// Runtime regression for `lv_obj_report_style_change`, inheritance and cache
/// rebuilds. This is a Rust behavioral test, not an upstream reference image.
#[test]
fn style_shared_mutation_refreshes_inherited_font_and_selector_cache() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let parent = display.ui.obj_create(display.screen());
    display.ui.obj_set_size(parent, 300, 200);
    let shared = display.ui.style_create(Style::new());
    display
        .ui
        .obj_add_style(parent, shared, Part::MAIN | State::CHECKED);
    let label = Label::create(&mut display.ui, parent);
    label.set_text(&mut display.ui, "Inherited");
    let original = display.snapshot().unwrap();
    let original_size = (
        display.ui.obj_get_width(label.obj()),
        display.ui.obj_get_height(label.obj()),
    );

    let style = display.ui.style_mut(shared).unwrap();
    style.set_text_font(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_28));
    style.set_text_color(Color::hex(0xff0000));
    display.ui.obj_report_style_change(Some(shared));
    // The attached selector has not matched yet.
    assert_eq!(display.snapshot().unwrap(), original);
    display.ui.obj_add_state(parent, State::CHECKED);
    let large = display.snapshot().unwrap();
    assert_ne!(large, original);
    assert!(display.ui.obj_get_width(label.obj()) > original_size.0);
    assert!(display.ui.obj_get_height(label.obj()) > original_size.1);
    assert_eq!(
        display.ui.obj_get_style_text_color(label.obj(), Part::MAIN),
        Color::hex(0xff0000)
    );

    display
        .ui
        .obj_set_style_text_color(label.obj(), Color::hex(0x0000ff), DEFAULT);
    display
        .ui
        .obj_set_style_enabled(parent, shared, Part::MAIN | State::CHECKED, false);
    display.snapshot().unwrap();
    assert_eq!(display.ui.obj_get_height(label.obj()), original_size.1);
    // A local child value continues to override inherited values.
    assert_eq!(
        display.ui.obj_get_style_text_color(label.obj(), Part::MAIN),
        Color::hex(0x0000ff)
    );
    display
        .ui
        .obj_set_style_enabled(parent, shared, Part::MAIN | State::CHECKED, true);
    display
        .ui
        .style_mut(shared)
        .unwrap()
        .remove_prop(StyleProp::TEXT_FONT);
    display.ui.obj_report_style_change(Some(shared));
    display.snapshot().unwrap();
    assert_eq!(display.ui.obj_get_height(label.obj()), original_size.1);
    assert_eq!(
        display.ui.obj_get_style_text_color(label.obj(), Part::MAIN),
        Color::hex(0x0000ff)
    );
}

/// `test_style.c::test_style_remove_theme`'s local-style guarantee, exercised
/// on the implemented base-object theme instead of the unported switch widget.
#[test]
fn style_remove_theme_keeps_local_geometry_and_color() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let obj = display.ui.obj_create(display.screen());
    display.ui.obj_set_size(obj, 100, 50);
    display
        .ui
        .obj_set_style_bg_color(obj, Color::hex(0xff0000), DEFAULT);
    display.snapshot().unwrap();
    assert_eq!(display.ui.obj_get_style_bg_opa(obj, Part::MAIN), 255);
    display.ui.obj_remove_theme(obj, Part::MAIN | State::ANY);
    display.snapshot().unwrap();
    assert_eq!(display.ui.obj_get_width(obj), 100);
    assert_eq!(display.ui.obj_get_height(obj), 50);
    assert_eq!(
        display.ui.obj_get_style_bg_color(obj, Part::MAIN),
        Color::hex(0xff0000)
    );
    assert_eq!(display.ui.obj_get_style_bg_opa(obj, Part::MAIN), 0);
    assert_eq!(display.ui.obj_get_style_bg_opa(obj, Part::SCROLLBAR), 102);
}
