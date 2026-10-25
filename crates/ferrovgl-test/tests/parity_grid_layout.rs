//! Exact ports of LVGL v9.6.0 `tests/src/test_cases/test_grid.c`.
//!
//! `test_grid_fr.c` and `test_margin_grid.c` are ported in `parity_grid.rs`.
//! Every `test_grid.c` function starts with `setUp()`'s
//! `lv_obj_clean(lv_screen_active())` and builds its scene from scratch with
//! no input, so a fresh [`TestDisplay`] per test reproduces the shared suite
//! state the screenshots depend on.

use ferrovgl_core::core::obj::Obj;
use ferrovgl_core::layouts::LAYOUT_GRID;
use ferrovgl_core::layouts::grid::{GRID_TEMPLATE_LAST, GridAlign, grid_fr};
use ferrovgl_core::misc::style::TracksRef;
use ferrovgl_core::misc::text::BaseDir;
use ferrovgl_core::misc::types::coord;
use ferrovgl_core::{StyleSelector, Ui};
use ferrovgl_test::TestDisplay;
use ferrovgl_widgets::{Button, Label};

/// `test_grid.c` style selector `0` / `LV_PART_MAIN`.
const MAIN: StyleSelector = StyleSelector::DEFAULT;

/// `{LV_GRID_FR(1) x4, LV_GRID_TEMPLATE_LAST}`, used by every scene.
static FR4: [i32; 5] = [grid_fr(1), grid_fr(1), grid_fr(1), grid_fr(1), GRID_TEMPLATE_LAST];
/// `{LV_GRID_FR(1) x2, LV_GRID_TEMPLATE_LAST}`: the `test_grid_ltr/rtl` rows.
static FR2: [i32; 3] = [grid_fr(1), grid_fr(1), GRID_TEMPLATE_LAST];

/// `setUp()`: `lv_obj_clean(lv_screen_active())`.
fn set_up() -> TestDisplay {
    let mut display = TestDisplay::with_default_theme().unwrap();
    display.clean_screen();
    display
}

/// `button_create()` of `test_grid.c`.
fn button_create(ui: &mut Ui, parent: Obj, text: &str, x: i32, x_span: i32, y: i32, y_span: i32) {
    let btn = Button::create(ui, parent).obj();
    ui.obj_set_grid_cell(btn, GridAlign::Stretch, x, x_span, GridAlign::Stretch, y, y_span);

    let label = Label::create(ui, btn);
    label.set_text(ui, text);
    ui.obj_center(label.obj());
}

/// The common start of `test_subgrid_row()` and `test_subgrid_col()`: a
/// centred 700x300 4x4 FR grid, a sub-container spanning cells (1..3, 1..3)
/// and the three buttons of the main grid. Returns the sub-container.
fn subgrid_containers(display: &mut TestDisplay, sub_cols: bool) -> Obj {
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let cont_main = ui.obj_create(Some(screen));
    ui.obj_set_size(cont_main, 700, 300);
    ui.obj_center(cont_main);
    ui.obj_set_grid_dsc_array(cont_main, TracksRef::Static(&FR4), TracksRef::Static(&FR4));

    let cont_sub = ui.obj_create(Some(cont_main));
    ui.obj_set_grid_cell(cont_sub, GridAlign::Stretch, 1, 2, GridAlign::Stretch, 1, 2);
    if sub_cols {
        /* `col_dsc2`, rows inherited from the parent (`NULL`). */
        ui.obj_set_grid_dsc_array(cont_sub, TracksRef::Static(&FR4), None);
    } else {
        /* `row_dsc2`, columns inherited from the parent (`NULL`). */
        ui.obj_set_grid_dsc_array(cont_sub, None, TracksRef::Static(&FR4));
    }
    ui.obj_set_style_pad_all(cont_sub, 0, MAIN);

    button_create(ui, cont_main, "Main 0,0", 0, 1, 0, 1);
    button_create(ui, cont_main, "Main 3,3", 3, 1, 3, 1);
    button_create(ui, cont_main, "Main 2,2", 2, 1, 2, 1);
    cont_sub
}

/// `test_subgrid_row()`: the sub-container has its own columns and takes its
/// rows from the parent's row descriptor.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn subgrid_row() {
    let mut display = set_up();
    let cont_sub = subgrid_containers(&mut display, true);
    let ui = &mut display.ui;
    button_create(ui, cont_sub, "Sub 0,0", 0, 1, 0, 1);
    button_create(ui, cont_sub, "Sub 1,0", 1, 1, 0, 1);
    button_create(ui, cont_sub, "Sub 2,0", 2, 1, 0, 1);
    button_create(ui, cont_sub, "Sub 3,0", 3, 1, 0, 1);
    button_create(ui, cont_sub, "Sub 1,1", 1, 1, 1, 1);
    button_create(ui, cont_sub, "Sub 0,1", 0, 1, 1, 1);

    display.screenshot_compare("subgrid_row.png").unwrap();
}

