//! Exact scene ports from LVGL v9.6.0 `test_flex_grow.c` and `test_align_flex.c`.

use ferrovgl_core::core::obj::ScrollbarMode;
use ferrovgl_core::layouts::flex::{FlexAlign, FlexFlow};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{Obj, StyleSelector, Ui};
use ferrovgl_test::TestDisplay;

fn simple_style(ui: &mut Ui, obj: Obj) {
    let selector = StyleSelector::DEFAULT;
    ui.obj_set_style_pad_row(obj, 0, selector);
    ui.obj_set_style_pad_column(obj, 0, selector);
    ui.obj_set_style_pad_all(obj, 0, selector);
    ui.obj_set_style_border_width(obj, 0, selector);
    ui.obj_set_style_radius(obj, 0, selector);
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_grow_tracks_cover_container_without_rounding_gaps() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_clean(screen);
    simple_style(&mut display.ui, screen);
    let container = display.ui.obj_create(Some(screen));
    simple_style(&mut display.ui, container);
    display
        .ui
        .obj_set_style_bg_color(container, palette::main(Palette::Red), StyleSelector::DEFAULT);
    display.ui.obj_set_flex_flow(container, FlexFlow::Row);
    display
        .ui
        .obj_set_size(container, coord::pct(100), coord::SIZE_CONTENT);
    display.ui.obj_center(container);

    for grow_1 in 1..=3 {
        for grow_2 in 1..=3 {
            for fixed in 20..=24 {
                let first = display.ui.obj_create(Some(container));
                simple_style(&mut display.ui, first);
                display.ui.obj_set_style_bg_color(
                    first,
                    palette::main(Palette::Blue),
                    StyleSelector::DEFAULT,
                );
                display.ui.obj_set_size(first, 1, 10);
                display.ui.obj_set_flex_grow(first, grow_1);
                display.ui.obj_set_flex_in_new_track(first, true);

                let second = display.ui.obj_create(Some(container));
                simple_style(&mut display.ui, second);
                display.ui.obj_set_style_bg_color(
                    second,
                    palette::main(Palette::Green),
                    StyleSelector::DEFAULT,
                );
                display.ui.obj_set_size(second, 1, 10);
                display.ui.obj_set_flex_grow(second, grow_2);

                let fixed_child = display.ui.obj_create(Some(container));
                simple_style(&mut display.ui, fixed_child);
                display.ui.obj_set_style_bg_color(
                    fixed_child,
                    palette::main(Palette::Blue),
                    StyleSelector::DEFAULT,
                );
                display.ui.obj_set_size(fixed_child, fixed, 10);
            }
        }
    }
    display.screenshot_compare("flex_grow.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn flex_main_alignment_with_one_and_two_children() {
    // `test_align`: these style changes deliberately preserve the theme's
    // row/column gap, just as the original simple_style() does.
    fn style(ui: &mut Ui, obj: Obj) {
        let selector = StyleSelector::DEFAULT;
        ui.obj_set_style_radius(obj, 0, selector);
        ui.obj_set_scrollbar_mode(obj, ScrollbarMode::Off);
        ui.obj_set_style_border_width(obj, 0, selector);
        ui.obj_set_style_pad_all(obj, 0, selector);
        ui.obj_set_style_margin_all(obj, 0, selector);
        ui.obj_set_style_outline_pad(obj, 0, selector);
        ui.obj_set_style_outline_width(obj, 0, selector);
    }

    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_clean(screen);
    for (alignment, name) in [
        (FlexAlign::Center, "center"),
        (FlexAlign::SpaceEvenly, "space_evenly"),
        (FlexAlign::SpaceAround, "space_around"),
        (FlexAlign::SpaceBetween, "space_between"),
    ] {
        for count in 1..=2 {
            let row = display.ui.obj_create(Some(screen));
            display.ui.obj_set_size(row, coord::pct(100), coord::pct(100));
            display.ui.obj_set_flex_flow(row, FlexFlow::Row);
            display
                .ui
                .obj_set_flex_align(row, alignment, FlexAlign::Center, FlexAlign::Center);
            style(&mut display.ui, row);
            for _ in 0..count {
                let child = display.ui.obj_create(Some(row));
                display.ui.obj_set_size(child, 40, 40);
                display.ui.obj_set_style_bg_color(
                    child,
                    palette::main(Palette::Blue),
                    StyleSelector::DEFAULT,
                );
                style(&mut display.ui, child);
            }
            display
                .screenshot_compare(format!("align_flex_{name}{count}.png"))
                .unwrap();
            display.ui.obj_clean(screen);
        }
    }
}
