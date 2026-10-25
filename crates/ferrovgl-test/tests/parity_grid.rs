//! Exact ports of LVGL v9.6.0 test_grid_fr.c and test_margin_grid.c.

use ferrovgl_core::StyleSelector;
use ferrovgl_core::core::obj::ScrollbarMode;
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign, grid_fr};
use ferrovgl_core::misc::palette::{self, Palette};
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_test::TestDisplay;

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn grid_fractional_tracks_cover_screen_without_rounding_gaps() {
    static COLUMNS: [i32; 6] = [
        27,
        grid_fr(1),
        grid_fr(2),
        grid_fr(3),
        grid_fr(4),
        GRID_TEMPLATE_LAST,
    ];
    static ROWS: [i32; 6] = [
        9,
        grid_fr(1),
        grid_fr(1),
        grid_fr(1),
        grid_fr(1),
        GRID_TEMPLATE_LAST,
    ];
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_clean(screen);
    let selector = StyleSelector::DEFAULT;
    display
        .ui
        .obj_set_style_bg_color(screen, palette::main(Palette::Red), selector);
    display
        .ui
        .obj_set_grid_dsc_array(screen, TracksRef::Static(&COLUMNS), TracksRef::Static(&ROWS));
    display.ui.obj_set_style_pad_row(screen, 0, selector);
    display.ui.obj_set_style_pad_column(screen, 0, selector);
    for col in 0..5 {
        for row in 0..5 {
            let child = display.ui.obj_create(Some(screen));
            display.ui.obj_set_size(child, 1, 1);
            display
                .ui
                .obj_set_grid_cell(child, GridAlign::Stretch, col, 1, GridAlign::Stretch, row, 1);
            display.ui.obj_set_style_bg_color(
                child,
                palette::main(if (row + col) % 2 != 0 {
                    Palette::Green
                } else {
                    Palette::Blue
                }),
                selector,
            );
            display.ui.obj_set_style_radius(child, 0, selector);
            display.ui.obj_set_style_border_width(child, 0, selector);
        }
    }
    display.screenshot_compare("grid_fr.png").unwrap();
}

#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn grid_centered_cells_with_asymmetric_margins() {
    let mut display = TestDisplay::with_default_theme().unwrap();
    let screen = display.screen().unwrap();
    display.ui.obj_clean(screen);
    let selector = StyleSelector::DEFAULT;
    display
        .ui
        .obj_set_grid_align(screen, GridAlign::Center, GridAlign::Center);
    display.ui.obj_set_grid_dsc_array(
        screen,
        TracksRef::Static(&[80, 80, 80, 80, 80, 80, 80, 80, GRID_TEMPLATE_LAST]),
        TracksRef::Static(&[80, 80, 80, 80, 80, GRID_TEMPLATE_LAST]),
    );
    display.ui.obj_set_style_pad_row(screen, 0, selector);
    display.ui.obj_set_style_pad_column(screen, 0, selector);
    let colors = [
        Palette::Blue,
        Palette::Purple,
        Palette::Green,
        Palette::Pink,
        Palette::Amber,
        Palette::BlueGrey,
        Palette::Indigo,
    ];
    let mut children = Vec::new();
    for row in 0..5 {
        for col in 0..8 {
            let child = display.ui.obj_create(Some(screen));
            display.ui.obj_set_style_bg_color(
                child,
                palette::main(colors[(row * 5 + col) % colors.len()]),
                selector,
            );
            display.ui.obj_set_size(child, 150, 150);
            children.push(child);
        }
    }
    for row in 0..5 {
        for col in 0..8 {
            let child = children[row * 8 + col];
            display.ui.obj_set_style_radius(child, 0, selector);
            display.ui.obj_set_style_border_width(child, 0, selector);
            display.ui.obj_set_scrollbar_mode(child, ScrollbarMode::Off);
            display.ui.obj_set_width(child, 30);
            display.ui.obj_set_grid_cell(
                child,
                GridAlign::Center,
                col as i32,
                1,
                GridAlign::Stretch,
                row as i32,
                1,
            );
            display
                .ui
                .obj_set_style_margin_top(child, (row * 5) as i32, selector);
            display
                .ui
                .obj_set_style_margin_left(child, ((row + col) * 5) as i32, selector);
            display
                .ui
                .obj_set_style_margin_right(child, (row * 5) as i32, selector);
            display
                .ui
                .obj_set_style_margin_bottom(child, ((row + col) * 5) as i32, selector);
        }
    }
    display.ui.obj_update_layout(screen);
    display.screenshot_compare("margin_grid_0.png").unwrap();
}