/// `test_subgrid_col()`: the sub-container has its own rows and takes its
/// columns from the parent's column descriptor.
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn subgrid_col() {
    let mut display = set_up();
    let cont_sub = subgrid_containers(&mut display, false);
    let ui = &mut display.ui;
    button_create(ui, cont_sub, "Sub 0,0", 0, 1, 0, 1);
    button_create(ui, cont_sub, "Sub 0,1", 0, 1, 1, 1);
    button_create(ui, cont_sub, "Sub 0,2", 0, 1, 2, 1);
    button_create(ui, cont_sub, "Sub 0,3", 0, 1, 3, 1);
    button_create(ui, cont_sub, "Sub 1,0", 1, 1, 0, 1);
    button_create(ui, cont_sub, "Sub 1,1", 1, 1, 1, 1);

    display.screenshot_compare("subgrid_col.png").unwrap();
}

/// The body of `test_grid_ltr()` and `test_grid_rtl()`.
fn grid_base_dir(display: &mut TestDisplay, base_dir: BaseDir) {
    let screen = display.screen().unwrap();
    let ui = &mut display.ui;
    let grid = ui.obj_create(Some(screen));
    ui.obj_set_style_base_dir(grid, base_dir, MAIN);
    ui.obj_set_size(grid, coord::pct(100), coord::pct(100));
    ui.obj_set_grid_dsc_array(grid, TracksRef::Static(&FR4), TracksRef::Static(&FR2));

    button_create(ui, grid, "Button 1", 0, 2, 0, 1);
    button_create(ui, grid, "Button 2", 2, 2, 0, 2);
    button_create(ui, grid, "Btn 3", 0, 1, 1, 1);
    button_create(ui, grid, "Btn 4", 1, 1, 1, 1);
}

/// `test_grid_ltr()`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn grid_ltr() {
    let mut display = set_up();
    grid_base_dir(&mut display, BaseDir::Ltr);
    display.screenshot_compare("grid_ltr.png").unwrap();
}

/// `test_grid_rtl()`
#[test]
#[ignore = "requires pinned LVGL reference PNGs; cargo xtask refimgs sync"]
fn grid_rtl() {
    let mut display = set_up();
    grid_base_dir(&mut display, BaseDir::Rtl);
    display.screenshot_compare("grid_rtl.png").unwrap();
}

/// `test_grid_no_crash_on_invalid_settings()`: missing descriptors and
/// invalid cell positions/spans must not crash the layout or the refresh.
#[test]
fn grid_no_crash_on_invalid_settings() {
    let mut display = set_up();
    let ui = &mut display.ui;

    /*No col/row descriptors on screen*/
    let scr = ui.obj_create(None);
    ui.screen_load(scr);
    ui.obj_set_style_layout(scr, LAYOUT_GRID as u16, MAIN);
    ui.refr_now(None);

    /*No col/row descriptors on a widget*/
    let cont = ui.obj_create(Some(scr));
    ui.obj_set_style_layout(cont, LAYOUT_GRID as u16, MAIN);
    ui.refr_now(None);

    /*Set a cell without having row/col descriptor on the parent*/
    ui.obj_set_grid_cell(cont, GridAlign::Center, 0, 1, GridAlign::Center, 0, 1);
    ui.refr_now(None);

    /*Add grid descriptors*/
    ui.obj_set_grid_dsc_array(cont, TracksRef::Static(&FR4), TracksRef::Static(&FR2));
    ui.refr_now(None);

    let label = Label::create(ui, cont).obj();

    for (col_pos, col_span, row_pos, row_span) in [
        /*Zero span*/
        (0, 0, 0, 0),
        /*Too large span*/
        (0, 20, 0, 30),
        /*Negative span*/
        (0, -50, 0, -20),
        /*Too large position*/
        (30, 1, 20, 1),
        /*Negative position*/
        (-100, 1, -20, 1),
        /*Valid settings*/
        (1, 2, 0, 1),
    ] {
        ui.obj_set_grid_cell(
            label,
            GridAlign::Center,
            col_pos,
            col_span,
            GridAlign::Center,
            row_pos,
            row_span,
        );
        ui.refr_now(None);
    }
    /* `TEST_PASS()` */
    assert!(ui.obj_is_valid(label));
}
