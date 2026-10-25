//! LVGL v9.6.0 scrollbar, animated label and rotated-child scroll scenarios.

use std::cell::Cell;
use std::rc::Rc;

use ferrovgl_core::layouts::flex::FlexFlow;
use ferrovgl_core::misc::anim::AnimTarget;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Dir, EventCode, ObjFlags, StyleSelector};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Label, LabelLongMode};

fn scrollbar_scene(vertical: bool) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display
        .ui
        .obj_set_flex_flow(screen, if vertical { FlexFlow::Row } else { FlexFlow::Column });
    for extent in [100, 500, 1000, 5000, 100000, 500000, 1000000, 4000000] {
        let wrapper = display.ui.obj_create(Some(screen));
        display.ui.obj_set_flex_grow(wrapper, 1);
        if vertical {
            display.ui.obj_set_height(wrapper, coord::pct(100));
        } else {
            display.ui.obj_set_width(wrapper, coord::pct(100));
        }
        let inner = display.ui.obj_create(Some(wrapper));
        if vertical {
            display.ui.obj_set_size(inner, coord::pct(100), extent);
        } else {
            display.ui.obj_set_size(inner, extent, coord::pct(100));
        }
    }
    display
        .screenshot_compare(if vertical {
            "scrollbar_vertical.png"
        } else {
            "scrollbar_horizontal.png"
        })
        .unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn vertical_scrollbars_with_large_content() {
    scrollbar_scene(true);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn horizontal_scrollbars_with_large_content() {
    scrollbar_scene(false);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn scrolling_labels_keep_animation_position_when_text_changes() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_clean(screen);
    let text = [
        "This is a long text that we will update while scrolling",
        "THIS IS A LONG TEXT THAT WE WILL UPDATE WHILE SCROLLING",
    ];
    let mut labels = Vec::new();
    for (mode, y) in [(LabelLongMode::Scroll, 10), (LabelLongMode::ScrollCircular, 80)] {
        let label = Label::create(&mut display.ui, screen);
        label.set_long_mode(&mut display.ui, mode);
        label.set_text(&mut display.ui, text[0]);
        display.ui.obj_set_width(label.obj(), 150);
        display.ui.obj_set_pos(label.obj(), 10, y);
        labels.push(label);
    }
    for step in 0..20 {
        for label in &labels {
            label.set_text(&mut display.ui, text[step % 2]);
        }
        display
            .screenshot_compare(format!("widgets/label_scroll_{step}.png"))
            .unwrap();
        display.wait(783);
    }
}

fn rotated_child_scroll(rotation: i32) {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    let inputs = display.create_inputs();
    let container = display.ui.obj_create(Some(screen));
    display.ui.obj_set_size(container, 200, 200);
    display.ui.obj_center(container);
    display.ui.obj_set_scroll_dir(container, Dir::HOR);
    let began = Rc::new(Cell::new(false));
    let flag = began.clone();
    display.ui.obj_add_event_cb(
        container,
        Rc::new(move |_, _| flag.set(true)),
        EventCode::SCROLL_BEGIN,
        None,
    );
    let wide = display.ui.obj_create(Some(container));
    display.ui.obj_set_size(wide, 600, 200);
    let list = display.ui.obj_create(Some(container));
    display.ui.obj_set_size(list, 200, 200);
    display.ui.obj_set_scroll_dir(list, Dir::VER);
    display.ui.obj_remove_flag(list, ObjFlags::SCROLL_CHAIN_VER);
    let rotated = display.ui.obj_create(Some(list));
    display.ui.obj_set_size(rotated, 100, 50);
    display.ui.obj_center(rotated);
    display
        .ui
        .obj_set_style_transform_rotation(rotated, 900, StyleSelector::DEFAULT);
    display.ui.obj_remove_flag(rotated, ObjFlags::SCROLLABLE);
    let inner = display.ui.obj_create(Some(rotated));
    display.ui.obj_set_size(inner, 80, 30);
    display.ui.obj_center(inner);
    display.ui.obj_remove_flag(inner, ObjFlags::SCROLLABLE);
    display.wait(50);
    if rotation == 0 {
        display
            .ui
            .obj_set_style_transform_rotation(rotated, 0, StyleSelector::DEFAULT);
        display.wait(50);
    }
    let area = display.ui.obj_get_coords(inner);
    inputs
        .mouse
        .move_to((area.x1 + area.x2) / 2, (area.y1 + area.y2) / 2);
    inputs.mouse.press();
    display.wait(50);
    for _ in 0..10 {
        inputs.mouse.move_by(10, 0);
        display.wait(16);
    }
    inputs.mouse.release();
    display.wait(50);
    assert!(began.get(), "the horizontal ancestor must receive SCROLL_BEGIN");
}

#[test]
fn horizontal_drag_through_rotated_child_reaches_scrollable_ancestor() {
    rotated_child_scroll(900);
}

#[test]
fn horizontal_drag_through_unrotated_child_reaches_scrollable_ancestor() {
    rotated_child_scroll(0);
}

#[test]
fn replacing_scroll_animation_can_delete_animations_from_scroll_end() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let obj = display.ui.obj_create(display.screen());
    let ends = Rc::new(Cell::new(0));
    let count = ends.clone();
    display.ui.obj_add_event_cb(
        obj,
        Rc::new(move |ui, _| {
            ui.anim_delete(AnimTarget::Obj(obj), None);
            count.set(count.get() + 1);
        }),
        EventCode::SCROLL_END,
        None,
    );
    display.ui.obj_scroll_by(obj, 0, 100, true);
    display.wait(20);
    display.ui.obj_scroll_by(obj, 0, 100, true);
    assert_eq!(ends.get(), 1);
}
